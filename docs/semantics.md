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

## Empty and zero-case scripts

Empty and whitespace-only scripts are syntax-valid inputs. Syntax validity only means the file can be parsed as a reportage script; it does not imply that execution has work to do.

When selected input is valid but produces zero concrete cases, the runner treats the run as a no-op success:

- the CLI exits with code `0`;
- no `$` command is executed;
- no checkpoint is generated;
- no assertion is evaluated;
- no case, checkpoint, or evidence artifacts are generated;
- human-readable CLI output states that no cases were found;
- the run result summary records `noop: true` and zero case, step, and assertion counts.

See ADR 20260703T000000Z_empty-and-whitespace-scripts-are-no-op-success for the rationale.

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

- files written by `write` steps;
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
- `before_each` may use `write` steps and ordinary shell setup steps such as `$ mkdir -p ...` or `$ cp -R ... ...`.
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

`write` steps (see "Write step" below) never expand variable bindings, whether or not the case is parameterized:

```reportage
write <".rellog/entries/001.kdl"> ```
  entry "entry" {
    kind "${ENTRY_KIND}"
  }
  ```
```

`${ENTRY_KIND}` above is preserved as a literal string, not expanded. Whether `write` should ever support expansion, and what a template-block form would look like, is a separate, not-yet-decided follow-up; see #71.

## Shell execution

A `$` step is executed by a POSIX shell.

```reportage
$ rellog check --json | jq .
```

The runner does not rewrite arbitrary shell syntax in v0. The shell is responsible for interpreting pipelines, redirections, variable expansion, conditionals, filesystem operations, and other shell constructs.

A `$` action can span multiple physical lines: a line ending in `\` immediately before the line break continues into the next physical line, with the `\` and the line break preserved verbatim in the command handed to the shell. See [`docs/syntax.md`](syntax.md) and [the ADR](adr/20260706T150000Z_action-line-continuation.md) for the exact continuation rule.

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

## Value literals

Reportage separates its single-line literal syntaxes by semantic domain. Each surface form maps to exactly one kind of value, independent of the position it appears in:

````text
"..."      = StringLiteral        (text domain)
```...```  = HeredocLiteral       (text domain)
<"...">    = WorkspacePath        (case-workspace filesystem reference)
@"..."     = FixtureReference     (test-definition-side file reference; reserved for #92)
````

These kinds group into two value categories:

```text
TextValue            = StringLiteral | HeredocLiteral
FileContentReference = WorkspacePath | FixtureReference
```

- A **`TextValue`** is text content: something to write, or something to compare against. See "Text literal" below.
- A **`WorkspacePath`** is a filesystem path resolved against the current concrete case's workspace root — the subject of `file` / `dir` checkpoints and the output path of `write`.
- A **`FixtureReference`** refers to a static fixture / snapshot file near the `*.repor` file itself. Its syntax is reserved by this grammar, but no argument position accepts it yet; it is introduced by #92.
- A **`FileContentReference`** is a reference that provides expected file content. No v0 expectation consumes one yet; it is the expected-value category for the future `contents_equals` family (#87).

There is **no implicit conversion** between these domains: a `TextValue` never converts to a `WorkspacePath` or `FixtureReference`, and vice versa. A `WorkspacePath` and a `FixtureReference` may hold the same string data internally, but they are distinct semantic types.

### Literal kind mismatch

Grammar-wise, every argument position accepts any single-line literal kind. Which kind a position's signature requires is checked while the AST is constructed, so a wrong-kind literal is a parse-able **semantic invalid case** with an actionable diagnostic (`semantic.literal.kind_mismatch`), never a bare syntax error. The diagnostic names the expected kind, the actual kind, and the suggested replacement:

```reportage
file "out.txt" exists
```

```text
`file` checkpoint subject requires a WorkspacePath, but "out.txt" is a StringLiteral; use <"out.txt"> instead
```

Conversely, a `WorkspacePath` in a text position:

```reportage
stdout contains <"expected.stdout">
```

```text
`stdout contains` expected text requires a TextValue, but <"expected.stdout"> is a WorkspacePath; use "expected.stdout" instead
```

The suggested replacement only names forms the position's grammar actually accepts: `write` content and `file contains` expected text suggest "a string literal or heredoc literal", while `stdout contains` / `stderr contains` — whose grammar only wires up the string literal form in v0 — suggest just the string literal, so following the suggestion can never itself produce a syntax error.

Literal kind validation and value validation are separate layers: `<"">`, `<"/abs">`, and `<"../up">` are correctly-kinded workspace path literals whose unescaped values still fail the existing workspace path policy (`semantic.workspace_path.*`). The inner quoted content of `<"...">` reuses the string literal escape rules; the workspace path policy applies to the value after unescaping.

### Signatures of path-taking constructs

The v0 constructs that take a path take a `WorkspacePath`:

```text
write <WorkspacePath> <TextValue>

