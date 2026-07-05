<!-- Generated from crates/reportage-core/src/reportage.pest by scripts/gen-grammar-doc.sh.
     DO NOT EDIT MANUALLY — run `just lang-docs-gen` to regenerate. -->

# Reportage Grammar

> **This file is auto-generated.** Do not edit it manually.
> To update the grammar, modify
> [`crates/reportage-core/src/reportage.pest`](../crates/reportage-core/src/reportage.pest)
> and run `just lang-docs-gen`.

`crates/reportage-core/src/reportage.pest` is the normative syntax source for
Reportage v0. Any syntax not expressible in that file is not part of v0.

## Syntax conformance vs. semantic conformance

This document covers *syntax* only — whether a script is accepted by the
parser. Semantic behaviour (execution order, assertion evaluation, workspace
lifecycle) is defined separately in [`docs/semantics.md`](semantics.md).

## Grammar

```pest
// Reportage v0 grammar — canonical syntax source of truth.
//
// This file is the normative syntax definition for Reportage scripts.
// Any syntax not expressible here is not part of v0.
// See docs/syntax.md for the human-readable generated reference
// (produced by `just lang-docs-gen`; see docs/adr/ for the ADR).

// ─── Primitives ───────────────────────────────────────────────────────────────

ws         = _{ " " | "\t" }
nl         = _{ "\r\n" | "\n" }
blank_line = _{ ws* ~ nl }
// Comment marker `//` to end of line. Discarded at parse time; never appears
// in the AST. Only spliced in at specific "end of logical line" and "whole
// line" positions below (via `trail` and `comment_line`) — never as generic
// inter-token filler — so it cannot split a token sequence (e.g. between a
// case name and `{`) or swallow a closing brace on the same line.
comment      = _{ "//" ~ (!nl ~ ANY)* }
comment_line = _{ ws* ~ comment ~ (nl | EOI) }
// Trailing whitespace, and an optional trailing comment, before a newline or
// end of file. Used wherever a line ending may carry incidental trailing
// spaces or an inline comment.
trail        = _{ ws* ~ comment? ~ (nl | EOI) }

// ─── Script ───────────────────────────────────────────────────────────────────

// ws* before EOI handles a trailing whitespace-only line with no final newline.
script = { SOI ~ (blank_line | comment_line | case_block)* ~ ws* ~ EOI }

// ─── Case block ───────────────────────────────────────────────────────────────

case_block = {
    ws* ~ "case" ~ ws+ ~ quoted_string ~ ws* ~ "{" ~ trail
    ~ (blank_line | comment_line | case_step)*
    ~ ws* ~ "}" ~ trail
}

// Silent: action_step, assertion_block, and write_step are promoted directly
// into case_block. write_step consumes its own trailing line ending (the
// closing fence line, per its own no-inline-comment rule) so it does not
// share the `trail` suffix that action_step / assertion_block rely on.
case_step = _{ ws* ~ ((action_step | assertion_block) ~ trail | write_step) }

// ─── Action step ──────────────────────────────────────────────────────────────

action_step = { "$" ~ ws* ~ command }
// Captures everything up to the newline; Rust trims trailing whitespace.
command     = @{ (!nl ~ ANY)* }

// ─── Write step (fenced raw text block) ────────────────────────────────────────
//
// `write "<path>" ``` ... ``` ` writes a dedented raw text block to a file in
// the concrete case workspace. See docs/semantics.md — Write step.
//
// The opening fence's backtick run is pushed onto pest's match stack so the
// closing fence can be recognized dynamically: PEEK requires at least that
// many backticks (same character), and DROP clears the stack entry once the
// block is fully matched. Neither the opening nor closing fence line accepts
// an inline comment, unlike ordinary steps' `trail`.

opening_fence = @{ "`"{3,} }

// A body line is ordinary content; it must end in an actual newline, never
// EOI, so an unterminated fenced block cannot be silently accepted as an
// empty tail — the mandatory closing_fence_line after raw_block_body then
// fails to match, surfacing as a syntax error.
raw_block_line = _{ (!nl ~ ANY)* ~ nl }

// Indentation is captured verbatim (not `ws`, which is silent) so the parser
// can dedent body lines by literal string prefix, without tab/space width
// normalization.
closing_fence_indent = @{ (" " | "\t")* }

// PEEK matches exactly the pushed opening fence; the trailing `"`"*` allows
// the closing fence to be longer than the opening fence. No inline comment
// is permitted after the fence.
closing_fence_line = { closing_fence_indent ~ PEEK ~ "`"* ~ ws* ~ (nl | EOI) }

