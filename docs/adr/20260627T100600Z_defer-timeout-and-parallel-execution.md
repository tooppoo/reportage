# Defer Timeout and Parallel Execution

- Status: Accepted
- Created: 2026-06-27T10:06:00Z

## Context

Timeouts and parallel execution are useful for E2E runners, but they complicate process lifecycle management, child cleanup, adapter behavior, and artifact output.

reportage v0 first needs to establish serial execution semantics, isolated workspaces, command shims, assertions, diagnostics, and artifacts.

## Decision

Defer command timeout and parallel execution from the initial v0 scope.

Target direction:

```text
timeout: v0.1.x candidate
--jobs: v0.2.x candidate
```

## Alternatives Considered

Adding timeout in v0 immediately was considered, but it requires cancellation and child cleanup behavior before the command execution layer is stable.

Adding `--jobs` in v0 immediately was considered, but parallel execution before artifacts and adapters are stable would make failures harder to inspect.

## Consequences

### Positive Consequences

- v0 can focus on deterministic serial semantics.
- Artifacts and diagnostics can stabilize before concurrency is introduced.
- Adapter capability modeling can be designed with real constraints.

### Negative Consequences

- Early v0 runs may hang if the target command hangs.
- Large test suites will run serially until `--jobs` is introduced.

### Neutral Consequences

- The internal model should still expand scripts into independent concrete case plans.
