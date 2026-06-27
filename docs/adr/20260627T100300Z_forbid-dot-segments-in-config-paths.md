# Forbid Dot Segments in Config Path-Like Values

- Status: Accepted
- Created: 2026-06-27T10:03:00Z

## Context

reportage config contains path-like values such as test glob patterns and command executable targets. Shell-style forms such as `./target/debug/rellog` are common, but they make config review and validation weaker.

## Decision

Forbid dot segments uniformly in config path-like values. Absolute paths are also forbidden.

This applies to `tests.path`, `commands.command.exec`, and future path-like config values unless explicitly stated otherwise.

## Alternatives Considered

Allowing `./` while rejecting `../` was considered, but it creates a special case. Normalizing paths and rejecting only escaped paths was also considered, but it makes normalization part of the user-facing semantics.

## Consequences

### Positive Consequences

- Config path rules are simple.
- Path review is easier.
- Repository-local execution is more reproducible.

### Negative Consequences

- Users cannot write common shell-style paths such as `./target/debug/rellog`.

### Neutral Consequences

- Examples must consistently show paths such as `target/debug/rellog`.
