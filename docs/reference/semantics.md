# Semantics

This document is the overview and entry point for reportage's semantics documentation set. It also holds the language semantic rules that have not yet been migrated to the generated semantic rule catalog (see "Semantics document set" below).

For syntax, see [the generated syntax reference](syntax.md).

## Semantics document set

Reportage's semantics are split across several documents by responsibility, rather than kept as one hand-written normative source. See [ADR: Semantics Documentation Strategy and Semantic Rule Coverage](../adr/20260708T061500Z_semantics-documentation-strategy.md) for the rationale.

| Concern | Document |
| --- | --- |
| Execution model / runtime semantics — runner execution order, case workspace, action execution, checkpoint lifecycle, `before_each`, shell execution, coverage adapter lifecycle, cleanup | [Execution model](execution-model.md) |
| Command resolution shim model — shim purpose, shim target, event protocol, observability | [Shims](shims.md) |
| Language semantic rules — value literals, expectations, assertion evaluation, logical composition | This document (below), and the generated semantic rule catalog at [`semantic-rules.md`](semantic-rules.md). |
| Artifact / result JSON semantics | [Artifacts](artifacts.md) and [the JSON execution report contract](../../spec/output/json-report/README.md) |
| Diagnostics | [Parse diagnostics](diagnostics.md) and [Semantic and assertion diagnostics](semantic-diagnostics.md) |
| Rationale for individual decisions | [`adr/`](../adr/README.md) |
| Deferred / undecided items | [Deferred topics](../planning/TBD.md) |

The sections below cover language semantic rules that have not yet moved to the generated catalog: parameter bindings, assertion blocks, value literals, text literals, the `write` step, expectations, logical composition, evidence requirements, file/directory assertions, and `jq` assertions.

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

