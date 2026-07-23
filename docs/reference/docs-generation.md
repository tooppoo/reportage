# Documentation Generation

This document defines the `reportage docs` subcommand: how sources are selected, how the generated document is derived, the plain text serialization contract, and the output replacement guarantees.
Exit codes are defined in [Exit codes](exit-codes.md) — `docs` exit codes.
The rationale for the command's internal boundaries is recorded in [ADR: Documentation Generation Command](../adr/20260723T070556Z_documentation-generation-command.md).

## Synopsis

```console
reportage docs '<pattern>'... --out-dir <directory> [--format plain] [--layout single-file]
```

Example:

```console
reportage docs 'examples/**/*.repor' --out-dir generated/docs
```

`reportage docs` aggregates the `document file` / `document case` metadata and the original case source of every selected `.repor` file into generated documentation under `--out-dir`.

- `--out-dir` is required.
- `--format` defaults to `plain`; `plain` is the only v0 format.
- `--layout` defaults to `single-file`; `single-file` is the only v0 layout.
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

## Plain text serialization

`--format plain` with `--layout single-file` writes exactly one document:

```text
<out-dir>/index.txt
```

The serialization contract:

- The document starts with the fixed title `Reportage Documentation`.
- Blocks (`Group`, `File`, `Source path`, `Description`, `Case`, `Reportage source`) are separated by exactly one empty line.
- `Group` / `File` / `Source path` / `Case` / `Description` values are indented by two spaces per logical line; absent descriptions omit the whole `Description` block.
- `Reportage source` lines are indented by four spaces; empty source lines stay empty, so no line carries trailing whitespace.
- Line endings are normalized to LF for the whole document, including source blocks from CRLF sources.
- Whether or not a source ends with a final newline, exactly one empty line separates it from the next block, and the document ends with exactly one LF.
- Beyond that presentation indentation, LF normalization, and the block labels, case source content is reproduced without loss or replacement.

The representative example and the ordering / fallback / zero-case shapes are fixed by the generated example documents under [`tests/fixtures/docs/`](../../tests/fixtures/docs/), enforced byte for byte by `crates/reportage-cli/tests/docs_generation.rs`.

## Output directory and replacement

`--out-dir` is validated independently of format and layout:

- A missing directory is created recursively — but only after source resolution, loading, and rendering all succeeded, so a failing generation never creates it.
- An existing path must be a regular directory; a regular file or a symlink is rejected.
- Generated files are confined to the output root; layouts cannot address paths outside it.

`index.txt` replacement is existing-output-preserving:

- An existing regular file is overwritten; an existing directory or symlink is rejected.
- The document is fully written to a temporary file in the output directory first, then moved into place with a platform-appropriate replace, so a failure before the replace leaves the previous `index.txt` unchanged and never leaves a partially written one.
- Unrelated files in the output directory are never modified or deleted.
- Power-loss durability (`fsync`) is out of scope in v0; the guarantee covers process-level failures.

## Out of scope in v0

Markdown / HTML formats, multi-file layouts, a `--check` mode, config-driven source discovery, embedding execution results or evidence, and cleaning the output directory are out of scope.
The single supported document format `plain` also means there is no `--format json` CLI envelope for this subcommand in v0; error details are reported as `error:` lines on stderr in a deterministic order.