file <WorkspacePath> exists
file <WorkspacePath> contains <TextValue>

dir <WorkspacePath> exists
dir <WorkspacePath> contains <existing-dir-contains-expected>
```

`dir contains`'s expected side keeps its existing semantics (a single entry name as a string literal); only the `dir` subject is a `WorkspacePath`.

The `file` / `dir` checkpoint subject accepts a `WorkspacePath` **only** — never a `FixtureReference`. A fixture reference will be usable as expected content (`FileContentReference`), not as a checkpoint subject: assertions observe the workspace under test, while fixtures supply what to compare it against.

### Design constraints for future expectations

Future `text_equals` / `contents_equals` expectations (#87, #88) must follow these type rules:

```text
file <WorkspacePath> text_equals <TextValue>
file <WorkspacePath> contents_equals <FileContentReference>

stdout contains <TextValue>
stdout text_equals <TextValue>
stdout contents_equals <FileContentReference>

stderr contains <TextValue>
stderr text_equals <TextValue>
stderr contents_equals <FileContentReference>
```

`contents_equals` takes a `FileContentReference` as its expected value uniformly, regardless of subject — a reader who sees `contents_equals` always knows the expected side is `<"...">` or `@"..."`. `text_equals`, like `contains`, always takes a `TextValue`. The conformance cases for these expectations are added by their own introducing issues.

See [ADR: Workspace Path Literal Syntax](adr/20260706T160000Z_workspace-path-literal-syntax.md) for why the surface syntaxes are separated rather than contextually typed.

## Text literal

A `text_literal` is the syntax category `string literal | heredoc literal` — the two interchangeable ways v0 accepts multi-purpose text. Both a `write` step's content and a `file ... contains` expectation's expected text are written as a `text_literal`.

- A **string literal** is an ordinary `"..."` string, subject to v0's escape rules (`\\`, `\"`, `\n`, `\t`; no raw newlines). See [ADR: String Literal Escape Sequences](adr/20260701T214658Z_string-literal-escape-sequences.md).
- A **heredoc literal** is a dedented, fenced ` ``` ... ``` ` block, introduced for the `write` step and now reusable wherever a `text_literal` is accepted. See "Heredoc literal" below for its grammar, and [ADR: Heredoc Literal and TextValue](adr/20260706T104151Z_heredoc-literal-and-text-value.md) for why this construct is named "heredoc literal" (superseding the earlier internal names "fenced raw text block" / "fenced text literal") and how it relates to `text_literal`.

Both forms resolve to the same `TextValue` at the semantic level: a `write` step writes a `TextValue`'s UTF-8 bytes to a file, and a `file ... contains` expectation checks whether a `TextValue`'s UTF-8 bytes occur as a substring of a file's bytes. Neither `write` nor `file ... contains` branches on which literal form produced the `TextValue` — the two forms are chosen for readability (a heredoc literal avoids `\n`-escaping multi-line content), not for any difference in runtime behavior.

### Heredoc literal

```reportage
write <"expected/stdout.txt"> ```
  expected output
  ```
```

Grammar and semantics:

- The opening fence is three or more backticks; the closing fence uses the same character and must be at least as long as the opening fence. Use a longer opening fence to embed a shorter run of backticks (e.g. an embedded ` ``` ` Markdown block) as literal content.
- Neither the opening nor the closing fence line accepts an inline `#` comment.
- The content is dedented against the closing fence's indentation: every non-blank body line must start with that indentation as a literal string prefix (not a tab/space width equivalence), and that prefix is stripped. Blank and whitespace-only lines are exempt from this check and are dedented to an empty line. A non-blank line indented less than the closing fence is a parse error.
- Line endings (LF or CRLF) are preserved exactly as written; they are never normalized.
- An empty block (opening fence immediately followed by a closing fence) resolves to an empty string. Otherwise, the block's final line ending is included in the resolved content.
- A heredoc literal performs no parameter or variable expansion. `${VAR}`-shaped text inside the block is preserved verbatim. See "Parameter bindings" above.

A heredoc literal missing its own closing fence does not always fail with a syntax error: like a heredoc missing its terminator, the parser scans forward for the next line shaped like a valid closing fence, which may belong to a different, later heredoc literal. When that happens, everything in between — including that later literal's own opening line — is silently absorbed as literal content, with no diagnostic. Keep each heredoc literal's opening and closing fence visually paired to avoid this.

Because a heredoc literal spans multiple physical lines and its closing fence line consumes its own trailing line ending (with no inline comment allowed), it cannot appear in a single-line `assert { ... }` body — only in the multi-line form. It can, however, be used directly after `write <"path">` (which is inherently a multi-line construct already).

## Write step

A `write` step writes a `text_literal`'s resolved content to a file in the current concrete case's isolated workspace:

```reportage
write <"expected/stdout.txt"> "expected output\n"
```

```reportage
write <"expected/stdout.txt"> ```
  expected output
  ```
```

`write` is a **side-effecting step**: unlike an action or an assertion block, it changes workspace state directly rather than executing an action or verifying a checkpoint. It is one of three step kinds a `case` body may contain — action (`$ ...`), assertion block (`assert { ... }`), and side-effecting step (`write ...`) — evaluated in source order, exactly like actions and assertion blocks.

Semantics:

- `write <"path"> <text_literal>` is create-only. If `<path>` already exists (as a file, directory, or symlink), the step fails rather than silently overwriting it.
- `<path>` is resolved relative to the current concrete case's workspace root, never the repository root. See "Repository root and workspace boundary" below.
- Parent directories are created automatically. If a regular file, a symlink, or any other non-directory entry already occupies part of the parent path, the step fails — a symlink is rejected rather than followed, so a symlink planted by an earlier `$` action (e.g. `$ ln -s /tmp escape`) cannot be used to make a later `write` step escape the isolated workspace.
- When `<text_literal>` is a string literal, the step is an ordinary single-line construct and may carry a trailing `#` comment, like every other single-line step. When it is a heredoc literal, see "Heredoc literal" above for its own line-ending and dedent rules.

### Side-effecting step failure classification

A `write` step's failure is never an assertion failure — there is no expectation being compared against evidence, only an operation that either succeeds or does not:

- Malformed syntax (an unterminated fenced block, a fence line with an inline comment, a non-blank body line indented less than the closing fence) is a **parse error**.
- An unsafe workspace path — empty, absolute, or containing a `.` / `..` segment — is a **parse-domain validation error** (`semantic.workspace_path.*`), detected before any file I/O is attempted. See [`docs/diagnostics.md`](diagnostics.md).
- A regular file blocking the parent path, an already-existing target, or an OS-level I/O failure is a **runtime step error** (`step.write.*`), detected while the step actually runs.

A runtime step error stops the concrete case at that point, the same way an assertion block failure does: later steps in the same case do not run, but the runner may proceed to the next concrete case. Unlike an assertion block failure, a runtime step error is a `runtime_error` run outcome (exit code `3`), not a `test_failed` outcome — see [`docs/exit-codes.md`](exit-codes.md).

### Repository root and workspace boundary

A `write` step's path is always relative to the current concrete case's workspace, never the repository root. v0 has no mechanism for a `write` step, or any file expectation, to implicitly reference a file under the repository root. A future repository-fixture mechanism (`fixture` / `copy` / `import`, or a repository path literal) would need to make that boundary explicit rather than allowing repository paths where a workspace path is expected.

## Expectation

An expectation is an individual expected condition within an assertion block.

Examples: `exit 0`, `stderr empty`, `dir <".rellog"> exists`, `file <".rellog/config.yml"> exists`.

Each expectation has an evidence requirement that determines what checkpoint state must be available for it to be evaluated. Expectations are side-effect-free. Failures are reported per expectation, independently of other expectations in the same block.

## Logical composition

`not { ... }`, `all { ... }`, and `any { ... }` compose expectation expressions into a single expectation expression, block-form only. See ADR 20260704T150000Z_block-form-logical-composition for why v0 rejects infix `A and B` / `A or B`, `and { ... }` / `or { ... }` aliases, and predicate-level negation (`file <"path"> not exists`) in favor of this form.

A logical composition block's body accepts the same single-line or multi-line expectation forms as `assert { ... }`, and may contain nested `not` / `all` / `any` blocks in addition to atomic expectations.

Semantics:

- `all { ... }` succeeds when every expectation expression inside it succeeds.
- `any { ... }` succeeds when at least one expectation expression inside it succeeds.
- `not { ... }` succeeds when the expectation expressions inside it, taken together, do not succeed.
- The multiple expectations directly inside `assert { ... }` are an implicit `all`, exactly as before this feature existed.
- A `not` block with multiple expectation expressions negates their implicit-`all` grouping, not each expectation individually: `not { A B }` evaluates as `not(all(A, B))`, never as `not(A) and not(B)`.
- Evaluation is recursive: a nested `not` / `all` / `any` is itself evaluated by the same rules before its result is used by its parent.
- A logical composition block must contain at least one expectation expression. An empty `not { }` / `all { }` / `any { }` is a **script error** — the same category of error as an assertion block with no expectations — not an assertion failure, because there is no evidence comparison to perform.
- A logical composition's evidence requirement is inherited from whichever of its (possibly nested) descendants needs one: wrapping a process expectation (`exit`, `stdout`, `stderr`) in `not` / `all` / `any` still requires a preceding action, exactly like using that expectation bare.

## Checkpoint

A checkpoint is the observable evidence context available at a point in case execution.

A checkpoint is not a full filesystem snapshot. It is a reference to the evidence needed to evaluate the expectations in an assertion block.

Checkpoints are maintained by the runner as it processes case body steps.

## Initial checkpoint

The initial checkpoint is established after the case workspace is created, `before_each` has run, and before the first step of the case body executes.

The initial checkpoint has:

- workspace state (the current case workspace, including any files written by `before_each`);
- no last action result.

Workspace expectations (`dir <"path"> exists`, `file <"path"> exists`, etc.) are valid at the initial checkpoint.

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

- `dir <"path"> exists`
- `dir <"path"> contains "<name>"`
- `file <"path"> exists`
- `file <"path"> contains "<text>"`
- `file-count <glob> <op> <n>`

`file-count` is conceptual / future syntax and is not part of v0. `dir <"path"> exists`, `dir <"path"> contains "<name>"`, `file <"path"> exists`, and `file <"path"> contains "<text>"` are implemented in v0; see "File assertions" and "Directory assertions" below.

### Process expectations

Require the last action result. A script error if used at a checkpoint with no last action result (i.e., before any `$` action in the same case).

- `exit <code>`
- `stdout empty`
- `stdout contains <string>`
- `stderr empty`
- `stderr contains <string>`

### stdout/stderr evidence representation

stdout and stderr are held and compared as raw process output bytes, not decoded text. `stdout contains <string>` / `stderr contains <string>` unescape the string literal to its UTF-8 bytes and match those bytes against the raw output as a byte-level substring search — there is no decoding of the actual output on either side of the comparison.

`stdout empty` / `stderr empty` pass only when the actual output is zero bytes. Whitespace, tabs, LF, CRLF, and bare CR are all output — a stream containing only whitespace is not empty.

Non-UTF-8 process output is not rejected. Reportage does not perform encoding-aware assertions (e.g. decoding Shift-JIS) in v0; only raw byte comparisons are defined. Lossy UTF-8 decoding is used only for human-readable display (CLI diagnostics, and the optional `text` helper field in artifact / result JSON), never for evaluating an expectation.

### Structured output expectations

Require the corresponding process output from the last action result.

- `stdout jq <expression>`
- `stderr jq <expression>`

In v0, structured output expectations use external `jq`.

## File assertions

`file <"path"> exists` and `file <"path"> contains <text_literal>` are v0 workspace expectations. `file <"path">` is the subject; `exists` and `contains <text_literal>` are predicates on that subject. See [ADR: Adopt Subject-First File Assertion Syntax](adr/20260704T112155Z_subject-first-file-assertion-syntax.md) for why this shape was chosen over an expectation-first form.

```reportage
assert {
  file <".reportage/runs/self-test/result.json"> exists
  file <".reportage/runs/self-test/result.json"> contains "\"result\""
}
```

`contains`'s expected text is a [`text_literal`](#text-literal): either an ordinary string literal (as above) or a heredoc literal, useful for multi-line expected content:

```reportage
assert {
  file <"out/report.html"> contains ```
    <li>expected row</li>
    ```
}
```

A heredoc literal here follows the same grammar, dedent, and line-ending rules described in "Heredoc literal" above (it can only be used inside the multi-line `assert { ... }` form, for the same reason it cannot appear in a single-line assertion block). `write` and `file ... contains` share this one heredoc literal implementation; neither behaves differently based on which `text_literal` form produced the value being written or compared.

Path resolution:

- The path is resolved relative to the current concrete case's isolated workspace root (see "Workspace lifecycle" above). A `cd` performed inside a `$` action never changes this, because each action runs in a fresh child shell with the workspace root as its working directory; only that workspace root is used to resolve file assertion paths.
- The path must be relative. Absolute paths are rejected.
- `.` and `..` path segments are rejected.
- These path policy violations are semantic errors (`semantic.file_path.absolute`, `semantic.file_path.dot_segment`), not assertion failures: the evaluator rejects them before attempting any filesystem evidence comparison. See [`docs/semantic-diagnostics.md`](semantic-diagnostics.md).

`exists` semantics:

- Succeeds when the path resolves (following symlinks) to a regular file.
- Fails when the path does not exist, or resolves to something other than a regular file (e.g. a directory).

`contains` semantics:

- Succeeds when the path is a readable UTF-8 regular file whose content contains the expected text as a plain substring.
- Fails when the path does not exist, is not a regular file, cannot be read, or is not valid UTF-8.
- Fails when the file is readable UTF-8 but does not contain the expected substring.
- The match is a plain byte/`str` substring match: no regex, no line-based matching, no newline or Unicode normalization.

`file` is scoped to regular files in v0. Directory assertions use the separate `dir` subject; see "Directory assertions" below.

## Directory assertions

`dir <"path"> exists` and `dir <"path"> contains "<name>"` are v0 workspace expectations. `dir <"path">` is the subject; `exists` and `contains "<name>"` are predicates on that subject, mirroring the `file` subject's shape. See [ADR: Adopt Subject-First Directory Assertion Syntax](adr/20260706T000000Z_subject-first-directory-assertion-syntax.md) for why this shape was chosen, and for how it relates to the `file` subject.

```reportage
assert {
  dir <"artifacts"> exists
  dir <"artifacts"> contains "result.json"
}
```

Path resolution:

- The path is resolved relative to the current concrete case's isolated workspace root, exactly like a `file` assertion path. A `cd` performed inside a `$` action never changes this.
- The path must be relative, non-empty, and free of `.` / `..` segments — the same `WorkspacePath` subject path rule the `write` step and `file` assertions follow.
- These path policy violations are semantic errors (`semantic.workspace_path.empty`, `semantic.workspace_path.absolute`, `semantic.workspace_path.dot_segment`), not assertion failures: the evaluator rejects them before attempting any filesystem evidence comparison. See [`docs/semantic-diagnostics.md`](semantic-diagnostics.md).

`exists` semantics:

- Succeeds when the path resolves (following symlinks) to a directory.
- Fails when the path does not exist, or resolves to something other than a directory (e.g. a regular file).

`contains` semantics:

- `<name>` is a single directory entry name, not a path: it must be non-empty, must not contain a path separator (`/`), must not be `.` or `..`, and must not contain control characters. Violating this is a semantic error (`semantic.dir_entry_name.empty`, `.path_separator`, `.dot_entry`, `.control_char`), rejected the same way an invalid subject path is.
- Succeeds when the subject path resolves to a directory and it has an entry named `<name>` directly under it, regardless of that entry's file type.
- Fails when the subject path does not exist, is not a directory, or is a directory without an entry named `<name>` directly under it.
- The check is never recursive, never a glob match, and never a file content search: only the exact entry name directly under the subject path is compared. A symlink entry's link target is not inspected.

## Example: checkpoint model in action

```reportage
case "init creates workspace" {
  assert {
    not {
      dir <".rellog"> exists
    }
  }

  $ rellog init

  assert {
    exit 0
    dir <".rellog"> exists
    file <".rellog/config.yml"> exists
  }
}
```

Walkthrough:

- The first `assert { ... }` block evaluates the **initial checkpoint**.
- `not { dir <".rellog"> exists }` is a workspace expectation and is valid at the initial checkpoint.
- `$ rellog init` executes the action and updates the checkpoint with the action result and post-action workspace state.
- The second `assert { ... }` block evaluates the **action-updated checkpoint**.
- `exit 0` is a process expectation and requires the last action result — valid because `$ rellog init` has run.
- `dir <".rellog"> exists` and `file <".rellog/config.yml"> exists` are workspace expectations and observe the post-action workspace state.

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