// Atomic: the whole span between the opening fence's line ending and the
// closing fence line is captured as one raw string, preserving original
// whitespace and line endings exactly.
raw_block_body = @{ (!closing_fence_line ~ raw_block_line)* }

write_step = {
    "write" ~ ws+ ~ quoted_string ~ ws* ~ PUSH(opening_fence) ~ ws* ~ nl
    ~ raw_block_body
    ~ closing_fence_line ~ DROP
}

// ─── Assertion block ──────────────────────────────────────────────────────────

assertion_block = {
    "assert" ~ ws* ~ "{"
    ~ (single_assert | multi_assert)
    ~ ws* ~ "}"
}

// Single-line form: assert { exit 0 } or assert {exit 0}
// Accepts exactly one expectation; rejects both empty and multiple expectations.
single_assert = { ws* ~ expectation ~ ws* }

// Multi-line form: expectations each on their own line.
// trail handles optional trailing whitespace (and an optional comment) on
// the `assert {` line. comment_line allows comment-only lines interspersed
// with expectations, but a leading/trailing run of comment_line alone must
// not satisfy the block: at least one assertion_line is required, so a
// comment-only assertion block (no real expectation) is rejected the same
// way an empty assertion block is.
multi_assert   = { trail ~ comment_line* ~ assertion_line ~ (comment_line | assertion_line)* ~ ws* }
assertion_line = _{ ws* ~ expectation ~ trail }

// ─── Expectations ─────────────────────────────────────────────────────────────

expectation     = { exit_exp | stdout_exp | stderr_exp | file_exp | logical_composition }

exit_exp        = { "exit" ~ ws+ ~ exit_code }
exit_code       = @{ ASCII_DIGIT+ }

stdout_exp      = { "stdout" ~ ws+ ~ output_matcher }
stderr_exp      = { "stderr" ~ ws+ ~ output_matcher }

output_matcher  = { output_contains | output_empty }
output_contains = { "contains" ~ ws+ ~ quoted_string }
output_empty    = { "empty" }

// `file "<path>" exists` / `file "<path>" contains "<text>"`.
// Subject-first: `file <path>` is the common subject, `exists` / `contains`
// are predicates on that subject. See
// docs/adr/20260704T112155Z_subject-first-file-assertion-syntax.md.
file_exp        = { "file" ~ ws+ ~ quoted_string ~ ws+ ~ file_predicate }
file_predicate  = { file_contains | file_exists }
file_exists     = { "exists" }
file_contains   = { "contains" ~ ws+ ~ quoted_string }

// ─── Logical composition ──────────────────────────────────────────────────────
//
// Block-form logical composition over expectation expressions: `not { ... }`,
// `all { ... }`, `any { ... }`. Infix `A and B` / `A or B`, `and { ... }` /
// `or { ... }` aliases, and predicate-level negation are deliberately not
// expressible in this grammar and are rejected as plain syntax errors. See #25
// and the accompanying ADR.

logical_composition = { not_block | all_block | any_block }

// Reuses single_assert / multi_assert (the assertion_block body forms) so a
// composition block's multiple expectations are grouped exactly like an
// assertion block's implicit `all` — plus empty_composition_body, which lets
// a body with zero expectation expressions parse successfully so Reportage
// can reject it as a semantic error (semantic.expectation.empty_block)
// instead of conflating it with a generic syntax error.
not_block = { "not" ~ ws* ~ "{" ~ (single_assert | multi_assert | empty_composition_body) ~ ws* ~ "}" }
all_block = { "all" ~ ws* ~ "{" ~ (single_assert | multi_assert | empty_composition_body) ~ ws* ~ "}" }
any_block = { "any" ~ ws* ~ "{" ~ (single_assert | multi_assert | empty_composition_body) ~ ws* ~ "}" }

empty_composition_body = { trail? ~ (blank_line | comment_line)* ~ ws* }

// ─── String literals ──────────────────────────────────────────────────────────

// v0 forbids raw newlines (LF, CRLF, and bare CR) inside string literals and
// allows exactly four escape sequences: \\, \", \n, \t. Any other backslash
// sequence is rejected. See docs/adr/20260701T214658Z_string-literal-escape-sequences.md.
quoted_string = { "\"" ~ string_inner ~ "\"" }
string_inner  = @{ string_char* }
string_char   = _{ escape_seq | (!("\"" | "\\" | "\r" | "\n") ~ ANY) }
escape_seq    = _{ "\\" ~ ("\\" | "\"" | "n" | "t") }
```
