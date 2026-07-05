
# Line and Inline Comments

- Status: Superseded by [20260705T184047Z_use-hash-comment-marker.md](20260705T184047Z_use-hash-comment-marker.md)
- Created: 2026-07-02T03:24:58Z

## Context

Reportage scripts double as human-read E2E scenario documentation (`#32`), so authors need a way to write explanatory text inline with the script without it affecting evaluation. `#26` migrated `crates/reportage-core/src/reportage.pest` to be the canonical syntax source of truth, but that grammar defines no `WHITESPACE` or `COMMENT` implicit-skip rule — every whitespace and line-ending position is handled explicitly via `ws` / `nl` / `blank_line` / `trail`. `#28` (syntax conformance fixtures) and `#38` (AST snapshots) deliberately left comment syntax out of scope; this ADR and `#32` are where it is defined.

Two constraints make this more than a trivial "add a skip rule" change:

1. `$` action lines (`$ <shell command>`) capture everything up to the newline as opaque shell text (`command = @{ (!nl ~ ANY)* }`). A `//` inside a shell command (a URL, an actual shell comment, a literal string) must not be stripped or reinterpreted as a Reportage comment.
2. A comment must not be able to split a token sequence or hide a closing brace. For example, `case "x" // comment\n{` must remain a syntax error (the comment must not let the parser treat the next line's `{` as still belonging to the `case` statement), and `assert { exit 0 // comment }` must remain a syntax error (the comment must not let the same-line closing `}` be swallowed as part of the comment).

## Decision

The comment marker is `//`, extending to the end of the line. Comments are discarded at parse time and never appear in the AST, semantic model, or result artifacts.

Reportage does **not** use pest's implicit `COMMENT` skip mechanism. Instead, two silent grammar rules are added and spliced in only at the specific positions listed below — never as generic inter-token filler:

```pest
comment      = _{ "//" ~ (!nl ~ ANY)* }
comment_line = _{ ws* ~ comment ~ (nl | EOI) }
trail        = _{ ws* ~ comment? ~ (nl | EOI) }   // was: ws* ~ (nl | EOI)
```

`trail` already was the grammar's single choke point for "end of a logical line" (used after a case block's opening `{`, after its closing `}`, after every case step, after every multi-line assertion expectation, and after the `assert {` opener). Making `trail` comment-aware, and adding `comment_line` as a `blank_line`-like alternative at three loop positions, covers every position `#32` requires with no other grammar changes:

- `script = { SOI ~ (blank_line | comment_line | case_block)* ~ ws* ~ EOI }` — top-level comment-only lines.
- `case_block`'s inner loop gains `comment_line` — comment-only lines inside a case block.
- `multi_assert = { trail ~ (comment_line | assertion_line)+ ~ ws* }` — comment-only lines inside a multi-line assertion block, plus (via the leading `trail`) a trailing comment on the `assert {` opener line.
- `trail` itself, unchanged in every other call site, now also covers: trailing comment on a case block's open line and close line, on an assertion block's close line (both the multi-line form's own `}` and the single-line form `assert { exit 0 } // c`, since both are wrapped by `case_step`'s trailing `trail`), and on each expectation line inside a multi-line assertion block.

Comment support is intentionally **not** added to `single_assert`'s inner `ws*`, `case_block`'s pre-`{` `ws*`, or `assertion_block`'s pre-`{` `ws*`. Leaving those untouched is what makes the disallowed positions rejections fall out of the grammar by construction, not by any additional validation code:

- `case "x" // c\n{` — `case_block`'s `ws*` before `"{"` only matches space/tab, so it cannot skip past `//` to reach the `{` on the next line. Syntax error.
- `assert // c\n{` — same reasoning on `assertion_block`'s pre-`{` `ws*`. Syntax error.
- `assert { exit 0 // c }` — after `single_assert` matches `exit 0`, `assertion_block` requires `ws* ~ "}"` immediately, not more same-line content; `multi_assert` also fails because its leading `trail` requires an `nl`/`EOI` right after `{`, not further tokens on the same line. No alternative accepts the input. Syntax error.
- `exit // c\n0` — `exit_exp`'s `ws+` after `"exit"` must be followed directly by `exit_code` (`ASCII_DIGIT+`), not `//`. Syntax error.
- A block whose real closing `}` is itself written inside a `// ...` comment leaves genuinely too few unescaped braces in the file, so the block (or the file) ends up unclosed. Syntax error.
- A multi-line assertion block containing only comment-only lines and no real expectation (e.g. `assert {\n  // comment only\n}`) is rejected the same way an empty assertion block already is. `multi_assert` requires at least one real `assertion_line` — `comment_line* ~ assertion_line ~ (comment_line | assertion_line)*` — rather than `(comment_line | assertion_line)+`, precisely so a run of `comment_line` alone cannot satisfy the block. (An earlier version of this grammar used `(comment_line | assertion_line)+`, which let a comment-only block reach `parser::parse_assertion_block` with an empty expectations list and panic on the `AssertionBlock::new(...).expect(...)` call that assumes the grammar already guarantees at least one expectation; this was caught in review before merge.)

