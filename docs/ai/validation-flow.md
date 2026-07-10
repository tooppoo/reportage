# AI validation flow

How to validate a `.repor` file after generating or editing it, using commands that exist in the current CLI.

## Run the script with `--format=json`

```sh
reportage <file.repor> --format=json
```

This is the same invocation `reportage docs --format=json` advertises in its `validation.command` field — treat that field, not this document, as the source of truth if the two ever disagree, since it is generated from the running binary rather than hand-written.

`reportage check <file> --format=json` does not exist yet. Do not suggest running it; see [`docs/ai/common-mistakes.md`](common-mistakes.md).

## Reading the JSON output, minimally

The document on stdout has these top-level fields: `schemaVersion`, `tool`, `status`, `processExitCode`, `artifactRoot`, `summary`, `diagnostics`, and `tests`. For a first pass:

- `status` is `"passed"`, `"failed"`, or `"error"` — not a boolean. `"failed"` means the script was valid and ran, but an assertion did not pass. `"error"` means the script or config could not be treated as valid input, or the runner hit an infrastructure failure before it could produce a pass/fail result.
- `processExitCode` is the reportage process's own exit code, separate from any exit code of the command a case ran. See [`docs/exit-codes.md`](../exit-codes.md) for the full table and what each code means.
- `diagnostics` carries structured parse/validation/runtime errors, each with a stable `category` and `code`. Read [`docs/diagnostics.md`](../diagnostics.md) for parser/validator codes and [`docs/semantic-diagnostics.md`](../semantic-diagnostics.md) for `semantic.*` / `assertion.*` / `step.*` codes before guessing at what a code means from its name alone.
- `tests` holds the per-case results; a failed assertion appears here, not as a `diagnostics` entry — do not conflate an assertion failure (the script was valid, the check did not pass) with a script/semantic error (the script itself was rejected).

The full contract for this document, including every field's exact shape, is [`spec/output/json-report/README.md`](../../spec/output/json-report/README.md) and its schema at [`spec/output/json-report/schema.json`](../../spec/output/json-report/schema.json). Do not rely on this document's summary above for anything beyond a first read.

## Artifacts

Every run also writes an artifact bundle under `.reportage/`, described in [`docs/artifacts.md`](../artifacts.md). Its manifest contract is [`spec/artifacts/run-result/README.md`](../../spec/artifacts/run-result/README.md) and [`spec/artifacts/run-result/schema.json`](../../spec/artifacts/run-result/schema.json). Use the artifact bundle, not memory of the stdout JSON, when you need evidence from the run after the fact.
