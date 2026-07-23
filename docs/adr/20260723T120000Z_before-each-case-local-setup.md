# `before_each` Is Write-Only Case-Local Setup

- Status: Accepted
- Created: 2026-07-23T12:00:00Z

## Context

Reportage's semantics give every concrete case an isolated workspace (see [ADR: Adopt `write` Step, Fenced Raw Text Block, and Real Per-Case Workspace Isolation](20260704T183546Z_write-step-and-per-case-workspace-isolation.md)). When several cases in one module need the same input fixture, config file, or expected file, each case body has to repeat the same `write` steps.

#70 introduces `before_each { ... }` as a module-level block whose steps are replayed inside each concrete case's workspace, after the workspace is created and before the case body's first step. This ADR records why the block is shaped as case-local setup with a `write`-only body, rather than any of the more permissive designs.

## Decision

### `before_each` is case-local setup, not a repository-level fixture archive

testscript/txtar-style runners describe shared fixtures as a repository-level archive that is unpacked for every test. Reportage instead keeps setup inside the script's own source order and workspace evidence model: `before_each` is a block of ordinary Reportage steps, replayed per concrete case. Setup therefore stays visible in the same file, uses the same `write` syntax, path safety policy, and diagnostics as case body writes, and produces files that are ordinary workspace evidence.

### `before_each` is replayed inside each concrete case workspace, never shared

`before_each` runs once per concrete case — including every case produced by parameter variants — inside that case's own workspace. It is not executed once into a shared location, and no state carries over between cases.

Sharing a setup product across cases would recreate exactly the shared mutable test state that per-case workspace isolation exists to eliminate: one case mutating a seeded file would leak into the next case's observed evidence. Replay keeps every case's starting state identical and explicit, at the cost of repeating the writes, which are cheap local file operations.

### The initial checkpoint is established after `before_each` has run

The initial checkpoint's workspace evidence includes every file `before_each` wrote, so a case body's first assertion block can verify the seeded state before any action runs. If the checkpoint were established before `before_each`, precondition assertions could not see the setup — which is the primary reason the setup exists.

### The body is limited to `write` steps

`before_each` accepts `write` steps only, and must contain at least one.

`write` is deterministic, declarative, and confined by the workspace path safety policy; replaying it per concrete case is guaranteed to produce identical state each time. Every other current step kind breaks one of those properties (see the bans below).

### Action steps are banned uniformly, regardless of kind or purpose

A `$` action inside `before_each` is rejected (`parse.before_each.action_step`) whether it is a "setup-oriented" command (`$ mkdir -p`, `$ cp -R`) or not.

Distinguishing acceptable setup commands from unacceptable ones is not mechanically decidable: any shell command can be non-deterministic, environment-dependent, or mutating beyond the workspace. A replayed non-deterministic action would silently give different cases different starting states, defeating the purpose of shared setup. A uniform ban keeps the rule explainable in one sentence and keeps every command execution inside case bodies, where its result is attributed to a named case and observable through the normal action/checkpoint model. Setup commands remain fully expressible — written explicitly in each case body that needs them.

### Assertion blocks are banned; verification belongs to case bodies

An `assert` block inside `before_each` is rejected (`parse.before_each.assertion_block`). An assertion needs a failure classification and reporting attribution (which case failed, at which step), and `before_each` has neither a case name nor a case body step index; it would also fire once per concrete case replay. Rather than settle those questions prematurely, v0 verifies setup with workspace expectations at the start of each case body. Whether `before_each` should ever accept assertion blocks is deferred with its open questions recorded in [Deferred topics](../planning/TBD.md).

### Enforcement shape

The grammar accepts the full case-body step surface inside `before_each` and the parser rejects banned step kinds during AST construction, so each violation gets an actionable diagnostic naming the ban and the alternative (`parse.before_each.action_step`, `parse.before_each.assertion_block`) instead of a bare syntax error. The domain model then makes the policy structural: `BeforeEach` holds side-effecting steps only, so an action step or assertion block is unrepresentable downstream of the parser. Placement (at most one block, before the first case, never between a `document case` block and its target case) is enforced the same way as document block placement (`parse.before_each.duplicate`, `parse.before_each.after_case`).

## Alternatives Considered

### Repository-level fixture archive (testscript/txtar style)

A per-module or per-repository fixture directory or archive, unpacked into every case workspace.

Rejected: it moves setup out of the script's source order into a second format, bypasses the `write` step's path safety and diagnostics, and makes the seeded state invisible when reading the script. A future bulk-import mechanism can still be added on top of the workspace boundary rules; it is explicitly out of scope for #70.

### Allowing setup-oriented shell actions in `before_each`

Permitting `$ mkdir -p ...` / `$ cp -R ...` style commands, as an earlier draft of the execution model suggested.

Rejected: no mechanical line separates setup commands from arbitrary ones, so the permission is either unenforceable prose or an ever-growing allowlist. Replayed shell commands also reintroduce non-determinism into what must be an identical starting state for every concrete case.

### Running `before_each` once and copying its result into each workspace

Execute the block once, snapshot the resulting files, and copy them into every case workspace.

Rejected: with a `write`-only body the observable result is identical to replay, so the copy adds a hidden intermediate state for no observable gain — and if the body ever grew action steps, the snapshot would become genuinely shared state produced by a single execution, with cross-case leakage on any non-determinism.

### Establishing the initial checkpoint before `before_each`

Rejected: precondition assertions at the top of a case body could then not observe the seeded files as workspace evidence, so scripts could not verify their own setup.

## Consequences

### Positive Consequences

- Shared Arrange state is written once per module instead of once per case, without introducing shared mutable state between cases.
- Every concrete case — including parameter variant expansions — starts from an identical, explicitly declared workspace state.
- Setup files are ordinary workspace evidence at the initial checkpoint, so case bodies can assert on them before any action.
- The write-only rule is structural (`BeforeEach` cannot hold an action step), so downstream layers cannot regress it.
- Banned constructs fail with diagnostics that name the ban and the alternative, not with generic syntax errors.

### Negative Consequences

- Setup that genuinely requires a command (unpacking an archive, generating a keypair) must be repeated in each case body; `before_each` cannot express it.
- Setup results cannot be asserted centrally; each case body that cares must repeat its own precondition assertions.

### Neutral Consequences

- `before_each` failure is a runtime step error for the concrete case being set up, attributed to the module-level block by message (with no case body step index).
- `before_all`, `after_each`, `after_all`, module-scope parameters, and variant-binding access from `before_each` remain out of scope, unchanged by this decision.
