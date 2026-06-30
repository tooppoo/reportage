# Semantics

This document describes the intended v0 execution semantics for reportage scripts.

For syntax, see [syntax.md](syntax.md).

## Suite pre-execution validation

When running a suite of test files (config-driven or multi-script explicit mode), reportage performs a validation phase before executing any `$` actions.

The validation phase:

1. Read each selected file. A file that cannot be read produces a `read_error`.
2. Parse each successfully-read file. A file that cannot be parsed produces a `parse_error`.
3. Collect all file-level errors from the full set of selected files.
4. If any file has a read or parse error, abort before executing any `$` actions.

All file-level errors are reported in a single run. The run exits with code `2` and the artifact result is `script_error`.

If the validation phase passes with no errors, execution proceeds normally across all files.

See ADR 20260628T000000Z_validate-before-execute for the rationale.

## Core model

A reportage script file is a test module.

A module contains:

```text
before_each? case*
```

A `case` without `params` produces one concrete case.

A `case` with `params` produces one concrete case per `variant`.

For each concrete case, reportage creates an isolated execution environment, runs setup and case steps, evaluates assertion blocks, collects artifacts, and then removes the workspace unless preservation is explicitly requested for debugging.

## Concrete case expansion

A non-parameterized case:

```reportage
case "check" {
  $ rellog check
  assert {
    exit 0
  }
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

  assert {
    exit 0
    stderr empty
  }
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
7. Establish the initial checkpoint.
8. Run the concrete case body steps in source order.
9. Collect command results, logs, and coverage artifacts.
10. Destroy the case workspace unless preservation is enabled.

`before_each` and `case` steps run in the same concrete case workspace.

Files created by `before_each` and files created by the `case` are both isolated to that concrete case and are discarded with the workspace.

## Workspace lifecycle

Each concrete case receives its own workspace.

The workspace is the root for:

- files written by `file` steps;
- commands executed by `$` steps, including ordinary shell filesystem operations such as `mkdir`, `cp`, `mv`, and `rm`;
- file and directory expectations;
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
- Primary system-under-test actions and assertion blocks should usually live in `case` blocks.
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

The same bindings may also be used in expectation arguments where expansion is enabled:

```reportage
assert {
  exit ${EXPECT_EXIT}
}
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

## Action

An action is written as `$ ...` and represents an operation performed against the target system or its surrounding environment.

In v0, actions are executed through a POSIX-compatible shell (`sh -c`).

When an action completes, the current checkpoint is updated with the action result (exit status, stdout, stderr) and the post-action workspace state.

## Assertion block

An assertion block is written as `assert { ... }` and is a checkpoint-level verification construct.

An assertion block is not attached to the nearest preceding action. It verifies the **current checkpoint** — whatever evidence is observable at the point in the case body where the block appears.

Semantics:

- All expectations within a block are evaluated independently.
- Failures are reported per expectation.
- If one or more expectations fail, the block is a failure.
- After a block failure, the same concrete case does not proceed to its next action. The runner may proceed to the next concrete case.
- An assertion block is side-effect-free. It does not modify the checkpoint.

## Expectation

An expectation is an individual expected condition within an assertion block.

Examples: `exit 0`, `stderr empty`, `dir exists .rellog`, `file exists .rellog/config.yml`.

Each expectation has an evidence requirement that determines what checkpoint state must be available for it to be evaluated. Expectations are side-effect-free. Failures are reported per expectation, independently of other expectations in the same block.

## Checkpoint

A checkpoint is the observable evidence context available at a point in case execution.

A checkpoint is not a full filesystem snapshot. It is a reference to the evidence needed to evaluate the expectations in an assertion block.

Checkpoints are maintained by the runner as it processes case body steps.

## Initial checkpoint

The initial checkpoint is established after the case workspace is created, `before_each` has run, and before the first step of the case body executes.

The initial checkpoint has:

- workspace state (the current case workspace, including any files written by `before_each`);
- no last action result.

Workspace expectations (`dir exists`, `file exists`, etc.) are valid at the initial checkpoint.

Process expectations (`exit`, `stdout`, `stderr`) require a last action result. Using a process expectation in an assertion block at the initial checkpoint is a **script error**.

## Action-updated checkpoint

After a `$ ...` action completes, the checkpoint is updated with:

- the action result (exit status, stdout, stderr);
- the post-action workspace state.

Subsequent assertion blocks reference this updated checkpoint until the next action updates it again.

## Evidence requirement

Different expectations require different evidence from the current checkpoint.

### Workspace expectations

Require only workspace state. Valid at the initial checkpoint.

- `dir exists <path>`
- `dir not exists <path>`
- `file exists <path>`
- `file contains <path> <string>`
- `file-count <glob> <op> <n>`

### Process expectations

Require the last action result. A script error if used at a checkpoint with no last action result (i.e., before any `$` action in the same case).

- `exit <code>`
- `stdout empty`
- `stdout contains <string>`
- `stderr empty`
- `stderr contains <string>`

### Structured output expectations

Require the corresponding process output from the last action result.

- `stdout jq <expression>`
- `stderr jq <expression>`

In v0, structured output expectations use external `jq`.

## Example: checkpoint model in action

```reportage
case "init creates workspace" {
  assert {
    dir not exists .rellog
  }

  $ rellog init

  assert {
    exit 0
    dir exists .rellog
    file exists .rellog/config.yml
  }
}
```

Walkthrough:

- The first `assert { ... }` block evaluates the **initial checkpoint**.
- `dir not exists .rellog` is a workspace expectation and is valid at the initial checkpoint.
- `$ rellog init` executes the action and updates the checkpoint with the action result and post-action workspace state.
- The second `assert { ... }` block evaluates the **action-updated checkpoint**.
- `exit 0` is a process expectation and requires the last action result — valid because `$ rellog init` has run.
- `dir exists .rellog` and `file exists .rellog/config.yml` are workspace expectations and observe the post-action workspace state.

## Example: script error — process expectation at initial checkpoint

```reportage
case "invalid initial process expectation" {
  assert {
    exit 0
  }
}
```

This is a script error. The initial checkpoint has no last action result, so `exit 0` — a process expectation — cannot be evaluated.

## jq assertions

`assert ... jq ...` uses external `jq` in v0.

The runner should fail clearly if a jq expectation is used and `jq` is unavailable.

Example diagnostic intent:

```text
error: `stdout jq` requires external jq, but jq was not found in PATH
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
