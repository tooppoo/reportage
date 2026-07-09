# Artifact Run Result as Canonical Manifest

- Status: Accepted
- Created: 2026-07-08T13:05:00Z

## Context

`reportage run` always writes an artifact bundle to `.reportage/runs/<run-id>/` (see [`20260627T100400Z_generate-artifacts-by-default.md`](20260627T100400Z_generate-artifacts-by-default.md)). Before issue #102 its `result.json` used an early, ad-hoc shape: snake_case fields, a `result: pass|fail|script_error` vocabulary, and captured stdout/stderr embedded inline as base64 envelopes (`{"data": ..., "encoding": "base64", "text": ...}`).

Separately, issue #89 defined an external contract for `reportage run --format=json`'s CLI stdout document ([`spec/output/json-report/schema.json`](../../spec/output/json-report/schema.json)): camelCase, a `status: passed|failed|error` vocabulary, document-local ids, a diagnostics model, and captured output referenced by `artifactRef` / `sizeBytes` instead of inlined.

That left two diverging machine-readable descriptions of the same run, with no defined relationship between them: consumers could not tell which one was authoritative, the artifact `result.json` had no schema, no fixtures, and no snapshot coverage, inline base64 made the manifest grow with action output size, and [`docs/artifacts.md`](../artifacts.md) described the artifact shape by hand with nothing detecting drift.

