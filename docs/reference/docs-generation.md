# Documentation Generation

This document defines the `reportage docs` subcommand: how sources are selected, how the generated document is derived, the plain text and Markdown serialization contracts, and the output replacement guarantees.
Exit codes are defined in [Exit codes](exit-codes.md) — `docs` exit codes.
The rationale for the command's internal boundaries is recorded in [ADR: Documentation Generation Command](../adr/20260723T070556Z_documentation-generation-command.md); the Markdown format's serialization decisions are recorded in [ADR: Markdown Documentation Format](../adr/20260723T143711Z_markdown-documentation-format.md).

## Synopsis

```console
reportage docs '<pattern>'... --out-dir <directory> [--format plain|markdown] [--layout single-file] [--title <string>] [--index-file-name <name>]
```

Example:

```console
reportage docs 'examples/**/*.repor' --out-dir generated/docs --format markdown --title 'Project documentation'
```

`reportage docs` aggregates the `document file` / `document case` metadata and the original case source of every selected `.repor` file into generated documentation under `--out-dir`.

- `--out-dir` is required.
- `--format` defaults to `plain`; `plain` and `markdown` are the supported formats.
- `--layout` defaults to `single-file`; `single-file` is the only v0 layout.
- `--title` sets the document title for every format; it defaults to `Reportage Documentation`.
- `--index-file-name` sets the generated index document's name; it defaults to `index` with the extension chosen by `--format`. See [Index file name](#index-file-name).
- The generated document is written under `--out-dir`, never to stdout.
- On success, each written file is reported on stdout as `generated: <path>`.

Sources are parsed but never executed: no case action runs, no assertion is evaluated, and no `.reportage/` artifact is written.

## Input patterns

Each positional argument is a glob pattern resolved relative to the current working directory.
A literal relative file path is the degenerate pattern without glob metacharacters, so the command works whether or not the shell pre-expanded the pattern.
Quote patterns to let reportage expand them itself.

v0 supports at least `*`, `?`, `[...]`, and `**`.

Pattern restrictions:

- Absolute patterns are rejected.
- Patterns containing `..` are allowed only while they stay inside the current working directory after lexical normalization; patterns that can escape it are rejected.

Eligibility and resolution:

- Only regular files with the `.repor` extension are eligible sources.
- A source is not eligible when the file itself, or any path component between the working directory and the file, is a symlink.
- Directories are never recursed into implicitly; use an explicit `**/*.repor`.
- Every pattern must match at least one eligible source; a pattern matching nothing — or matching only non-`.repor` files, directories, or symlinks — is an error, so a typo never silently drops sources.
- An OS-level I/O error during glob traversal fails the whole generation instead of producing a partial document.
- Sources selected more than once (by several patterns, or through different lexical routes) are documented once, deduplicated on the normalized display path below.
- Any unreadable or unparsable source fails the whole generation; errors are reported per source, ordered by display path.

## Source paths in the generated document

Every path shown in the generated document is a display path: relative to the working directory, lexically normalized (no `.` or `..`), `/`-separated, UTF-8.
The same display path is the identity used for deduplication and ordering.
A source path that cannot be represented as UTF-8 is rejected; it is never displayed lossily.

## Document structure

The generated document groups files, then lists each file's cases:

- file title: `document file.title`, or the source file stem when unspecified.
- file group: `document file.group`, or the fixed default group `Index` when unspecified.
- file description: `document file.description`, omitted when unspecified.
- case title: `document case.title`, or the case name when unspecified.
- case description: `document case.description`, omitted when unspecified.
- case source: the exact `case` block text from the source file.

A valid source with zero cases still appears with its file metadata and source path, without case sections.

Ordering is deterministic:

1. groups by ascending name,
2. within a group, files with a `document file.order` before files without one,
3. ordered files by ascending `order`,
4. ties — equal `order`, or both unspecified — by ascending source path,
5. cases in source order.

