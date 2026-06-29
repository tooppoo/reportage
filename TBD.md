# TBD

This document records intentionally deferred features and design topics.

Items listed here are not accepted v0 requirements unless another document explicitly promotes them into scope.

## Command shim model

### Non-compliant or third-party shims

A shim that does not write to the `REPORTAGE_SHIM_EVENT_DIR` side channel
following the event contract described in ADR
`20260628T210000Z_shim-invocation-event-side-channel` is indistinguishable,
from reportage's perspective, from a direct target or ambient command
invocation. Its presence will not appear in action results, diagnostics, or
artifacts.

Future mitigation may include a validation interface — for example,
`reportage shim test <shim-file>` — that checks whether a shim follows the
protocol contract. Defining and stabilizing that interface, including what
passing or failing the check means and how it is surfaced, is out of scope for
the current implementation.

### Dedicated diagnostic side channel for shim infrastructure warnings

Runner-generated shim event write failures currently emit a prefixed diagnostic
to the action's stderr:

```
reportage shim warning: failed to write shim invocation event: <path>
```

This diagnostic is deliberately observable stderr. It is not filtered out from
stdout/stderr assertions, so a `stderr empty` assertion will fail if a shim
write failure occurs.

A dedicated diagnostic side channel or run-level warning file for shim
infrastructure warnings may avoid polluting the action's stderr. Such a channel
would let `stderr empty` assertions pass even when shim infrastructure warnings
are present. However, it introduces additional concerns around write failure,
attribution, ordering, reporting, and artifact integration. For the initial
implementation, prefixed stderr diagnostics are accepted as the reporting
mechanism. A dedicated channel is deferred.

### Non-UTF-8 executable invocations

`ExecutableInvocation` currently requires that both `program` and `args` are valid UTF-8, enforced at construction time. Non-UTF-8 values are rejected explicitly with a clear error rather than silently converted.

Whether non-UTF-8 program paths or fixed arguments should be supported in a later version is TBD.

Supporting them would require generating POSIX wrapper scripts that embed byte sequences which cannot be represented as UTF-8 strings. This may be possible using POSIX `printf` or octal escapes, but the added complexity — and the rarity of such paths in practice — makes this a candidate for a future issue rather than an immediate requirement.

Until this is resolved, users with non-UTF-8 paths must work around the limitation at the OS level (e.g., by creating a symlink with an ASCII name).

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