Issue [#102](https://github.com/tooppoo/reportage/issues/102) requires the artifact bundle to be treated as the canonical record of a run, `result.json` as its canonical manifest with a verifiable contract, and `--format=json` as a projection of it — without redesigning the #89 stdout contract.

## Decision

### Artifact bundle as canonical record, `result.json` as canonical manifest

The artifact bundle under `.reportage/runs/<run-id>/` — `result.json` plus the raw evidence files it references — is the canonical record of a `reportage run` execution.

`result.json` is the bundle's canonical manifest: run-level outcome, summary, concrete case results, action results, assertion/expectation results, diagnostics and failure classification, references to raw evidence files, document-local ids, schema version, and tool version.

### One canonical document; `--format=json` is a stdout-safe projection

The canonical run result document is built in one place, `reportage-core/src/run_result.rs` (`build_run_result_document`), and shares the #89 document's camelCase vocabulary and structure. `ArtifactWriter::write` persists it as `result.json`.

The `--format=json` renderer does not build its own document. It derives its stdout document from the canonical one (`reportage-cli/src/render/json.rs`, `project_run_result`), applying exactly three differences:

- add `artifactRoot` (the manifest resolves `artifactRef` against its own directory; a stdout consumer must be told where that directory is);
- drop `noop` (recognizable in the stdout document from empty `tests` and a zeroed summary);
- drop the evidence `sha256` digests (bundle-integrity information, not stdout summary information).

The #89 schema itself is unchanged: the projection reproduces it byte-for-byte, which the pre-existing `json_report_fixtures.rs` snapshots verify. The two contracts remain separate documents with independently versioned `schemaVersion` fields.

Projection parity is additionally verified per representative fixture in `crates/reportage-cli/tests/run_result_fixtures.rs`: field-level checks for the #102 minimum parity items (status, processExitCode, summary, diagnostic code/category/severity, test/action/assertion ids, action exitCode, expectation kind/status, captured stdout/stderr artifactRef/sizeBytes), plus a strict check that applying the three defined differences to `result.json` reproduces the stdout document exactly.

### Raw byte evidence is referenced, never inlined

Captured stdout/stderr bytes are stored only as raw files inside the bundle (`test-<n>/action-<m>/{stdout,stderr}.bin`) and referenced from the manifest as `{ artifactRef, sizeBytes, sha256 }`.

- `artifactRef`: path relative to the directory containing `result.json`.
- `sizeBytes`: byte size of the evidence file.
- `sha256`: required lowercase hex SHA-256 digest of the evidence file's bytes, so a consumer can verify the bundle's evidence file is the one the manifest describes.

Inline base64 is removed: it bloated the manifest proportionally to action output, duplicated bytes that already existed as artifact files, and contradicted the #89 decision ([`20260707T050000Z_json-stdout-and-captured-output-artifact-contract.md`](20260707T050000Z_json-stdout-and-captured-output-artifact-contract.md)) that raw output belongs in artifact files.

### Schema source of truth and validation policy

The canonical contract is defined by a JSON Schema at [`spec/artifacts/run-result/schema.json`](../../spec/artifacts/run-result/schema.json), the human- and tool-facing source of truth.

CI validation is typed Rust deserialization with `#[serde(deny_unknown_fields)]` (`crates/reportage-cli/tests/run_result_fixtures.rs`), following [`20260707T050100Z_json-output-schema-and-validation-policy.md`](20260707T050100Z_json-output-schema-and-validation-policy.md); no external JSON Schema validator is required.

Unlike the #89 typed structs (deliberately a fixture-exercised subset of that schema), the artifact result structs model the full stable contract the schema defines — every expectation kind, observation enum, and diagnostic shape — because the manifest is canonical: a schema/validation gap on a rarely-exercised shape must be visible in review even when no fixture happens to produce it.

Representative fixtures live in `tests/fixtures/run_result/` (passed, assertion_failure, parse_error, semantic_error, runtime_error, partial_execution_after_runtime_error, expectation_kinds, noop), each with a normalized snapshot refreshed via `UPDATE_RUN_RESULT_SNAPSHOTS=1`. Evidence integrity (referenced file exists, sizeBytes and sha256 match) is verified per fixture run.

### Breaking change to the pre-existing artifact contract

The pre-#102 `result.json` shape is treated as an early v0 experimental contract and is replaced, not migrated: no compatibility fields, no dual output. Early v0 artifact contracts may continue to change this way until stabilization (see [`docs/artifacts.md`](../artifacts.md) — Stability).

### Docs boundary: generated / checked / handwritten

[`docs/artifacts.md`](../artifacts.md) declares an explicit boundary:

- JSON examples are marked `<!-- checked-against: <snapshot path> -->` and verified byte-for-byte against fixture snapshots by a test, so representative examples cannot silently drift;
- field lists/enums are enforced by the typed structs, and layout/evidence references by the evidence-integrity test and `e2e/artifacts/run-result-manifest.repor`;
- purpose, positioning, projection relationship, evidence policy, and compatibility policy remain handwritten prose, maintained by review with rationale in this ADR.

No section is machine-generated today; "generated or checked" is satisfied by the checks above.

### `artifact` stays out of the semantic rule catalog

Per issue #85's decision, artifact/result JSON shapes are not semantic language rules: they describe runtime output contracts, not `.repor` language semantics. They are therefore specified under `spec/artifacts/` with schema/fixture/snapshot validation, not as a new `artifact` category in `spec/language/semantics/`.

## Alternatives Considered

### Keep two independent document builders and verify parity by tests only

Keep the old artifact `build_json` alongside the #89 renderer and add cross-document parity assertions.

Rejected: two ~500-line builders describing the same run would drift continually, and every vocabulary difference (snake_case vs camelCase, `result` vs `status`) would need a permanent translation table in the parity test. Deriving the stdout document from the canonical one makes the projection relation hold by construction and leaves one place to change.

### Inline evidence with a size threshold

Keep small captured outputs inline (base64) and reference only large ones.

Rejected: a threshold makes the manifest shape input-dependent, forces every consumer to implement both paths, and keeps the duplicated-bytes problem for the inline case. A single reference shape with `sha256` is simpler and always verifiable.

### Make `--format=json` print the canonical document as-is

Rejected: that would be a redesign of the #89 external contract (adding `noop` and `sha256`, changing `artifactRoot` semantics), which issue #102 explicitly excludes. The stdout contract stays as defined; only its implementation becomes a projection.

### Model only fixture-exercised shapes in the typed validation structs

Mirror `json_report_fixtures.rs`'s subset approach.

Rejected for the canonical manifest: issue #102 requires the typed structs to cover the schema's full stable contract so that schema/validation drift is reviewable even for shapes no representative fixture produces.

## Consequences

### Positive Consequences

- One authoritative description of a run; the stdout document is derivable from it by construction.
- The artifact contract is now schema-defined, fixture/snapshot-pinned, and CI-validated.
- Manifest size no longer grows with captured output; evidence is verifiable via `sha256`.
- Docs examples cannot silently drift from real output.

### Negative Consequences

- Breaking change: consumers of the old snake_case/inline-base64 `result.json` must migrate.
- Reading captured output now always requires opening the referenced evidence file.
- The `sha256` digest adds a small cost per action to artifact writing.

### Neutral Consequences

- `reportage-core` gains a `sha2` dependency.
- Step indices and per-step counts of the old shape are no longer recorded in the manifest; failure positions are carried by diagnostics instead.
- The two `schemaVersion` fields currently share the value `1` but version independently.
