# Execution model

This document describes reportage's runtime execution model: how a module's concrete cases are planned and run, how the case workspace and checkpoint evolve during a case, and how the runner hands off to shims and coverage adapters.

For syntax, see [the generated syntax reference](syntax.md).
For language semantic rules — value literals, expectations, assertion evaluation, logical composition, and diagnostics for individual expectations — see [Language semantics](semantics.md).
For the shim model used for command resolution (shim purpose, shim target, event protocol, and observability), see [Shims](shims.md).
For the decision rationale behind PATH overlay shims, see [ADR: Use PATH Overlay Shims for Command Resolution](../adr/20260628T061500Z_path-overlay-shims-for-command-resolution.md).

## Core model

A reportage script file is a test module.

A module contains:

```text
before_each? case*
```

A `case` without `params` produces one concrete case.

A `case` with `params` produces one concrete case per `variant`.

For each concrete case, reportage creates an isolated execution environment, runs setup and case steps, evaluates assertion blocks, collects artifacts, and then removes the workspace unless preservation is explicitly requested for debugging.

## Parsing and the source-level model

Parsing a script yields a source-level model (`SourceFile`), not the execution model directly.
The source-level model associates each parsed case with the original source text and the case block's byte range within it, so source-oriented consumers (such as documentation features) can recover a case's source after parsing.
It also carries the file's `document file` metadata when the source declares one, and each case's `document case` metadata when a block precedes that case (see [Language semantics](semantics.md) — Document block).
A document block is not part of any case span, and neither are the blank lines or comment lines between a block and the case that follows it; each case span remains exactly the pest `case_block` pair's range.
The suite loader projects the source-level model into the execution `Script` before execution; executors, evaluators, and artifact writers depend only on the execution model, and no source-level information — documentation metadata included — appears in execution reports or artifacts.

See [ADR: Parser Returns a Source-Level Model Instead of the Execution Script](../adr/20260712T090000Z_parser-returns-source-level-model.md) for the rationale and the case span contract.

## Suite pre-execution validation

When running a suite of test files (config-driven or multi-script explicit mode), reportage performs a validation phase before executing any `$` actions.

The validation phase:

1. Read each selected file. A file that cannot be read produces a `read_error`.
2. Parse each successfully-read file. A file that cannot be parsed produces a `parse_error`.
3. Collect all file-level errors from the full set of selected files.
4. If any file has a read or parse error, abort before executing any `$` actions.

All file-level errors are reported in a single run. The run exits with code `2`, and the artifact manifest records `status: "error"` with one `diagnostics[]` entry per file-level error (`category: "parse"` for parse errors, `category: "internal"` for read errors; see [Artifacts](artifacts.md)).

If the validation phase passes with no errors, execution proceeds normally across all files.

See [ADR: Validate Before Execute](../adr/20260628T000000Z_validate-before-execute.md) for the rationale.

## Empty and zero-case scripts

Empty and whitespace-only scripts are syntax-valid inputs. Syntax validity only means the file can be parsed as a reportage script; it does not imply that execution has work to do.

When selected input is valid but produces zero concrete cases, the runner treats the run as a no-op success:

- the CLI exits with code `0`;
- no `$` command is executed;
- no checkpoint is generated;
- no assertion is evaluated;
- no case, checkpoint, or evidence artifacts are generated;
- human-readable CLI output states that no cases were found;
- the run result manifest records `noop: true` and a zeroed summary.

See [ADR: Empty and Whitespace Scripts Are a No-Op Success](../adr/20260703T000000Z_empty-and-whitespace-scripts-are-no-op-success.md) for the rationale.

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

