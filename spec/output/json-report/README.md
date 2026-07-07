# JSON Execution Report

This directory records the external contract for `reportage run --format=json`'s CLI stdout document.

## Relationship to other machine-readable outputs

This is **not** the same document as `result.json`, the pre-existing artifact written under `artifactRoot` by `ArtifactWriter::write` (see [`docs/adr/20260706T001018Z_separate-execution-report-from-output-rendering.md`](../../../docs/adr/20260706T001018Z_separate-execution-report-from-output-rendering.md)). `result.json` uses its own, older, snake_case vocabulary (`result: pass|test_failed|script_error|config_error|runtime_error`, see [`docs/exit-codes.md`](../../../docs/exit-codes.md)) and predates this schema. The `--format=json` document defined here uses camelCase and a different `status` vocabulary (`passed|failed|error`). This divergence is intentional and documented, not a bug — see the ADRs below.

## JSON Schema

`schema.json` defines the expected structure of the `--format=json` document and is useful for editor integration (autocomplete, inline validation).

CI validation is performed by typed Rust deserialization in `crates/reportage-cli/tests/json_report_fixtures.rs`, following the same approach as `spec/language/semantics/schema.json` / `crates/reportage-core/tests/semantic_specs.rs`: each fixture's JSON output is deserialised into Rust structs marked `#[serde(deny_unknown_fields)]`, which rejects unknown fields and enforces required fields and enum constraints, without an external JSON Schema validator dependency.

## Decision records

- [`docs/adr/20260707T045900Z_json-output-as-structured-execution-report.md`](../../../docs/adr/20260707T045900Z_json-output-as-structured-execution-report.md) — JSON output as a structured execution report, not a human-output derivative; the diagnostic/failure model.
- [`docs/adr/20260707T050000Z_json-stdout-and-captured-output-artifact-contract.md`](../../../docs/adr/20260707T050000Z_json-stdout-and-captured-output-artifact-contract.md) — CLI stdout vs. captured stdout/stderr; artifact reference policy; `processExitCode`.
- [`docs/adr/20260707T050100Z_json-output-schema-and-validation-policy.md`](../../../docs/adr/20260707T050100Z_json-output-schema-and-validation-policy.md) — schema/validation/fixture policy; `location`/`origin` fallback.

## Representative fixtures

`tests/fixtures/json_report/*.repor` holds one fixture script per required scenario (passed, assertion failure, parse error, semantic error, runtime error, partial execution after a runtime error), each with a companion `*.snapshot.json` normalized-output snapshot. See `crates/reportage-cli/tests/json_report_fixtures.rs`.
