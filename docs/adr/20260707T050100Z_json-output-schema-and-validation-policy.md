# JSON Output Schema and Validation Policy

- Status: Accepted
- Created: 2026-07-07T05:01:00Z

## Context

`reportage run --format=json`'s output is a machine-readable contract that AI, CI, and external tooling depend on. It should not simply be whatever `serde_json::json!` happens to produce from internal Rust values (`crates/reportage-cli/src/render/json.rs` builds `serde_json::Value` directly, with no typed structs on the producer side) — that leaves consumers with no explicit, checkable definition of what fields exist, which are required, and which values are allowed.

At the same time, forcing the external JSON document to be exactly isomorphic to the internal `ExecutionReport` would leak internal implementation choices into the external contract, making internal refactors into external breaking changes.

An unbounded, unconstrained set of JSON fields also makes it hard for a consumer to know what they may safely depend on, and for CI to catch accidental field drift.

Reportage already has a directly analogous precedent for this exact problem, for a different machine-readable contract: `spec/language/semantics/`. Its ADR, [`20260630T000000Z_json-semantic-specs.md`](20260630T000000Z_json-semantic-specs.md), decided to maintain a JSON Schema file for editor tooling, but to perform CI validation via typed Rust deserialization (`#[serde(deny_unknown_fields)]`) rather than an external JSON Schema validator (ajv, jsonschema-cli), specifically to avoid adding a non-Rust step to the `cargo nextest` pipeline.

