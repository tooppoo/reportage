# Adopt Raw Byte Semantics for stdout/stderr

- Status: Accepted
- Created: 2026-07-05T01:20:22Z

## Context

Issue #29 defines stdout/stderr as process output bytes, not decoded text: `stdout contains <string>` / `stderr contains <string>` are byte-level substring matches, and `stdout empty` / `stderr empty` require zero bytes. PR #61's semantic conformance fixtures already encode checkpoint stdout/stderr as base64 `data` with an optional `text` helper view, matching that model.

Production code did not match it. `executor::execute_action` captured stdout/stderr via `String::from_utf8_lossy(...).into_owned()`, so non-UTF-8 process output was silently replaced with U+FFFD before any assertion ever saw it — an unrecoverable, one-way transformation. To keep the semantic conformance harness passing against that behavior, PR #61 lossy-decoded the fixture's base64 bytes the same way before handing them to the evaluator, which papered over the mismatch rather than resolving it, and PR #61 explicitly deferred that resolution to this issue.

Separately, `stdout empty` / `stderr empty` were implemented as `actual.trim().is_empty()`. This is a real bug relative to #29: output consisting only of whitespace, a tab, an LF, a CRLF, or a bare CR was wrongly treated as empty, because `str::trim` strips exactly that whitespace before the emptiness check.

This ADR records the decision to align `ActionResult`, the evaluator, the artifact/result JSON representation, and the semantic conformance fixture harness with the #29 raw byte contract, and to fix the `empty` bug as part of doing so.

## Decision

- `ActionResult.stdout` / `ActionResult.stderr` hold raw process output bytes (`Vec<u8>`), not a decoded `String`. This is the internal semantic model for stdout/stderr end to end: executor capture, evaluator input, and the conformance fixture harness's checkpoint construction all agree on it.
- `executor::execute_action` no longer converts stdout/stderr to `String`. It assigns the process's own stdout/stderr byte buffers directly.
- `stdout contains <string>` / `stderr contains <string>`: the expected string literal — already guaranteed valid UTF-8 by the parser — is compared to the raw output bytes as a byte-level substring search (`expected.as_bytes()` against `actual: &[u8]`), never by decoding `actual`.
- `stdout empty` / `stderr empty` pass only when `actual` is zero bytes (`actual.is_empty()`). Whitespace, tabs, LF, CRLF, and bare CR are output; they do not count as empty.
- Non-UTF-8 stdout/stderr is not rejected anywhere in this path. Reportage does not define encoding-aware assertions in v0 (e.g. decoding Shift-JIS); only raw byte comparisons are normative.
- The semantic conformance fixture harness (`crates/reportage-core/tests/semantic_specs.rs`) decodes `checkpoint.stdout.data` / `checkpoint.stderr.data` from base64 straight to raw bytes and feeds those bytes to the evaluator, with no intermediate lossy decode. This makes the fixture harness's evaluator input identical in kind to what production `execute_action` produces.
- Artifact / result JSON represents stdout/stderr (both the action-level result and the `actual` field of `stdout_contains` / `stderr_contains` / `stdout_empty` / `stderr_empty` expectation results) as `{"data": <base64>, "encoding": "base64", "text": <optional>}`. `data` is the canonical raw-bytes representation. `text` is an optional human-readable helper view, present only when the bytes are valid UTF-8, and is never used for semantic comparison or treated as machine-readable canonical data by any consumer.
- Lossy UTF-8 decoding (`String::from_utf8_lossy`) is retained, but only in display layers: the CLI's failed-expectation diagnostic printer, and the optional `text` field described above. No semantic evaluator input is ever produced by a lossy decode.
- The semantic spec schema's `matchSemantics.comparison` enum keeps the name `byteSubstring` (`spec/language/semantics/schema.json`). It already names the comparison as byte-oriented; introducing a different term would not make the semantics any clearer, only add churn to existing spec files and generated docs.
- Encoding-aware assertions (e.g. asserting against Shift-JIS-decoded content) remain out of scope, as they were in #29.

## Alternatives Considered

### Keep `String` with lossy decoding at capture time (status quo)

Rejected. Lossy decoding at capture time is a one-way, unrecoverable transformation: once a non-UTF-8 byte becomes U+FFFD, no downstream code — evaluator, artifact writer, or CLI — can tell whether the process actually emitted U+FFFD or an arbitrary invalid byte. This makes it impossible to assert precisely on non-UTF-8 output and can produce false positives/negatives in `contains` matches near the substitution.

### Reject non-UTF-8 stdout/stderr outright

Rejected for v0. Reportage runs arbitrary shell commands; refusing to capture output that happens to be non-UTF-8 would make otherwise-passing test cases fail for reasons unrelated to what they are asserting. #29 already decided non-UTF-8 output should not be rejected; this ADR does not revisit that.

### Rename `byteSubstring` to a different byte-oriented term

Considered, since #62's acceptance criteria explicitly ask whether to keep or rename this term. Rejected: `byteSubstring` already unambiguously names both the comparison (substring) and its domain (bytes, not decoded characters). Renaming it would only cost every existing spec file, the generated docs, and this ADR a diff with no semantic gain.

## Consequences

### Positive Consequences

- Non-UTF-8 process output survives capture and evaluation unmodified; `contains` and `empty` behave correctly against it.
- `stdout empty` / `stderr empty` now match the documented and fixture-pinned semantics (zero bytes), fixing a real bug where whitespace-only output was misclassified as empty.
- Production `execute_action` and the semantic conformance fixture harness now feed the evaluator the same *kind* of input (raw bytes), so conformance cases genuinely validate production behavior instead of validating a lossy re-encoding of it.
- Artifact / result JSON gains a stable, canonical byte representation for stdout/stderr that downstream tooling can rely on regardless of encoding, with `text` as a convenience that degrades gracefully (omitted) for non-UTF-8 output.

### Negative Consequences

- `reportage-core`'s production code now depends on the `base64` crate (previously a test-only dependency), to encode canonical bytes into artifact / result JSON.
- Any code constructing or matching on `ActionResult.stdout` / `stderr` as a `String` (production code, tests, or future consumers) must be updated to work with `Vec<u8>`.

### Neutral Consequences

- CLI diagnostic display continues to use lossy UTF-8 decoding, unchanged in spirit from before — only now it is explicitly confined to that one layer instead of being the canonical representation.
