
# `stdout` / `stderr` `text_equals` Evaluation

- Status: Accepted
- Created: 2026-07-10T10:09:18Z

## Context

#88 wired `text_equals` end to end for the `file` checkpoint subject: a byte-for-byte comparison of the actual file's bytes against an inline `TextValue` resolved from a `TextLiteral` (string literal or heredoc literal), reusing `contents_equals`'s comparison and bounded mismatch diagnostics (see [ADR: `text_equals` Evaluation](20260708T045332Z_text-equals-evaluation.md)).
That ADR deliberately left `text_equals` unwired for `stdout` / `stderr`, mirroring `output_contains`'s string-literal-only restriction, and noted that extending it to captured output was a separate, future concern.

The asymmetry is user-visible: `stdout contents_equals <"expected.txt">` can compare a captured stream byte-for-byte against another file, but the only way to compare a stream against text written inline in the script was `contains`, a substring check.
Asserting a command's exact output — the primary use of `text_equals` — therefore required writing the expected text to a file first.
This change wires `text_equals` for `stdout` / `stderr`, in both the string-literal and heredoc-literal forms.

## Decision

### `stdout` / `stderr text_equals` takes a `TextValue`, exactly like `file text_equals`

`stdout text_equals <ExpectedValue<TextValue>>` / `stderr text_equals <ExpectedValue<TextValue>>` is the semantic signature.
The expected operand is a `TextLiteral`, resolved to its `TextValue` and encoded as UTF-8 bytes; a `WorkspacePath` or `FixtureReference` literal in that position is a `semantic.literal.kind_mismatch`, exactly as in the `file text_equals` position.
`contents_equals` remains the expectation for comparing a stream against another file's contents; `text_equals` remains deliberately narrower, comparing only against text written inline in the script.
The type rule is uniform across subjects: a reader who sees `text_equals` always knows the expected side is inline script text, regardless of whether the actual side is a file or a captured stream.

### Grammar mirrors the `file text_equals` split between string-literal and heredoc forms

`output_matcher` gains `output_text_equals` (the string-literal form, shared by `stdout_exp` / `stderr_exp`), and `heredoc_expectation` gains `stdout_text_equals_heredoc` / `stderr_text_equals_heredoc` (the heredoc forms, reachable only through `multi_assert`, one rule per stream because the stream name must survive into AST construction).
Both forms build the same AST shape, `OutputMatcher::TextEquals(TextLiteral)`, mirroring `FileMatcher::TextEquals(TextLiteral)`; the evaluator never branches on which literal form produced the value.

### The captured stream is the actual side, with no actual-side failure modes

Evaluation reuses `ContentsEqualsComparison::compare` on the captured stream's raw bytes, exactly like `stdout` / `stderr contents_equals`.
Unlike a `file` subject, a captured stream has no `ActualMissing` / `ActualNotRegularFile` / `ActualUnreadable` observations: once an action has run, captured output always exists (possibly empty), so the comparison outcome is the whole result.
Each stream gets one rule-owned diagnostic code — `assertion.stdout.text_equals.mismatch` / `assertion.stderr.text_equals.mismatch` — mirroring `contents_equals`'s stream codes.
Like `file text_equals`, there is no expected-side test-definition error: the expected value is already present in the parsed AST.

New `ExpectationKind::StdoutTextEquals` / `StderrTextEquals` variants carry `TextEqualsExpectedSource` (the display-only `Quoted` / `Heredoc` source introduced by #88) plus the `ContentsEqualsComparison`, and serialize as `stdoutTextEquals` / `stderrTextEquals` expectation kinds with the same `actualRef` per-action artifact reference the other stream expectations use.

### Human rendering names the operator the author wrote

`print_contents_equals_detail` previously hard-coded `contents_equals` in its subject description line, so a failing `file text_equals` comparison printed `file "..." contents_equals ... — bytes differ`.
The operator keyword is now a parameter, so `text_equals` failures (file and stream) print `text_equals` and `contents_equals` failures keep printing `contents_equals`.
The heredoc-source rendering rule is unchanged: a `Quoted` source renders the literal compactly, a `Heredoc` source renders the bounded `<heredoc literal>` label.

## Rationale

Reusing `OutputMatcher`'s existing evaluation shape (`contents_equals`'s stream arms) and `file text_equals`'s expected-side resolution means the only genuinely new runtime behavior is the pairing of the two — there is no second comparison implementation, no new observation enum, and no new failure classification to reason about.

Registering `assertion.stdout.text_equals` / `assertion.stderr.text_equals` as fully spec-required rules (unlike `assertion.stdout.contents_equals` / `assertion.stderr.contents_equals`, which predate the semantic spec system and remain known rules awaiting a spec) keeps new rules at the coverage bar the registry now enforces.

## Consequences

### Positive Consequences

- A command's exact stdout / stderr can be asserted inline, in one line for short output (string literal) or without escaping newlines for multi-line output (heredoc literal).
- `text_equals` is now uniform across `file` / `stdout` / `stderr`, closing the asymmetry with `contents_equals`.
- The human-rendered comparison detail now names the operator the author wrote (`text_equals` vs. `contents_equals`), for file subjects as well as streams.

### Negative Consequences

- `stdout_text_equals_heredoc` / `stderr_text_equals_heredoc` are near-duplicate grammar rules; folding them into one rule would require a stream-name capture rule that the grammar does not otherwise need.
- The `spec/output/json-report` and `spec/artifacts/run-result` schemas gain a fifteenth expectation kind pair (`StreamTextEqualsExpectation`); as with earlier additions, this is additive and not yet pinned by an artifact schema stability contract.

### Neutral Consequences

- `stdout` / `stderr contents_equals` remain without a semantic spec; this change does not retrofit one.
- `output_contains` keeps its string-literal-only restriction; extending `contains` to heredoc literals remains out of scope.
