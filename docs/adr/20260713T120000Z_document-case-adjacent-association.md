# Case Documentation via an Adjacent `document case` Block

- Status: Accepted
- Created: 2026-07-13T12:00:00Z

## Context

#168 introduced `document <scope> { ... }` as first-class documentation syntax
and shipped its first scope, `document file`, together with the shared body
machinery: the block envelope, the documentation literal parser, empty-block
and duplicate-field validation, unknown-field rejection, and the literal kind
mismatch policy. Its ADR
([Document Block as First-Class Documentation Syntax](20260712T120000Z_document-block-first-class-documentation-syntax.md))
records why documentation is syntax rather than comment convention, why the
body is a grammar-level whitelist, and why the source-level model holds only
explicit values.

#169 (this decision) adds the `case` scope: documentation metadata for an
individual case. That raises questions #168 did not have to answer — where
the block lives relative to its case, how it is associated with the case, how
much of #168's field set the new scope shares, and what the placement
violations report. This ADR records those decisions.

## Decision

### `document case` is a prefix block at top level, not a statement in the case body

````reportage
document case {
  title "File creation"

  description ```
  Verifies that the command creates the file.
  ```
}

case "file exists" {
  $ touch test.txt

  assert {
    file <"test.txt"> exists
  }
}
````

A `document case` block placed inside the case body would sit in the middle of
the case's executable steps, forcing every consumer of the body — the step
parser, the executor, span-based tooling — to know about and skip a
non-executable construct, and inviting the misreading that documentation is a
step with a position in the execution sequence. As a top-level prefix block,
documentation stays out of the case body entirely: the case body grammar is
untouched, a case's steps remain exactly its executable content, and the
document block reads the way documentation conventionally does — immediately
above the thing it documents, like a doc comment.

### Association is by adjacency

A `document case` block attaches to the next top-level case. Blank lines and
ordinary comment lines may separate the two (they are discarded at parse time
and carry no structure); any other top-level item may not. The alternative —
naming the target case in the block (`document case "file exists" { ... }`) —
was rejected: it duplicates the case name, so renaming a case silently orphans
its documentation or, worse, re-attaches it to a different case, and it allows
documentation to sit arbitrarily far from what it documents. Adjacency keeps
the pair visually and structurally inseparable and makes the association
resilient to renames.

The parser manages the block as a pending association: the parsed
documentation waits until the next `case_block` arrives and is attached to it.
At most one `document case` may be pending at a time, and a pending block must
be resolved by a case before the source ends.

### The canonical top-level form is `document file? (document case? case)*`

Ignoring blank lines and comment lines, a source's top-level items follow:

```text
document file? (document case? case)*
```

`document file` describes the whole file, so it comes before everything it
describes — including every `document case`. A `document file` after a
pending `document case` is rejected with the existing placement code
`parse.document_file.after_case`, whose contract generalizes from "before the
first case" to "before all case-related items" without changing any
previously rejected source's outcome. The grammar itself accepts document
blocks anywhere at top level, any number of times; the canonical form is
enforced during parser construction so each violation gets an actionable
diagnostic instead of a bare pest error, following #168's precedent.

### Each scope has its own grammar-level field whitelist

`document case` accepts `title` and `description` only. `group` and `order`
are file-scope concerns — cases render in source order and group under their
file — so the case scope must reject them. Extending `document_scope` with
`case` while sharing one field-line rule would have accepted `group` / `order`
grammatically and forced a parser-construction rejection with a new
diagnostic. Instead, each scope is its own block rule with its own field-line
whitelist (`document_file_field_line` / `document_case_field_line`):
out-of-scope fields are unreachable in the grammar and fail as plain syntax
errors, exactly like unknown fields, and exactly as #168 treats execution
constructs inside a document block. The field rules themselves
(`document_title_field`, the two `description` forms) are shared between the
scopes, so the literal-kind policy and the `parse.document_block.*` body codes
stay identical across scopes.

### Orphan and duplicate association violations; duplicate wins

Two association violations get scope-specific stable codes, mirroring the
`parse.document_file.*` split from #168:

- `parse.document_case.duplicate` — a second `document case` appears while one
  is already pending. Reported at the second block's start line: the second
  block is the one that cannot be satisfied, and pointing at it tells the
  author which block to merge or move.
