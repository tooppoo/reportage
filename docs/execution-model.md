# Execution model

This document describes the PATH overlay shim injection model used by reportage for action command resolution.

For the decision rationale, see [ADR: Use PATH Overlay Shims for Command Resolution](adr/20260628T061500Z_path-overlay-shims-for-command-resolution.md).

For the self-testing use case, see [self-testing.md](self-testing.md).

## Overview

When reportage executes a `$` action step, it invokes the POSIX shell with `sh -c <command>`. The shell resolves command names according to the `PATH` environment variable.

The runner can inject one or more runner-owned directories into the front of `PATH` before each action is executed. This lets the runner control which executable invocation is used for a command name, without changing anything in the `.repor` script.

## PATH prefix injection

The runner maintains an ordered list of PATH prefix directories in the `ExecutionEnvironment`.

- Prefixes are prepended to the inherited `PATH` in the given order.
- For example, prefixes `[A, B]` produce `PATH=A:B:<inherited PATH>`.
- If the inherited `PATH` is absent or empty, the effective `PATH` contains only the provided prefixes.
- When no prefixes are configured, the action shell inherits `PATH` from the current process without modification.

Shell selection remains separate from PATH prefix injection. The runner always invokes `sh -c` to execute action commands, and the shim PATH is applied only to command resolution within that shell.

## Shim targets

A shim target is an executable invocation, not merely a binary path.

An executable invocation may be:

- a native executable;
- an executable script with a shebang;
- an interpreter and script invocation, such as `ruby tool.rb` or `node cli.js`.

A runner-owned wrapper script placed in a PATH prefix directory can delegate to any of these invocation forms.

## Use cases

PATH overlay shim injection is the general mechanism. Specific use cases build on top of it:

- **reportage self-testing**: same-name command interception, where the runner creates a wrapper named `reportage` so that `$ reportage ...` steps in `.repor` files resolve to the Cargo-built binary rather than any ambient installation. See [self-testing.md](self-testing.md).
- **application E2E testing**: the runner places an entrypoint wrapper for the system under test in a prefix directory so that `.repor` files can refer to it by command name.
- **coverage-aware adapters**: a wrapper can route command invocations through a runtime-specific coverage tool. PATH overlay controls command resolution; coverage collection remains the adapter's responsibility.
