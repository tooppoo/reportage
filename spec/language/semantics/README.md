# Semantic Specs

This document records the conventions used to write and identify semantic specs in `spec/language/semantics/`.

## Source of truth

Two different things are each a source of truth here, for different questions:

- **Rust const registry** (`reportage_core::semantic_rule_registry::SEMANTIC_RULE_REGISTRY`, `#[doc(hidden)]` in [`crates/reportage-core/src/semantic_rule_registry.rs`](../../../crates/reportage-core/src/semantic_rule_registry.rs)) is the source of truth for which semantic rules exist, their category, and whether each one requires a spec file, conformance cases, and a generated docs entry. It is a spec coverage inventory, not runtime implementation.
- **Semantic spec JSON** (this directory) is the source of truth for each rule's normative fields and conformance cases.
- **The generated semantic rule catalog** ([`docs/language/semantic-rules.md`](../../../docs/language/semantic-rules.md)) is read-only documentation generated from the semantic spec JSON; it assumes the registry and the specs already agree.

`just semantic-rule-coverage-check` verifies that the registry and this directory agree: every rule the registry marks `spec_required=true` must have a spec file here, and every spec file here must have a corresponding registry entry. See [`docs/adr/20260708T065700Z_semantic-rule-coverage-registry.md`](../../../docs/adr/20260708T065700Z_semantic-rule-coverage-registry.md) for the full rationale.

## Semantic spec ID format

Semantic spec IDs are stable identifiers. Renaming an ID is a breaking change.

Each ID uses the form:

```
<category>.<subject>.<operator-or-form>
```

- **category** — the broad classification of the rule. v0 defines three: `assertion`, `logical-composition`, and `value-reference`.
- **subject** — the checkpoint subject being verified (for `assertion`), the composition target (for `logical-composition`), or the literal/reference kind being resolved (for `value-reference`). Examples: `exit`, `stdout`, `file`, `dir`, `expectation`, `workspace-path`, `fixture-reference`.
- **operator-or-form** — the expectation operator or assertion form. Examples: `equals`, `contains`, `not`, `all`, `resolve`, `kind-mismatch`.

`subject` and `operator-or-form` may themselves be kebab-case when the concept they name is a compound one (e.g. `file-contents-reference`, `kind-mismatch`); this does not change the three-part, dot-separated shape of the id.

### `assertion` category

`assertion` rules verify a single checkpoint field (`exitCode`, `stdout`, `stderr`, `file`, or `dir`) against an expected value using one operator.

- `assertion.exit.equals`
- `assertion.stdout.contains`
- `assertion.stderr.contains`
- `assertion.stdout.empty`
- `assertion.stderr.empty`
- `assertion.file.exists`
- `assertion.file.contains`
- `assertion.file.contents_equals`
- `assertion.file.text_equals`
- `assertion.dir.exists`
- `assertion.dir.contains`

Each rule's syntax form is normative in its own spec file's `syntax` field, not restated here; see the generated catalog at [`docs/language/semantic-rules.md`](../../../docs/language/semantic-rules.md) for the full ID-to-syntax mapping.

Note: `exit <code>` does not spell out `equals` in syntax, but the semantic rule treats it as an exit code equals expectation.

`assertion.stdout.contents_equals` and `assertion.stderr.contents_equals` are known rules in the Rust const registry (see below) but do not yet have a semantic spec; they are out of scope for the issue that introduced the other `assertion` rules above.

### `logical-composition` category

`logical-composition` rules verify block-form logical composition (`not`/`all`/`any`) over nested expectations. A composition is not a checkpoint-field comparison, so its normative fields and conformance cases use a different shape from `assertion` (see below).

- `logical-composition.expectation.not`
- `logical-composition.expectation.all`
- `logical-composition.expectation.any`

Each rule's syntax form is normative in its own spec file's `syntax` field, not restated here; see the generated catalog at [`docs/language/semantic-rules.md`](../../../docs/language/semantic-rules.md) for the full ID-to-syntax mapping.

### `value-reference` category

`value-reference` rules verify acceptance/rejection of a literal or reference, not a checkpoint comparison. Each rule's normative fields are a free-form (but non-empty, banned-key-free) object, because these rules are heterogeneous point-facts rather than a uniform operator/comparison model.

| ID | What it governs |
|----|------------------|
| `value-reference.workspace-path.resolve` | `<"path">` lexical validation (empty/absolute/`.`/`..` segment rejection), shared by the `dir` subject, `write` step path, and a `contents_equals` workspace expected value. |
| `value-reference.fixture-reference.resolve` | `@"path"` lexical validation plus filesystem resolution relative to `repor_dir` (missing/not-a-regular-file/escapes-repor-directory rejection). |
| `value-reference.file-contents-reference.resolve` | Resolution of a `contents_equals` expected value, which is `FileContentsReference = WorkspacePath \| FixtureReference`. The `Fixture` variant's own resolution errors are owned by `value-reference.fixture-reference.resolve`, not duplicated here. |
| `value-reference.literal.kind-mismatch` | The argument-position kind check that rejects a literal of the wrong surface kind (`"..."` / `<"...">` / `@"..."`) at a position whose signature requires a different kind. |

