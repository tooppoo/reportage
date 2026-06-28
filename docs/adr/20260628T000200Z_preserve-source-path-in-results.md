# Preserve source_path in Case Results and Artifacts

- Status: Accepted
- Created: 2026-06-28T00:02:00Z

## Context

When a suite run processes multiple test files, case names alone are not sufficient to identify which file a case came from. Two different files could contain cases with identical names. Artifact consumers and human readers need to know the originating file for each case result.

## Decision

`CaseResult` carries an optional `source_path` field that records the file the case was loaded from. The artifact writer includes `source_path` in the case JSON. The CLI output format includes the source path alongside the case name.

Output format: `<STATUS>  <source_path> :: <case_name>`

`source_path` is `Option` in the internal type to allow the evaluator to remain agnostic of file loading. The CLI sets it after evaluation.

## Alternatives Considered

Embedding `source_path` into the evaluator was considered but rejected because the evaluator should not be responsible for file system context. Passing it as a parameter to `evaluate()` was also considered, but setting it after the fact is simpler and keeps the evaluator's signature stable.

## Consequences

### Positive Consequences

- Case results are fully traceable to their source file.
- Artifact JSON can be used to identify which file a case came from without re-reading source.
- Output is readable when multiple files contain cases with the same name.

### Negative Consequences

- `source_path` is `None` in the internal evaluator representation until the CLI sets it.
  This is an internal implementation detail, not user-visible.

### Neutral Consequences

- Artifact schema gains a `source_path` field on case objects.
