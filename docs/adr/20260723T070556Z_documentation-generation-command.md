
# Documentation Generation Command

- Status: Proposed
- Created: 2026-07-23T07:05:56Z

## Context

Issue #166 renamed the reference discovery command to `reportage references` and reserved the `docs` name for a documentation generation command (see [ADR: Rename `docs` Command to `references`](20260711T070008Z_rename-docs-command-to-references.md)).
Issue #167 made the parser return a source-level model that keeps source text, case spans, and `document file` / `document case` metadata (see [ADR: Parser Returns Source-Level Model](20260712T090000Z_parser-returns-source-level-model.md)), and issue #169 completed the `document case` association.

Issue #170 ships the first end-to-end slice of the real command:
`reportage docs '<pattern>'... --out-dir <dir>` aggregates documentation metadata and original case sources from glob-selected `.repor` files into a single plain text document.
This ADR records the boundary and contract decisions of that slice, because they constrain every later format (Markdown, HTML), layout (multi-file), and discovery extension, and are not recoverable from the code alone.

## Decision

### Separated stages: discovery, loading, Catalog, layout, output root

Generation is a pipeline of independently testable stages with explicit hand-offs:
glob discovery produces source identities, the loader produces unconsumed source-level models, the Catalog builder produces a renderer-ready model, the layout maps the Catalog to relative output paths, the format serializes, and `OutputDirectory` owns all output filesystem rules.
Each later extension point (a new format, a new layout, a new discovery mode) then changes exactly one stage.
`--out-dir` validation lives only in `OutputDirectory`, so adding formats or layouts cannot change output root rules;
layouts return only root-relative paths made of normal components (`single-file` + `plain` returns exactly `index.txt`), and the output writer rejects anything else, so nothing can be written outside the output root.

### Documentation discovery is separate from suite discovery

`reportage docs` has its own discovery policy instead of extending `suite::discover_files`:
the `.repor` extension requirement, symlink rejection (of the source file and of every path component from the working directory), per-pattern eligible-match validation, display path construction, and traversal error propagation are documentation-specific.
Normal execution and config-based suite discovery keep their existing acceptance and rejection rules unchanged.

Absolute patterns and patterns that can escape the working directory after lexical normalization are rejected in v0, keeping every input and every displayed path inside the working directory.

### Discovery and load errors never produce a partial document

An OS-level I/O error during glob traversal, a pattern with zero eligible matches, and any source read or parse error each fail the whole generation.
A documentation index that silently omits sources misrepresents the suite it documents — a typo in a pattern or an unreadable directory must surface as an error, not as a thinner document.
Load errors are collected across all sources and reported per display path in deterministic order, but one error is enough to skip Catalog construction and output writing entirely.

### Filesystem access path and display path are separate

Each selected source carries a `load_path` (used only to open the file) and a `display_path` following a fixed contract:
relative to the working directory, lexically normalized (`.` / `..` resolved), `/`-separated, UTF-8, compared with locale-independent case-sensitive `String` ordering.
The display path is the single identity used for deduplication, ordering, the Catalog's `source_path`, and every path in the generated document.
This keeps the generated document deterministic and platform-independent while filesystem access stays exact;
hard-link identity is deliberately not inspected, because normalized path identity is what a reader of the document can verify.
Non-UTF-8 source paths are rejected instead of being displayed lossily: a lossy path would name a file that does not exist.

### The Catalog is a renderer-ready model with a type boundary

The Documentation Catalog exposes only plain `String` values: it never leaks `SourceFile`, `SourceCase`, `SourceSpan`, or the source model's `DocumentationText`.
Renderers therefore cannot grow dependencies on parser internals, and the parser can evolve its source-level model without touching renderers.
The loader keeps the unconsumed `SourceFile` (never calling `SourceFile::into_script`), because the projection to the execution `Script` drops exactly the data documentation needs; the executor and evaluator are never invoked.

### Fallbacks live in the Catalog builder; the default group is `Index`

All display fallbacks — file stem as title, case name as case title, and the default group — are applied when the Catalog is built, never materialized on the source-level model.
The parser's model keeps stating only what the source states (its documented contract), and every renderer inherits identical fallback behavior from the single builder.
The default group name `Index` is a user-facing output contract, fixed by Catalog tests and generated-document snapshots: an undocumented suite still renders as a navigable index rather than an error or an empty group label.

### Zero-case sources are included

A valid source with no cases still appears with its file metadata and source path.
The generated document is an inventory of the selected sources; omitting case-less files would make "this file exists and is documented" unverifiable from the output.

### Exact source preservation in the Catalog, presentation in the renderer

