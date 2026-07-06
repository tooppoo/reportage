
# Support `\` Line Continuation In Action Steps

- Status: Accepted
- Created: 2026-07-06T15:00:00Z

## Context

A `$` action step (#action_step) was, until this issue, confined to a single physical line: `command = @{ (!nl ~ ANY)* }` captured everything up to the next line break, with no way to split a long shell command or pipe chain across several physical lines for readability.

Shell scripts conventionally support exactly this via `\` + newline line continuation: a trailing backslash immediately before a line break tells the shell to treat the next physical line as a continuation of the current logical line. This issue (#80) brings the same affordance to Reportage's `$` action syntax, scoped narrowly to that one syntactic position — not to assertion blocks, string literals, or any other multiline form, and not as a step toward heredoc support (a separate, explicitly out-of-scope concern here).

Reportage's `$` action body is already opaque to Reportage itself: it is handed to `sh -c` verbatim and Reportage does not parse shell syntax (see [`20260627T100500Z_use-posix-shell-and-path-shims.md`](20260627T100500Z_use-posix-shell-and-path-shims.md)). This ADR extends that stance: Reportage's job is only to decide *where the action body ends*, across possibly-many physical lines; the shell, not Reportage, is responsible for interpreting what `\` + newline means once it receives the text.

## Decision

### The grammar decides continuation with a single, literal, per-line rule

`command`'s grammar rule becomes:

```pest
command = @{ ("\\" ~ nl | (!nl ~ ANY))* }
```

At every position, a `\` immediately followed by a line break (LF or CRLF, matching the file's existing `nl` rule) is consumed as a continuation unit — both characters become part of `command`, and matching carries straight on into the next physical line as more command text. Any other character (including a `\` **not** immediately followed by a line break) is ordinary command text. A line break not immediately preceded by `\` cannot be matched by either alternative, so the repetition stops there — that unconsumed line break is what `case_step`'s existing `trail` rule consumes as the step's terminator, exactly as it already did for a single-line action.

This is a **sequential, per-physical-line** rule, not a "continuation mode" the parser enters and exits: each line's own trailing marker is what pulls in exactly the next physical line, and whether that pulled-in line continues *again* depends only on how *it* ends. Concretely, given the grammar above:

- A line ending in `\`+newline pulls in the next physical line unconditionally, whatever it contains — blank, a comment-like `#...` line, or something that looks like `assert {` — because the grammar has no way to "look ahead" at the next line's shape before committing to consume the marker.
- That pulled-in line becomes part of `command` in full. If *it* also ends in `\`+newline, the process repeats. If it does not, the repetition stops right after it, and the *following* line is parsed as an ordinary `case_step` again.
- A blank line pulled in this way ends the continuation there (it cannot itself end in `\`+newline), and is fully absorbed into the action step's span — the same as any other non-continuing pulled-in line, not a special case.
- Because of this, `$ true \` immediately followed by a line that looks like `assert {` is a **caution/invalid example, not a supported one**: the `assert {` line is swallowed whole into the preceding action's `command` (it doesn't end in a marker, so continuation stops right after it), leaving whatever follows (e.g. `exit 0` / `}`) to be parsed as bare, invalid top-level syntax. Reportage does not special-case this to "rescue" the following lines back into a real assertion block; that would require look-ahead the sequential rule deliberately does not have. See "Invalid/caution example" test coverage below.

### Only a literal trailing `\` counts; nothing about it is special-cased

- `\` followed by trailing whitespace before the line break (`\ \n`, `\<tab>\n`), or followed by `#` (`\# comment\n`), is **not** a continuation marker. In both cases the character *immediately preceding* the line break is not `\`, so the ordered-choice grammar rule's first alternative simply doesn't match there; the second alternative consumes the `\` (and whatever follows it) as ordinary command text, and the line break ends the command normally. This intentionally mirrors the shell's own rule (`\` followed by anything other than an immediate newline does not trigger shell-level continuation either) — Reportage rescuing a whitespace- or comment-trailed `\` into a continuation would desync the action body's boundary from where the shell would actually end the logical line.
- The check is a **literal single-character check**, not shell-aware escape interpretation: `\\` (two backslashes) immediately before a line break still continues, because only the character adjacent to the line break is examined — the first `\` is ordinary text, the second is the marker. Reportage does not implement `\\` as "an escaped, literal backslash that cancels continuation"; that would require reimplementing a slice of shell/string escaping semantics that this project deliberately avoids (see "Reportage does not reinterpret shell syntax" below).

### The shell body preserves `\` and newline verbatim

The action body handed to `sh -c` keeps every `\` and every line break exactly as consumed by the grammar — Reportage does not delete the `\`+newline pair to normalize a multi-physical-line action into one logical line before executing it. `parser.rs`'s `parse_action_step` also stops trimming with `str::trim()` (which strips *all* Unicode whitespace, including newlines) in favor of trimming only space and tab from both ends. This matters because the grammar's `command` can legitimately end in a preserved `\`+newline pair (e.g. continuation into a blank final line): blindly calling `.trim()` would strip the trailing newline half of that pair while leaving the `\` behind, corrupting exactly the sequence this feature exists to preserve. Interior indentation, blank lines, and `\` characters within a continued action are left untouched either way — only the leading/trailing space/tab convenience-trim (already relied on by `trailing_whitespace_is_accepted`) survives, now scoped to the characters it was actually meant for.

### Reportage does not reinterpret shell syntax

Reportage is not a shell parser (see the ADRs referenced above) and does not gain quote-awareness, comment semantics, or escape-sequence interpretation for action bodies as part of this issue. The line-continuation rule is deliberately "dumb": a single physical-character check per line, with no notion of whether a `\` sits inside a quoted string, a comment, or anywhere else meaningful to the shell. This keeps Reportage's syntax rule simple and independent of the target shell's dialect, at the cost of the caution/invalid-example behavior above (an action-adjacent `assert {` typo can produce a confusing downstream syntax error rather than a targeted diagnostic) — accepted because fixing that would require Reportage to understand shell syntax well enough to know when a trailing `\` is "real," which is explicitly out of scope.

### No dedicated "unterminated continuation" error

A continuation marker immediately followed by EOF is not treated as a special error class. The grammar's `command` rule simply consumes what it can (the marker's `\`, with nothing after it since there is no following line) and the surrounding grammar behaves exactly as it would for any other input that runs out before an enclosing block (`case_block`, `assertion_block`, and — when introduced — heredocs) is closed: a plain, generic `parse.syntax` error. Minting a `parse.unterminated_continuation`-style code would single out one specific way to leave a block unclosed, when the general "block wasn't closed before EOF" failure mode is already handled uniformly and adequately by the existing pest-driven syntax error path.

## Alternatives Considered

### Track "in continuation" as explicit parser state instead of a per-line grammar rule

Considered writing this as a stateful, hand-rolled continuation loop in `parser.rs` (post-processing raw lines) rather than expressing it directly in the pest grammar.

Rejected: the grammar's `("\\" ~ nl | (!nl ~ ANY))*` already expresses the exact sequential, per-line rule this issue specifies, with the pulled-in-line semantics falling out naturally from ordered-choice repetition — no separate state machine, no risk of the hand-written version drifting from the grammar (which is the project's normative syntax source; see `reportage.pest`'s own header comment).

### Let a pulled-in `assert {`-shaped line "rescue" the following lines back into a real assertion block

Considered detecting when a swallowed continuation line looks like `assert {` (or other Reportage syntax) and treating that as a signal to stop the continuation *before* absorbing it, so `$ true \` immediately followed by `assert { ... }` would parse as two ordinary steps.

Rejected per the review note on this issue: this would require the grammar to look ahead at the *shape* of the next line before deciding whether the previous line's trailing `\` is a continuation marker, breaking the "each line's own trailing marker decides, independent of what follows" sequential rule and reintroducing exactly the kind of shell/Reportage-syntax disambiguation this ADR's "Reportage does not reinterpret shell syntax" section rejects elsewhere. Kept as a documented caution/invalid example instead.

### Introduce a dedicated unterminated-continuation diagnostic code

Considered adding a `parse.unterminated_continuation` code specifically for "EOF right after a trailing `\`".

Rejected: EOF-after-marker is just one of several ways an action's enclosing block can fail to close before EOF (an unclosed `case { `, an unclosed `assert { `, and eventually an unclosed heredoc all hit the same generic pest syntax-error path already). Special-casing this one variant would add a diagnostic code that carries no more actionable information than the existing `parse.syntax` code already does for the same underlying "ran out of input" failure.

## Consequences

### Positive Consequences

- Long shell commands and pipe chains in `$` actions can be split across physical lines for readability, without Reportage reinventing shell continuation semantics.
- The continuation rule is expressible as a two-alternative addition to the existing `command` grammar rule, keeping `reportage.pest` the single normative source for this syntax (mirroring every other syntax decision in this file).
- The rule composes for free with existing action-body behavior: `#` inside a continued action body is still not treated as a Reportage comment (#77's rule already applies uniformly to the whole, now-possibly-multiline, `command` capture), and trailing per-line whitespace trimming still only touches the very ends of the whole captured command.

### Negative Consequences

- An action line immediately followed by a line that happens to look like `assert {` (missing its own trailing content on subsequent lines, or simply a typo) silently becomes part of the action body instead of surfacing a targeted diagnostic; the resulting error is a generic, possibly confusing `parse.syntax` failure further down in the script. This is an accepted trade against the alternative of teaching the grammar to look ahead (see "Alternatives Considered").
- `\\` (two backslashes) immediately before a line break continues, which may surprise anyone expecting shell-style "escaped backslash" semantics. This is accepted because implementing that would require Reportage to interpret backslash-escaping the way a shell or string-literal grammar does, which this project deliberately avoids for action bodies.

### Neutral Consequences

- Heredoc support, assertion-block/string-literal multiline forms, and CRLF-specific continuation handling remain explicitly out of scope, per the issue's stated non-goals; this ADR does not preclude any of them, but does not address them either.
- `docs/syntax.md` is regenerated (via `just lang-docs-gen`) directly from the updated `command` grammar rule and its comment, so the human-readable grammar reference stays the single generated view of this decision rather than a separately maintained prose description.
