Generated from [crates/reportage-core/src/reportage.pest](../../crates/reportage-core/src/reportage.pest)
by [scripts/gen-grammar-doc.sh](../../scripts/gen-grammar-doc.sh).
DO NOT EDIT MANUALLY — run `just lang-docs-gen` to regenerate.

# Reportage Grammar

> **This file is auto-generated.** Do not edit it manually.
> To update the grammar, modify
> [`crates/reportage-core/src/reportage.pest`](../../crates/reportage-core/src/reportage.pest)
> and run `just lang-docs-gen`.

[crates/reportage-core/src/reportage.pest](../../crates/reportage-core/src/reportage.pest) is the normative syntax source for
Reportage v0. Any syntax not expressible in that file is not part of v0.

## Syntax conformance vs. semantic conformance

This document covers *syntax* only — whether a script is accepted by the
parser. Semantic behaviour is defined separately: execution order and
workspace lifecycle in [`execution-model.md`](execution-model.md),
and assertion evaluation in [`semantics.md`](semantics.md).

## Grammar

```pest
// Reportage v0 grammar — canonical syntax source of truth.
//
// This file is the normative syntax definition for Reportage scripts.
// Any syntax not expressible here is not part of v0.
// See docs/reference/syntax.md for the human-readable generated reference
// (produced by `just lang-docs-gen`; see docs/adr/ for the ADR).

// ─── Primitives ───────────────────────────────────────────────────────────────

ws         = _{ " " | "\t" }
nl         = _{ "\r\n" | "\n" }
blank_line = _{ ws* ~ nl }
// Comment marker `#` to end of line. Discarded at parse time; never appears
// in the AST. Only spliced in at specific "end of logical line" and "whole
// line" positions below (via `trail` and `comment_line`) — never as generic
// inter-token filler — so it cannot split a token sequence (e.g. between a
// case name and `{`) or swallow a closing brace on the same line.
comment      = _{ "#" ~ (!nl ~ ANY)* }
comment_line = _{ ws* ~ comment ~ (nl | EOI) }
// Trailing whitespace, and an optional trailing comment, before a newline or
// end of file. Used wherever a line ending may carry incidental trailing
// spaces or an inline comment. An inline comment only starts at a `#`
// separated from the preceding syntax element by at least one space/tab
// (`ws+ ~ comment`), so a `#` glued to a token (`exit 0#c`) is a syntax
// error rather than a comment — leaving room for future bare tokens that
// contain `#` (paths, URL fragments).
trail        = _{ (ws+ ~ comment)? ~ ws* ~ (nl | EOI) }

// ─── Script ───────────────────────────────────────────────────────────────────

// ws* before EOI handles a trailing whitespace-only line with no final newline.
// Document blocks are grammar-legal anywhere at top level (and any number of
// times) so that their placement rules — the canonical top-level form
// `document file? (document case? case)*` — can be rejected during parser
// construction with actionable diagnostics (parse.document_file.duplicate /
// parse.document_file.after_case / parse.document_case.duplicate /
// parse.document_case.orphan) instead of a bare pest syntax error.
script = { SOI ~ (blank_line | comment_line | document_file_block | document_case_block | case_block)* ~ ws* ~ EOI }

// ─── Document blocks ──────────────────────────────────────────────────────────
//
// `document <scope> { ... }` attaches documentation metadata to a source
// construct as first-class syntax, distinct from `#` comments (which are
// discarded at parse time and never reach any model). v0 supports two
// scopes: `file`, whose metadata describes the whole source file (#168), and
// `case`, whose metadata attaches to the immediately following case (#169).
// See the accompanying ADRs. Any other scope keyword is a plain syntax
// error.
//
// Each scope is its own block rule with its own field whitelist: only that
// scope's field rules are reachable inside its block. Actions, assertions,
// write steps, case blocks, nested document blocks, fields of another scope
// (`group` / `order` inside `document case`), and any future step or
// statement are not alternatives there, so they are rejected at parse time
// by construction rather than by enumerating them in a blacklist. An unknown
// field name likewise fails to match and is a plain syntax error.
//
// A body with zero fields parses successfully (the field line is starred) so
// Reportage can reject an empty document block during parser construction
// (parse.document_block.empty) instead of conflating it with a generic
// syntax error — the same approach as empty_composition_body above.
document_file_block = {
    ws* ~ "document" ~ ws+ ~ "file" ~ ws* ~ "{" ~ trail
    ~ (blank_line | comment_line | document_file_field_line)*
    ~ ws* ~ "}" ~ trail
}

