# Introduce Stable Diagnostic Codes for Parser and Validator Errors

- Status: Accepted
- Created: 2026-07-01T06:00:00Z

## Context

The syntax conformance fixtures (#28) verify that invalid fixtures are
rejected by the production `parse()` entrypoint, but deliberately avoid
depending on the full text of pest-derived error messages. Pest message text
is grammar-dependent: it can change whenever the grammar changes, even when
the accept/reject behavior of a script does not. Depending on it directly
in tests or tooling would make the test suite fragile against unrelated
grammar cleanup.

At the same time, Reportage must distinguish more than "accepted" vs.
"rejected". Today `ParseError` (`crates/reportage-core/src/parser.rs`) mixes
two kinds of failure: `Syntax`, produced by the pest grammar, and
parse-domain validation failures that Reportage detects explicitly while
constructing the AST (`EmptyCase`, `MissingAssertionBlock`, `EmptyAction`,
`InvalidExitCode`). A future semantic evaluator is expected to introduce a
third kind, semantic validation errors, though designing that is out of
scope here.

Tests, tooling, and future user-facing tooling (e.g. a JSON diagnostic
output) need a way to refer to "this specific kind of error" that does not
break every time an error message is reworded or the pest grammar is
restructured. `ParseError`'s enum variant names are a candidate, but tying
external stability to internal Rust type names would block refactoring the
error representation later.

## Decision

Reportage introduces a stable, machine-readable diagnostic code, independent
of both human-facing message text and internal Rust error variant names.

Diagnostic codes are dot-separated strings in the form `<domain>.<reason>`,
e.g. `parse.syntax`, `parse.empty_case`, `parse.missing_assertion_block`,
`parse.empty_action`, `parse.invalid_exit_code`. A future `semantic.*`
namespace is reserved for semantic validation diagnostics; this ADR does not
define that namespace.

v0 uses coarse-grained and fine-grained codes together. Pest-derived syntax
failures are wrapped, by default, in the single code `parse.syntax`.
Reportage-detected parse-domain validation errors that are worth
distinguishing get their own fine-grained code. Fine-grained codes are added
conservatively: introducing one for every possible pest failure shape now
would lock in detail that could block later grammar cleanup.

A diagnostic is represented as a struct
(`reportage_core::diagnostic::Diagnostic`) that separates:

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

`DiagnosticCode` is a Rust enum, but its external contract is the string
returned by `DiagnosticCode::as_str()` (e.g. `"parse.invalid_exit_code"`).
Tests and tooling must depend on this string form (or the enum value), not
on `ParseError`'s `Display` output.

`details` carries auxiliary information (pest message text, an
expected-token summary, an offending raw value). `details` is part of the
diagnostic model, but its inner contents do not carry the same stability
guarantee as `code`: pest-derived message text and expected-token summaries
are grammar-dependent and must not be a test's primary dependency.

`ParseError` gains `code()`, returning a `DiagnosticCode` directly, and
`to_diagnostic()`, converting to the full `Diagnostic` struct. Both let
callers depend on stable identity instead of parsing `Display` text.

`DiagnosticCode`, `DiagnosticDetails`, and `Diagnostic` are declared
`#[non_exhaustive]`. An exhaustive public enum or struct would make "adding
a code" or "adding a details field" a breaking change in practice, even
though this ADR classifies both as non-breaking: downstream code could write
an exhaustive `match` over `DiagnosticCode`, or a full struct literal /
exhaustive field pattern over `DiagnosticDetails`, either of which stops
compiling the moment a variant or field is added. `#[non_exhaustive]` keeps
the stated compatibility policy true in practice, not just on paper.

### Compatibility policy

- Renaming or removing an existing code is a breaking change.
- Adding a new code is not a breaking change.
- Improving `message` text is not a breaking change.
- Correcting or refining `line` / `column` is not a breaking change.
- Adding fields to, or changing the contents of, `details` is not a breaking
  change, because `details` is not a stable API surface.

v0 does not commit to a strict semver policy for diagnostic codes. If an
existing code must be renamed or removed, the reason must be recorded in an
issue comment or ADR at the time of the change.

## Alternatives Considered

### Use `ParseError`'s enum variant name (via `Debug` or a `variant_name()` method) as the stable identifier

Rejected. It ties an external, test-depended-upon contract to Rust's
internal type structure, which prevents restructuring `ParseError` (e.g.
splitting or merging variants, or replacing it with a different
representation) without it counting as a breaking change to external
consumers. A dedicated `DiagnosticCode` type decouples the two.

### Give every pest grammar failure shape its own fine-grained code immediately

Rejected for v0. Pest's grammar can fail in many ways, and cataloguing every
shape now would require freezing grammar structure prematurely. Coarse
`parse.syntax` is used as the default; fine-grained codes are added only
where Reportage explicitly detects and distinguishes a failure.

### Skip the struct-based `Diagnostic` model and only add `ParseError::code()`

Rejected. A bare code accessor solves immediate test needs but does not
give a place to attach `location` and `details` in a structured way, which a
future JSON diagnostic output would need. Introducing the struct now avoids
a second migration later.

## Consequences

### Positive Consequences

- Tests can assert on `err.code().as_str()` instead of parsing or matching
  `Display` text, making them robust against message wording and pest
  grammar changes.
- `ParseError`'s internal representation can be refactored without breaking
  external consumers, as long as diagnostic codes are preserved.
- The struct-based `Diagnostic` model gives a natural shape for a future
  JSON diagnostic output, without redesigning error handling at that point.

### Negative Consequences

- Every new `ParseError` variant now requires a corresponding
  `DiagnosticCode` entry and a decision about whether it deserves a
  fine-grained code or should fall back to `parse.syntax`-equivalent
  coarse-graining within its own domain.
- Diagnostic codes become a second thing (alongside the enum variant) that
  must be kept in sync when `ParseError` changes, which is small ongoing
  maintenance overhead.
- `#[non_exhaustive]` on `DiagnosticDetails` means external construction goes
  through `DiagnosticDetails::default()` plus individual field assignment
  rather than a single struct literal, which is slightly more verbose for
  callers that want to build one from scratch (expected to be rare, since
  `DiagnosticDetails` is normally produced by `ParseError::to_diagnostic()`,
  not hand-built by consumers).

### Neutral Consequences

- This ADR does not define the `semantic.*` namespace or semantic evaluator
  diagnostics; that is deferred to whichever issue designs the semantic
  evaluator.
- This ADR does not require every existing invalid syntax fixture to gain an
  individual code assertion; only fixtures that produce a fine-grained code
  are asserted individually.
