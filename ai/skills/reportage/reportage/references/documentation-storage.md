# Documentation Storage and Cache

## Purpose

This document defines how an agent must select a local storage directory, save the documentation index, cache version-matched reportage documents, and safely reuse them.

The main skill contains the blocking rules. This document provides the detailed procedure.

## Repository Root Resolution

Determine the current repository root before selecting a storage location:

```sh
git rev-parse --show-toplevel
```

If the current directory is not inside a Git repository, the project does not satisfy the default storage model described by this skill.

Do not silently substitute:

- the current working directory
- the user's home directory
- a global cache directory
- an operating-system temporary directory

Ask the user to identify an appropriate project directory or storage location.

## Eligible Storage Directory

Prefer an existing project-defined cache or temporary directory documented in repository instructions.

Otherwise, inspect existing repository-local candidates such as:

```text
tmp
.tmp
.cache
var/tmp
```

These names are candidates only. A directory is eligible only when all of the following are true:

1. It already exists.
2. It resolves inside the repository root.
3. It is writable.
4. Git confirms that it is ignored.
5. It is stable enough to remain available during later agent steps.
6. It is not automatically replaced, cleared, or deleted by a command likely to run during the task.
7. It does not resolve through a symbolic link to a location outside the repository.

Do not create a new top-level temporary or cache directory without user approval.

## Ignore-Rule Verification

Verify ignored status with Git:

```sh
git check-ignore -q -- <candidate>
```

Do not infer ignored status from:

- the directory name
- a comment in `.gitignore`
- the fact that files in the directory are currently untracked
- global Git ignore configuration alone

When necessary, inspect which rule matched:

```sh
git check-ignore -v -- <candidate>
```

The selected storage directory must be ignored by the repository's effective ignore rules.

Do not modify `.gitignore` unless the user explicitly requests that change.

## Symbolic Link and Path Safety

Resolve the repository root and candidate directory to canonical paths before use.

Reject a candidate when:

- the canonical candidate path is outside the canonical repository root
- the candidate itself is a symbolic link to an external path
- a parent component redirects outside the repository
- the final cache path overlaps a tracked project directory
- the path cannot be resolved reliably

A path that appears repository-local textually is not sufficient. Its resolved location must remain inside the repository.

## Directories That Must Not Be Used

Do not use:

- the repository root
- `/tmp` or another operating-system temporary directory
- the user's home directory
- a global cache directory
- a tracked directory
- a directory whose contents are generated and destructively replaced by normal project commands
- `.reportage/` when reportage execution may recreate or remove it
- a symbolic link that resolves outside the repository

Build directories may be used only when repository instructions establish that they are stable and will not be cleared during the task. By default, treat build output directories as unsuitable.

## Missing Storage Directory

If no eligible storage directory exists, stop before fetching documentation or making version-sensitive reportage changes.

Ask the user for a storage location using a message equivalent to:

> reportage のバージョン対応ドキュメントを保存するため、リポジトリ内の Git 管理対象外で継続利用できるディレクトリが必要です。保存先を指定してください。例: `./tmp`
>
> 既存の `.gitignore` は変更しません。

Do not silently:

- create a new top-level directory
- add an ignore rule
- fall back to `/tmp`
- store documents only in memory
- fetch documents without retaining them

The absence of an eligible storage directory is a blocking condition.

## Dedicated Cache Directory

After selecting an eligible parent directory, create:

```text
<candidate>/reportage-docs/
```

The dedicated child directory may be created without further approval because its parent has already been verified as ignored and writable.

Do not mix reportage documentation with unrelated temporary files when a dedicated child can be used.

## Version-Specific Layout

Store documentation under a directory specific to the running reportage tag:

```text
<candidate>/
└── reportage-docs/
    └── <tool-tag>/
        ├── docs-index.json
        ├── docs/
        │   ├── ai/
        │   │   └── README.md
        │   └── syntax.md
        └── spec/
            └── output/
                └── json-report/
                    └── README.md
```

Use the `tool.tag` value from `reportage docs --format=json` as the version directory name only after validating that it is safe as a single path component.

Reject or safely encode a tag that:

- is empty
- is `.` or `..`
- contains a path separator
- resolves outside the cache root
- collides with a file where a directory is required

Do not merge documentation from different tags into one directory.

## Saving the Documentation Index

Run:

```sh
reportage docs --format=json
```

Parse stdout as structured JSON.