For how a concrete case is expanded from a `case` with `params`, see [execution-model.md — Concrete case expansion](execution-model.md#concrete-case-expansion).

## Assertion block

An assertion block is written as `assert { ... }` and is a checkpoint-level verification construct. See [execution-model.md — Checkpoint](execution-model.md#checkpoint) for what a checkpoint is and when it is updated.

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
@"..."     = FixtureReference     (test-definition-side file reference)
````

These kinds group into two value categories:

```text
TextValue            = StringLiteral | HeredocLiteral
FileContentReference = WorkspacePath | FixtureReference
```

- A **`TextValue`** is text content: something to write, or something to compare against. See "Text literal" below.
- A **`WorkspacePath`** is a filesystem path resolved against the current concrete case's workspace root — the subject of `file` / `dir` checkpoints and the output path of `write`.
- A **`FixtureReference`** refers to a static fixture / snapshot file near the `*.repor` file itself, resolved relative to that file's directory (see "Fixture reference value" below).
- A **`FileContentReference`** is a reference that provides expected file content: a `WorkspacePath` or a `FixtureReference`. It is the expected-value category for `file <"path"> contents_equals` and `stdout` / `stderr contents_equals`.

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

### `text_equals` and `contents_equals` positions

`text_equals` and `contents_equals` follow these type rules:

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

`contents_equals` takes a `FileContentReference` as its expected value uniformly, regardless of subject — a reader who sees `contents_equals` always knows the expected side is `<"...">` or `@"..."`. `text_equals`, like `contains`, always takes a `TextValue`, uniformly across `file` / `stdout` / `stderr`.

Both positions share parsing, AST construction, and literal-kind validation (so a wrong-kind literal, e.g. `file <"out.txt"> text_equals @"expected.txt"` or `stdout text_equals <"expected.txt">`, is already a `semantic.literal.kind_mismatch`), and `contents_equals` additionally shares the fixture reference resolution/materialization mechanism described below. `contents_equals` evaluation and `text_equals` evaluation (both the string-literal and heredoc-literal forms) are implemented for `file`, `stdout`, and `stderr`.

See [ADR: Workspace Path Literal Syntax](../adr/20260706T160000Z_workspace-path-literal-syntax.md) for why the surface syntaxes are separated rather than contextually typed, [ADR: Fixture Reference Value Syntax](../adr/20260706T170000Z_fixture-reference-value-syntax.md) for the fixture reference literal itself, [ADR: `contents_equals` Comparison Evaluation](../adr/20260707T012055Z_contents-equals-evaluation.md) for `contents_equals`'s comparison semantics and diagnostics, [ADR: `text_equals` Evaluation](../adr/20260708T045332Z_text-equals-evaluation.md) for `text_equals`'s comparison semantics and diagnostics, and [ADR: `stdout` / `stderr` `text_equals` Evaluation](../adr/20260710T100918Z_output-text-equals-evaluation.md) for the captured-stream forms.

### `contents_equals` comparison semantics

`contents_equals` compares actual and expected bytes byte-for-byte. No normalization is ever applied: trailing newlines, CRLF vs. LF, leading/trailing whitespace, and Unicode normalization all participate in the comparison exactly as captured. Two empty inputs are equal.

The actual side and the expected side are classified differently when either cannot be read, because one names the subject under test and the other names the test definition's own expected value:

- **Actual side** (`file`'s subject; `stdout` / `stderr`'s captured bytes): a missing, non-regular-file, or unreadable actual `file` is an **assertion failure** — the subject under test did not produce the expected output. `stdout` / `stderr` have no such failure mode; captured output is always available once an action has run.
- **Expected side** (`contents_equals`'s `FileContentReference` operand): a missing, non-regular-file, or unreadable expected `WorkspacePath` is a **test-definition error** (`CaseStatus::ScriptError`, exit code 2, `semantic.file_contents_reference.*`), not an assertion failure — the expected value itself could not be sourced. An unresolvable `FixtureReference` is classified the same way, using the existing `semantic.fixture_reference.*` codes (see "Fixture reference value" below). A `contents_equals` expected-value error nested inside a `not` / `all` / `any` composition aborts the whole case immediately, exactly like a bare (non-composed) one — it is never swallowed as an ordinary failing child.

A mismatch's diagnostic is bounded: CLI stdout/stderr (`--format=json` and the human renderer) never print the full actual/expected bytes, only the actual/expected byte lengths, the first differing byte offset, the byte-line number it falls on (LF-delimited, CRLF not normalized), and an escaped, size-capped context window around it (falling back from line-context to a fixed byte window for a huge single line or binary-like content). Persisting the full mismatch bytes as run evidence is not required; the `.reportage/runs/<id>/result.json` artifact records only the same bounded mismatch information (actual/expected byte lengths, first-diff offset and line, escaped context windows) and never embeds the full actual/expected bytes — raw byte evidence is stored as separate artifact files and referenced by `artifactRef` / `sizeBytes` / `sha256` (see [ADR: Artifact Run Result as Canonical Manifest](../adr/20260708T130500Z_artifact-run-result-canonical-manifest.md)).

### `text_equals` comparison semantics

`text_equals` resolves its `<text_literal>` operand (a string literal or a heredoc literal) to a `TextValue`, encodes that `TextValue` as UTF-8 bytes, and compares those bytes against the actual side's bytes byte-for-byte, reusing exactly the same comparison and diagnostic machinery `contents_equals` uses. The actual side is the actual file's bytes for `file`, and the captured stream's raw bytes for `stdout` / `stderr`. No normalization is ever applied, for the same reasons given above. String literal and heredoc literal are transparent to this comparison: the same text written either way compares identically, because both resolve to the same `TextValue` before comparison ever runs (see [ADR: Heredoc Literal and Text Value](../adr/20260706T104151Z_heredoc-literal-and-text-value.md)).

Unlike `contents_equals`, `text_equals` has only one failure classification to make, not two: its expected value is always an inline `TextValue` already present in the parsed script, so there is nothing to resolve and no expected-side test-definition error is possible. The **actual side** of `file` is classified exactly like `contents_equals`'s actual side: a missing, non-regular-file, or unreadable actual `file` is an **assertion failure**, using its own `assertion.file.text_equals.actual_*` codes. A byte mismatch is likewise an assertion failure (`assertion.file.text_equals.mismatch` / `assertion.stdout.text_equals.mismatch` / `assertion.stderr.text_equals.mismatch`), with the same bounded, escaped diagnostic `contents_equals` produces. `stdout` / `stderr` have no actual-side `actual_*` failure modes: captured output is always available once an action has run, exactly as for `stdout` / `stderr contents_equals`.

Diagnostic presentation, not comparison semantics, may differ by which literal form produced the expected `TextValue`: a mismatch's subject description renders a string literal compactly (the literal text itself) and a heredoc literal as a plain label, since the bounded mismatch context already carries a line number and an escaped window for either form. See [ADR: `text_equals` Evaluation](../adr/20260708T045332Z_text-equals-evaluation.md) and [ADR: `stdout` / `stderr` `text_equals` Evaluation](../adr/20260710T100918Z_output-text-equals-evaluation.md).

### Fixture reference value

A `FixtureReference` (`@"<path>"`) names a static snapshot / fixture file kept alongside the referencing `*.repor` source file, for use as expected file contents in an assertion. It is only ever valid in a `FileContentsReference` expected position (`contents_equals`); everywhere else, including a `file` / `dir` checkpoint subject, `text_equals`, `write`, or a `*.repor` context outside any assertion block (e.g. an action line), it is rejected — either as a `semantic.literal.kind_mismatch` (a wrong-kind literal in a real argument position) or, for an action line, not interpreted as a fixture reference at all: `$ cat @"expected.json"` passes `@"expected.json"` to the shell verbatim, since Reportage never parses action bodies for embedded literals.

**Path resolution.** A fixture path resolves relative to the directory containing the referencing `*.repor` file — never the repository root, and never the case workspace. `@"expected.json"` next to `cases/cli-json/run.repor` resolves to `cases/cli-json/expected.json`, regardless of where the suite is invoked from.

**Path policy.** The same lexical policy as `WorkspacePath` applies to the raw fixture path: it must be non-empty and relative, and must not contain a `.` or `..` path segment. This is checked at AST construction time (`semantic.fixture_reference.empty` / `.absolute` / `.dot_segment`), exactly like `WorkspacePath`'s own policy.

**Symlink / traversal defense.** A lexical dot-segment ban alone cannot stop an escape through a symlink planted under the `*.repor` directory. Resolution therefore canonicalizes both the `*.repor` directory and the candidate fixture path and verifies the latter still lies under the former before treating it as valid (`semantic.fixture_reference.escapes_repor_directory`); a missing source or a non-regular-file source (e.g. a directory) are separate, equally rejected outcomes (`semantic.fixture_reference.missing`, `.not_a_regular_file`). See `fixture::resolve_fixture_source`.

**Validation vs. materialization timing.** Fixture validation (path resolution, containment, regular-file-ness) may run as early as parse or semantic validation, or as late as case planning. Materialization — copying the validated fixture's bytes into a runner-reserved area — happens only during assertion evaluation, never during the action phase: a fixture is never placed at an ordinary sandbox path, and its runner-reserved destination is not a path a script can address. See `fixture::materialize_fixture`.

## Text literal

A `text_literal` is the syntax category `string literal | heredoc literal` — the two interchangeable ways v0 accepts multi-purpose text. Both a `write` step's content and a `file ... contains` expectation's expected text are written as a `text_literal`.

- A **string literal** is an ordinary `"..."` string, subject to v0's escape rules (`\\`, `\"`, `\n`, `\t`; no raw newlines). See [ADR: String Literal Escape Sequences](../adr/20260701T214658Z_string-literal-escape-sequences.md).
- A **heredoc literal** is a dedented, fenced ` ``` ... ``` ` block, introduced for the `write` step and now reusable wherever a `text_literal` is accepted. See "Heredoc literal" below for its grammar, and [ADR: Heredoc Literal and TextValue](../adr/20260706T104151Z_heredoc-literal-and-text-value.md) for why this construct is named "heredoc literal" (superseding the earlier internal names "fenced raw text block" / "fenced text literal") and how it relates to `text_literal`.

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

`write` is a **side-effecting step**: unlike an action or an assertion block, it changes workspace state directly rather than executing an action or verifying a checkpoint. It is one of three step kinds a `case` body may contain — action (`$ ...`), assertion block (`assert { ... }`), and side-effecting step (`write ...`) — evaluated in source order, exactly like actions and assertion blocks. It is also the only step kind a module-level `before_each` block may contain (see [Execution model](execution-model.md#before_each)).

Semantics:

- `write <"path"> <text_literal>` is create-only. If `<path>` already exists (as a file, directory, or symlink), the step fails rather than silently overwriting it.
- `<path>` is resolved relative to the current concrete case's workspace root, never the repository root. See "Repository root and workspace boundary" below.
- Parent directories are created automatically. If a regular file, a symlink, or any other non-directory entry already occupies part of the parent path, the step fails — a symlink is rejected rather than followed, so a symlink planted by an earlier `$` action (e.g. `$ ln -s /tmp escape`) cannot be used to make a later `write` step escape the isolated workspace.
- When `<text_literal>` is a string literal, the step is an ordinary single-line construct and may carry a trailing `#` comment, like every other single-line step. When it is a heredoc literal, see "Heredoc literal" above for its own line-ending and dedent rules.

### Side-effecting step failure classification

A `write` step's failure is never an assertion failure — there is no expectation being compared against evidence, only an operation that either succeeds or does not:

- Malformed syntax (an unterminated fenced block, a fence line with an inline comment, a non-blank body line indented less than the closing fence) is a **parse error**.
- An unsafe workspace path — empty, absolute, or containing a `.` / `..` segment — is a **parse-domain validation error** (`semantic.workspace_path.*`), detected before any file I/O is attempted. See [Parse diagnostics](diagnostics.md).
- A regular file blocking the parent path, an already-existing target, or an OS-level I/O failure is a **runtime step error** (`step.write.*`), detected while the step actually runs.

A runtime step error stops the concrete case at that point, the same way an assertion block failure does: later steps in the same case do not run, but the runner may proceed to the next concrete case. Unlike an assertion block failure, a runtime step error is a `runtime_error` run outcome (exit code `3`), not a `test_failed` outcome — see [Exit codes](exit-codes.md).

### Repository root and workspace boundary

A `write` step's path is always relative to the current concrete case's workspace, never the repository root. v0 has no mechanism for a `write` step, or any file expectation, to implicitly reference a file under the repository root. A future repository-fixture mechanism (`fixture` / `copy` / `import`, or a repository path literal) would need to make that boundary explicit rather than allowing repository paths where a workspace path is expected.

## Expectation

An expectation is an individual expected condition within an assertion block.

Examples: `exit 0`, `stderr empty`, `dir <".rellog"> exists`, `file <".rellog/config.yml"> exists`.

Each expectation has an evidence requirement that determines what checkpoint state must be available for it to be evaluated. Expectations are side-effect-free. Failures are reported per expectation, independently of other expectations in the same block.

## Logical composition

`not { ... }`, `all { ... }`, and `any { ... }` compose expectation expressions into a single expectation expression, block-form only. See [ADR: Block-Form Logical Composition](../adr/20260704T150000Z_block-form-logical-composition.md) for why v0 rejects infix `A and B` / `A or B`, `and { ... }` / `or { ... }` aliases, and predicate-level negation (`file <"path"> not exists`) in favor of this form.

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

## Evidence requirement

Different expectations require different evidence from the current checkpoint (see [execution-model.md — Checkpoint](execution-model.md#checkpoint)).

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

`file <"path"> exists` and `file <"path"> contains <text_literal>` are v0 workspace expectations. `file <"path">` is the subject; `exists` and `contains <text_literal>` are predicates on that subject. See [ADR: Adopt Subject-First File Assertion Syntax](../adr/20260704T112155Z_subject-first-file-assertion-syntax.md) for why this shape was chosen over an expectation-first form.

```reportage
assert {
  file <".reportage/runs/self-test/result.json"> exists
  file <".reportage/runs/self-test/result.json"> contains "\"status\""
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

- The path is resolved relative to the current concrete case's isolated workspace root (see [execution-model.md — Workspace lifecycle](execution-model.md#workspace-lifecycle)). A `cd` performed inside a `$` action never changes this, because each action runs in a fresh child shell with the workspace root as its working directory; only that workspace root is used to resolve file assertion paths.
- The path must be relative. Absolute paths are rejected.
- `.` and `..` path segments are rejected.
- These path policy violations are semantic errors (`semantic.file_path.absolute`, `semantic.file_path.dot_segment`), not assertion failures: the evaluator rejects them before attempting any filesystem evidence comparison. See [Semantic and assertion diagnostics](semantic-diagnostics.md).

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

`dir <"path"> exists` and `dir <"path"> contains "<name>"` are v0 workspace expectations. `dir <"path">` is the subject; `exists` and `contains "<name>"` are predicates on that subject, mirroring the `file` subject's shape. See [ADR: Adopt Subject-First Directory Assertion Syntax](../adr/20260706T000000Z_subject-first-directory-assertion-syntax.md) for why this shape was chosen, and for how it relates to the `file` subject.

```reportage
assert {
  dir <"artifacts"> exists
  dir <"artifacts"> contains "result.json"
}
```

Path resolution:

- The path is resolved relative to the current concrete case's isolated workspace root, exactly like a `file` assertion path. A `cd` performed inside a `$` action never changes this.
- The path must be relative, non-empty, and free of `.` / `..` segments — the same `WorkspacePath` subject path rule the `write` step and `file` assertions follow.
- These path policy violations are semantic errors (`semantic.workspace_path.empty`, `semantic.workspace_path.absolute`, `semantic.workspace_path.dot_segment`), not assertion failures: the evaluator rejects them before attempting any filesystem evidence comparison. See [Semantic and assertion diagnostics](semantic-diagnostics.md).

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

- The first `assert { ... }` block evaluates the **initial checkpoint** (see [execution-model.md — Initial checkpoint](execution-model.md#initial-checkpoint)).
- `not { dir <".rellog"> exists }` is a workspace expectation and is valid at the initial checkpoint.
- `$ rellog init` executes the action and updates the checkpoint with the action result and post-action workspace state.
- The second `assert { ... }` block evaluates the **action-updated checkpoint** (see [execution-model.md — Action-updated checkpoint](execution-model.md#action-updated-checkpoint)).
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

## Document block (`document file` / `document case`)

A document block attaches documentation metadata to a source construct as first-class syntax:

````reportage
document file {
  title "File assertions"
  group "Filesystem"
  order 20

  description ```
  Collected examples of assertions against files.
  ```
}

document case {
  title "File creation"

  description ```
  Verifies that the command creates the file.
  ```
}

case "file exists" {
  $ touch test.txt

  assert {
    file <"test.txt"> exists
  }
}
````

v0 supports two scopes:
`file`, whose metadata describes the whole source file,
and `case`, whose metadata attaches to the immediately following case.
Any other scope keyword is a syntax error.

Documentation is deliberately not expressed through `#` comments:
comments are discarded at parse time and never reach any model,
while documentation metadata must survive parsing so documentation tooling can consume it.
See [ADR: Document Block as First-Class Documentation Syntax](../adr/20260712T120000Z_document-block-first-class-documentation-syntax.md)
and [ADR: Case Documentation via an Adjacent `document case` Block](../adr/20260713T120000Z_document-case-adjacent-association.md).

### Documentation fields

Each scope accepts exactly its own field whitelist;
an unknown field, or a field of another scope, is a syntax error.

`document file`:

| Field | Value | Meaning |
| --- | --- | --- |
| `title` | string literal | The file's display name in documentation. |
| `group` | string literal | The group name used when aggregating multiple sources. |
| `order` | non-negative integer | The file's display order within its group. |
| `description` | string literal or heredoc literal | A description of the whole file, plain text in v0. |

`document case`:

| Field | Value | Meaning |
| --- | --- | --- |
| `title` | string literal | The case's display name in generated documentation. |
| `description` | string literal or heredoc literal | A description of the case, plain text in v0. |

`group` and `order` are file-scope fields only:
a case has no grouping or ordering of its own (cases render in source order),
so `group` / `order` inside a `document case` block are syntax errors, exactly like an unknown field.

`title`, `group`, and `description` positions parse the kind-agnostic value literal,
so a wrong-kind literal (e.g. `title <"a.txt">`) is rejected as `semantic.literal.kind_mismatch`,
consistent with every other literal position (see "Value literals" above).
Declaring the same field twice in one block is rejected (`parse.document_block.duplicate_field`).
An `order` value that overflows the supported non-negative integer range (u64) is rejected
(`parse.document_block.invalid_order`).

### Placement and association rules

Ignoring blank lines and comment lines, a source's top-level items follow the canonical form:

```text
document file? before_each? (document case? case)*
```

- Document blocks are top-level constructs only; inside a case block they are syntax errors.
- A source may contain at most one `document file` block (`parse.document_file.duplicate`).
- `document file` must appear before `before_each`, every `document case` block, and every case (`parse.document_file.after_case`).
- `before_each` must likewise appear before every `document case` block and every case (`parse.before_each.after_case`); see [Execution model](execution-model.md#before_each) for its semantics.
- `document case` attaches to the next top-level case.
  Blank lines and comment lines may separate the two;
  a `document file` block or another `document case` block may not.
- At most one `document case` block may precede a case.
  A second block before the target case is rejected at the second block's start line
  (`parse.document_case.duplicate`).
- A `document case` block with no following case is rejected at the block's start line
  (`parse.document_case.orphan`).
- When one structure violates both rules — two `document case` blocks and no target case —
  the duplicate is reported, not the orphan.
- An empty document block — no documentation field, including a comment-only body — is rejected (`parse.document_block.empty`).
- A source without document blocks remains valid; documentation is always optional, per file and per case.

Each document block body is defined as a scope-specific whitelist of documentation fields in the grammar.
Actions, assertions, `write` steps, case blocks, and nested document blocks are not part of those whitelists,
so they are syntax errors inside a document block by construction,
and any future step or statement is rejected there automatically.

### Relationship to execution

Documentation lives on the source-level model only:
file documentation on `SourceFile`, case documentation on the parsed case (`SourceCase`).
The model holds exactly what the source states:
omitted fields stay unset, and no display fallback (file stem or case name as title, a default group, path-based order)
is materialized into it.
Display fallbacks are applied when the Documentation Catalog is built (#170),
where the source path, the source-level model, and the execution case name are all available.

Projection to the execution `Script` drops documentation metadata,
so execution behavior, case and step execution order, execution reports, and result artifacts are identical
whether or not a source declares document blocks.
A `document case` block is not a step of its case.
Document blocks are also not part of any case's source span —
a case's span starts at its `case` line,
excluding an associated `document case` block and the blank / comment lines separating it from the case
(see [Execution model](execution-model.md) — Parsing and the source-level model).

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
