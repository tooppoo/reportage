# Validate All Files Before Executing Any Actions

- Status: Accepted
- Created: 2026-06-28T00:00:00Z

## Context

When running a suite of test files (config-driven or multi-script explicit mode), the runner must decide whether to validate all files before executing any `$` actions, or to validate and execute each file independently.

If files are processed independently, a parse error in the second file is only discovered after the first file's commands have already run. This produces partial execution: some side effects occurred, but not all files were checked.

## Decision

All selected test files are read and parsed in a validation phase that runs before any `$` actions execute.

If any file has a read error or parse error, no `$` actions execute from any file. The run exits with code `2` and reports all file-level errors collected during validation.

## Alternatives Considered

Per-file validate-and-run was considered but rejected because partial execution is worse than no execution: CI environments would get misleading partial results, and side effects from early files would not be undone.

## Consequences

### Positive Consequences

- All validation errors are visible in a single run.
- No partial execution when any file is invalid.
- CI can always distinguish between "nothing ran" and "something ran and failed".

### Negative Consequences

- A valid file whose `$` actions would have passed is blocked if another file has a parse error.

### Neutral Consequences

- The validation phase is fast (file reads and parse only); the latency impact is negligible for typical suite sizes.
