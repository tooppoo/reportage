
# `text_equals` Evaluation

- Status: Accepted
- Created: 2026-07-08T04:53:32Z

## Context

#86 unified string literal and heredoc literal into a common `TextLiteral` syntax category, resolved at the semantic layer into a common `TextValue` runtime domain value. #92 introduced the `@"<path>"` `FixtureReference` literal and the `FileContentsReference = WorkspacePath | FixtureReference` expected-value category for `contents_equals`, and along the way wired enough grammar, AST construction, and literal-kind validation to make `file <"actual"> text_equals "<text>"` (the string-literal form only) parse and pass semantic validation. It deliberately left the comparison itself as a `todo!()` in `evaluator::evaluate_file_expectation`'s `FileMatcher::TextEquals` arm, and deliberately did not wire the heredoc-literal form into the grammar, leaving both for this issue (#88). #93 introduced the `<"...">` `WorkspacePath` literal that the `file` checkpoint subject now uses uniformly.

This issue wires up `text_equals` end to end: the heredoc-literal grammar form, resolving a `TextLiteral` to its `TextValue`, comparing that value's bytes against the actual file's bytes, classifying failures, and reporting a bounded diagnostic on mismatch.

## Decision

### `text_equals` takes a `TextValue`, never a `FileContentsReference`

`file <ActualValue<WorkspacePath>> text_equals <ExpectedValue<TextValue>>` is the semantic signature. `<text_literal>` in source is a `TextLiteral` (`TextLiteral::Quoted` or `TextLiteral::Heredoc`); the semantic layer resolves it to a `TextValue` via `TextLiteral::to_text_value`, and the evaluator encodes that `TextValue` as UTF-8 bytes and compares it byte-for-byte against the actual file's raw bytes.

`TextValue` and `FileContentsReference` are never implicitly converted into one another. `text_equals` only ever accepts a `TextLiteral`; a `WorkspacePath` or `FixtureReference` literal in that position is a `semantic.literal.kind_mismatch` (`RequiredKind::TextValueStringOrHeredoc`), exactly like any other wrong-kind literal — this rejection was already implemented and tested by #92 for the fixture-reference case, and this issue adds the workspace-path-literal case as the heredoc form is wired in. `contents_equals` remains the expectation for comparing against another file's contents, whether a workspace file (`<"...">`) or a fixture file (`@"..."`); `text_equals` is deliberately narrower, comparing only against text written inline in the script.

### String literal and heredoc literal are transparent to runtime comparison

The grammar gains `file_text_equals_heredoc`, reachable only through `multi_assert` (mirroring `file_exp_heredoc`), so `file <"actual"> text_equals <heredoc literal>` parses alongside the existing string-literal form `file_text_equals`. Both forms build the same AST shape, `FileMatcher::TextEquals(TextLiteral)`, and the evaluator's comparison never matches on which `TextLiteral` variant produced the value: it always goes through `TextLiteral::to_text_value()` first. A `text_equals` expectation written as a heredoc body therefore compares identically to the same text written as a quoted string literal.

### `text_equals` reuses `contents_equals`'s byte-for-byte comparison and actual-side classification

`text_equals` compares bytes with the same semantics `contents_equals` already established: no normalization of any kind. Trailing newlines, CRLF vs. LF, leading/trailing whitespace, and Unicode normalization (e.g. NFC vs. NFD) all participate in the comparison exactly as captured; two empty inputs are equal. The evaluator reuses `ContentsEqualsComparison::compare` and `ContentsEqualsObservation` verbatim rather than duplicating byte-comparison logic: `text_equals`'s actual-side observation (`Compared` / `ActualMissing` / `ActualNotRegularFile` / `ActualUnreadable`) is exactly `contents_equals`'s actual-side observation, because a `file` checkpoint subject's actual-file failure modes do not depend on what kind of expected value it is being compared against.

Unlike `contents_equals`, `text_equals` has no expected-side test-definition error. `contents_equals`'s expected value is a `FileContentsReference` that must be resolved and read from disk (or a fixture materialized) before comparison, and that resolution can fail (missing, not a regular file, unreadable). `text_equals`'s expected value is an inline `TextValue`, already fully present in the parsed AST — there is nothing to resolve, so there is no `ExpectedContentsError` path for it, and `evaluate_file_expectation`'s `TextEquals` arm always returns `Ok`.

