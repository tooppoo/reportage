# Document Block as First-Class Documentation Syntax

- Status: Accepted
- Created: 2026-07-12T12:00:00Z

## Context

Reportage sources double as executable tests and as usage documentation.
Turning them into browsable documentation (#170) needs metadata that describes a source —
a display title, a grouping key, an ordering hint, a prose description —
which today has no home in the language:
`#` comments are discarded at parse time and never reach any model,
and the execution model deliberately contains nothing but execution semantics.

#167 introduced the source-level model (`SourceFile`) precisely so that
source-side data like this has somewhere to live without touching execution.
#168 (this decision) adds the syntax that produces that data for the file scope,
and the model that holds it.

This ADR records three decisions the issue requires justification for:
why documentation is first-class syntax rather than convention over comments,
why the document block body is a grammar-level whitelist,
and why the source-level model holds only explicit values while display fallbacks are deferred to the Documentation Catalog (#170).

## Decision

### Documentation is first-class syntax, distinct from comments

`document <scope> { ... }` is a dedicated top-level construct:

```reportage
document file {
  title "File assertions"
  group "Filesystem"
  order 20

  description ```
  Collected examples of assertions against files.
  ```
}
```

Comments and documentation serve different consumers and get different guarantees:

- `#` comments are for the human reading the source in place.
  They are discarded at parse time, carry no structure, and tooling must never depend on them.
- Documentation metadata is for tooling that renders the source elsewhere.
  It must survive parsing as structured, validated data.

Encoding metadata in comment conventions (`# title: ...`) would make parse-time-discarded text
load-bearing for tooling, with no validation (a typo silently drops a field),
no stable grammar, and no way to reject invalid shapes.
First-class syntax gives documentation the same rigor as every other construct:
a grammar, field validation, and stable diagnostics.

v0 supports only the `file` scope; `document case` is #169.
The scope keyword is part of the grammar (`document_scope = { "file" }`),
so `document case` today is a plain syntax error, not a semantic rejection —
adding the scope later is purely additive.

### The document block body is a grammar-level field whitelist

The body accepts exactly the documentation fields `title`, `group`, `order`, and `description`.
Actions, assertions, `write` steps, case blocks, nested document blocks —
and any construct added to the language in the future —
are not grammar alternatives inside the block,
so they are rejected at parse time by construction.
The alternative, blacklisting known execution constructs inside the block,
would silently accept every construct added after the blacklist was written.
An unknown field name likewise fails to parse:
unknown fields are rejected, never ignored, so a typo cannot silently drop metadata.

Field value positions follow the language's existing literal conventions:
`title` / `group` / `description` parse the kind-agnostic `value_literal`,
so a wrong-kind literal is the established `semantic.literal.kind_mismatch` diagnostic with a suggestion,
and `order` is a bare non-negative integer mirroring `exit`.

Rules the grammar deliberately leaves open are enforced during parser construction,
so each violation gets a fine-grained, actionable code instead of a bare pest error,
following the `empty_composition_body` precedent:

- `parse.document_block.empty` — a block with zero fields (a comment-only body counts as empty);
- `parse.document_block.duplicate_field` — the same field declared twice;
- `parse.document_block.invalid_order` — an `order` digit run overflowing u64;
- `parse.document_file.duplicate` — more than one `document file` per source;
- `parse.document_file.after_case` — a `document file` after the first case.

The codes are split into `parse.document_block.*` for body rules every document scope will share,
and `parse.document_file.*` for placement rules specific to the file scope,
so #169 can reuse the former unchanged.
They live in the `parse.*` namespace because they are purely structural parse-domain validation:
unlike the `semantic.workspace_path.*` family, no part of this validation is shared with the evaluator.

### The source-level model holds explicit values only; display fallbacks belong to the Catalog

`FileDocumentation` keeps every field optional and stores exactly what the source states:

```rust
pub struct FileDocumentation {
    pub title: Option<String>,
    pub group: Option<String>,
    pub order: Option<u64>,
    pub description: Option<DocumentationText>,
}
```

Display fallbacks — file stem as title, a default group, deterministic path-based order —
are not materialized here, for two reasons.
First, the fallbacks need inputs the parser does not have and should not grow:
the source path is a loader concern, and cross-source ordering is a collection-level concern.
Second, materializing fallbacks erases the explicit/implicit distinction:
a Catalog builder (#170) that sees `title: None` can decide how to present an untitled file,
but one that sees a synthesized title can no longer tell it from an authored one.
The Documentation Catalog is the first place where the source path, the source-level model,
and the full set of sources are all available, so fallback policy lives there.

`DocumentationText` is a distinct type from the execution-side `TextLiteral` / `TextValue`
because the two answer to different rules:
execution text participates in assertion comparison and file writes,
documentation text is display-only plain text (Markdown interpretation is out of scope for v0).
The source-side literal form (string vs. heredoc) is not preserved; both resolve to the same plain text.

### Relationship to execution and to case spans

`SourceFile::into_script` drops documentation metadata,
so the execution `Script`, execution reports, and result artifacts are byte-identical
whether or not a source declares a `document file` block.
The case span contract from #167 is unchanged:
a span is exactly the pest `case_block` pair's range,
so the document block and any blank / comment lines between it and the first case
are never part of a case span.

## Test Strategy

Representative usage — a documented source, a heredoc description,
blank lines and comments between the block and the first case,
and the undocumented status quo — lives in `examples/` and `e2e/` fixtures
plus `tests/fixtures/syntax/valid/` (with AST snapshots recording the parsed metadata).
`e2e/documentation/document-file.repor` locks the behavioral contract:
identical execution and report output with documentation present,
no metadata leakage into the JSON report,
unchanged failure classification, and the fine-grained rejection codes through the CLI.
Focused parser unit tests and `tests/fixtures/syntax/invalid/` fixtures cover
field validation, placement rules, the whitelist rejections, and span exclusion.

## Alternatives Considered

### Structured comment conventions (`# title: ...`)

No grammar, no validation, and parse-time-discarded text becomes load-bearing for tooling.
A typo in a field name silently drops metadata instead of failing.
Rejected in favor of first-class syntax.

### Blacklisting execution constructs inside the document block

Rejecting `$`, `assert`, `write`, and `case` as explicit alternatives would work today,
but every future step or statement would need to remember to extend the blacklist.
A field whitelist inverts the default: new constructs are invalid in documentation automatically.

### Enforcing placement rules in the grammar

A `script` rule of the form `SOI ~ document_block? ~ case_block*` would reject
a duplicate or misplaced `document file` as a generic `parse.syntax` error at whatever
token happened to fail.
Accepting document blocks anywhere at top level grammar-wise and validating placement
during parser construction yields the actionable `parse.document_file.duplicate` /
`.after_case` codes instead, following the `empty_composition_body` precedent.

### Materializing display fallbacks in the source-level model

Rejected: the parser lacks the inputs (source path, sibling sources),
and synthesized values would be indistinguishable from authored ones downstream.
See the Decision section.

### Arbitrary key–value metadata

An open `key value` body would defer all field validation to consumers and
make typos valid metadata. v0 fixes the field set; custom fields remain out of scope.

## Consequences

### Positive Consequences

- Documentation tooling gets structured, validated, parse-surviving metadata.
- Unknown fields, duplicates, and misplaced blocks fail fast with stable, fine-grained codes.
- Future language constructs are automatically invalid inside document blocks.
- Execution behavior, reports, and artifacts are provably unaffected by documentation.
- #169 (`document case`) can reuse the block grammar, the field parser, and the `parse.document_block.*` codes.

### Negative Consequences

- The field set is closed: any new documentation field is a grammar and parser change.
- Documentation authors must learn a construct that looks like, but is not, a comment.

### Neutral Consequences

- `SourceFile` gained an `Option<FileDocumentation>` field; AST snapshots serialize it
  only for sources that declare a block, so pre-existing snapshots are unchanged.
- A documentation-only source (a `document file` block and zero cases) parses and runs
  as a no-op, consistent with other zero-case sources.
