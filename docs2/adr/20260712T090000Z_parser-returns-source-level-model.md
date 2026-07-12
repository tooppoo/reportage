# Parser Returns a Source-Level Model Instead of the Execution Script

- Status: Accepted
- Created: 2026-07-12T09:00:00Z

## Context

The parser used to produce the execution model (`Script` / `Case` / `Step`) directly.
That model deliberately contains nothing but execution semantics:
no source text, no positions, and no room for metadata that describes the source rather than the run.

Upcoming documentation features need exactly that source-side information.
`document file` (#168) and `document case` (#169) attach documentation metadata to a source file and to individual cases,
and documentation rendering needs each case's original source text.
The execution model is the wrong home for this data:
executors, evaluators, and artifact writers must stay independent of how the source was written.

## Decision

The parser returns a source-level model, `SourceFile`, instead of `Script`.

`SourceFile` is a source-aware semantic model:
the semantically interpreted `Case` structure, associated with the original source text and each case's byte range within it.

```text
SourceText
    ↓ parse
CST / SyntaxTree (future, optional)
    ↓ lower
SourceFile
    ↓ projection
Script
    ↓ execute
ExecutionReport
```

The concrete decisions:

- `SourceFile` owns a copy of the parser input as `SourceText`.
  Borrowed or self-referential representations are rejected:
  owning the text keeps `SourceFile` self-contained after parsing, with no lifetime coupling to the caller's buffer,
  and the cost of one owned `String` per file is negligible for test-suite-sized inputs.
- Each case carries a `SourceSpan`, a byte range into the owning file's `SourceText`.
  Spans are constructed only by the parser and validated on `SourceFile` assembly
  (`start <= end`, within the text, UTF-8 character boundaries, source order, non-overlapping).
  Source extraction goes through `SourceText::slice` / `SourceFile::case_source`, never raw range indexing at call sites.
- A case span equals the pest `case_block` pair's matched byte range.
  The grammar is the single definition of where a case block starts and ends;
  the parser does not trim or extend the pair's span.
  Concretely, the span includes the `case` line's leading indentation,
  the closing brace line's trailing whitespace and inline comment,
  and the closing brace line's line ending when the source has one.
  It excludes blank lines and comment lines before and after the block.
- Projection to the execution model is the explicit, consuming `SourceFile::into_script`.
  Consuming avoids introducing `Clone` across the whole `Case` / `Step` tree for a projection nobody needs twice;
  a borrowed view or non-consuming projection can be added later if a consumer requires one.
  Source text, spans, and future documentation metadata are dropped at projection.
- The suite loader projects immediately after parsing, so `ValidatedFile` and everything downstream keep receiving `Script`.
  `executor`, `evaluator`, `result`, and `artifact` do not depend on the `source` module.
  Source paths remain a loader concern, not a parser concern.

`SourceFile` is not a lossless CST and is not the one complete representation of the source.
Whitespace, comments, and raw literal spellings are not structurally preserved,
and round-tripping back to the original source is not supported.
This keeps #167 small while still unblocking documentation features,
which need case-level extraction (covered by spans over owned text), not token-level fidelity.
When a formatter, source rewriting, or syntax-oriented linting needs lossless syntax,
a CST / syntax tree can be added in front of the parser and lowered into `SourceFile` without changing its role.

## Test Strategy

Representative source shapes (multiple cases, comments, heredocs, trailing inline comments) live in `examples/` and `e2e/` fixtures,
which the corpus-wide tests (`grammar_fixtures.rs`, `source_model.rs`) parse, span-check, and project.
Focused unit and integration tests cover internal contracts that fixtures cannot express naturally:
byte offsets in multibyte sources, CRLF line endings, a missing final newline, leading indentation,
closing-brace inline comments, and span ordering / non-overlap.
AST snapshots record each case's span alongside its structure, without duplicating fixture source text.

## Alternatives Considered

- Keeping `Script` as the parser output and adding documentation fields to it.
  Rejected: it leaks source concerns into execution, and every execution-side consumer would see fields it must ignore.
- Returning both a `Script` and a side table of spans.
  Rejected: two parallel structures with index-based correlation are easy to desynchronize;
  one model that owns both sides cannot drift.
- A lossless CST now.
  Rejected as premature: no current feature needs token-level fidelity, and the layered design keeps that door open.
- Borrowed spans (`&str` slices) into the caller's source buffer.
  Rejected: it forces a lifetime parameter through every downstream API or a self-referential struct.

## Consequences

### Positive Consequences

- Documentation metadata (#168, #169) has a natural home that execution code never sees.
- Each case's original source text is retrievable after parsing, from the `SourceFile` alone.
- The source-model / execution-model boundary is explicit in the module graph and crossed at exactly one point.
- A latent crash was fixed along the way:
  a case block whose closing brace ends the file without a final newline made the parser panic on the `EOI` pair
  that pest emits inside `case_block` in that position.
  Such sources now parse, with the span ending at end of input.

### Negative Consequences

- Every parsed file's source text is kept alive until projection, roughly doubling the parser's transient memory per file.
- Callers that only want a `Script` now write one extra `into_script()` call.

### Neutral Consequences

- AST snapshots gained a `span` field, so snapshot files changed once wholesale.
- Valid / invalid classification, diagnostic codes and locations, execution order, report schema, and artifacts are unchanged.
