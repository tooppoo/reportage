
# String Literal Escape Sequences and Raw Newline Rejection

- Status: Accepted
- Created: 2026-07-01T21:46:58Z

## Context

Reportage v0 needs to define what a string literal (`quoted_string` in
`crates/reportage-core/src/reportage.pest`) may contain, beyond "any
character except an unescaped double quote". Two related issues were
undecided before this ADR:

1. Whether string literals may contain a raw (unescaped) newline.
2. Whether backslash-escape sequences are recognized inside string literals,
   and if so, what value they produce in the AST.

`#28` added syntax conformance fixtures, but deliberately left string literal
escaping out of scope, since the answer affects AST value representation and
`unescape` semantics, not just accept/reject behavior.

`stdout contains` / `stderr contains` are used to verify CLI output, which
routinely contains newlines and tabs. If string literals could not represent
a newline or tab, or if `\n` were kept as the two source characters
backslash + `n` instead of an actual newline, users could not write natural
assertions against multi-line or tab-containing output.

At the same time, allowing raw multiline strings directly inside a
`quoted_string` would complicate the line-based structure that
`case_block` / `assertion_block` and diagnostic line/column reporting rely
on, and would compete with a future heredoc design for genuinely large or
multi-line text.

## Decision

v0 string literals must not contain a raw newline. This includes:

- LF (`\n`)
- CRLF (`\r\n`)
- a bare CR (`\r`) not part of a CRLF pair

Any of these inside a `quoted_string` is rejected as a grammar failure
(`parse.syntax`), including the case where the string is never closed before
the newline or end of input.

v0 string literals recognize exactly four escape sequences:

- `\\` → backslash
- `\"` → double quote (does not terminate the string)
- `\n` → newline
- `\t` → tab

Any other backslash sequence (e.g. `\x`, `\r`, `\u{...}`) is rejected as a
grammar failure (`parse.syntax`). This is implemented directly in the pest
grammar (`crates/reportage-core/src/reportage.pest`):

```pest
quoted_string = { "\"" ~ string_inner ~ "\"" }
string_inner  = @{ string_char* }
string_char   = _{ escape_seq | (!("\"" | "\\" | "\r" | "\n") ~ ANY) }
escape_seq    = _{ "\\" ~ ("\\" | "\"" | "n" | "t") }
```

Because `string_char` excludes raw `"`, `\`, `\r`, and `\n` from the
non-escape branch, an unrecognized escape or a raw newline simply cannot be
consumed by `string_inner`; the surrounding `quoted_string` then fails to
find its closing `"`, which pest reports as a syntax error at that position.
This keeps invalid escape/raw-newline handling as ordinary grammar
rejection, consistent with `#35`'s policy that grammar rejections use the
coarse `parse.syntax` code unless Reportage explicitly detects and
distinguishes the failure as parse-domain validation.

The AST holds the **unescaped** value, not the source representation.
`stdout contains "a\nb"` is held in the AST as `a` + an actual newline + `b`.
A user who wants to match the literal two characters backslash + `n` writes
`stdout contains "a\\nb"`, which produces `a` + backslash + `n` + `b`.

Unescaping happens once, in `parser::extract_string_inner` /
`parser::unescape_string`, and applies uniformly everywhere `quoted_string`
is used (case name, `stdout contains`, `stderr contains`), not only to
output-matcher strings.

## Alternatives Considered

### Keep string literals as opaque raw text (no escape processing)

Rejected. Without escape processing, `stdout contains` could not express a
newline or tab at all (since raw newlines are rejected), which defeats the
main use case for `contains` against real CLI output.

### Preserve `\n` / `\t` as their two-character source form in the AST

Rejected. This mirrors the raw source text instead of the value a user
means to assert against, and would force every caller of `contains` to
double-escape (`\\n`) just to match an actual newline in output. The chosen
unescape-eagerly approach matches common expectations from other
languages' string literals.

### Allow raw multiline strings inside `quoted_string`

Rejected for v0. Raw multiline string content is deferred to a future
heredoc design (see `#36`'s non-goals). Allowing it inside an ordinary
`quoted_string` now would complicate assertion-block line structure and
diagnostic location reporting, and would need to be reconciled with heredoc
syntax later anyway.

### Support a wider escape set now (e.g. `\r`, `\xNN`, `\u{...}`)

Rejected for v0. Restricting to the minimal four escapes keeps the parser
and AST value space small and leaves room to add `\r` or unicode escapes
later without it being a breaking change (adding a new escape is additive;
an unrecognized sequence is a hard grammar error today, not silently passed
through).

## Consequences

### Positive Consequences

- `stdout contains` / `stderr contains` can naturally express multi-line or
  tab-containing expected output.
- String literal escape and raw-newline handling is defined identically for
  case names and output-matcher strings, since both use `quoted_string`.
- Unrecognized escapes and raw newlines fail as ordinary grammar errors
  (`parse.syntax`), without requiring new diagnostic codes.
- The four-escape set leaves room to add more escapes later as a
  non-breaking, additive grammar change.

### Negative Consequences

- The parser must unescape string content instead of treating a
  `quoted_string` as opaque source text, adding a small amount of parsing
  logic (`parser::unescape_string`).
- Users cannot express a genuinely multi-line string value inline; they must
  wait for a future heredoc design, or compose it from `\n`.

### Neutral Consequences

- This ADR does not design heredoc syntax, template heredocs, or file
  heredocs; that is out of scope for `#36` and left to a future issue.
- This ADR does not introduce a dedicated diagnostic code for string literal
  failures; they remain `parse.syntax` unless a later change makes them
  explicit parse-domain validation.