`$` action lines need no special-casing at all: `command`'s `@{ (!nl ~ ANY)* }` already consumes the rest of the line — including any `//` — before `trail` is reached, so `trail`'s new optional comment never gets a chance to fire on an action line. `$ echo hello // passed to shell` keeps `echo hello // passed to shell` as the literal command text.

String literals need no special-casing either: `comment` / `comment_line` are only spliced at line-end / whole-line positions, never inside `quoted_string`'s matching path, so `stdout contains "https://example.com"` is parsed by `string_char` exactly as before.

Because `comment` and `comment_line` are silent (`_{ }`) grammar rules, pest never emits a `Rule::comment*` pair for them. `parser.rs`'s AST-building code needs no changes: a comment structurally cannot reach `parse_case_block`'s `rule => unreachable!(...)` arm, and the AST types in `model.rs` gain no comment-related field. This is a stronger guarantee than filtering comments out in `parser.rs` would be — it is enforced by the grammar itself, not by parser discipline.

## Alternatives Considered

### Use pest's implicit `COMMENT` rule with global skip

Rejected. `reportage.pest` defines no `WHITESPACE`/`COMMENT` implicit-skip rule today; every whitespace position is explicit. Introducing implicit skipping now would apply it everywhere pest inserts implicit rules between tokens, including inside `case_block`'s pre-`{` sequence and `assertion_block`'s brace matching — exactly the positions this ADR's Decision leaves untouched on purpose to reject `case "x" // c\n{` and `assert { exit 0 // c }`. An implicit skip would silently accept both, defeating the issue's explicit requirement that comments cannot split a token sequence or swallow a closing brace.

### Allow comments to interrupt arbitrary token sequences

Rejected for the same reason: allowing `//` anywhere between tokens (not just at specific line-end/whole-line positions) reintroduces the `case "x" // c\n{` ambiguity and the `assert { exit 0 // c }` brace-swallowing problem that the issue explicitly calls out as required rejections.

### Treat `//` inside `$` action lines as a comment marker

Rejected. Action lines are opaque shell command text (`sh -c`); a legitimate command may contain `//` (a URL, a path, an actual shell `#`-based comment embedded differently, or shell syntax that happens to include a double slash). Stripping or reinterpreting it would silently change the command sent to the shell. `$` lines are excluded from Reportage-comment handling entirely.

## Consequences

### Positive Consequences

- Comment support required no changes to `parser.rs` or `model.rs`; it is enforced entirely by `reportage.pest`, keeping the "comments never reach the AST" guarantee structural rather than a matter of parser discipline.
- The disallowed-position rejections (comment splitting a case header, an `assert` keyword, or expectation tokens; a comment swallowing a single-line assertion block's closing brace) require no dedicated validation code — they are inherent to where `comment`/`comment_line` were (and were not) spliced in.
- `$` action line command text and string literal content are both unaffected by construction, with no special-casing needed in either the grammar or the parser.
- Comment syntax errors remain the generic `parse.syntax` diagnostic code, consistent with the `#35`/`#36` policy of not introducing new diagnostic codes for ordinary grammar rejections.

### Negative Consequences

- Comments are line-end/whole-line trivia only; there is no block comment, nested comment, or documentation comment syntax. Users who want to comment out a multi-line span must prefix each line individually.
- A comment can still visually mislead a human reader into thinking a block is closed when the real closing brace is inside the comment (the `unclosed_block_hidden_by_comment` fixture); the parser correctly rejects this, but the resulting diagnostic points at wherever the grammar actually ran out of input/braces, not at the specific commented-out brace.

### Neutral Consequences

- This ADR does not add comment preservation for formatting tools, nor does it change how `docs/syntax.md` presents the grammar beyond the new rules themselves (regenerated via `just lang-docs-gen`).