- `parse.document_case.orphan` — the source ends (or only blank lines /
  comments follow) with a block still pending. Reported at the unassociated
  block's start line.

When one structure violates both — two blocks and no case at all — the
duplicate is reported, not the orphan. The duplicate is detectable the moment
the second block appears, while the orphan is only knowable at end of input;
reporting the earlier, more local violation gives the author the first thing
to fix, and fixing it may make the orphan moot.

### Case documentation stays out of the execution model and the case span

`CaseDocumentation` lives on `SourceCase`, next to — not inside — the
execution `Case`:

```rust
pub struct SourceCase {
    documentation: Option<CaseDocumentation>,
    case: Case,
    span: SourceSpan,
}
```

`SourceFile::into_script` drops it, so execution behavior, execution order,
reports, and result artifacts are byte-identical with or without
documentation, extending #168's guarantee to the case scope. The case span
contract from #167 is unchanged: a span is exactly the pest `case_block`
pair's range, so the `document case` block and the blank / comment lines
between it and the case are never part of the span. `SourceCase` exposes the
documentation through a read-only accessor (`documentation()`); its fields
stay private so external code cannot re-pair a span or documentation with a
different case.

Following #168's model policy, `CaseDocumentation` holds only what the source
states: an omitted `title` stays `None`, and the display fallback to the
execution case name is applied when the Documentation Catalog is built (#170),
where both the documentation and `Case::name` are available. `None` for the
whole documentation means exactly "no `document case` block in the source" —
distinct from "fallback not yet applied".

## Test Strategy

Representative usage — a documented case, an undocumented case, blank lines
and comments between the block and its case, string and heredoc descriptions,
partial documentation across multiple cases, and coexistence with
`document file` in canonical order — lives in `examples/document-case.repor`,
`e2e/documentation/document-case.repor`, and `tests/fixtures/syntax/valid/`
(with AST snapshots recording the parsed metadata per case). The e2e fixture
locks the behavioral contract: identical execution with documentation present,
no metadata leakage into the JSON report, unchanged failure classification,
and the orphan / duplicate codes through the CLI. Focused parser unit tests
and `tests/fixtures/syntax/invalid/` fixtures cover the scope whitelist
(`group` / `order` rejection), body validation, orphan / duplicate placement,
the duplicate-over-orphan precedence, the `document file`–after–pending
conflict, and span exclusion.

`tests/fixtures/syntax/invalid/document_case_scope_not_supported.repor`
asserted that `document case` is a syntax error; its premise is gone, so the
fixture is deleted rather than repurposed. This is the only intentional change
to the existing fixture corpus.

## Alternatives Considered

### Documentation inside the case body

Rejected: it interleaves a non-executable construct with executable steps,
complicates every consumer of the case body, and suggests documentation has a
position in the execution sequence. See Decision.

### Naming the target case instead of adjacency

Rejected: duplicating the case name makes renames silently orphan or misbind
documentation, and lets documentation drift arbitrarily far from its case.
See Decision.

### One shared field grammar for all scopes, with parser-side scope checks

Rejected: `group` / `order` in a `document case` would parse and then need a
parser-construction rejection with a new diagnostic code, treating an
out-of-scope field as more valid than an unknown one. The grammar-level split
rejects both identically, and new scopes state their whitelist explicitly.

### Reporting the orphan before the duplicate

Rejected: the orphan is only knowable at end of input, while the duplicate is
local and actionable the moment the second block appears. See Decision.

## Consequences

### Positive Consequences

- Cases get documentation metadata with the same rigor as files: grammar,
  validation, stable codes, and guaranteed execution neutrality.
- Adjacency keeps documentation next to its case and rename-safe.
- The scope-specific whitelists make out-of-scope and unknown fields
  indistinguishable failures, and give future scopes an explicit template.
- The canonical top-level form is simple to state and to check.

### Negative Consequences

- A new document scope requires a new block rule and field-line rule in the
  grammar, not just a new keyword in `document_scope`.
- Adjacency admits no documentation for a case that is far from it; authors
  who want centralized documentation must keep it in the file scope.

### Neutral Consequences

- `SourceCase` gained an optional documentation field; AST snapshots serialize
  it only for documented cases, so pre-existing snapshots are unchanged.
- The `document_scope` grammar rule from #168 is gone; scope keywords now live
  in the per-scope block rules.
