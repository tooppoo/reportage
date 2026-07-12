
# Heredoc Literal Naming, and `text_literal` / `TextValue`

- Status: Accepted
- Created: 2026-07-06T10:41:51Z

## Context

#67 introduced a fenced ` ``` ... ``` ` block for the `write` step's content, internally named "fenced raw text block" (`RawTextBlock` in the AST). #86 needs `file "<path>" contains ...` to accept that same construct, alongside its existing plain string literal, and needs `write` to accept a plain string literal too. Once two constructs (`write`'s content, `file contains`'s expected text) each independently accept either a string literal or the fenced block, Reportage needs one shared name and one shared vocabulary for "the fenced block" and for "either of these two forms," rather than each call site inventing its own.

This ADR does not reopen #67's own decision (recorded in [`20260704T183546Z_write-step-and-per-case-workspace-isolation.md`](20260704T183546Z_write-step-and-per-case-workspace-isolation.md), section 6) that the fenced block is not a `<<NAME`-delimited heredoc. That decision — no delimiter name, fence-length-based closing — still holds exactly as written. What changes here is naming and reuse: the construct needed a name usable in prose across two call sites (and, later, more), and "fenced raw text block" read as `write`-specific and mechanical rather than as a general syntax category.

## Decision

### The fenced block is named "heredoc literal"

The ` ``` ... ``` ` construct is called a **heredoc literal** everywhere — grammar rule names (`heredoc_literal`, `heredoc_body`, `heredoc_body_line`), doc comments, `docs/semantics.md`, and diagnostic prose. "Fenced raw text block" and "fenced text literal" are retired names; where they appeared in code comments and docs, they have been reworded. This is a naming choice, not a syntax change: fence length, dedent, final-newline, and empty-block rules are unchanged from #67.

The one place the old name is *not* edited is `20260704T183546Z_write-step-and-per-case-workspace-isolation.md` itself — merged ADRs are left as the historical record of the decision as it stood at the time, per this repo's convention (see e.g. [`20260706T150000Z_action-line-continuation.md`](20260706T150000Z_action-line-continuation.md), which similarly extends an earlier ADR by cross-reference rather than by editing it).

### A new syntax category, `text_literal`

```text
text_literal = string literal | heredoc literal
```

`write "<path>" <text_literal>` and `file "<path>" contains <text_literal>` both accept either form. There is no single pest grammar rule named `text_literal`, because the two forms have incompatible line-ending rules: a heredoc literal is inherently multi-line and its closing fence line consumes its own trailing line ending (no inline comment allowed, per #67); a string literal is single-line and relies on the surrounding `trail` rule (which does allow a trailing comment). Every grammar position that accepts a `text_literal` is therefore expressed as two ordered alternatives instead of one shared rule:

- `write_step_string` (`"write" ~ quoted_string ~ quoted_string`, sharing `trail` like every other single-line step) and `write_step_heredoc` (`"write" ~ quoted_string ~ heredoc_literal`, self-terminating).
- `file_contains` (unchanged, still `"contains" ~ quoted_string`) and `file_exp_heredoc` (`"file" ~ quoted_string ~ "contains" ~ heredoc_literal`, self-terminating, reachable only through the multi-line `assert { ... }` form since a heredoc literal cannot fit inside a single physical line).

A side effect worth naming explicitly, not fighting: `write`'s string-literal form (`write_step_string`) now shares `trail`, so `write "<path>" "<text>" # comment` is accepted — consistent with every other single-line construct, whereas previously `write` had no single-line form at all.

### AST keeps literal kind visible; runtime resolves to `TextValue`

```rust
pub enum TextLiteral {
    Quoted(String),
    Heredoc(String),
}

impl TextLiteral {
    pub fn to_text_value(&self) -> TextValue { ... }
}

pub struct TextValue(String);
```

`TextLiteral` is the AST-level representation: it keeps which surface form a script used distinguishable, for diagnostics, AST snapshots, and docs generation fidelity. `TextValue` is the runtime-level representation: the value `write` actually writes as file bytes, and the value `file ... contains` actually substring-matches against, with its syntactic origin erased. `TextLiteral::to_text_value()` is the only sanctioned crossing point between the two — evaluator code must never match on `TextLiteral`'s variants, so `write` and `file contains` behave identically regardless of which literal form produced the value.

`WriteFileStep.content` and `FileMatcher::Contains`'s payload are both `TextLiteral`. `FileMatcher::Matches`, `OutputMatcher::*`, and `DirMatcher::Contains` are untouched — `stdout contains`, `stderr contains`, and a future `file text_equals` remain string-literal-only in v0. This is a deliberate deferral, not a design constraint: `heredoc_expectation` is its own grammar rule (not inlined into `expectation`), so adding e.g. `stdout_exp_heredoc` later is additive.

### No `Span` type

No AST node in this codebase carries a source span today; `ParseError` computes `line`/`column` ad hoc, only at error-construction time, via `pest::Pair::line_col()`. Introducing a `Span` type now, just so `TextLiteral` could carry one, would be new architecture unrelated to what #86 needs — the `Quoted`/`Heredoc` tag alone already satisfies "keep literal kind distinguishable in the AST."

## Alternatives Considered

### Keep `RawTextBlock`'s name; widen `write` / `file contains` to accept it directly

Rejected: this keeps "kind" as a `write`-specific implementation detail rather than something that lives on the runtime path, and doesn't scale cleanly to a third or fourth future call site, each of which would have to reinvent the same "is this a plain string or the fenced form" branch.

### Collapse to one AST type that erases literal kind immediately at parse time

Rejected: this would satisfy `write` and `file contains`'s runtime behavior, but violates the requirement that diagnostics, AST snapshots, and docs generation can still tell which surface form a script used.

## Consequences

### Positive Consequences

- One shared heredoc-literal parsing/dedent implementation (`parse_heredoc_literal`, `dedent_heredoc_body`) serves both `write` and `file contains` today, and future call sites without duplicating fence/dedent logic.
- `write`'s string-literal form gains trailing-comment support "for free," consistent with every other single-line construct.
- `stdout`/`stderr contains` and `file text_equals` can adopt heredoc-literal support later as an additive grammar change, not a rework.

### Negative Consequences

- `write_step` is now two grammar rules and two parser functions instead of one; `case_step` and `multi_assert` are correspondingly slightly more branchy.
- `file contains`'s heredoc form is a separate grammar rule from `file_contains`/`file_predicate`, rather than a variant nested inside them, since it must avoid the generic `trail` its sibling relies on.

### Neutral Consequences

- The stable diagnostic code is renamed from `parse.raw_block.shallow_indent` to `parse.heredoc_literal.shallow_indent`, alongside the internal Rust enum variant (`DiagnosticCode::ParseHeredocLiteralShallowIndent`) and its prose description. Renaming a diagnostic code is classified as breaking per [`docs/diagnostics.md`](../reference/diagnostics.md)'s compatibility policy, but v0 does not commit to a strict semver policy for diagnostic codes and only requires the reason to be recorded at the time of the change (this bullet is that record): leaving the old name in place would keep the one retired term ("raw block") this ADR otherwise removes everywhere else, in the one place — a code string — a script author is most likely to see it.
