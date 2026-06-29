# Self-testing

This document describes how reportage should test representative reportage CLI behavior with reportage scripts.

For the architectural decision behind the command-resolution model, see [ADR: Use PATH Overlay Shims for Command Resolution](adr/20260628T061500Z_path-overlay-shims-for-command-resolution.md).
For the shim model, see [shims.md](shims.md).

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

## Command resolution

Self-testing uses same-name command interception, which is one application of
the general PATH overlay shim model.

A Rust E2E harness for reportage self-testing may:

1. resolve the Cargo-built reportage executable;
2. create a POSIX shim named `reportage` in a runner-owned directory;
3. place that directory before the inherited `PATH`;
4. execute the self-test script normally;
5. let the POSIX shell resolve `reportage` through the shim.

In that model, the script remains:

```reportage
$ reportage --help
```

while the harness controls which executable invocation is used.

For the general shim model, executable invocation targets, and shim invocation
observability, see [shims.md](shims.md).

## Coverage

For reportage self-testing, the Rust harness can route `reportage` to the
Cargo-built executable used under the coverage run.

For the general distinction between PATH overlay command resolution and
runtime-specific coverage collection, see [shims.md](shims.md).

## Representative cases

v0.1.0 should contain at least two, and preferably three, representative self-tests.

Useful cases include:

- a success path, such as `reportage --help`;
- a failure path, such as an unknown option and asserted diagnostics;
- an artifact or evidence-output path.

These self-tests should complement, not replace, Rust unit and integration tests.
