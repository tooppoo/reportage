# TBD

This document records intentionally deferred features and design topics.

Items listed here are not accepted v0 requirements unless another document explicitly promotes them into scope.

## Semantic assertions

### Encoding-aware assertions

`stdout contains <string>` and `stderr contains <string>` perform byte-level substring matching in v0. The expected string literal is treated as UTF-8 bytes and matched against the raw output bytes without any encoding conversion.

Assertions that decode output as a specific encoding before matching — for example, treating Shift-JIS output as Shift-JIS before comparing — are not in scope for v0. Supporting such encoding-aware assertions would require specifying which encoding to use, how to handle decode errors, and whether the expected value is also re-encoded for comparison.

This is deferred. When it becomes concrete, it should be addressed in a separate issue that defines the encoding declaration syntax, the set of supported encodings, and the error model for invalid byte sequences.

## Command shim model

### Third-party shim validation

A shim that does not follow the reportage shim event protocol may be
indistinguishable from a direct target invocation or an ambient command
invocation.

Future mitigation may include a validation interface such as:

```sh
reportage shim test <shim-file>
```

This is deferred. The current shim event protocol only defines
runner-generated shim events and records non-compliant shim limitations.

### Dedicated shim diagnostic side channel

Runner-generated shim infrastructure warnings currently use prefixed stderr
diagnostics such as `reportage shim warning:` and remain observable stderr.

A dedicated diagnostic side channel or run-level warning file may avoid
polluting target stderr, but it introduces additional write-failure,
attribution, ordering, and reporting concerns.

This is deferred.

### Non-UTF-8 executable invocations

`ExecutableInvocation` currently requires that both `program` and `args` are valid UTF-8, enforced at construction time. Non-UTF-8 values are rejected explicitly with a clear error rather than silently converted.

Whether non-UTF-8 program paths or fixed arguments should be supported in a later version is TBD.

Supporting them would require generating POSIX wrapper scripts that embed byte sequences which cannot be represented as UTF-8 strings. This may be possible using POSIX `printf` or octal escapes, but the added complexity and the rarity of such paths in practice make this a candidate for a future issue rather than an immediate requirement.

Until this is resolved, users with non-UTF-8 paths must work around the limitation at the OS level, for example by creating a symlink with an ASCII name.

## v0.1.x candidates

### Explicit file selection alongside config-driven discovery

`reportage --config <path> <script>...` combining a config file with explicit script arguments is rejected in v0.

A future version may allow users to run a subset of config-discovered files by passing additional explicit paths. The interaction between pattern-discovered and explicitly-specified files needs a clear precedence rule before it can be added cleanly.

### Command timeout

Deferred because v0 should first establish the runner, parser, assertion model, config, shims, and artifact output.

Timeout support should be added after the command execution layer is stable enough to handle cancellation, child process cleanup, and diagnostics consistently.

### Artifact schema stabilization

Artifacts should be generated from the beginning, but the exact schema should remain experimental until real runs expose what needs to be preserved.

### Self-test run ID control

`--debug-run-id <id>` was implemented in #24 as the hidden, internal-only option anticipated below, so that reportage's own file-assertion self-test could assert a deterministic artifact path (`.reportage/runs/<id>/result.json`). It is not a public stable interface: it is hidden from `--help`, and the runner refuses to silently overwrite an existing run directory for a fixed id.

The public contract for run ID control remains TBD. Future options may include a built-in strategy such as UUID / counter / fixed value, or a run ID provider command that emits an ID for the runner to validate and use.

A provider command is deferred because it would introduce a runner-internal external command execution path distinct from `$` actions and shimmed target invocations. Before adopting it, reportage needs clear rules for failure handling, stdout parsing, stderr reporting, timeout, ID validation, collision behavior, and whether the provider itself participates in shim interception.

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

`assert { exit 0; stderr empty }`: a single-line block with multiple expectations.

The candidate separator is `;`. Rejected for v0 because v0 syntax does not define a multi-expectation separator. If added, `;` would be explicit and unambiguous.

### Single-line `assert ${expectation}` sugar

`assert exit 0`: a shorthand for a single-expectation block.

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

Rejected for v0. The fundamental problems are shell syntax ambiguity, need for standalone assertion blocks at the initial checkpoint, and added syntactic exceptions. They are better addressed after v0 establishes a stable baseline.

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
