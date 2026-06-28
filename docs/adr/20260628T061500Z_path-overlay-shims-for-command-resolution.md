# Use PATH Overlay Shims for Command Resolution

- Status: Accepted
- Created: 2026-06-28T06:15:00Z

## Context

reportage v0.1.0 aims to make reportage capable of describing and running representative CLI-level E2E tests for reportage itself.

A bootstrap self-test can pass the executable path through a harness-specific environment variable:

```reportage
case "with --help" {
  $ $REPORTAGE_BIN --help

  assert {
    exit 0
  }
}
```

This works as an implementation bridge, but it exposes harness mechanics in the `.repor` file. It also makes self-tests look different from ordinary user-facing E2E scripts.

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

## Decision

reportage will use runner-owned PATH overlay shims as the target command resolution model.

A reportage script should write the command name as the user understands it, such as `reportage`, `rellog`, or another CLI command under test. The runner, harness, or adapter may create an executable shim in a runner-owned directory and place that directory before the inherited `PATH` before executing `$` actions through the POSIX shell.

In the target self-testing model:

1. The `.repor` file writes `reportage` as a bare command.
2. The Rust E2E harness resolves the Cargo-built reportage executable.
3. The harness creates a POSIX wrapper named `reportage` in a runner-owned directory.
4. The harness prepends that directory to `PATH` when running self-tests.
5. The shell resolves `reportage` through the wrapper.
6. The wrapper `exec`s the intended executable invocation and forwards all arguments.

The initial shim materialization strategy is POSIX shell wrappers. Native Windows wrapper generation is out of scope. Windows users should use WSL, a devcontainer, or Linux-based CI.

A shim target must be modeled as an executable invocation, not merely as a binary path. An executable invocation may be:

- a native executable;
- an executable script with a shebang;
- an interpreter and script invocation, such as `ruby tool.rb` or `node cli.js`.

Runtime-specific coverage collection remains adapter responsibility. PATH overlay shims control command resolution; they do not by themselves define how coverage is collected or finalized.

## Alternatives Considered

### Use `$REPORTAGE_BIN` directly in self-tests

This is useful as a bootstrap step because it makes the target executable explicit and works before PATH overlay shims exist.

It is not the target model because it leaks harness-specific details into `.repor` files. It also prevents self-tests from exercising the same command-resolution path that user-facing tests and future adapters should use.

Decision: allowed only as a bootstrap mechanism.

### Write absolute executable paths in `.repor` files

This would make the executed target explicit without environment variables.

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
- The harness can run the Cargo-built reportage executable without exposing `$REPORTAGE_BIN` in the script.
- The same command-resolution model can support future coverage-aware adapters.
- Command names remain stable even when the execution mechanism changes.
- Native executables, executable scripts, and interpreter/script invocations fit the same conceptual model.

### Negative Consequences

- PATH shadowing can make failures harder to diagnose if the resolved shim and target are not recorded.
- The runner must eventually expose enough diagnostics or artifacts to show which shim and target were used.
- The implementation must avoid recursive wrappers by using absolute target paths.

### Neutral Consequences

- POSIX shell execution remains a v0 assumption.
- Cross-platform wrapper generation is intentionally out of scope.
- Coverage collection still depends on runtime-specific adapters and cannot be guaranteed for every target.
