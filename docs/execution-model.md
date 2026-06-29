# Execution model

This document describes how reportage executes action steps.

For the shim model used for command resolution, see [shims.md](shims.md).
For the decision rationale, see [ADR: Use PATH Overlay Shims for Command Resolution](adr/20260628T061500Z_path-overlay-shims-for-command-resolution.md).

## Overview

When reportage executes a `$` action step, it invokes the POSIX shell with `sh -c <command>`.

The shell resolves command names according to `PATH`. reportage does not parse
action command text to infer command resolution.

## PATH prefix injection

The runner can inject one or more runner-owned directories into the front of
`PATH` before each action is executed.

The runner maintains an ordered list of PATH prefix directories in the `ExecutionEnvironment`.

- Prefixes are prepended to the inherited `PATH` in the given order.
- For example, prefixes `[A, B]` produce `PATH=A:B:<inherited PATH>`.
- If the inherited `PATH` is absent or empty, the effective `PATH` contains only the provided prefixes.
- When no prefixes are configured, the action shell inherits `PATH` from the current process without modification.

Shell selection remains separate from PATH prefix injection. The runner invokes
`sh -c` to execute action commands, and the shim PATH is applied only to command
resolution within that shell.

For shim roles, executable invocation targets, self-testing interception,
application entrypoint shims, coverage-aware adapters, and shim invocation
observability, see [shims.md](shims.md).
