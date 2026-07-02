# Semantic Specs

This document records the conventions used to write and identify semantic specs in `spec/language/semantics/`.

## Semantic spec ID format

Semantic spec IDs are stable identifiers. Renaming an ID is a breaking change.

Each ID uses the form:

```
<category>.<subject>.<operator-or-form>
```

- **category** — the broad classification of the rule. In v0, the only defined value is `assertion`.
- **subject** — the checkpoint subject being verified. Examples: `exit`, `stdout`, `stderr`.
- **operator-or-form** — the expectation operator or assertion form. Examples: `equals`, `contains`.

### v0 IDs

| ID | Syntax form |
|----|-------------|
| `assertion.exit.equals` | `exit <code>` |
| `assertion.stdout.contains` | `stdout contains <string>` |
| `assertion.stderr.contains` | `stderr contains <string>` |

Note: `exit <code>` does not spell out `equals` in syntax, but the semantic rule treats it as an exit code equals expectation.

## Semantic spec file location

Semantic spec files live in `spec/language/semantics/`. Each file is named after its ID:

```
spec/language/semantics/<id>.json
```

Example: `spec/language/semantics/assertion.exit.equals.json`.

## JSON Schema

`spec/language/semantics/schema.json` defines the expected structure and is useful for editor integration (autocomplete, inline validation in VS Code and similar tools).

CI validation is performed by typed Rust deserialization in `crates/reportage-core/tests/semantic_specs.rs`. Each spec file is deserialised into Rust structs marked with `#[serde(deny_unknown_fields)]`, which rejects unknown fields and enforces required fields and enum constraints. The same test module runs every conformance case against the production semantic evaluator by converting the normalised assertion representation and checkpoint data into evaluator inputs. Parser/source consistency is checked separately and is not the primary purpose of semantic conformance.

The diagnostic code contract is defined in [`docs/semantic-diagnostics.md`](../../../docs/semantic-diagnostics.md). Expected diagnostic code checks remain optional: cases that carry an `expectedDiagnosticCode` can have that code verified once semantic conformance enables code verification (a follow-up to #41); cases without one are verified by pass/fail result only.

The generated human-readable semantic documentation lives at `docs/language/semantics.md`. The entire file is generated from these JSON specs and must not be edited directly. Run `just semantic-docs-gen` to regenerate it and `just semantic-docs-check` to verify that the checked-in copy is fresh.

## Required fields

Every semantic spec must include:

- `$schema` — relative JSON Schema path; currently `"./schema.json"`.
- `schemaVersion` — integer, currently `1`.
- `id` — stable string identifier in `<category>.<subject>.<operator-or-form>` form.
- `category` — enum value; currently `"assertion"`.
- `syntax` — the Reportage syntax form this rule covers.
- `normative` — structured normative fields (see below).
- `conformanceCases` — at least one static conformance case.

## Normative fields

The `normative` object carries structured, machine-processable normative data. It must include:

- `checkpointField` — which field of the checkpoint this expectation reads.
- `operator` — the matching operator (`"equals"` or `"contains"`).
- `expectedValueType` — the type of the expected value (e.g. `"uint8"`, `"utf8String"`).
- `matchSemantics` — structured description of the comparison behaviour (see below).

## Match semantics

The `matchSemantics` object describes how comparison is performed. It must include:

- `comparison` — the comparison kind: `"exact"` for equality, `"byteSubstring"` for substring search.

For `"byteSubstring"` comparisons, additional boolean fields document the behaviour:

- `caseSensitive` — whether matching is case-sensitive.
- `lineEndingNormalization` — whether line endings are normalised before comparison.
- `emptyExpectedAlwaysMatches` — whether an empty expected value always matches.

## Conformance cases

Each conformance case provides enough static data to run the semantic evaluator without executing an external command. Each case includes:

- `description` — a short sentence describing what the case verifies.
- `assertionSource` — the Reportage assertion source text.
- `assertion` — the normalised assertion representation: `subject`, `operator`, and `expected`.
- `checkpoint` — static checkpoint data used as input to the semantic evaluator.
- `expectedResult` — either `"pass"` or `"fail"`.
- `expectedDiagnosticCode` — optional diagnostic code string for diagnostic-code conformance. The value must be a dot-separated diagnostic code as defined in [`docs/semantic-diagnostics.md`](../../../docs/semantic-diagnostics.md) (e.g. `assertion.stdout.contains_mismatch`). Until semantic conformance enables code verification, CI may ignore this field and verify only `expectedResult`.

### Checkpoint bytes representation

Checkpoint `stdout` and `stderr` fields carry raw bytes encoded as base64:

```json
{
  "data": "aGVsbG8K",
  "encoding": "base64",
  "text": "hello\n"
}
```

- `data` is the normative byte representation.
- `encoding` is always `"base64"` in v0.
- `text` is an optional human-readable helper. It is not used for semantic comparison.
- If `text` is present, it must equal the UTF-8 decoding of the base64-decoded `data`. This is a fixture consistency constraint, not a semantic rule.

## What v0 semantic specs must not contain

Semantic spec files must not include:

- `notes`
- `explanation`
- `aiNote`
- `rationale`
- `status: tbd`
- Free-form prose in normative fields

Deferred or unresolved semantic questions belong in `docs/TBD.md`, not in spec files.
