# Shims

This document describes the role of shims in reportage.

For the detailed shim invocation event protocol, see [Shim event protocol](shim-event-protocol.md).

## Overview

reportage executes `$` actions through the POSIX shell with `sh -c <command>`. The shell resolves command names using `PATH`.

A shim is a runner-owned executable placed in a PATH prefix directory. The shim lets a command name written in a `.repor` script resolve to a controlled executable invocation.

```repor
case "with --help" {
  $ reportage --help

  assert {
    exit 0
  }
}
```

The script says `reportage`, while the runner may arrange for that command name to resolve to a shim.

## Purpose

Shims allow command resolution control without changing action text.

Use cases include:

- self-testing reportage with user-facing command text;
- application E2E entrypoint shims;
- future coverage-aware adapters;
- avoiding local build artifact paths in `.repor` scripts.

## PATH overlay shims

The runner prepends one or more runner-owned directories to `PATH`. Normal shell command resolution applies after that. A shim named `reportage` in the prefix directory shadows an ambient `reportage` found later on `PATH`.

This is command resolution control, not shell selection. For the low-level `ExecutionEnvironment` PATH behavior, see [Execution model](execution-model.md).

## Shim interception limits

PATH shims intercept registered commands only when shell PATH resolution is used.

These forms are interceptable:

```reportage
$ rellog check
$ RUST_LOG=debug rellog check
$ rellog check --json | jq .
$ cd subdir && rellog check
```

These forms are not guaranteed to be intercepted:

```reportage
$ ./rellog check
$ /usr/local/bin/rellog check
$ command rellog check
```

Scripts that want coverage-aware command execution should call registered commands by their registered names.

## Use cases

### Self-testing

Self-testing uses same-name command interception:

1. The `.repor` file contains `$ reportage --help`.
2. The harness resolves the Cargo-built reportage binary.
3. The harness creates a shim named `reportage`.
4. The harness prepends the shim directory to `PATH`.
5. The shell resolves `reportage` through the shim.

For self-testing policy and representative cases, see [Self-testing](../design/testing/self-testing.md).

### Application E2E testing

Same-name interception is not the only use case. Ordinary application tests may use a shim as a test-facing entrypoint, and the command name in the `.repor` file does not need to already exist in the ambient environment.

```repor
case "shows version" {
  $ myapp --version

  assert {
    exit 0
  }
}
```

The runner or adapter may provide a `myapp` shim that delegates to the intended application invocation.

### Coverage-aware adapters

PATH overlay shims and coverage collection are related but distinct. Shims decide which executable invocation runs. Runtime-specific coverage setup remains adapter responsibility.

A target may be runnable even when coverage is unavailable.

`reportage shim scaffold` (see [Shim scaffold](shim-scaffold.md)) generates a starting-point adapter script from a static template, but the generated file is then a project-owned artifact: reportage takes no further role in it once it is written.

## Executable invocation targets

A shim target is an executable invocation, not merely a binary path.

An executable invocation consists of:

- program path;
- zero or more fixed args;
- caller-provided args forwarded by the shim.

Examples:

- `/absolute/path/to/myapp`
- `/absolute/path/to/ruby /absolute/path/to/tool.rb`
- `/absolute/path/to/node /absolute/path/to/cli.js`

Conceptually, a POSIX wrapper delegates like this:

```sh
#!/bin/sh
exec '/absolute/path/to/program' 'fixed-arg-1' 'fixed-arg-2' "$@"
```

Absolute program paths avoid recursive wrapper invocation.

## Shim target examples

The shim decides how to run the actual system under test. reportage does not need to know those language-specific details; it only needs the shim to exist and be executable.

```text
Rust adapter:
  exec /path/to/coverage-instrumented/rellog "$@"

Node adapter:
  export NODE_V8_COVERAGE="$E2E_COVERAGE_DIR/node"
  exec node --enable-source-maps "$PROJECT_ROOT/dist/cli.js" "$@"

Ruby adapter:
  exec ruby -r "$BOOTSTRAP/simplecov.rb" "$PROJECT_ROOT/exe/mycli" "$@"

JVM adapter:
  exec java -javaagent:"$JACOCO_AGENT=destfile=$E2E_COVERAGE_DIR/jacoco.exec" \
    -cp "$INSTRUMENTED_CLASSPATH" com.example.Main "$@"
```

## Shim invocation observability

Shims can make failures harder to diagnose unless invocation metadata is recorded. reportage must not parse action command text to infer shim usage: command resolution belongs to the shell/runtime environment.

Protocol-compliant shims report invocation events through a runner-provided side channel. The runner attaches observed shim invocation metadata to action results, diagnostics, and artifacts.

For the protocol, see [Shim event protocol](shim-event-protocol.md).

## Observed metadata, not complete resolution tracing

Shim invocation metadata is observed evidence. If a protocol-compliant shim writes an event, reportage can record the invocation. If metadata is absent, no protocol-compliant shim invocation was observed.

Absence does not prove that no shim, wrapper, or ambient command was involved. Third-party or non-compliant shims may run without producing observable metadata.

## Error handling

Runner-generated shims should write invocation event data before delegating. If event writing fails, they should emit a prefixed stderr diagnostic such as `reportage shim warning:` and continue delegating to the target invocation.

These diagnostics remain observable stderr. reportage does not automatically filter them out from stdout/stderr assertions. A dedicated diagnostic side channel is deferred.

## Non-goals

- Do not parse action command text to infer resolution.
- Do not guarantee a complete PATH resolution trace.
- Do not implement runtime-specific coverage collection in the core runner.
- Do not support every third-party shim without protocol compliance.
- Do not add cross-platform wrapper generation in the initial POSIX-focused model.

## Related documents

- [Execution model](execution-model.md)
- [Self-testing](../design/testing/self-testing.md)
- [Shim event protocol](shim-event-protocol.md)
- [Shim scaffold](shim-scaffold.md) — generating a coverage-integration shim file from a static template, a distinct concern from the PATH-overlay shims described in this document