A new `ExpectationKind::FileTextEquals { path, expected_source, observation }` variant carries this outcome, with its own `TextEqualsExpectedSource { Quoted(String), Heredoc(String) }` display-only source enum (parallel to `ContentsEqualsExpectedSource { Workspace, Fixture }`) and its own diagnostic codes: `assertion.file.text_equals_mismatch`, `.text_equals_actual_missing`, `.text_equals_actual_not_a_regular_file`, `.text_equals_actual_unreadable`.

### Diagnostic presentation may differ by literal source form; comparison semantics never do

The bounded mismatch diagnostic (actual/expected byte lengths, first differing byte offset and line, an escaped and size-capped context window) is presentation, not comparison semantics, and `text_equals` reuses `contents_diagnostic::mismatch_context` unchanged for it. The one place literal source form is allowed to affect output is the "subject description" line naming what the expected value was: a `Quoted` source renders the literal compactly (`"expected text"`), while a `Heredoc` source renders a plain `<heredoc literal>` label rather than the full body, since the bounded mismatch context below it already carries a line number and an escaped window and printing the full heredoc body would risk unbounded CLI output. This is purely a rendering choice in `render::human` / `render::json`; the evaluator and `contents_diagnostic` never see or branch on which literal form produced a given `TextValue`.

## Rationale

Reusing `ContentsEqualsComparison` / `ContentsEqualsObservation` / `contents_diagnostic::mismatch_context` for `text_equals` avoids a second, drifting implementation of "compare two byte buffers and report a bounded mismatch." The only genuinely new behavior `text_equals` needs is resolving a `TextLiteral` to bytes, which is a single `to_text_value().as_str().as_bytes()` call rather than a resolution pipeline with its own failure modes.

Keeping `TextValue` and `FileContentsReference` strictly separate — no implicit conversion, no shared expected-value type — keeps `text_equals` and `contents_equals` legible from the call site alone: a reader who sees `text_equals` always knows the expected side is inline script text, never a path to another file, and vice versa for `contents_equals`.

Making string literal and heredoc literal transparent to the evaluator, while letting them differ in diagnostic presentation, follows #86's own design: the AST keeps the literal kind for diagnostics, snapshots, and docs generation, but runtime evaluation only ever sees `TextValue`.

## Consequences

### Positive Consequences

- `text_equals` is fully usable for `file`, against either a quoted string literal or a heredoc literal, with clear pass/fail semantics and no expected-side test-definition-error path to reason about.
- `text_equals` and `contents_equals` share one comparison and diagnostic implementation, so a future change to bounded mismatch rendering (context window size, escaping rules) applies to both automatically.
- The heredoc-literal form lets a multi-line expected text value be written without escaping newlines, matching `file contains`'s existing heredoc support.

### Negative Consequences

- `ExpectationKind::FileTextEquals` and `TextEqualsExpectedSource` duplicate the shape of `ExpectationKind::FileContentsEquals` and `ContentsEqualsExpectedSource` field-for-field, differing only in the expected-source type; a shared generic was not introduced, since `ContentsEqualsObservation`/`ContentsEqualsComparison` are already reused as-is and the two `ExpectationKind` variants exist mainly to carry a different, source-shaped `expected_source` through to rendering.
- `spec/output/json-report/schema.json` gained a thirteenth `fileTextEquals` expectation kind and a `TextExpectedSource` definition; this is additive, and, like `contents_equals`'s own schema addition, is not currently pinned by an artifact schema stability contract (see [TBD.md](../planning/TBD.md) — Artifact schema stabilization).

### Neutral Consequences

- `text_equals` is not wired for `stdout` / `stderr` in this issue, mirroring `contents_equals`'s own `file`-only-for-fixtures asymmetry and `output_contains`'s existing string-literal-only restriction; extending it to captured output is a separate, future concern if it becomes concrete.
- `write` step content, `file contains` expected text, and `file text_equals` expected text now all share the same `TextLiteral` → `TextValue` resolution path with no per-caller divergence, exactly as #86's `TextValue` doc comment anticipated.