document_case_block = {
    ws* ~ "document" ~ ws+ ~ "case" ~ ws* ~ "{" ~ trail
    ~ (blank_line | comment_line | document_case_field_line)*
    ~ ws* ~ "}" ~ trail
}

// One documentation field per line. The single-line fields share `trail`
// (trailing whitespace and an optional inline comment) like ordinary case
// steps; the heredoc form of `description` consumes its own trailing line
// ending (see "Heredoc literal" below), mirroring the write_step_string /
// write_step_heredoc split.
//
// The file scope accepts `title` / `group` / `order` / `description`; the
// case scope accepts only `title` / `description` — a case has no grouping
// or ordering of its own (cases render in source order), so `group` / `order`
// inside `document case` are unreachable and fail as syntax errors.
document_file_field_line = _{
    ws* ~ ((document_title_field | document_group_field | document_order_field | document_description_string_field) ~ trail | document_description_heredoc_field)
}

document_case_field_line = _{
    ws* ~ ((document_title_field | document_description_string_field) ~ trail | document_description_heredoc_field)
}

// `title` / `group` take a string literal; `description` takes a
// text_literal (string literal or heredoc literal). All literal positions
// parse the kind-agnostic `value_literal` so a wrong-kind literal (e.g.
// `title <"a.txt">`) is a semantic invalid case with an actionable
// diagnostic (semantic.literal.kind_mismatch), never a bare syntax error —
// see "Value literals" below.
document_title_field              = { "title" ~ ws+ ~ value_literal }
document_group_field              = { "group" ~ ws+ ~ value_literal }
document_description_string_field = { "description" ~ ws+ ~ value_literal }
document_description_heredoc_field = { "description" ~ ws* ~ heredoc_literal }

// `order` takes a bare non-negative integer, like exit_code. The grammar
// accepts any digit run; a value that overflows the model's u64 range is
// rejected during parser construction (parse.document_block.invalid_order),
// mirroring the exit_code range check.
document_order_field = { "order" ~ ws+ ~ document_order_value }
document_order_value = @{ ASCII_DIGIT+ }

// ─── Case block ───────────────────────────────────────────────────────────────

case_block = {
    ws* ~ "case" ~ ws+ ~ quoted_string ~ ws* ~ "{" ~ trail
    ~ (blank_line | comment_line | case_step)*
    ~ ws* ~ "}" ~ trail
}

// Silent: action_step, assertion_block, and the two write_step forms are
// promoted directly into case_block. `write <"path"> <text_literal>` accepts
// either a string literal or a heredoc literal (see "Text literals" below);
// the string-literal form (write_step_string) is an ordinary single-line
// construct and shares `trail` like action_step / assertion_block (so it
// also gains an optional trailing comment). The heredoc-literal form
// (write_step_heredoc) consumes its own trailing line ending (the closing
// fence line, per its own no-inline-comment rule) so it does not share the
// `trail` suffix.
case_step = _{ ws* ~ ((action_step | assertion_block | write_step_string) ~ trail | write_step_heredoc) }

// ─── Action step ──────────────────────────────────────────────────────────────

action_step = { "$" ~ ws* ~ command }
// Captures the action body, across one or more physical lines. `\` is a
// line continuation marker only when it is the last character immediately
// before the line break (no trailing whitespace or comment after it); the
// marker and the line break are both consumed as part of `command`, and
// matching carries on into the next physical line. A `\` not immediately
// followed by a line break, or a line break not immediately preceded by
// `\`, is ordinary command text / the end of the command, respectively.
// Reportage does not interpret the continuation further — it only decides
// where the action body ends; the shell re-reads `\` + newline itself. Rust
// trims leading/trailing spaces and tabs (never newlines, so a preserved
// trailing marker-newline pair is never split back apart).
// See #80 / docs/adr/20260706T150000Z_action-line-continuation.md.
command     = @{ ("\\" ~ nl | (!nl ~ ANY))* }

