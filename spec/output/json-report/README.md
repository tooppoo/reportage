# JSON Execution Report

This directory records the external contract for `reportage run --format=json`'s CLI stdout document.

## Relationship to other machine-readable outputs

This is **not** the same document as `result.json`,
the artifact manifest written under `artifactRoot` by `ArtifactWriter::write`
(see [`spec/artifacts/run-result/schema.json`](../../artifacts/run-result/schema.json), issue #102).
The artifact bundle is the canonical record of a run,
and the `--format=json` document defined here is a stdout-safe projection derived from the canonical run result document:
it adds `artifactRoot`, and omits the artifact-only `noop` field and the evidence `sha256` digests.
The two contracts version independently;
projection parity is verified by `crates/reportage-cli/tests/run_result_fixtures.rs`.
See [`docs/adr/20260708T130500Z_artifact-run-result-canonical-manifest.md`](../../../docs/adr/20260708T130500Z_artifact-run-result-canonical-manifest.md).

## JSON Schema

`schema.json` defines the expected structure of the `--format=json` document and is useful for editor integration (autocomplete, inline validation).

CI validation is performed by typed Rust deserialization in `crates/reportage-cli/tests/json_report_fixtures.rs`, following the same approach as `spec/language/semantics/schema.json` / `crates/reportage-core/tests/semantic_specs.rs`: each fixture's JSON output is deserialised into Rust structs marked `#[serde(deny_unknown_fields)]`, which rejects unknown fields and enforces required fields and enum constraints, without an external JSON Schema validator dependency.

## Decision records

- [`docs/adr/20260707T045900Z_json-output-as-structured-execution-report.md`](../../../docs/adr/20260707T045900Z_json-output-as-structured-execution-report.md) — JSON output as a structured execution report, not a human-output derivative; the diagnostic/failure model.
- [`docs/adr/20260707T050000Z_json-stdout-and-captured-output-artifact-contract.md`](../../../docs/adr/20260707T050000Z_json-stdout-and-captured-output-artifact-contract.md) — CLI stdout vs. captured stdout/stderr; artifact reference policy; `processExitCode`.
- [`docs/adr/20260707T050100Z_json-output-schema-and-validation-policy.md`](../../../docs/adr/20260707T050100Z_json-output-schema-and-validation-policy.md) — schema/validation/fixture policy; `location`/`origin` fallback.

## Representative fixtures

`tests/fixtures/json_report/*.repor` holds one fixture script per required scenario (passed, assertion failure, parse error, semantic error, runtime error, partial execution after a runtime error), each with a companion `*.snapshot.json` normalized-output snapshot. See `crates/reportage-cli/tests/json_report_fixtures.rs`.