Save the complete, unmodified stdout as:

```text
<candidate>/reportage-docs/<tool-tag>/docs-index.json
```

Preserve the exact index used for document discovery. Do not save only selected fields.

Before continuing, verify:

- `schema_version` is supported
- `tool.name` is `reportage`
- `tool.version` is present
- `tool.tag` is present
- `documents` is an array
- required document entries contain `id`, `path`, and `urls.ai`

## Saving Documents

For each selected document, reproduce its `documents[].path` below the version-specific cache directory.

Example index entry:

```json
{
  "id": "ai-readme",
  "path": "docs/ai/README.md"
}
```

Local path:

```text
<candidate>/reportage-docs/<tool-tag>/docs/ai/README.md
```

Mirroring the repository-relative path preserves relative links between cached documents.

Read the saved local copy after writing it. Do not use the transient network response as the only source consumed by the task.

## Document Path Validation

Before using `documents[].path` as a local filesystem path, verify that it:

- is relative
- is not empty
- does not begin with `/`
- does not contain a Windows drive prefix
- contains no `..` path traversal
- does not resolve outside the version-specific cache directory
- does not overwrite `docs-index.json`
- does not collide with another document path
- does not traverse a symbolic link that leaves the cache directory

Reject an unsafe path rather than normalizing it into a different location.

## Atomic Writes

When practical, write downloaded content atomically:

1. Create a temporary file inside the destination directory.
2. Write the complete content.
3. Flush and close the file.
4. Rename it to the final path.

Do not write a partially downloaded document directly over a previously valid cached copy.

If an atomic rename is unavailable, preserve the old file until the replacement has been fully written and validated.

## Cache Reuse

Always run `reportage docs --format=json` at the beginning of a reportage task, even when cached documents exist.

A cached document may be reused only when all of the following match the current index exactly:

- documentation index `schema_version`
- `tool.name`
- `tool.version`
- `tool.tag`
- document `id`
- document `path`
- document `urls.ai`

Also verify that:

- the cached `docs-index.json` parses successfully
- the cached document exists at the mirrored path
- the cached path remains inside the selected cache directory
- the file is readable

Do not reuse a document merely because the tag directory name matches.

## Refreshing the Cache

Fetch a selected document again when:

- the current index differs from the cached index
- the selected entry differs in ID, path, or AI URL
- the cached file is missing
- the cached file cannot be read
- the cached file is known to be incomplete
- provenance cannot be established

Save the current index before fetching selected documents.

Do not delete documentation for other tags unless the user explicitly asks for cache cleanup.

## Fetch Failure

When fetching a version-matched document fails:

1. Check for an exact matching cached entry.
2. Reuse it only if all cache-reuse conditions are satisfied.
3. Report that the remote fetch failed and an exact matching cache was used.

If no exact matching cached copy exists, stop before:

- generating version-sensitive syntax
- interpreting an undocumented diagnostic
- asserting that a feature exists
- validating against an assumed output contract

Do not substitute default-branch documentation.

## Following Relative Links

A cached document may reference another version-matched repository document that is not yet cached.

When following such a link:

1. Resolve it relative to the source document's mirrored local path.
2. Confirm that the resolved path remains inside the version-specific cache directory.
3. Derive the corresponding version-matched remote location.
4. Fetch the target.
5. Save it at the mirrored local path.
6. Read the saved copy.

Do not follow a relative link into:

- another version directory
- the repository working tree
- the repository default branch
- an unrelated external site
- a path outside the cache root

A link to an external source may be consulted when the task requires it, but it must not be treated as part of reportage's normative specification unless the version-matched reportage documentation explicitly delegates authority to it.

## Provenance Rules

Use the `urls.ai` location from the current index as the primary source.

Use `urls.human` only:

- to present a human-readable link
- when the AI URL is unavailable and the same versioned content can be retrieved reliably

Do not silently substitute:

- `main`
- `master`
- the repository default branch
- a newer release
- an older release
- a local document of unknown provenance
- a search-engine result

A repository-local document may be treated as normative only when its provenance can be shown to match the current `tool.tag` or the exact source revision of the running binary.

Otherwise, treat it as project context or an example, not as the specification.

## Completion Information

Record for the final task report:

- selected storage parent
- version-specific cache directory
- current reportage version and tag
- whether each document was fetched or reused
- any fetch failure
- any document that could not be obtained
- any storage condition that blocked work