Related issue: [#89](https://github.com/tooppoo/reportage/issues/89). See [`20260707T045900Z_json-output-as-structured-execution-report.md`](20260707T045900Z_json-output-as-structured-execution-report.md) and [`20260707T050000Z_json-stdout-and-captured-output-artifact-contract.md`](20260707T050000Z_json-stdout-and-captured-output-artifact-contract.md) for the decisions this ADR's schema encodes.

## Decision

The `--format=json` document's structure is defined by a JSON Schema at [`spec/output/json-report/schema.json`](../../spec/output/json-report/schema.json).

This schema defines the *external* contract. It is not required to be isomorphic to the internal `ExecutionReport`: the internal model may keep whatever shape is convenient for implementation, while the external JSON prioritizes compatibility, stability, and machine readability.

External JSON field naming uses camelCase (`schemaVersion`, `processExitCode`, `artifactRoot`, `artifactRef`, `diagnosticRef`, `exitCode`, `sizeBytes`, ...), matching the convention already used by `spec/language/semantics/*.json`. Internal Rust field/type names may follow ordinary Rust convention regardless.

### Schema validation via typed Rust structs, not an external validator

CI validation is performed by deserializing each representative fixture's JSON output into typed Rust structs marked `#[serde(deny_unknown_fields)]`, in `crates/reportage-cli/tests/json_report_fixtures.rs`, following the same rationale as `crates/reportage-core/tests/semantic_specs.rs`: this keeps validation inside `cargo nextest` without adding a Node.js/Python-based external JSON Schema validator to CI. `schema.json` itself remains the authoritative human/tool-facing definition (for editor autocomplete and for consumers outside this repository); the typed Rust structs are how *this* repository enforces it in CI.

### `schemaVersion` and `additionalProperties`

The JSON document carries a top-level `schemaVersion`, distinct from the reportage crate/CLI version (`tool.version`). `schemaVersion` versions this external contract specifically.

`additionalProperties: false` is used wherever an object is part of the stable external contract: the top-level document, `diagnostics[]` entries, `tests[]`/`actions[]`/`assertions[]` entries, and every expectation-kind object.

Where an object is intentionally left open (in v0, only `shimInvocations[]` entries, which are emitted by the shim runtime rather than this renderer and may gain fields independently of this document's `schemaVersion`), the schema's `description` states why `additionalProperties` is not `false` there, rather than leaving it silently unconstrained.

v0 does not allow unknown top-level fields. An unrecognized top-level field is a schema violation, not something a consumer should be expected to tolerate.

### Document-local ids are not long-term stable identifiers

Ids inside a JSON document (`diagnostics[].id`, `tests[].id`, `tests[].actions[].id`, `tests[].assertions[].id`) are unique within that one document and usable for in-document cross-referencing (e.g. `diagnosticRef`). They are deterministic for a given input and execution order, which supports snapshot testing, but they are not a stable identifier across separate runs and must not be treated as one by consumers.

### Fixture and snapshot validation policy

Representative fixtures are checked in at `tests/fixtures/json_report/*.repor`, covering at least: passed, assertion failure, parse error, semantic error, runtime error, and partial execution after a runtime error (at least one action/assertion recorded before a later runtime error brings the top-level status to `error`).

Every representative fixture's JSON output must pass schema validation (the typed-struct deserialization above). Each fixture also has a companion normalized snapshot (`<name>.snapshot.json`) with volatile fields (the run-id-derived `artifactRoot`, `tool.version`) replaced by fixed placeholders before comparison, refreshed via an `UPDATE_JSON_REPORT_SNAPSHOTS` environment variable, mirroring `syntax_conformance.rs`'s `UPDATE_AST_SNAPSHOTS` convention.

Comparison against human-readable output is against the *semantic information inventory* listed in [`20260707T045900Z_json-output-as-structured-execution-report.md`](20260707T045900Z_json-output-as-structured-execution-report.md), not against display wording: a fixture's test asserts that a given piece of structured information (e.g. a diagnostic code, a pass/fail outcome) is derivable from both renderers' output for the same run, not that their text matches.

### `location` and `origin` fallback

Each diagnostic carries `location` (`null`, or `{line, column?}`) and `origin` (`{kind: "source", source}` or `{kind: "test", test}`).

`location` is populated when a diagnostic is a parse-domain diagnostic (`category: "parse"`): `parser::ParseError::to_diagnostic()` already computes a real line, and for syntax errors a column, for every `ParseError` variant (`crates/reportage-core/src/parser.rs`). This ADR is what makes threading that value through into `FileErrorKind::ParseError` and `JsonRenderer`'s `diagnostics[]` in scope for issue #89, and out of scope is only extending source-range tracking to the evaluator side (semantic/assertion diagnostics), which this ADR explicitly defers.

For every other diagnostic category (`semantic`, `runtime`, `assertion`, `internal`), `location` is `null` and `origin` is the fallback: it identifies which source file or which test the diagnostic came from, without a line/column. Extending source-range tracking to the evaluator is deferred to a future issue.

## Alternatives Considered

### Validate using an external JSON Schema validator (ajv, jsonschema-cli)

Deferred, for the same reason [`20260630T000000Z_json-semantic-specs.md`](20260630T000000Z_json-semantic-specs.md) deferred it: it would require a Node.js or Python step in CI. Typed Rust deserialization with `deny_unknown_fields` provides equivalent structural validation inside the existing `cargo nextest` pipeline. If an external validator is added later, it should complement, not replace, the Rust tests.

### Make the external JSON document isomorphic to `ExecutionReport`

Rejected. This would make every internal refactor of `ExecutionReport` a potential external breaking change, and would leak Rust-internal modeling choices (e.g. `Option` nesting convenient for pattern matching) into a document meant for non-Rust consumers.

### Allow unknown top-level fields for forward compatibility

Rejected for v0. Allowing unknown top-level fields would make it harder to detect accidental field drift from a producer bug, and harder for a consumer to know what is and is not part of the contract. A field that genuinely needs to be optional and evolving should be an explicitly-declared optional field, or live inside an object explicitly marked open (like `shimInvocations[]`), with a stated reason.

### Treat document-local ids as stable, long-term identifiers

Rejected. Making ids stable across runs would require either a separate stable-id scheme or accepting nondeterminism costs. v0 only requires ids to support in-document cross-referencing and to be deterministic for snapshot testing, not to be a durable identifier a consumer could store and compare across runs.

## Consequences

### Positive Consequences

- Consumers have an explicit, checkable definition (`schema.json`) of field naming, required fields, enums, and `additionalProperties` behavior.
- `schemaVersion` and `additionalProperties: false` make unintended contract drift a CI-detectable failure rather than a silent change.
- Fixture + snapshot validation gives concrete, checked-in evidence that the six representative scenarios produce schema-valid, semantically complete JSON.
- Parse-domain diagnostics now carry a real `location`, closing a gap the #75 renderer explicitly deferred to this issue.

### Negative Consequences

- Maintaining both `schema.json` and the typed Rust validation structs in `json_report_fixtures.rs` is a duplicated, small ongoing cost, the same trade-off already accepted for `spec/language/semantics/`.
- Changing the JSON contract requires updating `schemaVersion`, the schema, fixtures, snapshots, and this ADR family together; it is not a one-file change.

### Neutral Consequences

- `shimInvocations[]` remains intentionally open (`additionalProperties: true`, with a stated reason) and may evolve independently of `schemaVersion`.
- Extending `location` to semantic/assertion/runtime diagnostics is deferred to a future issue; this ADR only closes the gap for parse-domain diagnostics.
