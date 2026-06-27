# Semantics

This document describes the intended v0 execution semantics for reportage scripts.

For syntax, see [syntax.md](syntax.md).

## Core model

A reportage script file is a test module.

A module contains:

```text
before_each? case*
```

A `case` without `params` produces one concrete case.

A `case` with `params` produces one concrete case per `variant`.

For each concrete case, reportage creates an isolated execution environment, runs setup and case steps, evaluates assertions, collects artifacts, and then removes the workspace unless preservation is explicitly requested for debugging.

## Concrete case expansion

A non-parameterized case:

```reportage
case "check" {
  $ rellog check
  assert exit 0
}
```

produces one concrete case:

```text
check
```

A parameterized case:

```reportage
case "check output" {
  params {
    variant "human" {
      ARGS = ""
    }

    variant "json" {
      ARGS = "--json"
    }
  }

  $ rellog check ${ARGS}
  assert exit 0
}
```

produces:

```text
check output / human
check output / json
```

v0 parameterization is case-local. Module-scope parameter definitions are intentionally not part of v0.

## Execution order

For each concrete case:

1. Create an isolated case workspace.
2. Create a case-local `bin` directory for PATH shims.
3. Generate registered command shims in the case-local `bin` directory.
4. Prepend the case-local `bin` directory to `PATH`.
5. Apply variant bindings, if the case is parameterized.
6. Run `before_each` steps, if the module defines `before_each`.
7. Run the concrete case body.
8. Evaluate assertions as they appear.
9. Collect command results, logs, and coverage artifacts.
10. Destroy the case workspace unless preservation is enabled.

`before_each` and `case` steps run in the same concrete case workspace.

Files created by `before_each` and files created by the `case` are both isolated to that concrete case and are discarded with the workspace.

## Workspace lifecycle

Each concrete case receives its own workspace.

The workspace is the root for:

- files written by `file` steps;
- commands executed by `$` steps, including ordinary shell filesystem operations such as `mkdir`, `cp`, `mv`, and `rm`;
- file assertions;
- temporary runtime artifacts produced by the system under test.

The exact internal layout is implementation-defined. A typical implementation may use:

```text
<case-root>/
  work/       # command working directory
  bin/        # PATH shims
  coverage/   # raw coverage artifacts
  artifacts/  # logs and preserved outputs
```

The user-facing working directory for `$` steps is the case workspace working directory, not the repository root.

## `before_each`

`before_each` is module-level setup.

It is executed before every concrete case, including every concrete case produced by parameter variants.

v0 rules:

- `before_each` is optional.
- At most one `before_each` block is allowed per module.
- `before_each` must appear before any `case` block.
- `before_each` is not shared state; it is replayed inside each concrete case workspace.
- v0 does not provide `before_all`, `after_all`, or `after_each`.

Recommended v0 semantic restriction:

- `before_each` should be deterministic module-level setup.
- `before_each` may use `file` steps and ordinary shell setup steps such as `$ mkdir -p ...` or `$ cp -R ... ...`.
- Primary system-under-test actions and assertions should usually live in `case` blocks.
- Variant-specific setup should usually live in the parameterized `case`, not in `before_each`.

This keeps `before_each` independent of case-local parameter context.

## Parameter bindings

A `variant` defines bindings:

```reportage
variant "json" {
  ARGS = "--json"
  EXPECT_EXIT = "0"
}
```

Bindings are available to the concrete case generated from that variant.

v0 treats binding values as strings.

Implementations may expose bindings to `$` steps as environment variables. This allows shell expansion:

```reportage
$ rellog check ${ARGS}
```

The same bindings may also be used by reportage-level expansion where enabled:

```reportage
assert exit ${EXPECT_EXIT}
```

and by explicit template heredocs:

```reportage
file ".rellog/entries/001.kdl" template <<'KDL'
entry "entry" {
  kind "${ENTRY_KIND}"
}
KDL
```

