# Self-testing

This document describes how reportage should test representative reportage CLI behavior with reportage scripts.

For the architectural decision behind the command-resolution model, see [ADR: Use PATH Overlay Shims for Command Resolution](adr/20260628T061500Z_path-overlay-shims-for-command-resolution.md).

## Goal

v0.1.0 aims to make reportage capable of describing and running its own CLI-level E2E tests.

This is representative self-testing, not full self-hosting.

Self-tests should verify user-visible CLI behavior while keeping lower-level validation in Rust tests:

- Rust unit tests verify parser, domain model, validation, and internal error classification.
- Rust E2E harnesses bootstrap execution of self-test scripts.
- reportage self-tests describe representative CLI-level behavior.

## Target style

Self-tests should use the same command form that a user-facing E2E script would use.

Preferred:

```reportage
case "with --help" {
  $ reportage --help

  assert {
    exit 0
    stdout contains "Usage: reportage [OPTIONS] [SUBCOMMAND]..."
  }
}
```

The `.repor` file should say which command is being tested. It should not expose where the harness found the executable.

## Bootstrap-only style

During early bootstrap work, a self-test may use a harness-provided executable path:

```reportage
case "with --help" {
  $ $REPORTAGE_BIN --help

  assert {
    exit 0
  }
}
```

This is allowed only as an implementation bridge. It is not the target style because it leaks harness-specific mechanics into the `.repor` file.

## Command resolution

Self-testing uses same-name command interception, which is one specific application of the general PATH overlay shim injection model.

In same-name command interception, the harness places a wrapper with the same name as the command under test (`reportage`) in a runner-owned directory, and prepends that directory to `PATH`. The POSIX shell then resolves `reportage` through the wrapper rather than any ambient installation.

For the general PATH overlay shim injection model, see [execution-model.md](execution-model.md).

Same-name command interception is not the only application of PATH overlay shims. Ordinary application E2E tests may use the same mechanism to place an entrypoint wrapper for the system under test, even when the command name in the `.repor` file is not `reportage`.

A Rust E2E harness for reportage self-testing may:

1. resolve the Cargo-built reportage executable;
2. create a POSIX wrapper named `reportage` in a runner-owned directory;
3. place that directory before the inherited `PATH`;
4. execute the self-test script normally;
5. let the POSIX shell resolve `reportage` through the wrapper.

In that model, the script remains:

```reportage
$ reportage --help
```

while the harness controls which executable invocation is used.

## Shim targets

A shim target is an executable invocation, not merely a binary path.

An executable invocation may be:

- a native executable;
- an executable script with a shebang;
- an interpreter/script invocation, such as `ruby tool.rb` or `node cli.js`.

This distinction matters because reportage should not encode Rust-native binaries as the only shape of command execution. The same command-resolution model should be usable by adapters for other runtimes.

## Coverage

PATH overlay command resolution and coverage collection are related but distinct.

The PATH overlay decides which executable invocation runs when a script writes a command name. Coverage still depends on the relevant runtime or adapter.

For reportage self-testing, the Rust harness can route `reportage` to the Cargo-built executable used under the coverage run. For other runtimes, adapters may need runtime-specific mechanisms, such as coverage environment variables, bootstraps, agents, or report finalizers.

A target may be runnable through reportage even when coverage collection is unavailable.

## Representative cases

v0.1.0 should contain at least two, and preferably three, representative self-tests.

Useful cases include:

- a success path, such as `reportage --help`;
- a failure path, such as an unknown option and asserted diagnostics;
- an artifact or evidence-output path.

These self-tests should complement, not replace, Rust unit and integration tests.
