# Use shim-emitted events for shim invocation observability

- Status: Accepted
- Created: 2026-06-29T00:00:00Z

## Context

- reportage executes actions through `sh -c`.
- Command resolution belongs to the shell/runtime environment.
- Parsing action command text to infer shim usage would require partial shell parsing.
- PATH overlay shims can make failures difficult to diagnose unless invocation metadata is observable.

## Decision

- reportage will not parse action command text to infer shim usage.
- Protocol-compliant shims will emit invocation events through a runner-provided side channel.
- The initial side channel is an action-scoped event directory.
- The runner reads the action directory after action completion and attaches events to action results, diagnostics, and artifacts.
- Runner-generated shim event write failure uses a prefixed stderr warning and continues target invocation.
- These stderr warnings are observable stderr and are not automatically filtered from stdout/stderr assertions.
- A dedicated diagnostic side channel or run-level warning file is deferred.

Shim-emitted events are used because the shim is the component that directly
knows it was invoked and which target invocation it will delegate to. This keeps
command resolution semantics with the shell instead of making reportage
speculate from command text.

Action-scoped event directories are used initially because they make event
attribution direct and prevent stale events from earlier actions from being
attached to later actions. They also support multiple shim invocations in one
action without requiring the runner to parse shell structure.

Runner-generated shim event write failure is warning-and-continue because the
target invocation should still run when observability fails. The prefixed stderr
warning keeps the failure visible to users and artifacts.

A dedicated diagnostic side channel is deferred because it introduces additional
write-failure, attribution, ordering, and reporting concerns.

## Consequences

Positive:

- avoids partial shell parsing;
- keeps command resolution semantics with the shell;
- makes shim invocation observable;
- supports multiple invocations per action.

Negative:

- protocol-compliant shims are required for observability;
- non-compliant shims may be indistinguishable from ambient commands;
- stderr warnings may affect stderr assertions;
- event directory/file management adds implementation complexity.

## References

- [docs/shims.md](../shims.md)
- [docs/shims/event-protocol.md](../shims/event-protocol.md)
- [docs/execution-model.md](../execution-model.md)
- [docs/self-testing.md](../self-testing.md)
