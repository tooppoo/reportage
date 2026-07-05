
# Use `#` as the Comment Marker

- Status: Accepted
- Created: 2026-07-05T18:40:47Z

## Context

`#32` introduced line and inline comments with `//` as the comment marker, recorded in [Line and Inline Comments](20260702T032458Z_line-and-inline-comments.md). `#77` revisits that choice.

Reportage's action steps are shell-like: an action body (`$ <command>`) is passed to the shell as opaquely as possible, and the project deliberately does not reimplement shell quoting, escaping, or comment semantics. With `//` as the Reportage marker, authors had to switch between two comment notations — `//` outside an action body and the shell's `#` inside one — even though the rest of the script is designed to read like a shell session.

At the same time, Reportage is not a shell parser. Whatever marker is chosen, the Reportage comment syntax outside action bodies must stay clearly separated from whatever the shell does with `#` inside action bodies, and it must not break future bare tokens that legitimately contain `#` (paths like `tmp/foo#bar`, URLs with fragments).

This changes the public script syntax, supersedes an accepted ADR, and constrains future token design, so it is recorded as an ADR.

## Decision

The Reportage comment marker is `#`. `//` is no longer a comment marker.

`#32`'s need for line and inline comments is unchanged; only the marker and the exact start-of-comment rule are redefined here. This ADR supersedes [Line and Inline Comments](20260702T032458Z_line-and-inline-comments.md); that ADR's structural approach — no implicit pest `COMMENT` skip rule, silent `comment` / `comment_line` rules spliced only at explicit "whole line" and "end of logical line" positions, comments discarded at parse time and never present in the AST, semantic model, or result artifacts — is carried over as-is.

The rules, expressed against `crates/reportage-core/src/reportage.pest`:

```pest
comment      = _{ "#" ~ (!nl ~ ANY)* }
comment_line = _{ ws* ~ comment ~ (nl | EOI) }
trail        = _{ (ws+ ~ comment)? ~ ws* ~ (nl | EOI) }
```

- A whole-line comment is `#` at the start of a line, optionally after indentation (`comment_line`).
- An inline comment is only allowed as end-of-line trivia after a completed syntax element, and only when the `#` is separated from that element by at least one space or tab (`trail`'s `ws+ ~ comment`). `exit 0 # comment` is valid; `exit 0#comment` is a syntax error.
- A comment cannot be inserted in the middle of a token sequence; `trail` / `comment_line` remain the only splice points, exactly as before.
- `#` inside a string literal is ordinary string content (`"hello # world"`, `"foo#bar"`).
- `#` inside an action body is not a Reportage comment. The `command` rule captures the rest of the line before `trail` can fire, so `$ echo hello # shell comment` passes `echo hello # shell comment` to the shell verbatim, and the shell's own semantics decide what `#` means there. This also holds under any future action line continuation syntax.

Requiring whitespace before an inline `#` is what keeps the marker change compatible with future bare tokens containing `#`: a `#` glued to a token never starts a comment, so values like `tmp/foo#bar` or `https://example.com/page#section` remain expressible without quoting tricks if bare tokens ever allow them.

## Alternatives Considered

### Keep `//` as the comment marker

Rejected. It forces two comment notations in one script — `//` outside action bodies, `#` inside them — which contradicts the shell-like reading experience the action syntax aims for and raises the reader's cognitive load for no offsetting benefit.

### Treat `#` exactly as the shell does (including inside action bodies)

Rejected. Reportage is not a shell parser and does not reimplement shell quoting/escaping/comment semantics. Stripping `#` comments from action bodies would require exactly that reimplementation, and any divergence from the real shell would silently change the executed command.

### Allow `exit 0#comment` (no whitespace requirement before inline `#`)

Rejected. Treating every unquoted `#` as a comment start would make it impossible to ever introduce bare tokens containing `#` (paths, URL fragments) without breaking scripts, and a glued `#` reads ambiguously to humans as well. Requiring one or more spaces/tabs keeps the comment grammar forward-compatible and makes the author's intent explicit.

## Consequences

### Positive Consequences

- One comment notation for the whole script: the same `#` reads correctly outside action bodies (consumed by Reportage) and inside them (passed to the shell), so a Reportage script now looks uniformly shell-like.
- The whitespace-before-`#` rule reserves room for future bare tokens containing `#` without a breaking syntax change.
- The structural guarantees of the superseded ADR are preserved unchanged: comments cannot split token sequences, cannot swallow a closing brace, never reach the AST, and required no `parser.rs` / `model.rs` changes.

### Negative Consequences

- Any pre-existing script using `//` comments breaks: `//` lines are now syntax errors instead of comments. Reportage is pre-1.0 and ships no migration tooling; scripts must be updated by hand.
- `exit 0#comment` being a syntax error may surprise authors coming from shells, where `0#comment` would simply be a word; the error is a plain `parse.syntax` diagnostic with no dedicated hint.

### Neutral Consequences

- A `//` inside an action body or a string literal was never comment-stripped before and still is not; its meaning is unchanged.
- Comment syntax errors remain the generic `parse.syntax` diagnostic code, consistent with the existing policy of not adding dedicated codes for ordinary grammar rejections.
