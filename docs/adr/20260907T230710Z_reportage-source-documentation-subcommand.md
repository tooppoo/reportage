# Reportage-Source Documentation Is Its Own Subcommand

- Status: Proposed
- Created: 2026-09-07T23:07:10Z

## Context

`reportage docs` (see [ADR: Documentation Generation Command](20260723T070556Z_documentation-generation-command.md) and [ADR: Markdown Documentation Format](20260723T143711Z_markdown-documentation-format.md)) publishes each selected case as its original `.repor` source, wrapped in the `document file` / `document case` metadata around it.

That projection documents reportage itself well: a reader of reportage's own documentation wants to see the DSL.
It serves a product that merely *uses* reportage badly.
In such a project the `.repor` sources describe the product — the files a user writes, the commands they run, the outcomes those commands are verified to produce — and publishing them as Reportage DSL puts reportage in front of the product it is supposed to document.

Issue [#257](https://github.com/tooppoo/reportage/issues/257) resolves this by giving reportage two documentation generators: a product-facing one and a Reportage-source one.
This ADR records how that split is expressed in the CLI, and why the existing generation moves to a new name instead of becoming a mode of the old one.
It does not record the product projection's model; that decision is recorded in [ADR: Product Documentation Projection](20260908T131134Z_product-documentation-projection.md).

## Decision

The Reportage-source projection must be a subcommand of its own, `reportage docs-reportage`, and must not be a mode of `reportage docs`.

`reportage docs-reportage` is the generator for reportage's own development and documentation.
It keeps the source-oriented contract `reportage docs` established: exact `case` and `before_each` source preservation, the plain and Markdown serializations, safe code fences, and the displayed source path.
Reportage's own generated documentation must be produced by it.

`reportage docs` is the product-facing generator.
It must not expose Reportage DSL, and its user-facing behavior change is a breaking change accepted deliberately: documenting a product that uses reportage is the common case, and the common case must own the plain name.

The two projections must not be selected by an option such as `--view product|reportage-source` or a `--reportage-source` flag.
A subcommand names a responsibility, and these are different responsibilities with different audiences, different intermediate models, and different output vocabularies; under a flag, the meaning of every rendered block would change with the flag while the interface claimed the command was the same.
Separate subcommands also let each help text state a single purpose.

Options that are independent of the projection — `--format`, `--layout`, `--title`, `--index-file-name`, the source pattern rules, and the output directory policy — must keep the same contract on both subcommands, so that changing the audience of a documentation build never means re-learning its invocation.

The migration is staged: `docs-reportage` is introduced first with the existing generation behind it, while `docs` keeps producing the same document until the product-facing generation replaces it.
During that interval the two subcommands generate identical output.
This is a transitional state, not a supported equivalence: nothing may be built on `docs` and `docs-reportage` agreeing.

> **Update (2026-09-14, issue [#257](https://github.com/tooppoo/reportage/issues/257)):** the staged migration is complete. `docs` now generates the product-facing projection described in [ADR: Product Document Serialization](20260914T161520Z_product-document-serialization.md), so the two subcommands no longer produce identical output.

## Alternatives Considered

### A `--view` / `--reportage-source` option on `docs`

One subcommand with a projection option keeps a single entry point and a single help page.
Rejected: the option would change what every block in the output means, not how it is presented, which is the kind of difference a subcommand boundary exists to make visible.
It would also force one help text to describe two audiences, and would invite generalizing the two intermediate models into one just to share the flag's plumbing.

### Keep the source projection on `docs` and give the product one a new name

This avoids a breaking change for current `docs` users.
Rejected: reportage's own documentation is one repository, while product documentation is every project that adopts reportage; leaving the plain name on the rarer, tool-internal use would misdirect users indefinitely to avoid a one-time migration.

### Generate both projections in a single invocation

A single run could write both documents.
Rejected: the two have different audiences and different output destinations, and nothing in the pipeline requires producing them together; a caller that wants both can invoke both.

## Consequences

### Positive Consequences

- Each subcommand's help, reference documentation, and tests describe exactly one audience.
- Reportage's own generated documentation keeps its exact source-preservation contract, pinned to the subcommand that owns it.
- The shared invocation surface stays a documented decision rather than an accident of one command implementing both.

### Negative Consequences

- `reportage docs` changes behavior for existing users; a project that wants the previous output must switch to `docs-reportage`.
- Two subcommands must keep the same presentation option contract, which is a coordination cost whenever either gains an option.

### Neutral Consequences

- Until the product-facing cutover, `docs` and `docs-reportage` produce identical documents, so the transitional state is testable but must not be relied on. (That interval has since ended; see the update note above.)
- The internal boundary between the two projections is left to [ADR: Product Documentation Projection](20260908T131134Z_product-documentation-projection.md); this decision constrains only the CLI surface.

## References

- Issue: [#257](https://github.com/tooppoo/reportage/issues/257)
- [ADR: Documentation Generation Command](20260723T070556Z_documentation-generation-command.md)
- [ADR: Markdown Documentation Format](20260723T143711Z_markdown-documentation-format.md)
- [ADR: Product Documentation Projection](20260908T131134Z_product-documentation-projection.md)
- [Documentation generation reference](../reference/docs-generation.md)
