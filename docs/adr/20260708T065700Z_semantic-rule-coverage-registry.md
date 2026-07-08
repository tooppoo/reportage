# Semantic Rule Coverage Registry as Source of Truth

- Status: Accepted
- Created: 2026-07-08T06:57:00Z

## Context

`docs/language/semantic-rules.md` is a generated semantic rule catalog produced from `spec/language/semantics/*.json`. The JSON specs are a machine-readable source of truth for a semantic rule's normative fields and conformance cases.

Relying on the spec directory alone cannot detect the opposite failure: a semantic rule that already exists in the implementation or the grammar, but has no corresponding semantic spec. #85 identified concrete examples of this gap — `dir` assertions, `file` assertions, `stdout empty`, `stderr empty`, logical composition, and value reference rules were all implemented and parseable, but absent from the generated catalog.

Deriving the rule inventory directly from a Rust enum, the AST, or the parser model was considered and rejected in #85. Enum variants and AST shapes carry implementation and syntax-representation concerns that do not map one-to-one onto semantic rule boundaries; some variants exist for structural reasons, not because they are independently specifiable rules. That approach also cannot express a rule that is known but intentionally deferred, since deferred rules would still need to appear (or not appear) as enum variants regardless of spec status.

This ADR implements the decision #85 made: a Rust const registry as the source of truth for semantic rule existence and coverage requirements, distinct from the semantic spec JSON's role as the source of truth for rule detail.

## Decision

`reportage_core::semantic_rule_registry::SEMANTIC_RULE_REGISTRY` (`crates/reportage-core/src/semantic_rule_registry.rs`) is a `const` slice of `SemanticRuleEntry` values, one per known semantic rule. Each entry carries:

- `id` — the stable rule identifier, in `<category>.<subject>.<operator-or-form>` form.
- `category` — `Assertion`, `LogicalComposition`, or `ValueReference`.
- `implementation_status` — `Implemented`, `Planned`, or `Deferred`.
- `spec_required` / `conformance_required` / `docs_required` — booleans gating the coverage check.
- `related_syntax_rule` — an optional pest grammar rule name, for cross-reference only.
- `related_diagnostic_codes` — an optional list of `DiagnosticCode::as_str()` strings, for cross-reference only.

The registry module is `#[doc(hidden)] pub`: it is visible to integration tests, the docs generator, and CI checks within this workspace, but it is not part of `reportage-core`'s stable public API and carries no semver guarantee for external consumers.

Semantic spec JSON (`spec/language/semantics/*.json`) remains the source of truth for each rule's normative fields and conformance cases; the registry does not duplicate that content, only the rule's existence and coverage requirements.

The coverage check (`crates/reportage-core/tests/semantic_rule_coverage.rs`, run via `just semantic-rule-coverage-check`) verifies both directions of drift:

- Every registry entry with `spec_required=true` has a matching spec file.
- Every spec file has a matching registry entry (an unmatched spec file is a CI failure, not a silent addition — this is what keeps the spec directory from becoming an implicit second inventory).
- Every registry entry with `conformance_required=true` has a spec file with at least one conformance case.
- Every registry entry with `docs_required=true` has a corresponding `## <id>` section in the generated `docs/language/semantic-rules.md`.
- `conformance_required=true` implies `spec_required=true`, and `docs_required=true` implies `spec_required=true`; both implications are asserted directly against the registry data, independent of which specs currently exist.

`implementation_status` is not read by any of the assertions above. It exists only so failure messages and any future inventory listing can say whether a missing-spec rule is implemented, planned, or deferred, rather than treating every gap identically.

Of the eighteen rules currently registered — eleven `assertion`, three `logical-composition`, four `value-reference` — only `assertion.exit.equals`, `assertion.stdout.contains`, and `assertion.stderr.contains` have `spec_required` / `conformance_required` / `docs_required` all `true`, matching the specs that exist today. The rest are registered with all three flags `false`: known, implemented rules awaiting a spec. #101 is the follow-up that adds those specs and flips the corresponding flags.

## Alternatives Considered

### Derive the registry from `Expectation` / AST / parser model

Rejected, per #85's decision. Enum variants such as `Expectation::FileCount` and `Expectation::Jq` exist for conceptual completeness ahead of parser support (see `model.rs`), so a direct derivation would either have to invent spec-required rules for unparseable variants or hardcode exclusions — reintroducing by another name the manual curation this ADR chooses to do explicitly instead.

### JSON registry instead of a Rust const

Rejected for the initial implementation. A JSON registry would need its own schema, its own loader, and its own CI-integrated validation, while gaining nothing today: the only consumers (integration tests, the docs generator, CI checks) already live in this Rust workspace. JSON is revisited if the registry needs to move out of the runtime crate, be read by non-Rust tooling, or gain schema validation independent of Rust's type system.

### Fold `logical-composition` and `value-reference` into the `assertion` spec schema

Rejected. `assertion` specs describe a checkpoint field, an operator, and a match semantics — a direct subject/expected comparison. Logical composition (`not` / `all` / `any`) composes other expectations rather than comparing a checkpoint field; value reference (workspace path resolution, fixture reference resolution, file contents reference resolution, literal kind mismatch) resolves or validates a value supplied to an assertion, rather than performing the comparison itself. Forcing either into the `assertion` schema's `checkpointField` / `operator` / `matchSemantics` shape would misrepresent what the rule actually specifies.

## Consequences

### Positive Consequences

- Both directions of spec/registry drift are caught in CI: a rule the registry expects but the specs lack, and a spec file the registry does not know about.
- Adding a new implemented-but-unspecified rule is a one-line registry entry, not a silent gap; the coverage check and `just check` both surface it immediately if its `_required` flags are later set without the accompanying spec.
- `#101` (and any future spec-adding work) has an explicit, mechanically checked to-do list: every registry entry with `spec_required=false`.

### Negative Consequences

- Every newly implemented or newly parseable semantic rule now requires a registry entry, in addition to whatever implementation and grammar changes it needed — one more place to remember to update.
- The registry's `related_syntax_rule` and `related_diagnostic_codes` fields are plain `&'static str` cross-references, not typed handles into `parser.rs` / `diagnostic.rs`; they can silently go stale if a pest rule is renamed or a diagnostic code changes without a corresponding registry update, since nothing currently asserts those cross-references against the grammar or `DiagnosticCode` directly.

### Neutral Consequences

- This ADR does not add semantic specs for `logical-composition` or `value-reference` rules; it only makes their absence visible and inert (`spec_required=false`) until #101.
- This ADR does not address `runner-lifecycle`, `artifact`, or `diagnostic` rule inventories; per #85, those are owned elsewhere.