// ─── Heredoc literal ────────────────────────────────────────────────────────
//
// A heredoc literal (the ``` ... ``` fenced block introduced by #67) is one
// of the two forms of a `text_literal` (see "Text literals" below), the
// other being an ordinary `quoted_string`. It is reusable wherever a
// `text_literal` is accepted: `write <"path"> <text_literal>`
// (write_step_heredoc) and `file <"path"> contains <text_literal>`
// (file_exp_heredoc). See docs/reference/semantics.md — Text literal and Write step.
//
// The opening fence's backtick run is pushed onto pest's match stack so the
// closing fence can be recognized dynamically: PEEK requires at least that
// many backticks (same character), and DROP clears the stack entry once the
// block is fully matched. Neither the opening nor closing fence line accepts
// an inline comment, unlike ordinary steps' `trail` — a heredoc literal
// always consumes its own trailing line ending itself, so any rule that
// embeds it must not also apply `trail` afterward (see write_step_heredoc /
// heredoc_expectation below).

opening_fence = @{ "`"{3,} }

// A body line is ordinary content; it must end in an actual newline, never
// EOI, so an unterminated heredoc literal cannot be silently accepted as an
// empty tail — the mandatory closing_fence_line after heredoc_body then
// fails to match, surfacing as a syntax error.
heredoc_body_line = _{ (!nl ~ ANY)* ~ nl }

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
heredoc_body = @{ (!closing_fence_line ~ heredoc_body_line)* }

heredoc_literal = {
    PUSH(opening_fence) ~ ws* ~ nl
    ~ heredoc_body
    ~ closing_fence_line ~ DROP
}

// ─── Write step ─────────────────────────────────────────────────────────────
//
// `write <"path"> <text_literal>` writes the text_literal's (dedented, in
// the heredoc case) content to a file in the concrete case workspace. The
// path is a workspace path literal and the content is a text_literal; both
// positions are parsed as the kind-agnostic `value_literal` so a wrong-kind
// literal is a semantic diagnostic, not a syntax error (see "Value literals"
// below). See docs/reference/semantics.md — Write step. Split into two grammar rules
// because the two text_literal forms have different line-ending rules (see
// case_step above and "Heredoc literal" above): write_step_string is an
// ordinary single-line construct; write_step_heredoc consumes its own
// trailing line.

write_step_string  = { "write" ~ ws+ ~ value_literal ~ ws+ ~ value_literal }
write_step_heredoc = { "write" ~ ws+ ~ value_literal ~ ws* ~ heredoc_literal }

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
//
// A heredoc literal (see "Heredoc literal" above) cannot fit inside
// single_assert's one-physical-line span, so `file ... contains
// <heredoc literal>` is only reachable via multi_assert, through the
// heredoc_assertion_line alternative below — never through single_assert.
// `not` / `all` / `any` blocks get this for free since they already reuse
// multi_assert. assertion_line is tried first (the common case, every other
// expectation and the quoted_string form of file contains) so an ordinary
// line never pays for a heredoc backtrack.
multi_assert              = { trail ~ comment_line* ~ assertion_or_heredoc_line ~ (comment_line | assertion_or_heredoc_line)* ~ ws* }
assertion_or_heredoc_line = _{ assertion_line | heredoc_assertion_line }
assertion_line            = _{ ws* ~ expectation ~ trail }
heredoc_assertion_line    = _{ ws* ~ heredoc_expectation }

// ─── Expectations ─────────────────────────────────────────────────────────────

expectation     = { exit_exp | stdout_exp | stderr_exp | file_exp | dir_exp | logical_composition }