`value-reference.file-path.validate` and `value-reference.dir-entry-name.validate` are also known rules in the Rust const registry but do not yet have a semantic spec.

`fixture-reference` is a `value-reference` rule in its own right, not a sub-concept of `workspace-path`: the two share a lexical validation shape (empty/absolute/dot-segment) but resolve against different roots (the case workspace vs. `repor_dir`) and have independent diagnostic codes.

Permission-based rejections (`assertion.dir.contains.subject_unreadable`, `semantic.file_contents_reference.read_error`) are documented in normative fields but intentionally have no conformance case: reproducing them portably would require `chmod`-based fixtures, which are unreliable in CI environments that run as root (root bypasses permission bits). Adding dedicated coverage for these codes is tracked as follow-up work, not this issue's scope.

## Semantic spec file location

Semantic spec files live in `spec/language/semantics/`. Each file is named after its ID:

```
spec/language/semantics/<id>.json
```

Example: [`spec/language/semantics/assertion.exit.equals.json`](assertion.exit.equals.json).

## JSON Schema

[`spec/language/semantics/schema.json`](schema.json) defines the expected structure and is useful for editor integration (autocomplete, inline validation in VS Code and similar tools).

CI validation is performed by typed Rust deserialization in [`crates/reportage-core/tests/semantic_specs.rs`](../../../crates/reportage-core/tests/semantic_specs.rs). Each spec file's top-level shape is deserialised into a Rust struct marked with `#[serde(deny_unknown_fields)]`; its `normative` field is then deserialised a second time into a category-specific struct (`assertion` and `logical-composition` categories) or checked as a non-empty, banned-key-free object (`value-reference` category), so unknown fields, missing required fields, and enum constraints are all rejected. The same test module runs every eval-shaped conformance case against the production semantic evaluator by converting the normalised assertion representation and checkpoint data into evaluator inputs, and every parser-shaped conformance case against the production parser directly. Parser/source consistency for eval-shaped cases is checked separately and is not the primary purpose of semantic conformance.

