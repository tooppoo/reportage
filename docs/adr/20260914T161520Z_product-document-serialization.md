# Product Document Serialization

- Status: Proposed
- Created: 2026-09-14T16:15:20Z

## Context

[ADR: Product Documentation Projection](20260908T131134Z_product-documentation-projection.md) fixed what a product-facing catalog holds: an ordered step sequence per example, and expectations as a subject with an operation on it.
It deliberately left how that is written down to the renderers, so that plain text and Markdown could phrase the same condition differently.

Writing it down is where a reader either understands the product or does not.
The decisions below are the ones a future contributor is most likely to question, and the ones a renderer cannot take back once published documentation depends on them.

## Decision

### Conditions are sentences, phrased from the subject and the operation together

A verified condition must be rendered as an English sentence built from the observed subject and the operation applied to it, never from the operation alone.
`file <"f"> contains "x"` reads as containment, and `dir <"d"> contains "x"` reads as a named entry, because only the pair disambiguates them.

The sentences must be identical across formats.
A shared phrasing layer builds them, and a format supplies only how a value is delimited — quoted in plain text, a code span in Markdown.
Two formats describing the same assertion differently would mean the documentation says two things about one verified fact.

No sentence may name a Reportage construct.
A reader is told what was verified, not which DSL expressed it.

### A logical composition is worded as the group it negates or joins

`not { A B }` is `not(all(A, B))`, never `not(A) and not(B)` (see [Language semantics](../reference/semantics.md) — Logical composition), so it must not be worded as "none of the following": that claims every child fails, which is a stronger condition than the one verified, and would make the document assert something the scenario never checked.

A `not` over several conditions is therefore worded as a negated group, and a `not` over exactly one is worded as a plain negation, because with one child the group *is* the child.
Wording that varies with arity is accepted here because the correct English varies with arity while the meaning does not.

### Conditions stay one line; file content keeps its lines

Expected text inside a condition must be reduced to one line, with `\`, newlines, and tabs escaped, because a list of verified conditions is read by scanning line starts and one multi-line value would break that for every condition after it.

File content must not be escaped and must keep its line structure, because it is a block a reader copies.

The two opposite rules apply to the same `DocumentedText` model, which is why the choice belongs here rather than in the model.

### A format makes its own delimiter unambiguous

Reducing a value to one line is shared; keeping it inside a quote or a code span is not.
Plain text escapes a `"` inside a quoted value, and Markdown sizes a code span past the longest backtick run in the value and pads it when the value's own edge is a backtick.

This matters for the inputs this projection targets: a CLI that quotes a file name in its output, or a configuration format that uses backticks, would otherwise close its own delimiter and corrupt the rest of the line.
The fence rule already protects block content; the inline rule is the same hazard one level down, and both live with the format that has it.

### A captured value is shown as its name

A value captured while the example runs has no text in the source, so it renders as `<name>` — the name of the value in the example — inside both conditions and file content.
Resolving it would require running the scenario, which documentation generation must not do.

### The default document title is projection-specific

Omitting `--title` must produce `Documentation` for `docs` and `Reportage Documentation` for `docs-reportage`.
A product's documentation headed `Reportage Documentation` puts reportage in front of the product on the document's most prominent line, which is the failure the product projection exists to avoid, and it would be the default every adopting project silently ships.

This is a deliberate exception to the rule that the presentation options keep the same contract on both subcommands ([ADR: Reportage-Source Documentation Is Its Own Subcommand](20260907T230710Z_reportage-source-documentation-subcommand.md)).
The option's meaning, validation, and verbatim treatment are unchanged; only the value used when the option is absent differs, and it differs because the absent value is itself a piece of content.

### The scenario's own path is not shown

Neither format shows the `.repor` display path.
It locates the scenario inside the product's repository, which is a detail for that product's maintainers, not for the readers this document is written for.
The Reportage-source projection keeps showing it, because there the scenario *is* the subject.

### Plain text: labelled blocks, with `Preparation` and `Steps` as separators

The plain format reuses the Reportage-source format's block shape — a label line, values indented two spaces, exactly one empty line between blocks — because those are properties of plain text rather than of either projection.
Its labels are `Group`, `Section`, `Description`, `Example`, `Preparation`, `Steps`, `File`, `Command`, and `Verified outcome`.