exit_exp        = { "exit" ~ ws+ ~ exit_code }
exit_code       = @{ ASCII_DIGIT+ }

stdout_exp      = { "stdout" ~ ws+ ~ output_matcher }
stderr_exp      = { "stderr" ~ ws+ ~ output_matcher }

output_matcher  = { output_contains | output_contents_equals | output_text_equals | output_empty }
output_contains = { "contains" ~ ws+ ~ value_literal }
output_empty    = { "empty" }

// `stdout` / `stderr contents_equals @"<path>"` / `contents_equals <"path">`:
// byte-for-byte comparison against a `FileContentsReference` (a workspace
// path literal or a fixture reference literal), parsed as the kind-agnostic
// `value_literal` (see "Value literals" below). See #92 and
// docs/adr/20260706T170000Z_fixture-reference-value-syntax.md.
output_contents_equals = { "contents_equals" ~ ws+ ~ value_literal }

// `stdout text_equals "<text>"` / `stderr text_equals "<text>"`:
// byte-for-byte comparison of the captured stream's bytes against inline
// expected text (a `TextValue`), mirroring file_text_equals. This rule only
// wires the string-literal form; the heredoc-literal forms are
// stdout_text_equals_heredoc / stderr_text_equals_heredoc below.
// See docs/adr/20260710T100918Z_output-text-equals-evaluation.md.
output_text_equals = { "text_equals" ~ ws+ ~ value_literal }

// `file <"path"> exists` / `file <"path"> contains "<text>"`.
// Subject-first: `file <"path">` is the common subject, `exists` / `contains`
// are predicates on that subject. The subject is a workspace path literal,
// parsed as a kind-agnostic `value_literal` (see "Value literals" below). See
// docs/adr/20260704T112155Z_subject-first-file-assertion-syntax.md.
file_exp        = { "file" ~ ws+ ~ value_literal ~ ws+ ~ file_predicate }
file_predicate  = { file_contains | file_contents_equals | file_text_equals | file_exists }
file_exists     = { "exists" }
file_contains   = { "contains" ~ ws+ ~ value_literal }

// `file <"path"> contents_equals @"<path>"` / `contents_equals <"path">`:
// byte-for-byte comparison against a `FileContentsReference` (a workspace
// path literal or a fixture reference literal). `file <"path"> text_equals
// "<text>"`: byte-for-byte comparison against inline expected text (a
// `TextValue`); this rule only wires the string-literal form, mirroring
// `output_contains` — the heredoc-literal form is file_text_equals_heredoc
// below. Both positions parse the kind-agnostic `value_literal` (see "Value
// literals" below). See #88, #92, and
// docs/adr/20260706T170000Z_fixture-reference-value-syntax.md.
file_contents_equals = { "contents_equals" ~ ws+ ~ value_literal }
file_text_equals     = { "text_equals" ~ ws+ ~ value_literal }

// `file <"path"> contains <heredoc literal>` / `file <"path"> text_equals
// <heredoc literal>` / `stdout text_equals <heredoc literal>` / `stderr
// text_equals <heredoc literal>`: the heredoc-literal forms of file_contains,
// file_text_equals, and output_text_equals. Deliberately separate rules from
// file_exp/file_predicate and stdout_exp/stderr_exp, not variants folded into
// file_predicate/output_matcher, because they must not be followed by the
// generic `trail` the way every other expectation is (see
// heredoc_assertion_line above) — the heredoc literal already consumed its
// own trailing line. Reachable only through multi_assert. See #88.
heredoc_expectation        = { file_exp_heredoc | file_text_equals_heredoc | stdout_text_equals_heredoc | stderr_text_equals_heredoc }
file_exp_heredoc           = { "file" ~ ws+ ~ value_literal ~ ws+ ~ "contains" ~ ws+ ~ heredoc_literal }
file_text_equals_heredoc   = { "file" ~ ws+ ~ value_literal ~ ws+ ~ "text_equals" ~ ws+ ~ heredoc_literal }
stdout_text_equals_heredoc = { "stdout" ~ ws+ ~ "text_equals" ~ ws+ ~ heredoc_literal }
stderr_text_equals_heredoc = { "stderr" ~ ws+ ~ "text_equals" ~ ws+ ~ heredoc_literal }

