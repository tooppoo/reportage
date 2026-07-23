# Diagnostic Codes

This document defines Reportage's stable diagnostic code system for parser and validator errors, and the compatibility policy that governs it.

See [ADR: Stable Diagnostic Codes](../adr/20260701T060000Z_stable-diagnostic-codes.md) for the decision record.

## Why

Reportage distinguishes three kinds of script-level error:

- syntax errors produced by the pest grammar,
- parse-domain validation errors detected by Reportage itself while constructing the AST (e.g. an empty case block, an out-of-range exit code),
- and, in the future, semantic validation errors.

Human-facing error messages are allowed to improve over time. Tests and tooling need something that does not change out from under them. Diagnostic codes are that stable identifier; messages are the separate, improvable display layer.

## Naming Convention

Diagnostic codes are dot-separated strings in the form:

```text
<domain>.<reason>
```

Examples:

- `parse.syntax`
- `parse.empty_case`
- `parse.missing_assertion_block`
- `parse.empty_action`
- `parse.invalid_exit_code`
- `parse.heredoc_literal.shallow_indent`
- `parse.document_block.empty`
- `parse.document_block.duplicate_field`
- `parse.document_block.invalid_order`
- `parse.document_file.duplicate`
- `parse.document_file.after_case`
- `parse.document_case.duplicate`
- `parse.document_case.orphan`
- `parse.before_each.duplicate`
- `parse.before_each.after_case`
- `parse.before_each.action_step`
- `parse.before_each.assertion_block`
- `parse.before_each.empty`