`Preparation` and `Steps` are label-only blocks that exist solely to bound the shared setup.
They must be emitted only when the example has preparation: with none, there is nothing to bound, and a lone `Steps` label would imply a section that was omitted.

A file's content is indented four spaces beneath its path, so a file step stays one block; an empty file is its path alone rather than a block with a blank line.

File content is reproduced verbatim inside that indentation, so a content line that carries trailing whitespace keeps it: trimming would corrupt the example a reader copies.
Renderer-generated lines add none, and a value's own trailing blank lines are dropped, because a block ending in a blank line cannot be told apart from the empty line that separates blocks.

### Markdown: the same navigation as the Reportage-source format

The Markdown format keeps the `## Contents` list, the explicit `<a id="...">` anchors, and the anchored-heading structure of the Reportage-source Markdown format, because `--format markdown` must produce the same *kind* of document whichever projection filled it.
Only the vocabulary changes: `group` / `section` / `example` instead of `group` / `file` / `case`, with anchor prefixes to match.

Preparation is labelled with bold paragraphs rather than headings, so it adds no anchor and no table-of-contents entry: it is shared setup, not a place a reader navigates to.
Heading levels and example anchor numbering are therefore identical with and without preparation.

A command is fenced with the `sh` info string and no prompt character, so the block can be copied and run as-is.
A file's content is fenced with no info string: the content is the product's own format, which reportage does not know and must not guess.

### Fences and slugs come from the shared Markdown primitives

Both Markdown formats compute anchor slugs, fence lengths, and code-span delimiters with the same code.
The slug normalization and the "one backtick longer than the longest run, at least three" rule are documented user-facing contracts, and a second implementation would let two documents claim to follow one rule while following two.

## Alternatives Considered

### Pre-render condition sentences in the catalog

Building the sentence when the catalog is built would remove the shared phrasing layer entirely.
Rejected in [ADR: Product Documentation Projection](20260908T131134Z_product-documentation-projection.md): it fixes wording for every format at once and puts presentation in the model.

### Let each format phrase conditions independently

Full independence would let Markdown exploit its own expressiveness.
Rejected: the freedom that matters is decoration, and the risk is two documents describing the same verified fact differently. The `ValueStyle` seam gives the first without the second.

### Wrap multi-line expected text instead of escaping it

Showing expected text as a block would preserve its shape.
Rejected: it breaks the one-condition-per-line property that makes the outcome list scannable, and the expected value of a `contains` is usually a fragment, not a document.

### Use `console` fences with a `$` prompt for commands

A prompt is the convention for shell transcripts.
Rejected: these blocks are meant to be copied, and a prompt character has to be deleted first. It would also put a `$` at the start of a command line in product documentation, which is exactly the character the Reportage DSL uses for an action.

### Give preparation its own heading level

A `#####` heading would make preparation navigable.
Rejected: it would add table-of-contents entries for setup shared by every example in a section, and would shift the heading hierarchy depending on whether a source declares setup.

## Consequences

### Positive Consequences

- The same verified condition reads identically in every format, while each format still decorates values idiomatically.
- A reader can copy a command block or a file block without editing it.
- Adding a format means implementing decoration and layout, not re-deriving what an assertion means.
- The Markdown document's navigation is the same whichever subcommand produced it.

### Negative Consequences

- Condition wording is now a user-facing contract fixed by snapshots; rephrasing a sentence is a visible documentation change for every downstream project.
- Escaped expected text is less readable than the original when the value is long.
- The plain format's `Preparation` / `Steps` labels are structure carried by convention rather than by indentation, so a consumer parsing the plain document has to know them.
- A file whose content ends with blank lines is documented without them, because the plain format's block separation has no way to express that.

### Neutral Consequences

- The `.repor` path is absent from product output, so a reader who wants to find the scenario behind an example must be told where scenarios live by other means.
- Expectation kinds the v0 grammar cannot produce still have phrasings, so the formats stay total over the expectation model.

## References

- Issue: [#257](https://github.com/tooppoo/reportage/issues/257)
- [ADR: Product Documentation Projection](20260908T131134Z_product-documentation-projection.md)
- [ADR: Reportage-Source Documentation Is Its Own Subcommand](20260907T230710Z_reportage-source-documentation-subcommand.md)
- [ADR: Markdown Documentation Format](20260723T143711Z_markdown-documentation-format.md)
- [Documentation generation reference](../reference/docs-generation.md)