// `dir <"path"> exists` / `dir <"path"> contains "<name>"`.
// Subject-first, mirroring file_exp: `dir <"path">` is the common subject,
// `exists` / `contains` are predicates on that subject. `dir` is scoped to
// directories only; `file` is scoped to regular files only (see file_exp
// above). See docs/adr/20260706T000000Z_subject-first-directory-assertion-syntax.md.
dir_exp         = { "dir" ~ ws+ ~ value_literal ~ ws+ ~ dir_predicate }
dir_predicate   = { dir_contains | dir_exists }
dir_exists      = { "exists" }
dir_contains    = { "contains" ~ ws+ ~ value_literal }

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

// ─── Value literals ───────────────────────────────────────────────────────────
//
// The three single-line literal kinds, each mapping to exactly one semantic
// domain regardless of context (see #93 and
// docs/adr/20260706T160000Z_workspace-path-literal-syntax.md):
//
//   "..."     string literal            — text domain (a TextValue form)
//   <"...">   workspace path literal    — case-workspace filesystem reference
//   @"..."    fixture reference literal — test-definition-side file reference,
//                                         valid only in a `FileContentsReference`
//                                         expected position (`contents_equals`;
//                                         see #92 and
//                                         docs/adr/20260706T170000Z_fixture-reference-value-syntax.md)
//
// Every step / expectation argument position parses the kind-agnostic
// `value_literal` union; which kind the position's signature actually
// requires is checked during parser construction. A wrong-kind literal
// (e.g. `file "out.txt" exists`) is therefore a parse-able semantic invalid
// case with an actionable diagnostic (semantic.literal.kind_mismatch) that
// names the expected kind, the actual kind, and the suggested replacement —
// never a bare syntax error.
//
// Both non-string kinds wrap an ordinary quoted_string, so all three kinds
// share the same escape rules (see "String literals" below). Workspace path
// validation (non-empty, relative, no `.` / `..` segments) applies to the
// unescaped value afterwards, on the WorkspacePath side; fixture reference
// literals apply the same lexical policy on the FixtureReference side (see
// `model::FixtureReference::parse`). No whitespace is permitted between a
// kind marker (`<`, `>`, `@`) and the quoted string it wraps.

workspace_path_literal    = { "<" ~ quoted_string ~ ">" }
fixture_reference_literal = { "@" ~ quoted_string }
value_literal             = { workspace_path_literal | fixture_reference_literal | quoted_string }

// ─── String literals ──────────────────────────────────────────────────────────

// v0 forbids raw newlines (LF, CRLF, and bare CR) inside string literals and
// allows exactly four escape sequences: \\, \", \n, \t. Any other backslash
// sequence is rejected. See [docs/adr/20260701T214658Z_string-literal-escape-sequences.md](../adr/20260701T214658Z_string-literal-escape-sequences.md).
//
// A quoted_string is one of the two forms of a `text_literal` (conceptually,
// text_literal = string literal | heredoc literal — see "Heredoc literal"
// above), the syntax category accepted by `write` and `file ... contains`.
// There is no single combined `text_literal` grammar rule: because the two
// forms have different line-ending rules (a heredoc literal self-terminates
// its own trailing line; a quoted_string relies on the surrounding `trail`),
// every position that accepts a text_literal is expressed as two ordered
// grammar alternatives instead (write_step_string / write_step_heredoc;
// file_contains / file_exp_heredoc). Both forms resolve to the same
// TextValue at the semantic level; see [docs/reference/semantics.md](semantics.md) — Text literal.
quoted_string = { "\"" ~ string_inner ~ "\"" }
string_inner  = @{ string_char* }
string_char   = _{ escape_seq | (!("\"" | "\\" | "\r" | "\n") ~ ANY) }
escape_seq    = _{ "\\" ~ ("\\" | "\"" | "n" | "t") }
```