Bindings declared by a `variant` (see [semantics.md — Parameter bindings](semantics.md#parameter-bindings)) are available to the concrete case generated from that variant.

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

- files written by `write` steps (see [semantics.md — Write step](semantics.md#write-step));
- commands executed by `$` steps, including ordinary shell filesystem operations such as `mkdir`, `cp`, `mv`, and `rm`;
- file and directory expectations (see [semantics.md — File assertions](semantics.md#file-assertions) and [Directory assertions](semantics.md#directory-assertions));
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

`before_each` is module-level case-local setup.

It is executed before every concrete case, including every concrete case produced by parameter variants.

~~~reportage
before_each {
  write <"reportage.kdl"> ```
    reportage {
      config {
        version 1
      }
    }
    ```
}

case "invalid config" {
  $ reportage validate

  assert {
    exit 2
    stderr contains "unsupported config version"
  }
}
~~~

v0 rules:

- `before_each` is optional.
- At most one `before_each` block is allowed per module (`parse.before_each.duplicate`).
- `before_each` must appear before any `case` block, and must not separate a `document case` block from its target case (`parse.before_each.after_case`).
- `before_each` is not shared state; it is replayed inside each concrete case workspace.
- Workspace state `before_each` produces — files it writes, and files its actions create — exists before the case body's first step, and is workspace evidence at the case body's initial checkpoint (see "Initial checkpoint" below).
- A `before_each` step that fails at runtime is a runtime step error for that concrete case; the case body does not run.
- A `before_each` assertion failure fails that concrete case; the case body does not run. Other concrete cases are still set up and run.
- v0 does not provide `before_all`, `after_all`, or `after_each`.

Allowed steps:

- `before_each` contains the same steps a case body contains, in source order, and must contain at least one (`parse.before_each.empty`).
- `$` action steps, `assert` blocks, `let` runtime evidence bindings, and `write` steps are accepted, and behave exactly as they do in a case body. An action only updates the checkpoint, so verify a setup action's result with an `assert` block inside `before_each`.

Runtime evidence binding scope:

- A binding declared in `before_each` is in scope for the rest of `before_each` and for the whole case body that follows.
- A binding declared in a case body is not in scope for `before_each`, which already ran: a `before_each` reference to such a name is `semantic.binding.undefined`, not a use-before-declaration.
- A case body must not redeclare a `before_each` binding name (`semantic.binding.duplicate`). There is no shadowing.
- A binding's provenance names the action it captured from, whichever phase that action is in.
- `let` requires a preceding action **in its own phase**. The body-entry checkpoint drops the last setup action's process evidence, so a case body `let` placed before the body's first action is `semantic.binding.requires_action` even when `before_each` ran actions.

Checkpoint lifecycle:

- `before_each` starts at a setup-entry checkpoint, which has workspace state and no last action result. A process expectation (`exit` / `stdout` / `stderr`) placed before the block's first action is therefore an initial-checkpoint error, exactly as at the start of a case body.
- When `before_each` completes, the case body starts at its own initial checkpoint. Workspace state carries over; the last setup action's process evidence does not. A case body's first `exit` / `stdout` / `stderr` describes something that case body did, never a command the module-level setup happened to end with.

Assertion requirement:

- A `before_each` assertion verifies setup, and does not satisfy a case body's own requirement to contain at least one `assert` block (`parse.missing_assertion_block`). A case whose only assertion lives in the shared setup verifies nothing about its own subject.

See [ADR: `before_each` Is a Case-Local Setup Phase](../adr/20260806T090000Z_before-each-case-local-setup-phase.md) and [ADR: Runtime Evidence Binding Scope Across `before_each` and the Case Body](../adr/20260806T140000Z_before-each-binding-scope.md) for the rationale.

Variant-specific setup should usually live in the parameterized `case`, not in `before_each`. This keeps `before_each` independent of case-local parameter context.

## Shell execution

A `$` step is executed by a POSIX shell.

```reportage
$ rellog check --json | jq .
```

The runner does not rewrite arbitrary shell syntax in v0. The shell is responsible for interpreting pipelines, redirections, variable expansion, conditionals, filesystem operations, and other shell constructs.

A `$` action can span multiple physical lines: a line ending in `\` immediately before the line break continues into the next physical line, with the `\` and the line break preserved verbatim in the command handed to the shell. See [the generated syntax reference](syntax.md) and [ADR: Action Line Continuation](../adr/20260706T150000Z_action-line-continuation.md) for the exact continuation rule.

```reportage
$ echo one \
  two
```

For fixture copying and ordinary file operations, use shell commands:

```reportage
$ mkdir -p .rellog/entries
$ cp -R fixtures/${FIXTURE}/. .
```

Native Windows shell execution is out of scope for v0. Windows users should use WSL, a devcontainer, or Linux-based CI.

## PATH prefix injection

The runner can inject one or more runner-owned directories into the front of `PATH` before each action is executed.

The runner maintains an ordered list of PATH prefix directories in the `ExecutionEnvironment`.

- Prefixes are prepended to the inherited `PATH` in the given order.
- For example, prefixes `[A, B]` produce `PATH=A:B:<inherited PATH>`.
- If the inherited `PATH` is absent or empty, the effective `PATH` contains only the provided prefixes.
- When no prefixes are configured, the action shell inherits `PATH` from the current process without modification.

Shell selection remains separate from PATH prefix injection. The runner invokes `sh -c` to execute action commands, and the shim PATH is applied only to command resolution within that shell.

For shim roles, executable invocation targets, self-testing interception, application entrypoint shims, coverage-aware adapters, and shim invocation observability, see [Shims](shims.md).

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

For the assertion block construct that verifies a checkpoint, see [semantics.md — Assertion block](semantics.md#assertion-block).

## Binding environment

Every concrete case owns an initially empty binding environment.
A successful `let` step decodes the selected stream from the current checkpoint, stores a typed bound value with its declaration span and capture provenance, and leaves the checkpoint unchanged.
Provenance identifies the source action, stream, and exact or single-line capture mode.

Assertions and writes resolve `&name` against this environment at the point where the step runs.
The environment is discarded with the case workspace and is never projected into a later action's process environment.
The underlying action output remains the canonical evidence for artifacts.

## Checkpoint

A checkpoint is the observable evidence context available at a point in case execution.

A checkpoint is not a full filesystem snapshot. It is a reference to the evidence needed to evaluate the expectations in an assertion block (see [semantics.md — Evidence requirement](semantics.md#evidence-requirement)).

Checkpoints are maintained by the runner as it processes case body steps.

## Initial checkpoint

An initial checkpoint is established at the start of each step sequence a concrete case runs, before that sequence's first step executes. It has:

- workspace state (the current case workspace);
- no last action result.

Workspace expectations (`dir <"path"> exists`, `file <"path"> exists`, etc.) are valid at an initial checkpoint.

Process expectations (`exit`, `stdout`, `stderr`) require a last action result. Using a process expectation in an assertion block at an initial checkpoint is a **script error**.

A concrete case that runs `before_each` therefore has two initial checkpoints:

- the **setup-entry checkpoint**, established after the case workspace is created and before `before_each`'s first step;
- the **body-entry checkpoint**, established after `before_each` completes and before the case body's first step.

The body-entry checkpoint carries the workspace state `before_each` produced — files its `write` steps wrote, and files its actions created — but not the last setup action's result. A case body's first `exit` / `stdout` / `stderr` describes something that case body did, never whichever command the shared setup happened to end with.

An assertion evaluated at either checkpoint reports `checkpoint: "initial"` in [JSON output](../../spec/output/json-report/schema.json) and the [run result artifact](../../spec/artifacts/run-result/schema.json). The two are told apart by which block the assertion is written in.

A concrete case with no `before_each` has one initial checkpoint, the body-entry one.

## Action-updated checkpoint

After a `$ ...` action completes, the checkpoint is updated with:

- the action result (exit status, stdout, stderr);
- the post-action workspace state.

Subsequent assertion blocks reference this updated checkpoint until the next action updates it again.

## Cleanup and preservation

By default, each concrete case workspace is destroyed after the case finishes.

Implementations should provide a debug mode to preserve failed workspaces, but preservation is not part of the DSL syntax.

Because the runner owns workspace cleanup, v0 does not need `after_each` for file cleanup.

If future use cases require external cleanup, such as stopping services or collecting extra artifacts, `after_each` may be reconsidered.
