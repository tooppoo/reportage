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
// Trailing whitespace before a newline or end of file.
// Used wherever a line ending may carry incidental trailing spaces.
trail      = _{ ws* ~ (nl | EOI) }

// ─── Script ───────────────────────────────────────────────────────────────────

// ws* before EOI handles a trailing whitespace-only line with no final newline.
script = { SOI ~ (blank_line | case_block)* ~ ws* ~ EOI }

// ─── Case block ───────────────────────────────────────────────────────────────

case_block = {
    ws* ~ "case" ~ ws+ ~ quoted_string ~ ws* ~ "{" ~ trail
    ~ (blank_line | case_step)*
    ~ ws* ~ "}" ~ trail
}

// Silent: action_step and assertion_block are promoted directly into case_block.
case_step = _{ ws* ~ (action_step | assertion_block) ~ trail }

// ─── Action step ──────────────────────────────────────────────────────────────

action_step = { "$" ~ ws* ~ command }
// Captures everything up to the newline; Rust trims trailing whitespace.
command     = @{ (!nl ~ ANY)* }

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
// trail handles optional trailing whitespace on the `assert {` line.
// Requires at least one expectation (+ quantifier), rejecting empty blocks.
multi_assert   = { trail ~ assertion_line+ ~ ws* }
assertion_line = _{ ws* ~ expectation ~ trail }

// ─── Expectations ─────────────────────────────────────────────────────────────

expectation     = { exit_exp | stdout_exp | stderr_exp }

exit_exp        = { "exit" ~ ws+ ~ exit_code }
exit_code       = @{ ASCII_DIGIT+ }

stdout_exp      = { "stdout" ~ ws+ ~ output_matcher }
stderr_exp      = { "stderr" ~ ws+ ~ output_matcher }

output_matcher  = { output_contains | output_empty }
output_contains = { "contains" ~ ws+ ~ quoted_string }
output_empty    = { "empty" }

// ─── String literals ──────────────────────────────────────────────────────────

// v0 forbids raw newlines (LF, CRLF, and bare CR) inside string literals and
// allows exactly four escape sequences: \\, \", \n, \t. Any other backslash
// sequence is rejected. See docs/adr/20260701T214658Z_string-literal-escape-sequences.md.
quoted_string = { "\"" ~ string_inner ~ "\"" }
string_inner  = @{ string_char* }
string_char   = _{ escape_seq | (!("\"" | "\\" | "\r" | "\n") ~ ANY) }
escape_seq    = _{ "\\" ~ ("\\" | "\"" | "n" | "t") }
```
