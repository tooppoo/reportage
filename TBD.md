# TBD

This document records intentionally deferred features and design topics.

Items listed here are not accepted v0 requirements unless another document explicitly promotes them into scope.

## v0.1.x candidates

### Explicit file selection alongside config-driven discovery

`reportage --config <path> <script>...` combining a config file with explicit script arguments is rejected in v0.

A future version may allow users to run a subset of config-discovered files by passing additional explicit paths. The interaction between pattern-discovered and explicitly-specified files needs a clear precedence rule before it can be added cleanly.

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

## Assertion syntax extensions

### Single-line assert block with multiple expectations

`assert { exit 0; stderr empty }` — a single-line block with multiple expectations.

The candidate separator is `;`. Rejected for v0 because v0 syntax does not define a multi-expectation separator. If added, `;` would be explicit and unambiguous.

### Single-line `assert ${expectation}` sugar

`assert exit 0` — a shorthand for a single-expectation block.

Rejected for v0 to keep the assertion model unambiguous. Could be added as syntax sugar over a single-expectation `assert { ... }` block, but increases the surface area and potentially confuses the block model.

### `require` / hard assertion

A hard assertion variant that stops the case immediately on failure, before evaluating remaining expectations in the block.

Deferred because v0 adopts a single model: assertion block failure stops the case, all expectations within a block are evaluated. Introducing a second mode prematurely splits a concept that is still simple.

### Action-attached assertion sugar

A future syntax for writing expectations inline with the action that produces them:

```reportage
$ rellog init {
  exit 0
  dir exists .rellog
}
```

Rejected for v0. The fundamental problems — shell syntax ambiguity, need for standalone assertion blocks at the initial checkpoint, and added syntactic exceptions — are better addressed after v0 establishes a stable baseline.

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
