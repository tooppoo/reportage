# Adopt `write` Step, Fenced Raw Text Block, and Real Per-Case Workspace Isolation

- Status: Accepted
- Created: 2026-07-04T18:35:46Z

## Context

External trial use of Reportage (#68) surfaces a recurring Arrange need: writing a text file into the workspace before or during a `case` — an expected file to compare stdout/stderr/another file against, or an input fixture the system under test reads. v0 had no way to express this without delegating to `$ cat > path <<'EOF' ... EOF` inside a shell action, which pulls file creation out of Reportage's own Arrange / evidence model and into ad-hoc shell heredoc syntax per script.

Separately, while implementing this, we found that Reportage's v0 evaluator did not actually give each concrete case an isolated workspace, despite `docs/semantics.md` already describing that model. `$` actions ran with no explicit working directory (inheriting the reportage process's own cwd), and `file "<path>" ...` expectations resolved paths against that same inherited cwd. In practice, every case in a run shared one directory — the directory the CLI was invoked from. This was invisible as long as the only way to get a fixture file into that directory was to seed it from outside the process (as the existing integration test suite did). A create-only `write` step makes the gap immediately observable: two cases in the same script writing the same relative path would collide if workspaces were not actually isolated per case. Introducing `write` therefore required implementing real per-case workspace isolation as a prerequisite, not a follow-up.

## Decision

### 1. `write` step, not a tail-of-file fixture archive

Reportage adopts an Arrange-position `write` step, evaluated in `case` body source order alongside `$` actions and `assert` blocks:

```reportage
case "invalid config" {
  write "reportage.kdl" ```
    reportage {
      config {
        version 999
      }
    }
    ```

  $ reportage validate

  assert {
    exit 2
    stderr contains "unsupported config version"
  }
}
```

This is rejected in favor of a testscript/txtar-style trailing file archive because Reportage's execution model is already organized around source-order evaluation of one case body, not a two-region file (script header + trailing fixture archive). A `write` step keeps the fixture's content next to the step that depends on it, rather than requiring the reader to jump to a named block elsewhere in the file.

### 2. `write` keyword, not `file` / `create file` / `create_file` / `create` / `out`

`file` was rejected because it collides with the `file "<path>" exists` / `file "<path>" contains "<text>"` assertion subject already in the grammar — the same token would mean "the thing I'm asserting about" in one position and "the thing I'm creating" in another. `create file` is unambiguous but verbose for a step expected to appear often. `create_file` reads as an internal function name, not DSL surface syntax. `create` is too general — v0 deliberately does not generalize "creation" as a step category (see below). `out` collides in spirit with `stdout` / `stderr` / output artifacts. `write` is short, names the operation (not a category of thing being created), and does not collide with any existing keyword.

### 3. Only text file content authoring is DSL-privileged; directory/symlink stay in `$` actions

`write` does not gain sibling `create dir` / `create symlink` forms in v0. Directory creation and symlink creation are already expressible with ordinary shell actions (`$ mkdir -p ...`, `$ ln -s ...`) and don't need a byte-for-byte content model the way file *content* does. Text file content is the one thing that is painful to author via shell redirection (quoting, escaping, indentation) and that is used constantly for comparison fixtures — so it alone is worth a privileged, indentation-aware DSL construct.

### 4. `write` is a side-effecting step, a third step kind alongside action and assertion block

```rust
pub enum Step {
    Action(ActionStep),
    AssertionBlock(AssertionBlock),
    SideEffect(SideEffectingStep),
}

pub enum SideEffectingStep {
    WriteFile(WriteFileStep),
}
```

An action executes the subject under test; an assertion block is side-effect-free and verifies a checkpoint; a side-effecting step changes workspace state directly and verifies nothing. Modeling `write` as its own `Step` variant (not folded into `ActionStep` or given a bespoke one-off enum) gives future side-effecting steps — should any be added — a shared home and a shared failure classification, without overloading what `assert { ... }` means.

### 5. A side-effecting step's failure is a runtime step error, never an assertion failure

`write` has nothing to compare against evidence, so "it failed" cannot be an assertion failure — there was no expectation. Reportage classifies `write` failures into three tiers, matching where in the pipeline they are detected:

- **Parse error** — malformed syntax: unterminated fenced block, an inline comment on a fence line, a non-blank body line indented less than the closing fence (`parse.raw_block.shallow_indent`).
- **Parse-domain validation error** — an unsafe `WorkspacePath`: empty, absolute, or containing a `.` / `..` segment (`semantic.workspace_path.empty` / `.absolute` / `.dot_segment`). This is detected by `WorkspacePath::parse` while the AST is being constructed, the same phase that already rejects an empty logical-composition block — so it is surfaced as a `ParseError`, exactly like that existing case, even though its code lives in the `semantic.*` namespace.
- **Runtime step error** — detected only once the step actually runs: the target path already exists (create-only), a regular file blocks part of the parent path, or the OS write itself fails (`step.write.target_exists` / `.parent_not_a_directory` / `.io_error`). This introduces a new `step.*` diagnostic namespace alongside `parse.*` / `semantic.*` / `assertion.*`.

