# Semantic Rule Coverage Registry as Source of Truth

- Status: Accepted
- Created: 2026-07-08T06:57:00Z
- Updated: 2026-07-09 (#146: typed semantic rule identity and verified cross-references)

## Context

`docs/language/semantic-rules.md` is a generated semantic rule catalog produced from `spec/language/semantics/*.json`. The JSON specs are a machine-readable source of truth for a semantic rule's normative fields and conformance cases.

Relying on the spec directory alone cannot detect the opposite failure: a semantic rule that already exists in the implementation or the grammar, but has no corresponding semantic spec. #85 identified concrete examples of this gap — `dir` assertions, `file` assertions, `stdout empty`, `stderr empty`, logical composition, and value reference rules were all implemented and parseable, but absent from the generated catalog.

Deriving the rule inventory directly from a Rust enum, the AST, or the parser model was considered and rejected in #85. Enum variants and AST shapes carry implementation and syntax-representation concerns that do not map one-to-one onto semantic rule boundaries; some variants exist for structural reasons, not because they are independently specifiable rules. That approach also cannot express a rule that is known but intentionally deferred, since deferred rules would still need to appear (or not appear) as enum variants regardless of spec status.

This ADR implements the decision #85 made: a Rust const registry as the source of truth for semantic rule existence and coverage requirements, distinct from the semantic spec JSON's role as the source of truth for rule detail.

The initial implementation kept the registry's `id`, `related_syntax_rule`, and `related_diagnostic_codes` fields as plain `&'static str`, which could silently go stale when a diagnostic code or grammar rule was renamed. #146 amended this ADR to make semantic rule identity typed and cross-references verifiable; the Decision and Consequences sections below describe the amended design.

## Decision

`reportage_core::semantic_rule_registry::SEMANTIC_RULE_REGISTRY` (`crates/reportage-core/src/semantic_rule_registry.rs`) is a `const` slice of `SemanticRuleEntry` values, one per known semantic rule. The registry is positioned as a coverage inventory plus the owner of semantic rule identity. Each entry carries:

- `id` — a `SemanticRuleId` enum value identifying the rule.
- `category` — `Assertion`, `LogicalComposition`, or `ValueReference`.
- `implementation_status` — `Implemented`, `Planned`, or `Deferred`.
- `spec_required` / `conformance_required` / `docs_required` — booleans gating the coverage check.
- `related_syntax_rule` — an optional pest grammar rule name, for cross-reference only.
- `related_diagnostic_codes` — a list of `RelatedDiagnostic` values wrapping typed `DiagnosticCode` references, for cross-reference only.

The registry module is `#[doc(hidden)] pub`: it is visible to integration tests, the docs generator, and CI checks within this workspace, but it is not part of `reportage-core`'s stable public API and carries no semver guarantee for external consumers.

Semantic spec JSON (`spec/language/semantics/*.json`) remains the source of truth for each rule's normative fields and conformance cases; the registry does not duplicate that content, only the rule's existence and coverage requirements.

### Semantic rule identity

Semantic rule identity is owned by the `SemanticRuleId` enum, defined in the registry module.

- `SemanticRuleId::as_str()` is the canonical string representation of a rule id, in `<category>.<subject>.<operator-or-form>` form.
- Spec JSON `id` fields, generated docs section headings, and coverage checks all match against `SemanticRuleId::as_str()`.
- Rust code that needs to name a semantic rule references a `SemanticRuleId` variant instead of a string literal.

The registry owns semantic rule identity, category, implementation status, coverage obligations, and cross-references to diagnostics and syntax rules. It does not own parser construction, semantic validation, or evaluator behavior; diagnostic code identity (owned by `DiagnosticCode`); pest grammar rule identity (owned by `reportage.pest`); or the normative fields and conformance cases of spec JSON.

### Diagnostic cross-references

`related_diagnostic_codes` references `DiagnosticCode` values, so a diagnostic rename or removal that misses the registry fails to compile instead of going stale. Because the rule-to-diagnostic relationship is not always one-to-one, each reference declares a relation kind:

```rust
pub enum RelatedDiagnostic {
    RuleOwned(DiagnosticCode),
    Shared(DiagnosticCode),
    SemanticValidation(DiagnosticCode),
}
```

- `RuleOwned(code)` — a diagnostic unique to this rule. CI verifies that `code.as_str()` is `<rule-id>.<reason>`: the `<rule-id>.` prefix including the separator dot (a bare `starts_with(rule_id)` would accept `assertion.file.existsX`), a non-empty reason, and no other registry entry referencing the code.
- `Shared(code)` — a diagnostic shared by several semantic rules, e.g. `semantic.expectation.empty_block` for `not` / `all` / `any`. Exempt from the prefix check; CI instead verifies that at least two registry entries reference it.
- `SemanticValidation(code)` — a diagnostic emitted by validating or resolving a value the rule describes. Exempt from the prefix check; CI instead verifies the `semantic.` namespace prefix and that exactly one registry entry references it.

`SemanticValidation` is an addition over the two-kind design #146 originally proposed. The value-reference rules' diagnostics (`semantic.workspace_path.*`, `semantic.file_path.*`, `semantic.dir_entry_name.*`, `semantic.fixture_reference.*`, `semantic.file_contents_reference.*`, `semantic.literal.kind_mismatch`) fit neither original kind: they are referenced by a single rule, so `Shared`'s at-least-two check rejects them, and renaming them to `<rule-id>.<reason>` would be wrong on two counts. First, the same validation fires outside the rule's own assertion position — `semantic.workspace_path.*` from a `write` step's path, `semantic.literal.kind_mismatch` from any argument position — so a rule-derived name would misattribute those emissions. Second, the `semantic.` namespace itself carries the failure classification (semantic error, not assertion failure) defined in `docs/semantic-diagnostics.md`, and value-reference rule ids such as `value-reference.literal.kind-mismatch` already name the failure condition, which would force degenerate names like `value-reference.literal.kind-mismatch.mismatch`.

### Diagnostic code naming convention

Rule-owned diagnostic codes follow `<semantic-rule-id>.<reason>`, e.g. `assertion.file.exists.missing` and `assertion.exit.equals.mismatch`. #146 renamed the pre-existing assertion diagnostic codes to this form (e.g. `assertion.file.exists_missing` → `assertion.file.exists.missing`, `assertion.stdout.not_empty` → `assertion.stdout.empty.not_empty`, `assertion.file.exists_not_a_file` → `assertion.file.exists.not_regular_file`), which the pre-1.0 compatibility policy in `docs/diagnostics.md` permits; this ADR is the record of that rename and its reason. Diagnostic codes that do not belong to a semantic rule (`parse.*`, `step.*`, runner-lifecycle codes) are outside this convention.

### Variant enumeration

`SemanticRuleId::ALL` and `DiagnosticCode::ALL` enumerate every variant. Unit tests keep each list synchronized with its enum's declaration order, and the coverage check uses them to verify that both enums' `as_str()` values are unique and that the registry has exactly one entry per `SemanticRuleId` variant. `DiagnosticCode` remains `#[non_exhaustive]` for downstream consumers; a crate-internal full enumeration does not weaken that.

### Coverage and cross-reference checks

The coverage check (`crates/reportage-core/tests/semantic_rule_coverage.rs`, run via `just semantic-rule-coverage-check`) verifies both directions of drift:

- Every registry entry with `spec_required=true` has a matching spec file.
- Every spec file has a matching registry entry (an unmatched spec file is a CI failure, not a silent addition — this is what keeps the spec directory from becoming an implicit second inventory).
- Every registry entry with `conformance_required=true` has a spec file with at least one conformance case.
- Every registry entry with `docs_required=true` has a corresponding `## <id>` section in the generated `docs/language/semantic-rules.md`.
- `conformance_required=true` implies `spec_required=true`, and `docs_required=true` implies `spec_required=true`; both implications are asserted directly against the registry data, independent of which specs currently exist.

It additionally verifies the cross-reference conventions:

- The relation-kind rules above (`RuleOwned` prefix and uniqueness, `Shared` reference count, `SemanticValidation` namespace and reference count), and that each diagnostic code uses a single relation kind across the registry.
- Every `related_syntax_rule` value names a rule that exists in `crates/reportage-core/src/reportage.pest`, extracted from the grammar file's rule definitions.
- Every `expectedDiagnosticCode` in spec conformance cases is a known `DiagnosticCode` string, and any rule-owned code is expected only by its owning rule's spec. Shared and semantic-validation codes may appear in other rules' specs, because e.g. an assertion rule's invalid-expected-value case legitimately expects `semantic.literal.kind_mismatch`.

`implementation_status` is not read by any of the assertions above. It exists only so failure messages and any future inventory listing can say whether a missing-spec rule is implemented, planned, or deferred, rather than treating every gap identically.

The `value-reference` category also covers lexical validation of assertion subject values, not only resolution of a referenced value: `value-reference.file-path.validate` and `value-reference.dir-entry-name.validate` register `file`'s and `dir`'s own subject-path / entry-name validation, which are implemented in `semantic::validate_file_path` and `semantic::validate_dir_entry_name` respectively and run before evidence comparison. `dir`'s subject path itself is deliberately not a third such entry: it reuses `value-reference.workspace-path.resolve`'s exact rule and diagnostic codes via `semantic::validate_dir_path`, rather than defining a parallel one.

## Alternatives Considered

### Derive the registry from `Expectation` / AST / parser model

Rejected, per #85's decision. Enum variants such as `Expectation::FileCount` and `Expectation::Jq` exist for conceptual completeness ahead of parser support (see `model.rs`), so a direct derivation would either have to invent spec-required rules for unparseable variants or hardcode exclusions — reintroducing by another name the manual curation this ADR chooses to do explicitly instead.

### JSON registry instead of a Rust const

Rejected for the initial implementation. A JSON registry would need its own schema, its own loader, and its own CI-integrated validation, while gaining nothing today: the only consumers (integration tests, the docs generator, CI checks) already live in this Rust workspace. JSON is revisited if the registry needs to move out of the runtime crate, be read by non-Rust tooling, or gain schema validation independent of Rust's type system. The #146 amendment strengthens this choice: typed `SemanticRuleId` and `DiagnosticCode` cross-references are only possible because the registry is Rust.

### Fold `logical-composition` and `value-reference` into the `assertion` spec schema

Rejected. `assertion` specs describe a checkpoint field, an operator, and a match semantics — a direct subject/expected comparison. Logical composition (`not` / `all` / `any`) composes other expectations rather than comparing a checkpoint field; value reference (workspace path resolution, fixture reference resolution, file contents reference resolution, literal kind mismatch) resolves or validates a value supplied to an assertion, rather than performing the comparison itself. Forcing either into the `assertion` schema's `checkpointField` / `operator` / `matchSemantics` shape would misrepresent what the rule actually specifies.

### Keep `related_diagnostic_codes` as plain strings (initial design)

Superseded by #146. Plain `&'static str` cross-references could silently go stale on a rename, which the initial version of this ADR recorded as a known negative consequence. Typed references plus the relation-kind checks replace that consequence; see Diagnostic cross-references above.

### Two relation kinds only (`RuleOwned` / `Shared`), renaming all single-rule diagnostics

Rejected. This was #146's original proposal. It cannot classify the value-reference rules' `semantic.*` diagnostics: they are single-rule references (failing `Shared`'s at-least-two check) whose emissions are not confined to the rule's own syntactic position (making a `<rule-id>.` rename a misattribution), and the `semantic.` namespace prefix is itself the documented failure-classification contract. `SemanticValidation` records this third relationship explicitly instead of forcing it into a naming convention built for assertion outcomes.

## Consequences

### Positive Consequences

- Both directions of spec/registry drift are caught in CI: a rule the registry expects but the specs lack, and a spec file the registry does not know about.
- Adding a new implemented-but-unspecified rule is a one-line registry entry, not a silent gap; the coverage check and `just check` both surface it immediately if its `_required` flags are later set without the accompanying spec.
- Semantic rule identity is defined once in `SemanticRuleId`; Rust code referencing a rule cannot hold a stale magic string.
- Registry-to-diagnostic cross-references are existence-checked by the type system, and rule-owned codes' naming is CI-checked, so a reference that clearly belongs to a different semantic rule fails CI.
- Pest grammar rule cross-references stay strings but are existence-checked against `reportage.pest` in CI, so a grammar rule rename surfaces immediately.
- Spec conformance cases' expected diagnostic codes are checked against `DiagnosticCode` and rule ownership, so a diagnostic rename cannot leave a stale code in spec JSON.

### Negative Consequences

- Every newly implemented or newly parseable semantic rule now requires a registry entry, in addition to whatever implementation and grammar changes it needed — one more place to remember to update.
- The registry module now depends on `DiagnosticCode`. The dependency is limited to identity (the enum), not to diagnostic construction or rendering, but the original "registry depends on nothing" property is gone.
- A diagnostic code rename now propagates beyond `DiagnosticCode::as_str()` into docs, semantic specs, conformance cases, parser/evaluator/CLI tests, and snapshots, all of which must be updated together (as #146 itself did for the assertion code renames).
- Declaring a relation kind per diagnostic reference makes registry entries more verbose than a plain code list, and the kind must be chosen correctly for the CI checks to mean anything.
- `SemanticRuleId::ALL` and `DiagnosticCode::ALL` are manually maintained enumerations; their sync tests catch reordering and mid-list omissions, but a variant appended after the last listed element relies on the adjacent doc comment being followed.

### Neutral Consequences

- This ADR does not address `runner-lifecycle`, `artifact`, or `diagnostic` rule inventories; per #85, those are owned elsewhere. `SemanticExpectationRequiresAction` (a process-expectation-before-any-action ordering error) is a `runner-lifecycle` concern under that split, not a registered semantic rule, even though it lives in the same `DiagnosticCode` enum as assertion diagnostics.
- Diagnostics do not carry their originating `SemanticRuleId` at emission time; the registry links identities statically, and runtime behavior is unchanged. Emission-time rule linkage (a `rule` field on `Diagnostic`) is a possible future strengthening tracked separately from this ADR.
