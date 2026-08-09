# JSON Execution Report

This directory records the external contract for `reportage run --format=json`'s CLI stdout document.

## Relationship to other machine-readable outputs

This is **not** the same document as `result.json`, the artifact manifest written under `artifactRoot` by `ArtifactWriter::write` (see [`spec/artifacts/run-result/schema.json`](../../artifacts/run-result/schema.json), issue #102). The artifact bundle is the canonical record of a run, and the `--format=json` document defined here is a stdout-safe projection derived from the canonical run result document: it adds `artifactRoot`, and omits the artifact-only `noop` field and the evidence `sha256` digests. The two contracts version independently; projection parity is verified by `crates/reportage-cli/tests/run_result_fixtures.rs`. See [`docs/adr/20260708T130500Z_artifact-run-result-canonical-manifest.md`](../../../docs/adr/20260708T130500Z_artifact-run-result-canonical-manifest.md).

## JSON Schema

`schema.json` defines the expected structure of the `--format=json` document and is useful for editor integration (autocomplete, inline validation). It is the stable path for external consumers.

### Which file to edit

`schema.internal.json` is the only file to edit. `schema.json` is generated from it by `just schema-artifacts-gen` and must never be hand-edited; `just schema-artifacts-check`, which `just check` runs, fails when the committed file drifts from what the internal source generates.

The two documents are identical apart from the `x-reportage-snapshot` snapshot normalization metadata, which the internal source carries and the generated public schema does not. `schema.internal.json` is repository tooling input rather than an external compatibility contract: its annotations and its path may change whenever the repository's own tooling needs change. See [`docs/adr/20260727T151234Z_json-schema-artifact-generation.md`](../../../docs/adr/20260727T151234Z_json-schema-artifact-generation.md).

### Validation

CI validates each representative fixture's output against this schema with the [`jsonschema`](https://docs.rs/jsonschema/) crate, inside `cargo nextest`. Both the internal source schema and the generated public schema are checked, and must agree.

Two further suites check properties the schema does not state: [`crates/reportage-cli/tests/json_report_fixtures.rs`](../../../crates/reportage-cli/tests/json_report_fixtures.rs) deserialises the output into typed Rust structs as a consumer compatibility test, and checks document-local invariants such as `diagnosticRef` resolution. [`crates/reportage-cli/tests/json_contract_schemas.rs`](../../../crates/reportage-cli/tests/json_contract_schemas.rs) validates the schema documents themselves and exercises each JSON Schema keyword the contract relies on against hand-built valid and invalid instances.

Typed deserialization is not schema validation: it enforces neither `const`, `pattern`, `minimum`, nor conditional requirements. See [`docs/adr/20260728T092956Z_json-contract-validation-policy.md`](../../../docs/adr/20260728T092956Z_json-contract-validation-policy.md) for what CI does and does not guarantee.

## Decision records

- [`docs/adr/20260707T045900Z_json-output-as-structured-execution-report.md`](../../../docs/adr/20260707T045900Z_json-output-as-structured-execution-report.md) — JSON output as a structured execution report, not a human-output derivative; the diagnostic/failure model.
- [`docs/adr/20260707T050000Z_json-stdout-and-captured-output-artifact-contract.md`](../../../docs/adr/20260707T050000Z_json-stdout-and-captured-output-artifact-contract.md) — CLI stdout vs. captured stdout/stderr; artifact reference policy; `processExitCode`.
- [`docs/adr/20260707T050100Z_json-output-schema-and-validation-policy.md`](../../../docs/adr/20260707T050100Z_json-output-schema-and-validation-policy.md) — schema/fixture policy; `location`/`origin` fallback. Its validation policy is superseded.
- [`docs/adr/20260728T092956Z_json-contract-validation-policy.md`](../../../docs/adr/20260728T092956Z_json-contract-validation-policy.md) — how this contract is validated in CI, and the boundary between schema validation, consumer compatibility, and domain invariants.

## Representative fixtures

`tests/fixtures/json_report/*.repor` holds one fixture script per required scenario (passed, assertion failure, parse error, semantic error, runtime error, partial execution after a runtime error, and a `before_each` phase case that makes `step.phase` appear as both `before_each` and `case`), each with a companion `*.snapshot.json` normalized-output snapshot. See `crates/reportage-cli/tests/json_report_fixtures.rs`.
