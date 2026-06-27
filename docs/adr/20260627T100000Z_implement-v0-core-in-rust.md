# Implement the v0 Core in Rust

- Status: Accepted
- Created: 2026-06-27T10:00:00Z

## Context

reportage needs a CLI runner, script parser, config validator, diagnostics, process execution, isolated workspaces, PATH shim generation, artifact output, and future adapter boundaries.

## Decision

Implement the v0 core in Rust.

## Alternatives Considered

Go was considered because it is strong for CLI implementation and process execution. TypeScript was considered for tooling compatibility, but would make the core depend on a host runtime.

## Consequences

### Positive Consequences

- Strong internal types for execution plans, diagnostics, artifacts, and adapter contracts.
- Lightweight executable distribution without a host runtime.
- Better support for preserving core, shim, adapter, and artifact boundaries.

### Negative Consequences

- Initial implementation may require more up-front parser and diagnostic design than a simpler Go CLI.

### Neutral Consequences

- Go remains a plausible fallback if the implementation model proves unnecessarily heavy.
