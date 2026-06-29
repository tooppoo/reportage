# Use PATH Overlay Shims for Command Resolution

- Status: Accepted
- Created: 2026-06-28T06:15:00Z

## Context

reportage v0.1.0 aims to make reportage capable of describing and running representative CLI-level E2E tests for reportage itself.

The target form should be the same form a user would write:

```reportage
case "with --help" {
  $ reportage --help

  assert {
    exit 0
    stdout contains "Usage: reportage [OPTIONS] [SUBCOMMAND]..."
  }
}
```

The problem is that the command text alone should not decide which executable implementation is used at runtime. For self-testing, the harness must run the Cargo-built, coverage-instrumented reportage executable rather than an installed `reportage` found in the ambient environment. For future coverage-aware adapters, the runner must also be able to route a command name through adapter-provided execution behavior.

For the canonical shim model, see [../shims.md](../shims.md).

## Decision

reportage uses runner-owned PATH overlay shim injection as the command execution foundation.

PATH overlay shim injection is the model for how the runner controls command
resolution for `$` action steps while leaving action text user-facing.

The initial shim materialization strategy is POSIX shell wrappers. Native Windows wrapper generation is out of scope. Windows users should use WSL, a devcontainer, or Linux-based CI.

Execution behavior, use cases, executable invocation targets, and observability
are defined in [../shims.md](../shims.md). PATH prefix mechanics are defined in
[../execution-model.md](../execution-model.md).

## Alternatives Considered

### Write harness-specific executable paths in `.repor` files

This would make the executed target explicit without PATH overlay shims.

It is not suitable because it makes scripts machine-specific and prevents adapters from mediating command execution by command name. It also weakens the readability of E2E scripts: the script should describe what command is being tested, not where a particular local build artifact happens to live.

Decision: rejected as the target model.

### Use runner-owned PATH overlay wrappers

This keeps `.repor` files user-facing while allowing the runner, harness, or adapter to control the executable invocation behind a command name.

It also creates a shared foundation for:

- reportage self-testing;
- coverage-aware CLI adapters;
- future runtime-specific execution adapters.

Decision: accepted.

## Consequences

### Positive Consequences

- Self-tests can be written in the same style as ordinary user-facing E2E scripts.
- The harness can run the Cargo-built reportage executable without exposing harness mechanics in the script.
- The same command-resolution model can support future coverage-aware adapters.
- Command names remain stable even when the execution mechanism changes.

### Negative Consequences

- PATH shadowing can make failures harder to diagnose if the resolved shim and target are not recorded.
- The runner must eventually expose enough diagnostics or artifacts to show which shim and target were used.
- The implementation must avoid recursive wrappers by using absolute target paths.

### Neutral Consequences

- POSIX shell execution remains a v0 assumption.
- Cross-platform wrapper generation is intentionally out of scope.
- Coverage collection still depends on runtime-specific adapters and cannot be guaranteed for every target.
