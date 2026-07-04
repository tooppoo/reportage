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

## Artifact / evidence self-testing

[`e2e/artifacts/file-assertion-evidence.repor`](../e2e/artifacts/file-assertion-evidence.repor)
is the representative artifact / evidence-output self-test. It:

1. runs a nested `reportage` invocation against a small inner script;
2. asserts the nested run's process-level behavior (`exit 0`);
3. asserts that the nested run's `result.json` artifact file exists, using
   `file "<path>" exists`;
4. asserts that `result.json` contains a stable marker, using
   `file "<path>" contains "<text>"`.

The marker asserted (`"result": "pass"`) is a field name and enum-like value
from the artifact schema, not a timestamp, absolute path, or
platform-specific string, so it stays stable across runs and machines.

The nested invocation uses the hidden `--debug-run-id <id>` option so its
artifact path is deterministic (`.reportage/runs/<id>/result.json`) instead
of the normal millisecond-timestamp run directory. `--debug-run-id` is an
internal self-testing / development affordance, not a public stable CLI
option — see [`docs/TBD.md`](TBD.md) — "Self-test run ID control".

The self-test removes any previous `.reportage/runs/<id>` directory for its
own fixed id before invoking the nested run, so repeated local runs do not
collide with a stale directory from an earlier run. A fixed run id that
does resolve to an existing run directory is a distinct, separately-tested
runner behavior (the runner refuses to silently overwrite it); see the
`for_fixed_run_rejects_existing_run_directory` unit test in
`crates/reportage-core/src/artifact.rs` and the
`debug_run_id_does_not_silently_overwrite_existing_run_directory`
integration test in `crates/reportage-cli/tests/integration_test.rs`.