The diagnostic code contract is defined in [`docs/semantic-diagnostics.md`](../../../docs/semantic-diagnostics.md). Expected diagnostic code checks remain optional: cases that carry an `expectedDiagnosticCode` can have that code verified once semantic conformance enables code verification (a follow-up to #41); cases without one are verified by pass/fail result only.

The generated semantic rule catalog lives at [`docs/language/semantic-rules.md`](../../../docs/language/semantic-rules.md). The entire file is generated from these JSON specs and must not be edited directly. Run `just semantic-docs-gen` to regenerate it and `just semantic-docs-check` to verify that the checked-in copy is fresh.

## Required fields

Every semantic spec must include:

- `$schema` — relative JSON Schema path; currently `"./schema.json"`.
- `schemaVersion` — integer, currently `1`.
- `id` — stable string identifier in `<category>.<subject>.<operator-or-form>` form.
- `category` — enum value: `"assertion"`, `"logical-composition"`, or `"value-reference"`.
- `syntax` — the Reportage syntax form this rule covers.
- `normative` — structured normative fields (see below); shape depends on `category`.
- `conformanceCases` — at least one conformance case (see below); shape depends on the rule.

## Normative fields

### `assertion` category

The `normative` object carries structured, machine-processable normative data. It must include:

- `checkpointField` — which field of the checkpoint this expectation reads: `"exitCode"`, `"stdout"`, `"stderr"`, `"file"`, or `"dir"`.
- `operator` — the matching operator: `"equals"`, `"contains"`, `"empty"`, `"exists"`, `"contentsEquals"`, or `"textEquals"`.
- `expectedValueType` — the type of the expected value: `"uint8"`, `"utf8String"`, `"fileContentsReference"`, or `"none"` (for `exists`/`empty`, which take no operand).
- `matchSemantics` — structured description of the comparison behaviour (see below).
- `referencedValueReferenceRule` — optional cross-reference to the `value-reference.*` rule ID governing this rule's expected-value resolution, when the expected value is itself a reference (e.g. `assertion.file.contents_equals` references `value-reference.file-contents-reference.resolve`).
- `noImplicitConversionFrom` — optional list of expected-value categories this rule's `expectedValueType` never implicitly converts from (e.g. `assertion.file.text_equals` lists `"fileContentsReference"`, since `TextValue` and `FileContentsReference` do not implicitly convert).

### `logical-composition` category

The `normative` object must include:

- `operator` — `"not"`, `"all"`, or `"any"`.
- `evaluatesAllChildren` — always `true` in v0: every child is evaluated regardless of earlier results, so a failing composition still reports each child's own outcome (no short-circuiting).
- `passCondition` — how the operator derives its own pass/fail from its children's pass/fail: `"notAllChildrenPassed"` (for `not`, which negates the implicit-`all` grouping of its children, not each child individually), `"allChildrenPassed"` (for `all`), or `"anyChildPassed"` (for `any`).
- `emptyBlockPolicy` — always `"semanticError"` in v0: an empty `not`/`all`/`any` block is rejected, never evaluated as vacuously true or false.
- `emptyBlockDiagnosticCode` — the diagnostic code for the empty-block rejection (`"semantic.expectation.empty_block"`).

### `value-reference` category

The `normative` object is a free-form object: at least one property, and none of the banned keys listed below. Its shape is not otherwise fixed by this schema, because `value-reference` rules are heterogeneous point-facts (e.g. which lexical forms a literal rejects, which diagnostic code each rejection produces, which other rule a union type defers to) rather than a uniform operator/comparison model. See the existing `value-reference.*.json` files for the shape each rule currently uses.

## Match semantics

The `matchSemantics` object (an `assertion`-category normative field) describes how comparison is performed. It must include:

- `comparison` — the comparison kind: `"exact"` (scalar equality, e.g. `exit`'s `uint8`), `"byteSubstring"` (raw-byte substring search, `stdout`/`stderr contains`), `"textSubstring"` (UTF-8 text substring search, `file contains`), `"byteExact"` (full-byte-buffer equality with no normalization, `contents_equals`/`text_equals`), `"existence"` (filesystem presence-and-type check, `exists`), `"emptiness"` (byte-length-zero check, `empty`), or `"entryNameEquality"` (exact match against one member of a directory entry name set, `dir contains`).

For `"byteSubstring"` and `"textSubstring"` comparisons, additional boolean fields document the behaviour:

- `caseSensitive` — whether matching is case-sensitive.
- `lineEndingNormalization` — whether line endings are normalised before comparison.
- `emptyExpectedAlwaysMatches` — whether an empty expected value always matches (given any other precondition, such as the file existing and being readable, is met).

## Conformance cases

Not every semantic rule is a checkpoint-field comparison, so `conformanceCases` items use one of two shapes. Every item in the array is checked independently, so a single spec file may mix both shapes.

### Eval cases

An eval case provides enough static data to run the production semantic evaluator without executing an external command. It includes:

- `description` — a short sentence describing what the case verifies.
- `assertionSource` — the Reportage assertion source text.
- `assertion` — the normalised assertion representation: `subject` (`"exit"`, `"stdout"`, `"stderr"`, `"file"`, `"dir"`, or `"logical"`), `path` (required when `subject` is `"file"` or `"dir"`), `operator`, `expected` (a string, integer, `null`, or `{"kind": "workspacePath"|"fixtureReference", "value": "..."}` for a `FileContentsReference`), and `children` (required, non-empty, and recursive when `subject` is `"logical"`).
- `checkpoint` — static checkpoint data used as input to the semantic evaluator: `exitCode`, `stdout`, `stderr`, and an optional `workspace` object (see below).
- `expectedResult` — `"pass"`, `"fail"`, or `"scriptError"` (the expected value failed to resolve to bytes, e.g. a missing `contents_equals` expected file — a test-definition problem, not an assertion outcome).
- `expectedDiagnosticCode` — optional diagnostic code string. Required for `"scriptError"` cases; optional otherwise. The value must be a dot-separated diagnostic code as defined in [`docs/semantic-diagnostics.md`](../../../docs/semantic-diagnostics.md) (e.g. `assertion.stdout.contains.mismatch`). The conformance runner currently compares this value against the actually emitted diagnostic only for `"scriptError"` and parser cases; for `"fail"` cases, CI verifies that the code exists and belongs to the owning rule (`semantic_rule_coverage.rs`), and asserting the emitted code is tracked as follow-up work.

`checkpoint.workspace`, when present, materializes real files and directories on disk before evaluation runs:

- `files` — files to create under the case workspace root, as `{"path": "...", "contents": <StreamData>}`.
- `dirs` — empty directories to create under the case workspace root.
- `reporDirFiles` — files to create under `repor_dir`, for resolving a `contents_equals` expected `@"..."` fixture reference.
- `reporDirDirs` — empty directories to create under `repor_dir`.
- `reporDirSymlinks` — symlinks to create under `repor_dir`, each as `{"path": "...", "outsideDirFiles": [<WorkspaceFile>...]}`: `path` becomes a symlink pointing at a freshly created directory containing `outsideDirFiles`, reproducing the symlink-escape scenario `value-reference.fixture-reference.resolve` guards against (a fixture reference with no `.`/`..` segment that still escapes `repor_dir` because a symlink planted under it points elsewhere). Requires a Unix target; CI runs on `ubuntu-latest`.

### Parser cases

A parser case is verified against the production parser directly, for rules that concern acceptance or rejection of syntax or a literal, not a checkpoint comparison (`value-reference.*` rules, and the empty-block cases of `logical-composition.*` rules). It includes:

- `description` — a short sentence describing what the case verifies.
- `assertionSource` — the Reportage assertion source text, parsed by wrapping it in a minimal `case { assert { ... } }` script.
- `expectedResult` — `"valid"` (the wrapped script parses successfully) or `"parseError"` (parsing fails).
- `expectedDiagnosticCode` — required for `"parseError"` cases: the diagnostic code the parse error must carry.

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

Deferred or unresolved semantic questions belong in [`docs/TBD.md`](../../../docs/TBD.md), not in spec files.
