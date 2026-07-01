# Use JSON for Machine-Readable Semantic Specifications

- Status: Accepted
- Created: 2026-06-30T00:00:00Z

## Context

Reportage's pest grammar defines the syntax layer with enough precision to generate a Rust parser and enforce syntactic correctness. However, pest grammar cannot express semantic behaviour. A grammar can parse `stdout contains "ok"` without being able to prove that `contains` means byte-level substring matching, that the assertion is evaluated against a specific checkpoint, or that an empty expected string always matches.

Semantic rules therefore need a separate, machine-readable source of truth that can be consumed by validators, test runners, and documentation generators.

The design constraint from ADR [executable-language-specification-sources](20260629T140600Z_executable-language-specification-sources.md) is that Reportage must manage its language specification through executable and machine-readable sources. Semantic rules must be represented as machine-readable JSON specifications for v0.

This ADR records the concrete decisions for the JSON semantic spec format, the schema validation approach, the checkpoint bytes representation, and the diagnostic model boundary.

Related issues:

- [#29](https://github.com/tooppoo/reportage/issues/29) — Phase 4: define machine-readable semantic specs in JSON

## Decision

### JSON as the semantic spec format

Semantic rules are defined as JSON files in `spec/language/semantics/`. Each file covers one semantic rule and is named after its stable ID (e.g. `assertion.exit.equals.json`).

JSON is chosen over KDL, TOML, or YAML because:

- JSON Schema tooling is widely available and supports `additionalProperties: false`, enum constraints, and required-field enforcement without custom tooling.
- Serde in Rust can deserialise JSON into typed structs with `deny_unknown_fields`, providing strict structural validation in CI tests.
- JSON is the de facto standard for machine-consumed specification data and is supported by editors, schema validators, and documentation generators without additional adapters.

KDL remains appropriate for user-authored configuration (`reportage.kdl`) where direct editability matters. JSON is preferred here because the primary consumers are tools, not humans.

### Schema validation via typed Rust loading

A JSON Schema file (`spec/language/semantics/schema.json`) is maintained for external tooling and editor support.
Each semantic spec declares that schema with `"$schema": "./schema.json"` so editors and validators can discover the local contract from the spec file itself.

CI validation is performed by Rust tests in `crates/reportage-core/tests/semantic_specs.rs`. Those tests deserialise each spec file into typed Rust structs marked `#[serde(deny_unknown_fields)]`. Unknown fields cause a deserialisation error; missing required fields cause a type mismatch. This approach combines schema validation with typed loading in a single step and integrates directly with the existing `cargo nextest` pipeline.

### No free-form prose in v0 semantic specs

Semantic spec files must not include free-form explanatory fields (`notes`, `explanation`, `aiNote`, `rationale`, `status: tbd`) in v0. Deferred or open design questions belong in `docs/TBD.md`.

This constraint keeps the normative content machine-checkable and avoids creating unchecked natural-language fragments inside spec files.

### Structured normative fields

Each spec's `normative` object carries structured, machine-processable data:

- `checkpointField` — which checkpoint field the expectation reads.
- `operator` — the matching operator (`equals` or `contains`).
- `expectedValueType` — the type of the expected value.
- `matchSemantics` — a structured object describing comparison behaviour.

`matchSemantics` is not a prose string. It contains typed fields such as `comparison`, `caseSensitive`, `lineEndingNormalization`, and `emptyExpectedAlwaysMatches`.

### Static conformance case data

Each semantic spec includes at least one conformance case. Conformance cases carry enough data for the semantic evaluator to be exercised without executing an external command:

- `assertionSource` — the Reportage assertion text.
- `assertion` — the normalised assertion representation (`subject`, `operator`, `expected`).
- `checkpoint` — static checkpoint data (exit code, stdout, stderr).
- `expectedResult` — `"pass"` or `"fail"`.

Conformance cases are static fixture data, not live runtime outputs.

### Checkpoint bytes as base64-encoded raw bytes

Checkpoint `stdout` and `stderr` fields in conformance cases use the following representation:

```json
{
  "data": "aGVsbG8K",
  "encoding": "base64",
  "text": "hello\n"
}
```

- `data` is the normative byte representation, base64-encoded.
- `encoding` is always `"base64"` in v0.
- `text` is an optional human-readable helper. It is not used for semantic comparison or machine access. If present, it must equal the UTF-8 decoding of the base64-decoded `data`. Fixture loading validates this consistency.

Raw bytes are the normative form because stdout and stderr are byte streams. Base64 encoding makes binary-safe fixture data representable in JSON.

### Byte-level substring match for stdout/stderr contains

`stdout contains <string>` and `stderr contains <string>` are defined as byte-level substring matches in v0:

- The expected string literal is treated as UTF-8 bytes.
- stdout and stderr are treated as raw bytes.
- No line-ending normalisation is performed.
- Matching is case-sensitive.
- An empty expected string always matches (the empty byte sequence is a substring of any byte sequence).
- Non-UTF-8 output is not rejected solely because of encoding.

Encoding-aware assertions (e.g. comparing Shift-JIS-decoded output) are deferred; see `docs/TBD.md`.

### Diagnostic model is out of scope

Assertion failure diagnostic codes, severity levels, span information, message formatting, and the overall diagnostic model are not defined in this phase. They will be addressed in a separate issue.

Conformance case `expectedResult` uses only `"pass"` or `"fail"`. No diagnostic detail fields are included in v0 conformance cases.

### File existence assertion is out of scope

File existence and directory assertions are not covered by this phase. They will be addressed in a separate issue.

## Alternatives Considered

### Use Rust-centric validation only (no JSON Schema file)

Rejected. A JSON Schema file provides value for editor integration (autocomplete, validation in VS Code) and for contributors who want to understand the format without reading Rust code. The cost of maintaining a JSON Schema file alongside the Rust structs is low.

### Validate using an external JSON Schema validator (ajv, jsonschema-cli)

Considered and deferred for v0. An external validator would require adding a Node.js or Python step to CI. The existing Rust-based serde deserialization with `deny_unknown_fields` provides equivalent structural validation within the current `cargo nextest` pipeline. If an external validator is added later, it should complement the Rust tests rather than replace them.

### Allow prose in normative fields

Rejected. Prose fields are unchecked and create a second, un-typed source of truth. They also make documentation generation harder because prose cannot be machine-processed. Deferred questions belong in `docs/TBD.md`.

### Store conformance cases separately from spec files

Rejected. Co-locating conformance cases with the rule definition keeps the spec self-contained. It also ensures that a new semantic rule ships with its own conformance evidence.

### Use KDL for semantic specs

Rejected. See [executable-language-specification-sources ADR](20260629T140600Z_executable-language-specification-sources.md). JSON has stronger tooling for schema validation, typed loading through Serde, and CI integration. KDL remains appropriate for user-authored configuration.

### Include text representation as normative in conformance cases

Rejected. `text` is a developer convenience for reviewing and editing fixtures. The normative bytes are in `data`. Semantic comparison uses `data`. Making `text` normative would require specifying encoding, handling non-UTF-8 output, and adding line-ending rules that contradict the byte-level match semantics.

## Consequences

### Positive

- Semantic rules are machine-readable and structurally validated in CI.
- Unknown fields in spec files are rejected by both JSON Schema and Rust deserialization.
- Conformance cases provide static, executable test data without external command execution.
- The byte-level match semantics for stdout/stderr contains are explicit and testable.
- The format boundary is clear: KDL for user configuration, JSON for machine-consumed specs.

### Negative

- JSON is more verbose than KDL or Markdown prose.
- Maintaining a JSON Schema file and Rust structs is a small ongoing cost.
- Semantic specs are minimal in v0 because free-form explanatory prose is excluded.

### Neutral

- Diagnostic model, encoding-aware assertions, and file existence assertions are deferred to separate issues.
- The `text` field in checkpoint stream data is optional and carries no semantic weight.
- Future documentation generation from semantic spec JSON is not implemented in this phase.

## Non-Goals

This ADR does not define the full v0 semantic model. It defines how semantic rules are represented and validated.

This ADR does not require documentation generation from semantic spec JSON. That is addressed in a later phase.

This ADR does not define the diagnostic model, diagnostic codes, or assertion failure detail format.