String ordering is locale-independent and case-sensitive (byte-wise `String` comparison), for group names and source paths alike.

## Document title

`--title <string>` sets the document title, shared by every format:

- Unspecified, the title is `Reportage Documentation`.
- `plain` uses it as the first line of the document; `markdown` uses it as the level 1 heading.
- The value is used verbatim: empty strings, newlines, and Markdown syntax are neither rejected nor trimmed nor escaped (see [Input text policy](#input-text-policy)).
- The title is a render option of the invocation, not a Catalog property: it never affects Catalog ordering, fallbacks, or anchor IDs.

## Index file name

`--index-file-name <name>` sets the name of the generated index document under `--out-dir`:

- Unspecified, the name is `index` and the extension follows `--format`, so the single-file layout writes `index.txt` for `plain` and `index.md` for `markdown`.
- Specified, the value is used verbatim, extension included: it is neither given the format's extension nor otherwise rewritten, so `--index-file-name readme.md` writes `readme.md`, and `--index-file-name overview` writes an extension-less `overview`.
- The value must be a single file name placed directly under `--out-dir`: an empty value, a value containing a path separator, or one with a `.`, `..`, or absolute-path component is a request validation error (see [Exit codes](exit-codes.md) — `docs` exit codes), reported before any output directory is created.
- The name selects only the output path; like the title, it is an option of the invocation and never affects the document body, Catalog ordering, fallbacks, or anchor IDs.

## Input text policy

The document title, group names, file titles, case titles, source paths, and descriptions are inserted into the generated document verbatim: no Markdown escaping, sanitization, trimming, or dedenting is applied, and metadata is never parsed or restructured as Markdown.
If a value contains Markdown syntax, raw HTML, newlines, control characters, heading markers, or link delimiters that break the rendered layout or the table of contents, that is the input's responsibility.
Publishing Markdown generated from untrusted `.repor` sources therefore requires sanitization by the downstream publisher or rendering environment, not by `reportage docs`.

Two structural exceptions are renderer-owned values, not metadata escapes: the generated explicit anchor IDs (ASCII only, see [Anchors and table of contents](#anchors-and-table-of-contents)) and the code fence wrapping each case source (see [Case source fences](#case-source-fences)).
Line endings are the one permitted normalization: CRLF sequences are normalized to LF throughout the generated document, metadata included; no other character is changed.

## Plain text serialization

`--format plain` with `--layout single-file` writes exactly one document, named `index.txt` unless `--index-file-name` overrides it (see [Index file name](#index-file-name)):

```text
<out-dir>/index.txt
```

The serialization contract:

- The document starts with the document title (`--title`, default `Reportage Documentation`).
- Blocks (`Group`, `File`, `Source path`, `Description`, `Case`, `Reportage source`) are separated by exactly one empty line.
- `Group` / `File` / `Source path` / `Case` / `Description` values are indented by two spaces per logical line; absent descriptions omit the whole `Description` block.
- `Reportage source` lines are indented by four spaces; empty source lines stay empty, so no line carries trailing whitespace.
- Line endings are normalized to LF for the whole document, including source blocks from CRLF sources.
- Whether or not a source ends with a final newline, exactly one empty line separates it from the next block, and the document ends with exactly one LF.
- Beyond that presentation indentation, LF normalization, and the block labels, case source content is reproduced without loss or replacement.

The representative example and the ordering / fallback / zero-case shapes are fixed by the generated example documents under [`tests/fixtures/docs/`](../../tests/fixtures/docs/) (`index.snapshot.txt` and `index.snapshot.md`), enforced byte for byte by [`crates/reportage-cli/tests/docs_generation.rs`](../../crates/reportage-cli/tests/docs_generation.rs).

## Markdown serialization

`--format markdown` with `--layout single-file` writes exactly one document, named `index.md` unless `--index-file-name` overrides it (see [Index file name](#index-file-name)):

```text
<out-dir>/index.md
```

Source selection, loading, Catalog construction, ordering, fallbacks, and `--out-dir` validation are identical to the plain text format; only the serialization differs.

The heading hierarchy is fixed:

| Section | Heading |
| --- | --- |
| document title | `#` |
| table of contents (`Contents`) | `##` |
| group | `##` |
| documented file | `###` |
| documented case | `####` |

The serialization contract:

- The document starts with `# <document title>`, followed by a `## Contents` section listing groups, files, and cases in Catalog order, each entry linking to its explicit anchor.
- Every group, file, and case heading is immediately preceded by its explicit `<a id="..."></a>` anchor on its own line.
- A file section carries `Source: <source_path>`; file and case descriptions follow their heading and are omitted entirely when absent — no empty paragraph or placeholder is generated.
- A zero-case file still gets its table of contents entry, heading, source path, and description; no empty case section is generated.
- Renderer-generated blocks are separated by exactly one empty line; a final newline carried by a description value never changes block separation.
- Line endings are normalized to LF for the whole document, and the document ends with exactly one LF.

### Anchors and table of contents

The table of contents links through explicit HTML anchors, never through a Markdown implementation's implicit heading slugs.
Anchor IDs combine the 1-based Catalog structure indices with a readability slug:

```text
group-{group-index}[-{slug}]
file-{group-index}-{file-index}[-{slug}]
case-{group-index}-{file-index}-{case-index}[-{slug}]
```

The slug normalization is fixed: ASCII letters and digits are kept (letters lowercased), every other run of characters collapses into a single `-`, leading and trailing `-` are stripped, and an empty result omits the `-{slug}` part entirely.
Uniqueness is guaranteed by the structure indices alone, so duplicate titles, titles that normalize to the same slug, and titles without any ASCII characters never collide.
Anchor IDs consist only of renderer-generated ASCII lowercase letters, digits, and hyphens; metadata is never inserted into an HTML attribute.
The same Catalog and title always produce the same anchors, and the document title does not participate in anchor IDs.

### Case source fences

Each case source is wrapped in a fenced code block with the `reportage` language identifier.
The fence is one backtick longer than the longest backtick run in the exact case source, and at least three, so a source containing backtick fences cannot terminate the block early.

Inside the fence, the source is reproduced with CRLF normalized to LF and no other change: no line is dropped, no character is replaced, and whitespace and comments are preserved.
A source without a final newline gets one structural LF so the closing fence sits on its own line; a source with a final newline gets no extra blank line before the closing fence.

## Output directory and replacement

`--out-dir` is validated independently of format and layout:

- A missing directory is created recursively — but only after source resolution, loading, and rendering all succeeded, so a failing generation never creates it.
- An existing path must be a regular directory; a regular file or a symlink is rejected.
- Generated files are confined to the output root; layouts cannot address paths outside it.

Replacement of the generated document (`index.txt` or `index.md` by default, or the `--index-file-name` value) is existing-output-preserving:

- An existing regular file is overwritten; an existing directory or symlink is rejected.
- The document is fully written to a temporary file in the output directory first, then moved into place with a platform-appropriate replace, so a failure before the replace leaves the previous document unchanged and never leaves a partially written one.
- Unrelated files in the output directory are never modified or deleted; in particular, generating one format never touches the other format's document, so `index.txt` and `index.md` coexist in the same output directory.
- Power-loss durability (`fsync`) is out of scope in v0; the guarantee covers process-level failures.

## Out of scope in v0

HTML formats, multi-file layouts (split-by-group / split-by-file), custom templates, themes, a `--check` mode, config-driven source discovery, embedding execution results or evidence, and cleaning the output directory are out of scope.
There is no `--format json` CLI envelope for this subcommand in v0 (`--format` selects the generated document serialization, not a CLI stdout format); error details are reported as `error:` lines on stderr in a deterministic order.
