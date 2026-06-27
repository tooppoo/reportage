# TBD

This document records intentionally deferred features and design topics.

Items listed here are not accepted v0 requirements unless another document explicitly promotes them into scope.

## v0.1.x candidates

### Command timeout

Deferred because v0 should first establish the runner, parser, assertion model, config, shims, and artifact output.

Timeout support should be added after the command execution layer is stable enough to handle cancellation, child process cleanup, and diagnostics consistently.

### Artifact schema stabilization

Artifacts should be generated from the beginning, but the exact schema should remain experimental until real runs expose what needs to be preserved.

### Coverage adapter finalization hook

Coverage is a first-class design concern, but reportage is not a coverage engine.

A finalization hook should be considered after the base artifact model and command shim model exist.

## v0.2.x candidates

### `--jobs`

v0 runs concrete cases serially.

Parallel execution should be considered after serial semantics, artifact output, and adapter boundaries are stable.

### Adapter capability model for parallel execution

Some adapters may support parallel execution. Others may not, especially when coverage tools require shared output files or explicit flushing.

### Machine-readable result format stabilization

The result format should become more stable once enough real examples exist.

## Later

### Browser automation integration

Deferred because reportage should not become a browser automation framework by default.

### HTTP syntax

Deferred because the first target is CLI E2E.

### Service lifecycle syntax

Deferred because service lifecycle management can easily make the core heavy.

### Rich HTML report

Deferred because v0 should produce evidence first. Rich reporting can be added as post-processing.

### Public KDL Schema

Deferred because runtime validation should be owned by reportage's internal validator first.

### Multiple runtime-specific coverage adapters

Deferred because the adapter boundary should be validated with a small initial implementation before adding many runtime-specific integrations.

### Native Windows shell execution

Deferred because v0 uses POSIX shell execution to keep shell semantics small and predictable.

### Embedded jq

Deferred because v0 uses external `jq`.

### Full shell parsing or shell rewriting

Deferred because v0 passes `$` steps to the POSIX shell. reportage should not parse and rewrite arbitrary shell syntax in the core runner.
