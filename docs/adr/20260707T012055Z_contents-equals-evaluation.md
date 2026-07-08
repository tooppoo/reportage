# `contents_equals` Comparison Evaluation

- Status: Accepted
- Created: 2026-07-07T01:20:55Z

## Context

#92 introduced the `@"<path>"` `FixtureReference` literal, the `FileContentsReference = WorkspacePath | FixtureReference` type, and enough grammar / AST / literal-kind validation to make `file <"actual"> contents_equals <expected>`, `stdout contents_equals <expected>`, and `stderr contents_equals <expected>` parse and pass semantic validation. It deliberately left the comparison itself as a `todo!()`: `evaluator::evaluate_file_expectation`'s `FileMatcher::ContentsEquals` arm and the `stdout` / `stderr` `OutputMatcher::ContentsEquals` arms panicked if actually evaluated, and no `*.repor` file's own path was threaded into evaluation, so `fixture::resolve_fixture_source` had nothing to resolve a `FixtureReference` against at runtime.

This issue (#87) wires up that comparison: reading actual and expected bytes, comparing them, classifying failures, and reporting a bounded diagnostic on mismatch.

## Decision

### Byte-for-byte comparison, no normalization

`contents_equals` compares actual and expected bytes with `==` on the raw byte slices. Trailing newlines, CRLF vs. LF, leading/trailing whitespace, and Unicode normalization are never adjusted before comparing. Two empty inputs are equal. This mirrors the existing raw-byte semantics `stdout contains` / `stderr contains` already established (see the raw byte semantics ADR): a byte-level assertion should not silently mask an output-format regression by normalizing it away.

### Actual-side failure is an assertion failure; expected-side failure is a test-definition error

A `file` subject's actual path can be missing, not a regular file, or unreadable — exactly the same failure shapes `file exists` / `file contains` already classify as assertion failures, because they describe what the subject under test produced (or failed to produce). `contents_equals` classifies them the same way: `ContentsEqualsObservation::ActualMissing` / `ActualNotRegularFile` / `ActualUnreadable`, each with its own `assertion.file.contents_equals_actual_*` code, contributing to `CaseStatus::Fail`.

The expected side is different. Its value is `ExpectedValue<FileContentsReference>` — either a `WorkspacePath` naming a second file inside the same isolated case workspace, or a `FixtureReference` naming a fixture near the `*.repor` file. Either way, that value is part of the *test definition*, not the subject under test's output. A missing, non-regular, or unreadable expected `WorkspacePath` is therefore a test-definition error: it surfaces as `CaseStatus::ScriptError` (top-level `script_error`, exit code 2) with a new `semantic.file_contents_reference.missing` / `.not_regular_file` / `.read_error` code — never as an assertion failure, and never silently treated as "expected empty bytes." An unresolvable `FixtureReference` is classified the same way (test-definition error, `CaseStatus::ScriptError`), reusing #92's existing `semantic.fixture_reference.*` codes rather than inventing parallel ones, since #92 already made that same missing/not-a-regular-file/escapes-directory classification for fixture resolution.

This asymmetry is a direct consequence of #92's own subject/expected asymmetry (`file <ActualValue<WorkspacePath>> contents_equals <ExpectedValue<FileContentsReference>>`): the same `WorkspacePath` *kind* plays a different semantic *role* depending on position, and that role is what determines failure classification here, not the kind itself.

### A `contents_equals` expected-value error aborts the case immediately, including inside a logical composition

Evaluating an expectation now returns `Result<ExpectationResult, ExpectedContentsError>` instead of a bare `ExpectationResult`. Every expectation kind except `contents_equals` always returns `Ok`; only `contents_equals`'s expected-value resolution can produce `Err`. `evaluate_case` treats an `Err` exactly like the pre-existing path-policy check that runs before any expectation in a block is evaluated: it aborts the whole case as a `CaseStatus::ScriptError` at that assertion block, without evaluating the block's remaining expectations.

A `not` / `all` / `any` composition propagates this the same way a bare expectation does: `Vec<Result<_, _>>::into_iter().collect::<Result<Vec<_>, _>>()` short-circuits on the first child error, so a `contents_equals` expected-value error nested inside a composition is never swallowed as an ordinary failing child that a `not` could turn into a passing composition. A composition combines assertion *outcomes*; it must not let a test-definition problem hide behind that combination.

### `*.repor` source path threads through `evaluate` → `evaluate_case` → `Checkpoint`

`evaluator::evaluate` now takes the referencing `*.repor` file's path as a third argument (previously, `main.rs` patched `CaseResult::source_path` onto the result after the fact, and no code path knew the source file's location during evaluation at all). `evaluate_case` derives `repor_dir` from that path's parent directory once, and it travels on every `Checkpoint` (`Checkpoint::initial` / `Checkpoint::after_action` both now take it) so `resolve_expected_contents` can call `fixture::resolve_fixture_source` against it when the expected value is a `FixtureReference`. Setting `CaseResult::source_path` moved into `evaluate_case` itself now that it already has the path, removing the post-hoc patch loop.