A runtime step error stops the concrete case at that point — later steps do not run — the same way an assertion block failure does, but it is reported as a `runtime_error` run outcome (exit code `3`), not `test_failed` (exit code `1`). See [exit-codes.md](../reference/exit-codes.md) and [semantic-diagnostics.md](../reference/semantic-diagnostics.md).

### 6. Fenced raw text block, not a `<<NAME` heredoc

```reportage
write "expected/stdout.txt" ```
  expected output
  ```
```

A heredoc (`<<TXT ... TXT`) is equally capable but requires picking and typing a delimiter name for every block — friction that adds up when `write` is used often for expected-output fixtures. A Markdown-familiar fenced block needs no delimiter name, and its length is meaningful: an opening fence of three or more backticks is closed by a run of at least as many of the same character, so a script that needs to embed a literal triple-backtick block (e.g. a Markdown fixture) just opens with four backticks instead of inventing a delimiter word.

Fence matching is implemented with pest's match-stack operators (`PUSH` / `PEEK` / `DROP`): the opening fence's backtick run is pushed once, every following line is tested against `PEEK` (at least that many of the same character) to decide whether it is the closing fence, and `DROP` clears the stack entry once the block is fully matched. This keeps fence-length tracking inside the grammar rather than in a hand-rolled post-parse scanner.

