# Fixture Reference Value Syntax

- Status: Accepted
- Created: 2026-07-06T17:00:00Z

## Context

`*.repor` e2e cases run inside a per-concrete-case isolated workspace (see [execution-model.md](../execution-model.md) — Workspace lifecycle). A `WorkspacePath` (`<"...">`, #93) refers to a file inside that workspace: it is where the code under test writes output, and it is the only kind of path a `file` / `dir` checkpoint subject accepts.

Snapshot / fixture files — expected CLI stdout JSON, expected file contents, and similar large expected values that are impractical to inline as a string literal or heredoc literal — belong conceptually to the test definition, not the workspace under test. They are naturally kept as static files next to the `*.repor` file that uses them, not inside a workspace that is created fresh, isolated, and destroyed per concrete case.

#93's ADR ([Workspace Path Literal Syntax](20260706T160000Z_workspace-path-literal-syntax.md)) already reserved `@"<path>"` as a fixture reference literal in the grammar and named the value categories:

```text
TextValue             = StringLiteral | HeredocLiteral
FileContentsReference = WorkspacePath | FixtureReference
```

but deferred `FixtureReference`'s actual introduction — its AST representation, its lexical and filesystem validation, its materialization mechanism, and which argument positions accept it — to this issue (#92).

Separately, #87 (`file` / `stdout` / `stderr contents_equals`) and #88 (`file text_equals`) are the issues that define the full comparison *behavior* those expectations perform. Because their acceptance criteria describe concrete usage (`file <"actual"> contents_equals @"expected"` must parse and be semantically valid; `file <"actual"> text_equals @"expected"` must be a semantic error), #92 needed a position that actually requires `FileContentsReference` / `TextValue` to make those rules observable — but introducing the full comparison behavior itself is explicitly #87 / #88's scope, not #92's. This ADR also records how that overlap is split.

## Decision

### `@"<path>"` is the `FixtureReference` literal

`@"<path>"` is a fixture reference literal, distinct from `"..."` (`StringLiteral`, text domain) and `<"...">` (`WorkspacePath`, case-workspace filesystem reference). `@` was chosen (over a keyword form like `fixture("path")`) because snapshot assertions are expected to be common, and a short sigil that visually pairs with `<"...">` fits the existing three-kind literal family better than a function-call-shaped keyword — the same reasoning #93 used to reject `path("...")` for `WorkspacePath`.

### `FixtureReference` is a `FileContentsReference`, never a `TextValue`

```text
FileContentsReference = WorkspacePath | FixtureReference
```

A `FixtureReference` is a reference to file content, not the content itself: resolving it requires reading a file from disk, exactly like a `WorkspacePath`. A `TextValue` (`StringLiteral | HeredocLiteral`) is inline text already present in the script. Treating a `FixtureReference` as a `TextValue` would mean silently reading a file wherever inline text is expected, which is surprising at the call site and blurs `text_equals` (inline expected text) and `contents_equals` (expected file contents) into the same expected-value shape. There is no implicit conversion between `TextValue` and `FileContentsReference` in either direction (see #93's ADR); `@"<path>"` is therefore valid as `contents_equals`'s expected value but never as `text_equals`'s.

### Semantic signature: `ExpectedValue<T>` / `ActualValue<T>`

Expectation / checkpoint operands are, at the specification level, typed as `ExpectedValue<T>` or `ActualValue<T>`:

```text
file <ActualValue<WorkspacePath>> contents_equals <ExpectedValue<FileContentsReference>>
stdout contents_equals <ExpectedValue<FileContentsReference>>
file <ActualValue<WorkspacePath>> text_equals <ExpectedValue<TextValue>>
exit <ExpectedValue<u32>>
```

A `file` / `dir` checkpoint subject is always an `ActualValue<WorkspacePath>`: it names what the code under test produced, inside the isolated workspace reportage itself controls. A `FixtureReference` is never valid there — accepting one would let an assertion subject point at the test definition's own files, inverting what a checkpoint observes vs. what it compares against. This is why the `file` subject and `contents_equals`'s expected side are asymmetric even though both may hold a `WorkspacePath`: the same literal *kind* plays a different semantic *role* (actual vs. expected) depending on position. Rust-level, this is the existing `RequiredLiteralKind` / `RequiredKind::FileContentsReference` mechanism (`parser.rs`, `model.rs`); the `ExpectedValue<T>` / `ActualValue<T>` vocabulary above is how the specification explains it without committing to that internal type name.

### `#92`'s scope vs. `#87` / `#88`'s scope

#92 implements:

- the `FixtureReference` / `FileContentsReference` AST types and their lexical validation (`FixtureReference::parse`, mirroring `WorkspacePath::parse`);
- the fixture resolution and materialization mechanism (`fixture::resolve_fixture_source`, `fixture::materialize_fixture`): resolving a fixture path against the referencing `*.repor` file's directory, rejecting symlink escapes via canonical containment, and copying validated fixture bytes into a runner-reserved area;
- minimal `contents_equals` (`file` / `stdout` / `stderr`) and `text_equals` (`file`) grammar, parsing, and literal-kind validation — enough that `RequiredKind::FileContentsReference` / `TextValue` positions exist for the rules above to be observable and conformance-tested.

#92 deliberately does **not** implement the comparison behavior itself: `evaluator::evaluate_file_expectation`'s `FileMatcher::ContentsEquals` / `FileMatcher::TextEquals` arms, and the `stdout` / `stderr` `OutputMatcher::ContentsEquals` arm, are `todo!()`. Reading actual and expected bytes, comparing them, classifying mismatches, and producing bounded diagnostics is #87's scope for `contents_equals` and #88's scope for `text_equals` — including threading the referencing `*.repor` file's source path into live evaluation so `resolve_fixture_source` can actually run against it. See [TBD.md](../TBD.md) — `contents_equals` / `text_equals` comparison evaluation.

This split means a script that uses `contents_equals` / `text_equals` today parses and passes semantic validation, but panics if actually evaluated. That is an accepted, temporary state: no example, e2e, or conformance script in this repository exercises evaluation of these expectations, only parsing (`tests/fixtures/syntax/**`, `crates/reportage-core/tests/syntax_conformance.rs`), so the gap is not user-visible until #87 / #88 land and replace the stub.

### Fixture paths are `*.repor`-file-relative, not repository-root-relative

`@"expected.json"` resolves relative to the directory containing the referencing `*.repor` file, not the repository root and not the case workspace. A fixture is conceptually an import local to one test file's directory, meant to travel with it: a case directory (its `*.repor` file plus its fixtures) can be moved, copied, or deleted as a unit. Repository-root-relative resolution was rejected because it reads as general repository file access, which sits awkwardly next to reportage's workspace-isolation story and invites fixtures shared across unrelated case directories — a shared-fixture-root design this ADR does not attempt (see Alternatives Considered).

### Fixture path policy mirrors `WorkspacePath`

A fixture path must be non-empty, relative, and free of `.` / `..` path segments — the same lexical policy as `WorkspacePath`, checked at AST construction time via `FixtureReference::parse` (`semantic.fixture_reference.empty` / `.absolute` / `.dot_segment`). Dot segments are banned in v0 for the same reason `WorkspacePath` bans them: a `..`-laden path reads as an attempt to reach outside the fixture's natural root, and there is no v0 use case that requires escaping the `*.repor` directory. This is deliberately conservative; if a real need for a shared fixture root emerges, it should be a new, explicit construct rather than a loosened dot-segment rule (see [TBD.md](../TBD.md) candidates this ADR does not create).

### Canonical containment check, not just lexical validation

A path with no `.` / `..` segment can still resolve outside the `*.repor` directory if a symlink planted under that directory points elsewhere. Lexical validation cannot see through a symlink; only resolving and canonicalizing the candidate path can. `fixture::resolve_fixture_source` therefore canonicalizes both the `*.repor` directory and the joined candidate path and verifies the latter still lies under the former (`semantic.fixture_reference.escapes_repor_directory`) before treating it as valid.

This is a different technique from `workspace.rs`'s `parent_path_is_blocked`, which defends the opposite direction (workspace content escaping out via a planted symlink) by rejecting any non-directory — including a symlink — found in a `write` step's ancestor path components (`symlink_metadata`, never `canonicalize`), so it never has to resolve where a symlink points at all. The two defenses share a goal — reject a symlink-mediated escape rather than only a lexically-visible one — but not an implementation shape: `fixture::resolve_fixture_source` must resolve *through* a possible symlink to compare final locations, since a fixture path may legitimately point at a symlink that still resolves inside the `*.repor` directory; `parent_path_is_blocked` instead rejects any symlink outright, since a `write` step's target should never traverse one.

Shell-level invalidity is deliberately not the mechanism for any of this. `$ cat @"expected.json"` is ordinary shell text, not a fixture reference — Reportage's grammar only recognizes `@"<path>"` inside `value_literal` positions that its own parser constructs, and action bodies are captured as opaque, unparsed command strings (`command = @{ ... }` in `reportage.pest`). The access boundary between "fixture available for assertion comparison" and "fixture available as an ordinary sandboxed file" is enforced by *phase separation* — fixtures are resolved and materialized during assertion evaluation, never during the action phase — not by trying to make the literal syntactically unrepresentable in shell text, which would be both unenforceable and orthogonal to what actually protects the sandbox.

### `FixtureReference` is valid only in an assertion block's `FileContentsReference` expected position

Every `value_literal` argument position already declares which `RequiredLiteralKind` it accepts (#93). `write`'s path and content positions require `WorkspacePath` / `TextValue`; `file` / `dir` subjects require `WorkspacePath`; `contains` positions require `TextValue` / `StringLiteral`. None of those accept `FixtureReference`, so a `FixtureReference` literal anywhere outside a `contents_equals` expected position — including outside any assertion block entirely, since `write` is the only non-assertion construct with a `value_literal` position — is rejected as an ordinary `semantic.literal.kind_mismatch`, exactly like any other wrong-kind literal. No separate "is this inside an assertion block" check was needed: the existing per-position kind system already makes assertion-block-only-and-`FileContentsReference`-only a single rule, not two.

### Fixtures are assertion input only; materialization happens at assertion evaluation time

A fixture is never made available as an ordinary sandbox path, and it is never placed in the workspace before an action runs. `fixture::resolve_fixture_source`'s validation may run as early as parse or semantic validation, or as late as case planning — v0 does not mandate one over the other, since nothing observable depends on exactly when validation happens as long as it happens before assertion evaluation. Materialization (copying validated bytes into a runner-reserved area) is scoped strictly to assertion evaluation time, so the guarantee "a fixture is never an action input file" holds regardless of implementation timing details. Exposing fixtures as action inputs (e.g. a future `write @"fixture" <"workspace-path">` that stages a fixture into the sandbox before an action runs) is a distinct, larger feature explicitly deferred — see Alternatives Considered.

## Alternatives Considered

### Treat `FixtureReference` as a `TextValue`

Would let `@"<path>"` work with both `text_equals` and `contents_equals`. Rejected: it forces `text_equals` to define an implicit file-read-and-decode step, blurring the "inline text" vs. "file contents" distinction the two expectations exist to keep separate, and it is a poor fit for `contents_equals`'s byte-for-byte, non-UTF-8-safe comparison contract.

### Repository-root-relative fixture paths

Considered because it would let fixtures be shared across case directories. Rejected for v0: it reads as unrestricted repository file access rather than a case-local import, undermines the "a case directory is a self-contained, movable unit" property, and a shared-fixture-root design needs its own resolution and collision rules that are out of scope here. Left as a TBD candidate if real demand emerges.

### A keyword/function form, e.g. `fixture("path")`

More explicit, but heavier at a call site that is expected to be common (snapshot assertions), and — as #93's ADR already concluded for `WorkspacePath` — reads as a function call in a language with no call syntax elsewhere, inviting authors to expect other callables. `@"..."` stays visually consistent with `<"...">` while being a plain sigil, not a call.

### Expose fixtures as action-input files

A future `write` variant that stages a fixture into the sandbox before an action runs, so the code under test could consume it directly. Deferred: it conflates "expected value for comparison" with "input file for the program under test," doubling the access-control surface (validate once for assertion-time reads, again for pre-action placement) for a use case v0 does not need yet. If it becomes necessary, it should be its own construct and its own issue, not an implicit side effect of `FixtureReference`.

### Snapshot update / approval mode

A `--update-snapshots`-style workflow that writes actual output back to the fixture file. Deferred: it introduces a write path back into the test-definition source tree, which is a materially different trust boundary than the read-only resolution this ADR defines, and needs its own safety and UX design (confirmation, diffing, partial-update semantics) independent of how fixture references are parsed and resolved.

### Introduce `contents_equals` / `text_equals` comparison evaluation in #92

Considered, since #92's acceptance criteria describe concrete `contents_equals` / `text_equals` usage. Rejected: #87 / #88 already own the comparison behavior, error classification, and bounded-diagnostic design for those expectations in detail (byte-for-byte semantics, `script_error` classification for a missing expected `WorkspacePath`, mismatch context rendering). Re-deciding that scope inside #92 would duplicate and risk diverging from #87 / #88's own design. #92 instead implements only as much grammar/parsing as is needed to make its own type rules observable, leaving evaluation as an explicit, documented `todo!()` for #87 / #88 to replace.

## Consequences

### Positive Consequences

- Fixture / snapshot files can live next to the `*.repor` file that uses them and move with it as a unit.
- The type system (`FileContentsReference = WorkspacePath | FixtureReference`, disjoint from `TextValue`) makes `text_equals` vs. `contents_equals` usage unambiguous at the call site, before either expectation's comparison behavior exists.
- Symlink escape is rejected by construction (canonicalize-then-contain), not by convention or documentation alone.
- `#87` / `#88` inherit a working `FixtureReference` literal, a tested resolution/materialization mechanism, and pre-validated positions to attach their comparison logic to, rather than needing to design the literal from scratch.

### Negative Consequences

- `contents_equals` / `text_equals` parse and pass semantic validation today but panic (`todo!()`) if actually evaluated, until #87 / #88 land. This is mitigated by keeping every conformance / example / e2e script parse-only for these expectations.
- Fixture paths cannot reference anything outside the `*.repor` file's own directory (no repository-root-relative option, no shared fixture root), which may require duplicating a fixture across multiple case directories until a shared-fixture-root design exists.
- Every fixture reference now requires two filesystem round-trips at evaluation time (canonicalize + read) versus zero for inline `TextValue`, though this is only paid when `@"..."` is actually used.

### Neutral Consequences

- `#87` / `#88` must call `fixture::resolve_fixture_source` / `fixture::materialize_fixture` from their own evaluation wiring, including threading the referencing `*.repor` file's source path into `evaluator::evaluate_case`, which does not currently receive it.
- Snapshot update / approval mode, repository-root-relative fixtures, a shared fixture root, and action-input fixtures remain open TBD candidates this ADR intentionally does not resolve.