Raw file heredocs do not perform parameter expansion.

## Shell execution

A `$` step is executed by a POSIX shell.

```reportage
$ rellog check --json | jq .
```

The runner does not rewrite arbitrary shell syntax in v0. The shell is responsible for interpreting pipelines, redirections, variable expansion, conditionals, filesystem operations, and other shell constructs.

For fixture copying and ordinary file operations, use shell commands:

```reportage
$ mkdir -p .rellog/entries
$ cp -R fixtures/${FIXTURE}/. .
```

Native Windows shell execution is out of scope for v0. Windows users should use WSL, a devcontainer, or Linux-based CI.

## Command resolution through PATH shims

reportage uses PATH shims to let adapters mediate command execution.

For each concrete case, the runner creates a case-local `bin` directory and prepends it to `PATH`. Registered commands are represented by executable shim files in that directory.

A script can write:

```reportage
$ rellog check --json
```

The POSIX shell resolves `rellog` via `PATH`. If `rellog` is a registered command, the case-local shim is executed.

The shim decides how to run the actual system under test.

Examples:

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

The runner does not need to know those language-specific details.

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

## Coverage adapter lifecycle

reportage does not implement coverage measurement.

Coverage-aware execution is delegated to adapters. A typical adapter lifecycle is:

1. Prepare coverage-aware command execution.
2. Generate or provide PATH shims for registered commands.
3. Let the runner execute concrete cases.
4. Collect raw coverage artifacts.
5. Finalize coverage reports, such as LCOV, Cobertura, or HTML.

The runner's responsibility is orchestration:

- create isolated workspaces;
- install adapter-provided shims;
- execute scripts;
- preserve raw artifacts as needed;
- call adapter finalization.

The adapter's responsibility is runtime-specific coverage behavior:

- instrumented binaries;
- runtime environment variables;
- coverage bootstraps;
- JVM agents;
- source-map remapping;
- report generation.

## Coverage capability is not universal

Some targets may not support coverage collection.

Examples:

- a remote staging service;
- an already-running external server;
- a process that cannot be started through a shim;
- a service that is killed before coverage data is flushed;
- a runtime without usable coverage tooling.

reportage should still be useful as a runtime-independent E2E runner when coverage is disabled or unavailable.

Future implementations may distinguish modes such as:

```text
coverage = required
coverage = best-effort
coverage = off
```

v0 should treat coverage integration as adapter capability, not a guarantee of every target.

## Assertions

Assertions are evaluated in order.

Process assertions such as `assert exit`, `assert stdout`, and `assert stderr` refer to the most recent `$` step.

File assertions such as `assert file exists` and `assert file-count` refer to the current concrete case workspace.

If a process assertion appears before any `$` step in the current concrete case, that is a script error.

## jq assertions

`assert ... jq ...` uses external `jq` in v0.

The runner should fail clearly if a jq assertion is used and `jq` is unavailable.

Example diagnostic intent:

```text
error: `assert stdout jq` requires external jq, but jq was not found in PATH
```

Embedded jq engines may be considered later. If added, the selected jq engine should be explicit rather than silently falling back between implementations.

## Cleanup and preservation

By default, each concrete case workspace is destroyed after the case finishes.

Implementations should provide a debug mode to preserve failed workspaces, but preservation is not part of the DSL syntax.

Because the runner owns workspace cleanup, v0 does not need `after_each` for file cleanup.

If future use cases require external cleanup, such as stopping services or collecting extra artifacts, `after_each` may be reconsidered.

## v0 exclusions

The following are intentionally outside v0:

- native Windows shell execution;
- dedicated `copy` syntax;
- `before_all`;
- `after_all`;
- `after_each`;
- module-scope parameters;
- embedded jq;
- hidden fixture namespaces such as `@fixture`;
- full shell parsing or shell rewriting;
- browser automation syntax;
- service lifecycle syntax.

Some of these may be added later if concrete use cases justify them.