**Known limitation (shared with heredocs and Markdown fences generally):** a `write` step that is missing its own closing fence does not always fail with a syntax error. Because the grammar scans forward for *any* line shaped like a valid closing fence (correct indentation, same fence character, sufficient length), a missing closing fence can instead be satisfied by a line meant as a *different*, later `write` step's own closing fence — silently absorbing everything in between, including that later step's opening line, as literal content, with no diagnostic at all. This is the same class of footgun as forgetting a heredoc's terminator: the parser cannot distinguish "the intended terminator is missing" from "the block legitimately contains a lot of content" without unbounded lookahead, which would conflict with pest's single-pass grammar model. The mitigation is scripting discipline (keep each `write` step's opening and closing fence visually paired, use `UPDATE_AST_SNAPSHOTS=1` / AST snapshot review to catch an unexpectedly-absorbed step), not a grammar change.

### 7. Raw text block is separate from variable expansion / template block

`${VAR}`-shaped text inside a `write` block is preserved as a literal string; v0 performs no expansion inside it, regardless of whether the case is parameterized. Whether Reportage should support variable expansion at all, and what a template-block form would look like, is deliberately left to a separate issue (#71) so that the raw text block's own fence / dedent / path semantics can ship without also having to settle expansion semantics at the same time.

### 8. File creation is confined to the case-local workspace; the repository root is never implicitly referenced

`write`'s path always resolves against the current concrete case's workspace root. There is no mechanism for a `write` step, or any file expectation, to implicitly reach a file under the repository root. This is more restrictive than testscript/txtar's transparent repository-relative fixture layout, but it keeps Reportage's evidence model legible: every file a `write` step touches is workspace-local, temporary, and disposable, never a repository file a reader has to trace back to a fixture directory. A future repository-fixture mechanism (`fixture` / `copy` / `import`, or a repository path literal) would have to make the repository/workspace boundary explicit — a `write` step's path and a file expectation's subject path will remain rejected as a semantic error if a repository path literal is ever passed where a workspace path is expected.

### 9. Workspace path is a domain type, with validation centralized in one parse function

```rust
pub struct WorkspacePath(String);

impl WorkspacePath {
    pub fn parse(raw: &str) -> Result<Self, WorkspacePathError> { ... }
}
```

The parser never holds a `write` step's path as a plain `String`; it must go through `WorkspacePath::parse`, which is the single place that rejects an empty path, an absolute path, and `.` / `..` segments. This means no future caller can construct a `WriteFileStep` with an unchecked path — the type itself is the guarantee. It also gives the model a place to grow: if a repository path literal is introduced later, model-layer code can distinguish `WorkspacePath` from a `RepositoryPath` by type, not by a runtime tag.

**Known scope limitation:** this PR only migrates `write`'s own path to `WorkspacePath`. `file "<path>" ...` assertions (`FileExpectation`) still hold `path: String` and are validated separately, at evaluation time, by `semantic::validate_file_path` (`semantic.file_path.absolute` / `.dot_segment`) rather than by `WorkspacePath::parse`. The two validations enforce the same rule set today, but as two independent implementations rather than one shared type — a future `dir` / `file-count` / repository-fixture addition could let them drift. Unifying `FileExpectation.path` onto `WorkspacePath` too is deliberately left to a follow-up rather than bundled into this already-large PR; until then, both call sites must be kept in sync by hand when the path-safety rule set changes.

### 10. Parent directories are created automatically; overwrite and append are deferred

`write` creates missing parent directories, because forcing every script to precede a nested `write` with `$ mkdir -p ...` would defeat the purpose of a terse Arrange step. Overwrite and append modes are explicitly not introduced in v0: silent overwrite is the one behavior `write` is designed to never do (see decision 5), and a forced-overwrite escape hatch, if ever needed, should be its own explicit keyword (e.g. `overwrite`) rather than an option-like modifier such as `write --force`, so a destructive capability is never one flag away from the default, terse form.

### 11. Real per-case isolated workspace, implemented now rather than deferred

Each concrete case now gets its own temporary directory (`reportage_core::workspace::Workspace`, backed by `tempfile::TempDir`): `$` actions run with it as their working directory, `write` steps write into it, and `file "<path>" ...` expectations resolve against it — replacing the prior behavior of inheriting the reportage process's own working directory for all of these. The workspace is destroyed when the concrete case's evaluation finishes (`Workspace`'s `Drop`), matching `docs/semantics.md`'s existing "workspace lifecycle" description, which up to this point had not been backed by an actual per-case directory.

This was necessary, not optional, for this issue: `write`'s create-only semantics only make sense if "the workspace" is actually scoped to one concrete case. Without it, two cases writing the same relative path in the same run would spuriously collide with `step.write.target_exists`, and a script author would have no way to reason about why.

## Alternatives Considered

### `create file "<path>" ``` ... ``` `

More explicit than `write`, but verbose for a step expected to be used often, and "creation" reads as a broader category than what v0 actually privileges (text file content only).

### `<<NAME` heredoc

Robust and Rust/shell-familiar, but requires a delimiter name per block. Rejected in favor of the fenced block for scripts that lean on `write` heavily for expected-file fixtures.

### Reject `write` step entirely; require `$ cat > path <<'EOF' ... EOF`

Adds no new core syntax, but pulls Arrange-phase file creation out of Reportage's evidence model entirely — the runner would have no first-class knowledge that a file was deliberately authored as fixture content versus incidentally produced by the subject under test.

### Treat a `write` step failure as an assertion failure

Rejected. `write` has no expectation to compare against evidence; collapsing "the write step itself failed" into the same failure category as "the assertion did not hold" would make `test_failed` mean two different things.

### Defer real workspace isolation to a later issue, ship `write` against the current shared-cwd model

Considered and rejected during implementation. It would make `write`'s create-only semantics unreliable the moment a script has more than one case, and would leave the already-written `docs/semantics.md` workspace lifecycle description backed by nothing.

## Consequences

### Positive Consequences

- Reportage gets a terse, indentation-aware way to author comparison and input fixture files directly in Arrange position, without falling back to shell heredocs.
- `write` failures are classified precisely (`parse.*` / `semantic.workspace_path.*` / `step.write.*`), each mapped to the exit code and run-outcome category that already existed for that severity tier.
- Every concrete case now runs in a genuinely isolated workspace, closing a real gap between `docs/semantics.md` and the implementation, and de-risking future features (`before_each`, parameterized `case` variants) that assume per-case isolation.
- `WorkspacePath` gives the model layer a reusable, type-enforced path-safety boundary that a future repository-fixture mechanism can build on without redoing this validation.

### Negative Consequences

- `$` action working-directory behavior changed: scripts (in this repository's own test suite) that relied on the reportage process's own cwd doubling as the workspace had to be rewritten to create their fixtures via `write` or a `$` action instead of external pre-seeding.
- The evaluator, executor, and `Checkpoint` / `WorkspaceState` model all had to thread a workspace root parameter that did not exist before; this is a larger diff than `write`'s own grammar and model would have required in isolation.
- A new `step.*` diagnostic namespace and a new `RawTextBlock` / `WorkspacePath` domain type add surface area that documentation, tests, and future step kinds now need to stay consistent with.
- A `write` step missing its own closing fence can silently absorb a later, syntactically valid `write` step as literal content instead of failing to parse (see decision 6). This is a known, documented, and tested limitation, not something this PR resolves.

### Neutral Consequences

- Variable expansion / template blocks remain unresolved and are explicitly deferred to #71; `write`'s raw text block ships without prejudging that decision.
- `before_each` still does not exist in v0; this ADR only makes `write` usable inside a `case` body, not inside a not-yet-implemented `before_each` block.
- Binary fixtures, file mode / executable bit, symlink creation via `write`, append mode, and bulk repository fixture import remain out of scope, unchanged from the issue's stated non-goals.
