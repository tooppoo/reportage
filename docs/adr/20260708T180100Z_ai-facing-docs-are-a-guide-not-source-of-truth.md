
# AI-Facing Documentation Is a Guide, Not a Source of Truth

- Status: Proposed
- Created: 2026-07-08T18:01:00Z

## Context

Getting an AI to use reportage correctly requires documentation the AI can read quickly. But if AI-facing documentation duplicates detailed specification content, it risks diverging from the generated syntax docs, semantics docs, diagnostics docs, and JSON Schema that already serve as sources of truth.

There is a related risk: if an AI cannot distinguish already-implemented syntax from TBD or future syntax, it may generate `.repor` files that use syntax that does not exist yet, degrading generation quality.

This is the second of three ADRs recorded from issue #136. It follows [Tag-Based GitHub URLs and `reportage docs` as the v0 AI Documentation Discovery Path](20260708T180000Z_ai-documentation-discovery-core-path.md), which establishes `reportage docs` as the core discovery path, and records how the content reached through that path should be scoped.

Related issues: #136, #137, #142.

## Decision

AI-facing documentation must not act as the source of truth for detailed specification content. Instead, it must act as a guide that gives an AI:

- a recommended reading order;
- `.repor` generation rules;
- explicit prohibitions;
- common mistakes to avoid;
- a validation procedure;
- links into the authoritative documents.

The source of truth remains:

- generated syntax docs;
- semantics docs;
- diagnostics docs;
- the JSON Schema;
- valid/invalid examples.

AI-facing documentation must not present TBD or future syntax as ordinary, currently usable syntax.

The following command is treated as the primary post-generation validation step referenced from AI-facing documentation:

```sh
reportage check <file> --format=json
```

`reportage check` does not exist yet as of this ADR. This decision reserves its name and its `--format=json` output as the first-class validation entrypoint that AI-facing documentation should reference once it ships. Its own scope and behavior are defined by whichever issue introduces it; see issue #142's acceptance criteria for the currently expected usage.

## Alternatives Considered

### Let AI-facing documentation restate the full specification

Rejected. Duplicating specification detail in AI-facing documentation creates a second source of truth that can silently drift from the generated syntax/semantics/diagnostics docs and the JSON Schema.

### Allow AI-facing documentation to describe TBD/future syntax as usable

Rejected. Presenting unimplemented syntax as usable would directly cause AI-generated `.repor` files to reference syntax that reportage cannot yet execute.

## Consequences

### Positive Consequences

- The minimal information an AI needs can be presented concisely.
- The specification stays concentrated in a small number of authoritative sources.
- The risk of an AI fabricating unimplemented syntax is reduced.
- AI-facing documentation composes naturally with the document ordering returned by `reportage docs`.

### Negative Consequences

- AI-facing documentation alone does not fully explain detailed specification questions.
- Links from AI-facing documentation into generated docs, semantics, diagnostics, and the schema must be kept up to date.
- Broken-link checking between AI-facing documentation and authoritative docs may become necessary.

## References

- [#136](https://github.com/tooppoo/reportage/issues/136)
- [#137](https://github.com/tooppoo/reportage/issues/137)
- [#142](https://github.com/tooppoo/reportage/issues/142)
