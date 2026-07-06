
# Adopt Subject-First Directory Assertion Syntax

- Status: Accepted
- Created: 2026-07-06T00:00:00Z

## Context

#24 added the first workspace filesystem assertions, `file "<path>" exists` and `file "<path>" contains "<text>"`, and deliberately scoped `file` to regular files only. Directory existence and directory-entry assertions were split out into this issue (#66) so that `file` would not have to mean "regular file" for one predicate and "any filesystem path" for another.

`model.rs` already defined `Expectation::Dir(DirExpectation)` and a `DirMatcher` with `Exists` / `NotExists` variants as forward-looking placeholders (see #13/#28-era work), but no grammar rule or parser code produced them, and the evaluator did not evaluate them.

This issue adds the first real directory assertion syntax and evaluator support: `exists` and `contains`. It intentionally does not add recursive directory assertions, glob assertions, directory snapshot/equality assertions, or a general `path` subject — see "Out of scope" below.

## Decision

reportage v0 adopts the same **subject-first** shape #24 chose for `file`, applied to a `dir` subject:

```text
dir-expectation =
  "dir" string-literal "exists"
  | "dir" string-literal "contains" string-literal
```

```reportage
assert {
  dir "artifacts" exists
  dir "artifacts" contains "result.json"
}
```

`dir "<path>"` is the common subject; `exists` and `contains "<name>"` are predicates on that subject. The expectation-first form (`dir exists "<path>"` / `dir contains "<path>" "<name>"`) is **not** adopted, for the same reasons #24 rejected it for `file`: a shared subject lets path policy validation live in one place regardless of predicate, keeps argument position uniform as predicates grow, and mirrors the evaluator's "observe once per subject, then dispatch on predicate" shape.

### `file` and `dir` remain separate subjects

`file` is scoped to regular files; `dir` is scoped to directories. Neither subject silently accepts the other's filesystem entry type: `file "<path>" exists` fails against a directory, and `dir "<path>" exists` fails against a regular file. A single subject that changed meaning per predicate would weaken the "subject declares what kind of evidence is being observed" property #24 established.

### `dir` subject path reuses `WorkspacePath`, not a second path-policy implementation

#24's `file` subject path is a plain `String`, validated by a dedicated `semantic::validate_file_path` function that duplicates two of the three rules already encoded in `model::WorkspacePath` (rejects absolute paths and `.`/`..` segments, but not empty paths — `file` assertions have never rejected an empty path explicitly).

The `dir` subject follows the same subject path rule `file` does (relative, non-empty, no `.`/`..` segments) — but rather than writing a third copy of that rule, `semantic::validate_dir_path` calls `WorkspacePath::parse` directly and maps its `WorkspacePathError` to the diagnostic codes already defined for it (`semantic.workspace_path.empty` / `.absolute` / `.dot_segment` — the same codes the `write` step's path already uses). This means:

- the empty-path case, which `file` assertions never rejected explicitly, is rejected for `dir` for free, because `WorkspacePath::parse` already covers it;
- `dir` and `write` share one path-policy implementation and one diagnostic code family instead of `dir` inventing a fourth;
- `file`'s separate `semantic.file_path.absolute` / `semantic.file_path.dot_segment` codes are left as-is — this ADR does not migrate `file` onto `WorkspacePath` retroactively, to avoid an unrelated breaking rename of an already-shipped diagnostic code.

Like `file`, this validation runs in the evaluator (`evaluate_case`), before evidence comparison, as a **semantic error** — not in the parser. See "Path policy violations are semantic errors, not parse errors" below.

### `contains` entry name policy

`dir "<path>" contains "<name>"` checks for exact, non-recursive, non-glob, non-content match of a single directory entry name directly under `<path>`. `<name>` is a directory entry name, not a path: it must be non-empty, must not contain `/`, must not be `.` or `..`, and must not contain control characters. Violating any of these is a **semantic error** (`semantic.dir_entry_name.empty` / `.path_separator` / `.dot_entry` / `.control_char`), rejected before evidence comparison, the same way an invalid subject path is.

`dir "artifacts" contains "a/b"` is rejected rather than interpreted as a nested-path check: if a caller wants to check `artifacts/a/b`, the subject path already expresses directory nesting (`dir "artifacts/a" contains "b"`), so `<name>` never needs to.

### Path policy violations are semantic errors, not parse errors

Both the `dir` subject path and the `contains` entry name are validated in `evaluate_case`, mirroring `file`'s ADR decision, not in the parser the way the `write` step's `WorkspacePath` is. The script and expectation shape are syntactically valid; only the *value* violates a policy the evaluator must reject before evidence comparison — the definition of a semantic error, not a parse error, per [`docs/semantic-diagnostics.md`](../semantic-diagnostics.md).

This also matters operationally: a parser-level rejection (like `write`'s) fails the whole file during the pre-execution validation phase, before any case in *any* selected file runs. A `dir` assertion with a bad path is scoped to the one case that uses it, reported as that case's `script_error`, exactly like an invalid `file` assertion path.

### Assertion failure diagnostic codes

Distinct codes exist per predicate and per failure shape, so tooling can distinguish "the directory does not exist" from "the directory is something else" from "the entry inside it is missing":

```text
assertion.dir.exists_missing
assertion.dir.exists_not_directory
assertion.dir.contains_subject_missing
assertion.dir.contains_subject_not_directory
assertion.dir.contains_entry_missing
```

`contains_subject_missing` and `contains_subject_not_directory` are separate from `exists_missing` / `exists_not_directory` even though they describe the same underlying subject-path observation: a failing `contains` should be identifiable as "the subject wasn't even a directory" without conflating it with a bare `exists` failure, so tooling can tell which expectation actually failed from the code alone.

### Symlink policy

Matches #24's `file` symlink policy: no symlink-specific assertion is introduced.

- `dir "<path>" exists` follows symlinks; if the target resolves to a directory, it succeeds, and if it resolves to a regular file, it does not.
- `dir "<path>" contains "<name>"` checks entry name existence only; it does not inspect whether a matching entry is itself a symlink, or follow it if so.
- Broken symlinks, symlink-specific assertions, and link-target assertions remain out of scope.

## Alternatives Considered

### Expectation-first form: `dir exists "<path>"` / `dir contains "<path>" "<name>"`

Rejected for the same reasons #24 rejected it for `file`: duplicated path validation across argument positions, and predicate-specific argument order instead of a uniform "path first" subject.

### Minting new `semantic.dir_path.*` diagnostic codes instead of reusing `semantic.workspace_path.*`

Considered defining `dir`-specific codes for its subject path violations, paralleling `file`'s `semantic.file_path.absolute` / `.dot_segment`.

Rejected because the `dir` subject path is, structurally, exactly a `WorkspacePath` — the same domain type and the same three rules the `write` step already validates and already has codes for. Minting a fourth near-identical code family would add no distinguishing information (the failure shape — empty / absolute / dot-segment — is identical across `write`, and now `dir`) while duplicating maintenance. `file`'s existing codes are left alone rather than retroactively renamed onto `WorkspacePath`, since that would be an unrelated breaking change to an already-shipped code.

### Folding `dir` semantics into `file`

Rejected in #24's ADR already; reaffirmed here now that `dir` is real: a single subject that means "regular file" for some predicates and "directory" for others would conflate two different evidence shapes under one subject name.

### Recursive / glob / content-based `contains`

Considered letting `contains` search recursively, glob-match, or inspect file content, to make one predicate cover more cases.

Rejected for v0: each of these is a materially different evidence shape (a recursive walk, a pattern match, a content read) with its own failure modes and diagnostic needs. Keeping `contains` to "does this exact entry name exist directly under this directory" keeps its semantics simple enough to specify completely in this issue. A future `path` subject, glob assertion, or recursive directory assertion can build on top of this without revising it.

## Consequences

### Positive Consequences

- `dir` and `file` remain independently evolvable subjects with no shared ambiguity about what kind of filesystem entry they observe.
- `dir` subject path policy is exactly `WorkspacePath`'s rule, reusing its existing diagnostic codes instead of adding a fourth near-duplicate code family.
- `dir "<path>" contains "<name>"` has a single, fully specified evidence shape (direct-child, exact-match, type-agnostic), leaving recursive/glob/content search to a clearly separate, future subject.
- Failing `dir` assertions carry enough diagnostic code granularity (missing vs. wrong type vs. missing entry) for tooling to distinguish failure shapes without parsing message text.

### Negative Consequences

- `dir`'s subject path diagnostic codes (`semantic.workspace_path.*`) are not named after `dir` at all, unlike `file`'s (`semantic.file_path.*`) — a caller reading `semantic.workspace_path.absolute` cannot tell from the code alone whether it came from a `dir` assertion or a `write` step. This is an accepted trade against not duplicating the `WorkspacePath` rule a third time.
- `file`'s own path codes are not migrated onto `WorkspacePath`, so reportage now has two different implementations of "the same" relative/non-empty/no-dot-segment rule (`validate_file_path` and `WorkspacePath::parse`) that happen to agree today but are not structurally the same code. Unifying them is left to a future issue, since it would rename `file`'s already-shipped diagnostic codes.

### Neutral Consequences

- `DirMatcher::NotExists` remains defined in `model.rs` as a forward-looking placeholder, not produced by the v0 parser or evaluated by the v0 evaluator — the same treatment #24 gave `FileMatcher::NotExists` / `FileMatcher::Matches`.
- Recursive directory assertions, glob assertions, directory snapshot/equality assertions, and a general `path` subject remain explicitly out of scope and are expected to build on top of this decision rather than revise it.