`DocumentedCase.source` owns the byte-exact case span text: indentation, interior whitespace and comments, LF / CRLF, and final-newline presence are preserved; the Catalog never trims, dedents, or normalizes.
Renderers own presentation: the plain text renderer adds four-space indentation to non-empty source lines, normalizes the whole document to LF, and separates blocks with exactly one empty line, but never drops or replaces source characters beyond that.
Keeping the two concerns apart lets a future format make different presentation choices against the same preserved source.

### Existing-output-preserving replacement, without `fsync`

Output writing starts only after discovery, loading, Catalog construction, and rendering all succeeded; a failing generation never creates the output directory.
`index.txt` is replaced by writing the full document to a temporary file in the output directory and renaming it into place:
a failure before the rename leaves an existing `index.txt` unchanged, and a partially written document is never observable at the output path.
An existing directory or symlink at `index.txt`, and a regular file or symlink at `--out-dir`, are rejected rather than replaced.
The guarantee covers process-level failure; power-loss durability (`fsync` of file and directory) is explicitly out of scope in v0, matching the cost/benefit of a regenerable artifact.

### A dedicated tooling exit code table

`reportage docs` classifies failures as `0` success, `2` request / source validation, `3` filesystem / runtime infrastructure, `4` CLI usage — reusing the shared meanings of `2` / `3` / `4` established by the run and `shim scaffold` tables (see [Exit codes](../reference/exit-codes.md)).
Source read errors are `2`, matching the run command's treatment of selected sources that cannot be used as valid input; traversal and output I/O failures are `3` because retrying with different arguments cannot fix them.

### `--out-dir`, not `--output`

The option names a directory root that layouts populate, not an output file.
`--output` conventionally accepts a file path (and often `-` for stdout); with multi-file layouts planned, an option that must always be a directory avoids re-teaching the flag's meaning later, and the v0 requirement that the document never goes to stdout stays visible in the interface.

## Alternatives Considered

### Extend `suite::discover_files` with documentation options

Adding flags (extension filter, symlink policy, per-pattern validation) to the existing discovery would share code but couple two policies that must evolve independently, and every documentation policy change would risk regressing run discovery.
Rejected: the run pipeline's acceptance rules are a stable contract; sharing the `glob` crate is enough reuse.

### Build the Catalog from `ValidatedFile { script }`

Reusing the run pipeline's load path would consume the source-level model via `into_script`, losing spans, source text, and documentation metadata — exactly the data this feature exists to surface.
Rejected; the loader keeps the unconsumed `SourceFile` instead.

### Render directly from the source-level model

Skipping the Catalog would save a model, but every renderer would re-implement fallbacks and ordering, and parser model changes would ripple into all formats.
Rejected: the Catalog is the compatibility boundary that keeps formats cheap.

### Write `index.txt` in place

Truncating and rewriting the existing file is simpler but leaves a torn document behind on any mid-write failure.
Rejected in favor of temp-file-plus-rename; full durability (`fsync`) was also considered and deferred as out of proportion for a regenerable artifact.

### A single generic failure exit code

Collapsing all failures into one code (or reusing `1`) would make CI unable to distinguish "fix your pattern / source" from "the filesystem failed".
Rejected: the repository's exit code policy requires distinguishable categories, and the `2` / `3` / `4` meanings already exist.

## Consequences

### Positive Consequences

- Formats and layouts can be added without touching discovery, loading, Catalog semantics, or output root rules.
- The generated document is deterministic across platforms, glob expansion order, and filesystem enumeration order.
- Renderers are free of parser types, so source-level model changes stay local.
- CI can branch on exit codes: usage (4) vs input (2) vs infrastructure (3).
- A failed generation never corrupts or partially updates existing output.

### Negative Consequences

- Discovery logic is intentionally duplicated in spirit (glob expansion appears in both suite and docs discovery), at the cost of a second policy to maintain.
- The Catalog copies strings out of the source-level model, an extra allocation per file and case.
- Symlinked sources and non-UTF-8 paths are hard errors or exclusions in v0, which may require restructuring for repositories that rely on them.

### Neutral Consequences

- `DocumentedCase.source` preservation makes Catalog equality byte-sensitive; snapshot updates must be deliberate.
- The `Index` default group and the plain text block layout are frozen as user-facing contracts; changing them is a breaking output change.
- `--check`, Markdown/HTML, multi-file layouts, and config-driven discovery remain open, constrained only by the stage boundaries above.

## References

- Issue: [#170](https://github.com/tooppoo/reportage/issues/170)
- [ADR: Rename `docs` Command to `references`](20260711T070008Z_rename-docs-command-to-references.md)
- [ADR: Parser Returns Source-Level Model](20260712T090000Z_parser-returns-source-level-model.md)
- [Documentation generation reference](../reference/docs-generation.md)
- [Exit codes reference](../reference/exit-codes.md)
