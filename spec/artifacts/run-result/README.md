# Artifact Run Result (`result.json`)

This directory records the canonical contract for the artifact `result.json` written to `.reportage/runs/<run-id>/` by `reportage run`.

## Canonical record

The artifact bundle — `result.json` plus the raw evidence files it references — is the canonical record of a run. `result.json` is the bundle's canonical manifest: run-level outcome, summary, per-test action and assertion results, diagnostics, evidence references, document-local ids, schema version, and tool version.

Raw byte evidence (captured stdout/stderr) is never inlined into `result.json`. Each action's captured stream is stored as a separate file inside the bundle (`<test-id>/<action-id>/{stdout,stderr}.bin`) and referenced as `{ artifactRef, sizeBytes, sha256 }`. `artifactRef` is relative to the directory containing `result.json`, and `sha256` makes it verifiable that the referenced file is the one the manifest describes.

## Relationship to `--format=json`

`reportage run --format=json`'s CLI stdout document ([`spec/output/json-report/schema.json`](../../output/json-report/schema.json), issue #89) is a stdout-safe projection derived from this canonical document. The #89 stdout contract is maintained as-is and was not redesigned by issue #102.

The two contracts version independently. Their intentional differences are:

- the stdout document carries `artifactRoot` (this manifest resolves `artifactRef` against its own directory instead);
- the stdout document does not carry `noop`;
- the stdout document's `stdout` / `stderr` evidence references omit `sha256`.

Everything else is shared. The projection is implemented as a document transformation (`reportage-cli/src/render/json.rs`, `project_run_result`) over the canonical builder (`reportage-core/src/run_result.rs`), so the relation holds by construction, and projection parity is additionally verified by `crates/reportage-cli/tests/run_result_fixtures.rs`.

## JSON Schema

`schema.json` defines the expected structure of the artifact `result.json` and is useful for editor integration (autocomplete, inline validation). It is the stable path for external consumers.

### Which file to edit

`schema.internal.json` is the only file to edit. `schema.json` is generated from it by `just schema-artifacts-gen` and must never be hand-edited; `just schema-artifacts-check`, which `just check` runs, fails when the committed file drifts from what the internal source generates.

The two documents are identical apart from the `x-reportage-snapshot` snapshot normalization metadata, which the internal source carries and the generated public schema does not. `schema.internal.json` is repository tooling input rather than an external compatibility contract: its annotations and its path may change whenever the repository's own tooling needs change. See [`docs/adr/20260727T151234Z_json-schema-artifact-generation.md`](../../../docs/adr/20260727T151234Z_json-schema-artifact-generation.md).

### Validation

CI validates each representative fixture run's `result.json` against this schema with the [`jsonschema`](https://docs.rs/jsonschema/) crate, inside `cargo nextest`. Both the internal source schema and the generated public schema are checked, and must agree.

Two further suites check properties the schema does not state: [`crates/reportage-cli/tests/run_result_fixtures.rs`](../../../crates/reportage-cli/tests/run_result_fixtures.rs) deserialises the manifest into typed Rust structs as a consumer compatibility test, and checks evidence integrity, document-local invariants, and projection parity. [`crates/reportage-cli/tests/json_contract_schemas.rs`](../../../crates/reportage-cli/tests/json_contract_schemas.rs) validates the schema documents themselves and exercises each JSON Schema keyword the contract relies on against hand-built valid and invalid instances.

Because `result.json` is the canonical manifest, the typed consumer structs model the full stable contract this schema defines — every expectation kind, observation enum, and diagnostic shape — not only the shapes the representative fixtures happen to exercise. That is a requirement on the Rust consumer model; contract coverage itself comes from the schema validator. See [`docs/adr/20260728T092956Z_json-contract-validation-policy.md`](../../../docs/adr/20260728T092956Z_json-contract-validation-policy.md) for what CI does and does not guarantee.

## Representative fixtures

`tests/fixtures/run_result/*.repor` holds one fixture script per required scenario:

- `passed` — every assertion holds;
- `assertion_failure` — the run completes but an assertion fails;
- `parse_error` — a source file cannot be parsed;
- `semantic_error` — a script-domain rule is violated at evaluation time;
- `runtime_error` — a runtime infrastructure failure before any action ran;
- `partial_execution_after_runtime_error` — evidence recorded before a later runtime error survives;
- `expectation_kinds` — exercises file/dir/text-equals/empty/logical expectation shapes beyond the exit/stdoutContains kinds the scenarios above use;
- `contents_equals` — exercises `fileContentsEquals` / `stdoutContentsEquals` with a workspace expected source, including a bounded `mismatch` object;
- `noop` — valid zero-case input recorded as `noop: true` with empty `tests` and a zeroed summary.

The first six scenarios mirror `tests/fixtures/json_report/`'s required scenario set, so projection parity can be checked over the same run shapes. Each fixture has a companion `<name>.snapshot.json` normalized-output snapshot (`tool.version` replaced by a placeholder), refreshed via `UPDATE_RUN_RESULT_SNAPSHOTS=1`. See `crates/reportage-cli/tests/run_result_fixtures.rs`.

## Compatibility

The pre-#102 artifact `result.json` (snake_case, inline base64 stream envelopes, `result: pass|fail|script_error`) was an early v0 experimental contract. Issue #102 replaced it with this canonical schema as a deliberate breaking change; see [`docs/adr/20260708T130500Z_artifact-run-result-canonical-manifest.md`](../../../docs/adr/20260708T130500Z_artifact-run-result-canonical-manifest.md). The schema remains v0-experimental until the stabilization milestones in [`docs/reference/artifacts.md`](../../../docs/reference/artifacts.md).

## Decision records

- [`docs/adr/20260708T130500Z_artifact-run-result-canonical-manifest.md`](../../../docs/adr/20260708T130500Z_artifact-run-result-canonical-manifest.md) — artifact bundle as canonical record; `result.json` as canonical manifest; `--format=json` as stdout-safe projection; evidence reference policy.
- [`docs/adr/20260728T092956Z_json-contract-validation-policy.md`](../../../docs/adr/20260728T092956Z_json-contract-validation-policy.md) — how this contract is validated in CI, and the boundary between schema validation, consumer compatibility, and domain invariants.
- [`docs/adr/20260707T045900Z_json-output-as-structured-execution-report.md`](../../../docs/adr/20260707T045900Z_json-output-as-structured-execution-report.md) — the diagnostic/failure model both contracts share.
- [`docs/adr/20260707T050000Z_json-stdout-and-captured-output-artifact-contract.md`](../../../docs/adr/20260707T050000Z_json-stdout-and-captured-output-artifact-contract.md) — CLI stdout vs. captured stdout/stderr; artifact reference policy.