`parse.*` covers pest grammar syntax errors and parse-domain validation errors raised while constructing the AST. This includes a heredoc literal's indentation (`parse.heredoc_literal.shallow_indent`, applicable both to a `write` step's heredoc literal and to a `file ... contains` expectation's heredoc literal — see [Language semantics](semantics.md) — Text literal), a document block's body rules (`parse.document_block.empty` / `.duplicate_field` / `.invalid_order`, shared by every document scope), the `document file` placement rules (`parse.document_file.duplicate` / `.after_case`), the `document case` association rules (`parse.document_case.duplicate` for a second block before the target case, reported at the second block's start line, and `parse.document_case.orphan` for a block with no following case, reported at the unassociated block's start line; the duplicate wins when one structure violates both — see [Language semantics](semantics.md) — Document block), the `before_each` placement and body rules (`parse.before_each.duplicate` / `.after_case` for placement, `.action_step` / `.assertion_block` for banned step kinds reported at the offending step's line, and `.empty` for a block with no steps — see [Execution model](execution-model.md) — `before_each`), a `write` step's workspace path safety validation (`semantic.workspace_path.*`, surfaced as a `ParseError` because `WorkspacePath::parse` runs during AST construction — see [Language semantics](semantics.md) — Write step), a fixture reference literal's lexical validation (`semantic.fixture_reference.empty` / `.absolute` / `.dot_segment`, surfaced as a `ParseError` for the same reason, because `FixtureReference::parse` also runs during AST construction — see [Language semantics](semantics.md) — Fixture reference value), and literal kind mismatches (`semantic.literal.kind_mismatch`, surfaced as a `ParseError` because argument positions check their literal kind during AST construction — see [Language semantics](semantics.md) — Value literals). The `semantic.*` and `assertion.*` namespaces cover the semantic evaluator side and are defined in [Semantic and assertion diagnostics](semantic-diagnostics.md); this document does not define them. The `step.*` namespace covers runtime failures of side-effecting steps and is also defined in [Semantic and assertion diagnostics](semantic-diagnostics.md).

A diagnostic code is **not** the same thing as the Rust error enum variant name that produces it. Internal enum variants (e.g. `ParseError::EmptyCase`) may be renamed or restructured freely. The code string (e.g. `parse.empty_case`) is the external, stable identifier that tests and tooling depend on.

## Code Granularity

v0 uses coarse-grained and fine-grained codes together:

- Pest-derived syntax failures are, by default, wrapped in the single coarse code `parse.syntax`. Pest's grammar can produce many distinct failure shapes; giving each one its own code now would lock in detail that later grammar cleanup would have to preserve.
- Parse-domain validation errors that Reportage detects explicitly, and that are worth distinguishing for users, tests, or tooling, get their own fine-grained code (`parse.empty_case`, `parse.missing_assertion_block`, `parse.empty_action`, `parse.invalid_exit_code`).

Not every invalid script needs its own code. Only add a fine-grained code when there is a concrete reason to distinguish that failure from a generic syntax error.

## Diagnostic Model

Diagnostics are represented as a struct (`reportage_core::diagnostic::Diagnostic`) that separates four concerns:

```rust
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub message: String,
    pub location: Option<DiagnosticLocation>,
    pub details: DiagnosticDetails,
}

pub struct DiagnosticLocation {
    pub line: usize,
    pub column: Option<usize>,
}
```

- `code` — the stable, machine-readable identifier (see below).
- `message` — a human-facing, improvable display string.
- `location` — line, and column when available.
- `details` — auxiliary information (see "The `details` field" below).

`ParseError::to_diagnostic()` converts a parser error into this model. `ParseError::code()` returns just the `DiagnosticCode` when the full diagnostic struct is not needed.

## `DiagnosticCode` External Representation

`DiagnosticCode` is a Rust enum internally, but its external contract is the string returned by `DiagnosticCode::as_str()` (e.g. `"parse.invalid_exit_code"`). Tests and tooling must assert against this string form (or the enum value itself), not against `Display` message text produced by `ParseError`.

```rust
let err = parse(source).unwrap_err();
assert_eq!(err.code().as_str(), "parse.invalid_exit_code");
```

A future JSON diagnostic output, if added, would serialize `code` using this same string form.

## The `details` Field

`details` exists to carry auxiliary information alongside a diagnostic — today, the raw pest message for syntax errors and the offending raw value for validation errors (e.g. an out-of-range exit code literal or a case name).

The `details` field itself is part of the diagnostic model, but its *inner contents* do not carry the same stability guarantee as `code`. In particular:

- pest-derived message text, and
- pest "expected token" summaries,

are grammar-dependent and may change whenever the pest grammar changes. They are not a stable API. Tests must depend on `code`, not on the contents of `details` or on `ParseError`'s `Display` text.

## Compatibility Policy

Diagnostic codes are an external identifier that tests and tooling may depend on. The following changes are classified as follows:

| Change | Classification |
|---|---|
| Renaming or removing an existing code | Breaking |
| Adding a new code | Non-breaking |
| Improving `message` text | Non-breaking |
| Correcting or refining `line` / `column` | Non-breaking |
| Adding fields to `details` | Non-breaking |
| Changing the contents of existing `details` fields (e.g. pest message wording) | Non-breaking (details are not a stable API) |

v0 does not commit to a strict semver policy for diagnostic codes. If an existing code must be renamed or removed, record the reason in an issue comment or ADR at the time of the change.

### Why `#[non_exhaustive]`

`DiagnosticCode`, `DiagnosticDetails`, and `Diagnostic` are all `#[non_exhaustive]`. Without it, an exhaustive Rust enum or struct is itself a breaking-change trap: downstream code that writes an exhaustive `match` over every `DiagnosticCode` variant, or a struct literal / exhaustive field pattern over every `DiagnosticDetails` field, would fail to compile the moment Reportage adds a new code or a new details field — even though this document classifies both as non-breaking. `#[non_exhaustive]` forces downstream `match` expressions to include a wildcard (`_`) arm and forces struct construction to go through `Default::default()` (then set individual fields) rather than a full struct literal, so additive changes stay additive for consumers too.

## Pest-Derived Errors

Pest error message text and expected-token summaries are not a stable API. Reportage wraps all pest grammar failures in the single `parse.syntax` code and preserves pest's message as auxiliary detail. Tests must assert on the `parse.syntax` code, not on the full text of the wrapped pest message.

## Relationship to Syntax Conformance Fixtures

The syntax conformance fixtures introduced for #28 (see [Syntax conformance fixtures](../design/testing/syntax-conformance.md)) primarily verify that invalid fixtures are rejected. This document's diagnostic code API is what lets a fixture test additionally assert *which* code an invalid fixture produces. Not every invalid fixture needs an individual code assertion — only fixtures where Reportage produces a fine-grained code beyond `parse.syntax` are asserted individually today (see [`crates/reportage-core/tests/syntax_conformance.rs`](../../crates/reportage-core/tests/syntax_conformance.rs)).