A `FixtureReference` expected value is resolved and materialized (`fixture::resolve_fixture_source` then `fixture::materialize_fixture`, into a fresh `tempfile::TempDir` created per resolution) exactly as #92's ADR described: assertion evaluation never reads fixture bytes directly from the test-definition source tree.

### Bounded, escaped mismatch diagnostics; CLI stdout/stderr never carry raw bytes

A mismatch computes only bounded facts (`ContentsMismatch`: actual/expected byte lengths, first differing byte offset) at comparison time (`ContentsEqualsComparison::compare`), alongside the full actual/expected byte buffers (kept in-memory, exactly like `ExpectationKind::StdoutContains` already keeps full captured output). Turning those bounded facts plus the full buffers into a human-readable diagnostic — the first differing byte-line number, and an escaped, size-capped context window around it — is a presentation-layer concern, not a comparison-semantics one: it lives in a new `reportage_core::contents_diagnostic` module shared by both the human renderer and the `--format=json` renderer, rather than being duplicated in each.

The context window prefers up to two lines before and after the first differing line (LF-delimited; CRLF is not normalized, so a bare CR is an ordinary byte within a line, not a boundary). If either side's window would exceed a fixed byte cap — a single huge line, or binary-like content — both sides fall back to a fixed-radius byte window centered on the offset instead. Every window is escaped: valid UTF-8 is kept legible with only control characters (NUL, ESC, bare CR, etc.) backslash-escaped; invalid UTF-8 falls back to a per-byte hex escape for the whole window. Neither renderer ever prints the full actual/expected bytes — only this bounded, escaped context, the byte lengths, and the offset/line number.

This bound applies specifically to *this process's own stdout/stderr* (the CLI's human output and its single `--format=json` document) — not to the `.reportage/runs/<id>/result.json` evidence artifact, which already embeds full captured bytes for other expectation kinds (`stdout contains`, `stdout empty`, etc.) via `stream_json`. `contents_equals` follows that same existing convention: `artifact.rs` embeds the full actual/expected bytes for `file` / `stdout` / `stderr contents_equals` too. Persisting mismatch bytes as evidence was not made mandatory by this issue; it falls out for free from reusing the existing artifact convention rather than being a new obligation.

## Rationale

Classifying actual-side and expected-side failures differently is what makes `contents_equals` usable as a real assertion at all: without it, a typo'd expected fixture path would silently report as an assertion failure indistinguishable from a genuine output regression, instead of surfacing as the test-definition bug it actually is.

Aborting a case immediately on an expected-value error — including inside a logical composition — keeps the existing "semantic error stops the case before evidence comparison" story intact. Letting a composition's `not` silently convert a broken test definition into a passing assertion would be far worse than a bare `todo!()` panic.

Sharing one `contents_diagnostic` module between both renderers avoids re-implementing bounded/escaped rendering twice and drifting the two implementations' definitions of "bounded" apart.

## Consequences

### Positive Consequences

- `contents_equals` is fully usable for `file`, `stdout`, and `stderr`, against either a workspace file or a fixture file, with clear pass/fail/script-error semantics.
- A broken expected value (typo'd path, missing fixture) is diagnosable as a test-definition problem (exit code 2), not confusable with a genuine SUT regression (exit code 1).
- CLI output stays bounded even when comparing large or binary-like files, while the run's evidence artifact keeps full bytes for anyone who needs to inspect a mismatch in full.

### Negative Consequences

- `evaluate_expectation_at_checkpoint`'s signature changed from returning `ExpectationResult` to `Result<ExpectationResult, ExpectedContentsError>`, a breaking change for any caller outside this crate (there are none yet; the crate is unreleased).
- `evaluate` gained a required `source_path` parameter; every caller (including test helpers) had to be updated, even those that never use `contents_equals`.
- The line-context window's 2-lines-before/after heuristic and byte-size caps are judgment calls, not something derived from a stricter specification; they may need revisiting once real large-file or binary-content usage is observed.

### Neutral Consequences

- `file text_equals` (#88) still `todo!()`s; it does not share `contents_equals`'s comparison logic (byte equality vs. text equality) but will likely share `ExpectedContentsError`-shaped test-definition-error classification once implemented.
- The `.reportage/runs/<id>/result.json` evidence schema gained `fileContentsEquals` / `stdoutContentsEquals` / `stderrContentsEquals` expectation kinds and an `expectedSource` field; this is additive and not currently pinned by an artifact schema stability contract (see [TBD.md](../TBD.md) — Artifact schema stabilization).
