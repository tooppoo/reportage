# JSON Stdout Contract and Captured Output Artifact Policy

- Status: Accepted
- Created: 2026-07-07T05:00:00Z

## Context

`reportage run --format=json` writes a JSON document to the CLI process's own standard output. Separately, actions Reportage executes (`$ ...` steps) produce their own stdout/stderr — captured output recorded on `ActionResult`, not part of the CLI process's own stdout.

Conflating these two kinds of stdout/stderr causes concrete problems:

- Human-readable log lines or captured action output could leak into `--format=json`'s stdout.
- A JSON parser would then be unable to read CLI stdout as a single JSON document.
- Captured stdout/stderr's raw byte semantics could be corrupted by JSON text encoding (Reportage treats stdout/stderr as raw bytes, not necessarily valid UTF-8).
- Large captured output would bloat CI logs and the JSON document itself.
- External tooling would have an unclear boundary between "the JSON document" and "captured evidence."

Related issue: [#89](https://github.com/tooppoo/reportage/issues/89). See [`20260707T045900Z_json-output-as-structured-execution-report.md`](20260707T045900Z_json-output-as-structured-execution-report.md) for the broader structured-report decision this refines.

## Decision

CLI stdout and captured stdout/captured stderr are distinct and must not be conflated.

- **CLI stdout** is `reportage run --format=json`'s own standard output. When `--format=json` is given, CLI stdout contains exactly one valid JSON document and nothing else — no human-readable log lines, ANSI color, progress output, debug output, or raw captured bytes.
- **Captured stdout** / **captured stderr** are the standard output/error produced by an action's `$ ...` command. In v0 JSON, these are never inlined as string or byte data. They are written to disk as artifact files by `reportage_core::artifact::ArtifactWriter::write_captured_output` (`<artifactRoot>/<test_id>/<action_id>/{stdout,stderr}.bin`), and the JSON document only references them by relative path (`artifactRef`) plus `sizeBytes`.

Any execution error representable within `ExecutionReport` is surfaced through `diagnostics[]`. CLI-level failures that occur before an `ExecutionReport` can even be constructed (argument parsing errors, a fatal failure before the JSON renderer starts) are ordinary CLI errors, not JSON diagnostics.

### Captured stdout/stderr artifact policy

`artifactRef` is a path relative to the document's top-level `artifactRoot`. A JSON consumer resolves captured output by joining `artifactRoot` and `artifactRef`.

```json
{
  "artifactRoot": ".reportage/runs/1720312345678",
  "tests": [
    {
      "actions": [
        {
          "stdout": { "artifactRef": "test-1/action-1/stdout.bin", "sizeBytes": 6 },
          "stderr": { "artifactRef": "test-1/action-1/stderr.bin", "sizeBytes": 0 }
        }
      ]
    }
  ]
}
```

`artifactRef` is a reference a JSON consumer resolves against the filesystem; it is never the captured bytes themselves.

### Process exit code

The top-level JSON document includes `processExitCode`: the exit code `reportage run` will terminate with once the JSON document has been printed. `JsonRenderer` computes this from `ExecutionReport::exit_code()`, and the CLI process exits with that same value.

`processExitCode` is distinct from any individual action's own `exitCode` (recorded per action under `tests[].actions[]`).

The basic invariant is:

```text
status == "passed" -> processExitCode == 0
status == "failed" -> processExitCode != 0
status == "error"  -> processExitCode != 0
```

This ADR does not redefine the specific non-zero exit code taxonomy; that is `docs/exit-codes.md`'s concern.

## Alternatives Considered

### Mix the JSON document and human-readable log lines on CLI stdout

Rejected. External tooling could no longer treat CLI stdout as a single parseable JSON document. Anything a consumer needs from the human-readable log belongs in `diagnostics[]` or another structured field, not in interleaved plain text.

### Inline captured stdout/stderr as UTF-8 strings in v0 JSON

Rejected. Captured output is raw bytes and is not guaranteed to be valid UTF-8. Inlining as a string would either corrupt non-UTF-8 output or require lossy conversion, and would let large captured output bloat the JSON document and any CI log that prints it.

### Inline captured stdout/stderr as base64 in v0 JSON

Considered and deferred, not rejected outright. Base64 can represent raw bytes losslessly (`data` + `encoding: "base64"`, the same approach `spec/language/semantics/*.json` already uses for fixture checkpoint bytes). v0 prioritizes keeping the JSON document small and keeping captured output out of CI logs over inline convenience. A future option to opt into inline base64 output remains possible without breaking this contract, since it would be an additive field.

### Omit `processExitCode` from the JSON document

Rejected. A JSON consumer reading only the artifact after the process has exited (e.g. in CI) would have no way to recover the CLI's actual exit semantics from the document alone.

## Consequences

### Positive Consequences

- `--format=json`'s stdout can be piped directly into any JSON parser.
- Captured output retains its raw-byte semantics because it never passes through JSON string encoding.
- A JSON consumer can determine the CLI's final exit status from the document alone, without also inspecting the process exit code out-of-band.

### Negative Consequences

- Reading captured output requires an extra filesystem read (`artifactRoot` + `artifactRef`), rather than being available directly in the JSON document.
- `processExitCode` must be kept in sync with the CLI's actual exit code by construction; a future refactor that changes how the CLI process exits must not let the two diverge.

### Neutral Consequences

- Inline base64 captured output remains a possible additive future field; this ADR does not commit to or against it beyond v0.
