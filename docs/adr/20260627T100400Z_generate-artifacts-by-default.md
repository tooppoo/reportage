# Generate Artifacts by Default

- Status: Accepted
- Created: 2026-06-27T10:04:00Z

## Context

reportage is designed to execute E2E scenarios and produce evidence: command results, logs, assertion outcomes, generated files, and coverage artifacts when adapters provide them.

## Decision

Generate artifacts by default for every run.

There is no `--no-artifacts` option in the current v0 direction.

## Alternatives Considered

Generating artifacts only on failure was considered, but it makes successful run evidence unavailable. Adding `--no-artifacts` immediately was considered, but it weakens artifact generation before the project has inspected real output.

## Consequences

### Positive Consequences

- Every run produces inspectable evidence.
- CI failures can preserve useful output.
- Future post-processing tools can consume a consistent artifact location.

### Negative Consequences

- Every run writes files under `.reportage/`.
- Artifact write failures must be handled as runtime infrastructure errors.

### Neutral Consequences

- The artifact schema is experimental in early v0.
