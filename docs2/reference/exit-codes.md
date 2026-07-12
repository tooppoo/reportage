# Exit Codes

This document defines the reportage CLI exit code policy and the exit codes introduced in v0.

Everything below "v0 Exit Code Table" through "Precedence" describes the default run behavior (`reportage <script>...` / `reportage --config <path>`): running `.repor` scripts and writing the `result.json` artifact. `reportage shim scaffold` is a separate subcommand with its own, narrower exit code table; see "`shim scaffold` exit codes" below.

## Policy

- Exit code `0` means success: every case in the run passed, or the run was a no-op because no cases were found in otherwise-valid selected input.
- Non-zero exit codes must be as specific and distinguishable as reasonably possible.
- Test assertion failure, parse/validation error, and execution/runtime error must not share the same exit code.
- Exit codes are process-level severity signals for CI and shell callers.
- The artifact manifest's `status` / `processExitCode` / `diagnostics` record run outcome categories for humans and tools reading reportage output.

## v0 Exit Code Table

| Code | Meaning |
|------|---------|
| `0`  | **Success** — all cases passed, or no cases were found in otherwise-valid selected input. |
| `1`  | **Test/assertion failure** — the run completed but at least one assertion did not pass. |
| `2`  | **Script/config validation error** — the selected reportage scripts or configuration could not be treated as valid test input. Examples include read errors, parse errors, unsupported syntax, invalid config, a case with no assertion, an assertion with no preceding action, or an exit code outside `0..=255`. |
| `3`  | **Action execution/runtime error** — the runner could not execute an action, write required artifacts, run a side-effecting step such as `write`, or perform required runtime infrastructure work. This is distinct from a non-zero action exit code, which is a normal execution outcome. |

## Run outcome categories in the artifact manifest

The run outcome categories below are conceptual severity classes. The artifact manifest (`result.json`, see [Artifacts](artifacts.md)) records them as the combination of top-level `status` (`passed` / `failed` / `error`, not a boolean pass/fail field), `processExitCode`, and `diagnostics[]` category / code.

v0 outcome categories:

| Category | `status` | Exit code | Meaning |
|----------|----------|-----------|---------|
| `pass` | `passed` | `0` | All selected cases executed and passed, or the run was a no-op success with `noop: true`. |
| `test_failed` | `failed` | `1` | The selected scripts were valid and execution completed, but one or more assertions failed. |
| `script_error` | `error` | `2` | One or more selected reportage script files could not be used as valid test definitions. Examples include read errors, parse errors, unsupported syntax, and invalid script structure. |
| `config_error` | `error` | `2` | The configuration itself is invalid, unsupported, or cannot be used for discovery. |
| `runtime_error` | `error` | `3` | The runner failed due to infrastructure/runtime conditions such as shell spawn failure or required artifact write failure. |

`script_error` is intentionally broader than a parse error. Read errors and parse errors for selected reportage files both produce `status: "error"` and process exit code `2`.

Concrete causes must remain distinguishable in structured diagnostics: the manifest's `diagnostics[]` entries carry `category` (`parse` / `internal` for file-level errors) and stable `code` values, so the collapsed `status: "error"` never loses the failing layer.

## Precedence

When a run contains multiple cases with different error types, the exit code reflects the highest-severity result:

```text
3 (runtime error) > 2 (script/config error) > 1 (assertion failure) > 0 (success)
```

For the run outcome category, use the corresponding highest-severity one. If multiple categories share exit code `2`, prefer the category that identifies the failing layer:

```text
runtime_error > config_error > script_error > test_failed > pass
```

This means `config_error` is used when discovery/configuration cannot produce a valid selected script set, while `script_error` is used when selected reportage script files themselves cannot be used as valid test definitions.

## Notes

- A non-zero exit code from an action (`$ false` exits with `1`) is not itself an error. It is captured as the action's result and evaluated by explicit `assert exit` assertions.
- Exit code `3` is reserved for infrastructure failures such as the POSIX shell not being found on `PATH`, or required artifact generation failing. It does not mean "the action exited non-zero".
- A `write` step's runtime step error (create-only target already exists, a regular file blocking its parent path, or an OS-level I/O failure) is exit code `3`, not `1`. There is no expectation being compared against evidence, so it is never an assertion failure. See [Language semantics](semantics.md) — Write step.
- Empty and whitespace-only scripts are syntax-valid inputs. At execution time they produce a no-op success with exit code `0`, no command execution, and no assertion evaluation.

## `shim scaffold` exit codes

`reportage shim scaffold` (see [Shim scaffold](shim-scaffold.md)) never runs a `.repor` script, so the "Test/assertion failure" and run outcome categories above do not apply to it. It uses a smaller, independent table:

| Code | Meaning |
|------|---------|
| `0`  | **Success** — the template was rendered and written to `--out`. |
| `2`  | **Request validation error** — `--template`, `--entry-point`, or `--out` was missing, empty, or otherwise invalid (including an unknown template name), or `--out` conflicts with an existing directory or symlink, or an existing file without `--force`. Nothing is written to disk when this code is returned. |
| `3`  | **Runtime/infrastructure error** — creating `--out`'s parent directory, writing the rendered file, or setting its permissions failed at the OS level. |
| `4`  | **CLI usage error** — clap itself rejected the invocation (e.g. an unrecognized flag, or `shim` given without a further subcommand). Shared with every other malformed invocation of the `reportage` binary; see [`e2e/options/unknown-options.repor`](../../e2e/options/unknown-options.repor) for an example on the default run command. |

Code `2` here intentionally reuses the same number as the run command's "script/config validation error": both mean "the requested operation could not be treated as valid input," even though `shim scaffold` has no script or config file to speak of. Code `3` likewise reuses "runtime/infrastructure error" for the same reason the run command does: an OS-level failure while doing required I/O, not a normal outcome the caller is expected to branch on.

## `references` and reserved `docs` exit codes

`reportage references` is a side-effect-free tooling subcommand that only prints the reference URL index (see [`spec/output/references-index/`](../../spec/output/references-index/)).
It exits `0` after printing, or `4` when clap rejects the invocation (e.g. an unsupported `--format` value), the same CLI usage error code as everywhere else.

`docs` is reserved for a future documentation generation command and is not implemented (see [ADR: Rename `docs` Command to `references`](../adr/20260711T070008Z_rename-docs-command-to-references.md)).
Every `reportage docs` invocation, whatever tokens follow it, prints a not-implemented error to stderr and exits `2`, reusing the "requested operation could not be treated as valid input" meaning above.
The future real command replaces this behavior with its own exit code table.
