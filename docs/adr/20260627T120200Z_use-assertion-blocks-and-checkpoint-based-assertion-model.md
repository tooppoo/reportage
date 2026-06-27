# Use Assertion Blocks and Checkpoint-Based Assertion Model

- Status: Accepted
- Created: 2026-06-27T12:02:00Z
- Supersedes: [20260627T120100Z action-assertion-separation](20260627T120100Z_action-assertion-separation.md)

## Context

The action / assertion separation established in the superseded ADR captures the right core principle: actions and assertions are distinct step kinds, source order is preserved, and phased execution is explicitly rejected.

However, the assertion syntax and grouping model needed further refinement:

- The original `assert ${expectation}` / `assert exit 0` form handles one expectation per line. When a single checkpoint requires multiple expectations, grouping is implicit and the failure aggregation unit is unclear.
- Consecutive single-line assertions were described as targeting "the same preceding action as a group", but block boundaries defined by source proximity are fragile — empty lines, comments, and reader expectations can make the grouping ambiguous.
- The `1 assertion = 1 expression` model causes multiple expectations per checkpoint to be treated as independent assertions without an explicit grouping construct. This is similar to assertion roulette: individual failures may be reported, but the reader cannot tell which failures belong to the same verification intent.
- reportage needs to express a single checkpoint assertion with multiple expectations and report all of them in one pass, rather than stopping at the first failing expression.
- Defining an assertion as "a check against the most recent action" makes it awkward to write precondition assertions before any action has run, or multi-action assertions that verify state after several steps.
- Describing the relationship between action and assertion as "attached" risks being misread as phased execution, where all actions run before any assertions are evaluated. This is explicitly wrong for reportage.

## Decision

### Assertion syntax

v0 assertion syntax is `assert { expectations }`.

The single-line `assert ${expectation}` form (e.g., `assert exit 0`) is **not** adopted as v0 syntax.

### Assertion block

`assert { ... }` is an **assertion block**. An assertion block is a checkpoint-level verification construct, not a construct attached to the nearest action.

### Expectation

Each item within an assertion block is an **expectation**. An expectation is an individual expected condition. Examples: `exit 0`, `stderr empty`, `dir exists .rellog`, `file exists .rellog/config.yml`.

Concept: `1 assertion block : n expectations`.

### Checkpoint model

An assertion block evaluates the **current checkpoint**. A checkpoint is the observable evidence context available at a point in case execution.

- At case start, there is an **initial checkpoint**. The initial checkpoint has workspace state but no last action result.
- After a `$ ...` action completes, the checkpoint is updated with the action result (exit status, stdout, stderr) and the post-action workspace state. This is an **action-updated checkpoint**.
- An assertion block does not modify the checkpoint.

### Evidence requirements

Expectations declare their evidence requirement:

- **Workspace expectations** (`dir exists`, `dir not exists`, `file exists`, `file contains`, `file-count`, etc.) require only workspace state. They are valid at the initial checkpoint.
- **Process expectations** (`exit`, `stdout`, `stderr`) require the last action result. If a process expectation appears in an assertion block at a checkpoint with no last action result (i.e., before any `$` action in the same case), it is a **script error**.

### Block evaluation semantics

- All expectations within a block are evaluated independently.
- Failures are reported per expectation.
- If one or more expectations fail, the assertion block is a failure.
- After a block failure, the same concrete case does not proceed to its next action. The runner may proceed to the next concrete case.

### Assertion blocks are not side-effectful

Assertion blocks and expectations are side-effect-free. They observe the checkpoint; they do not modify it.

### Source order

Case body steps are executed in source order. Actions and assertion blocks are not separated into phases. Specifically: all actions are not run first and assertions evaluated afterward. That model is explicitly rejected.

## Alternatives Considered

### `assert ${expectation}` / `assert exit 0`

The original single-line form where each assertion is a separate statement.

Rejected because:
- One expectation per statement makes grouping multiple expectations per checkpoint implicit and ambiguous.
- Failure aggregation unit is unclear: does a failure on one line stop the next assertion line, or are they all evaluated?
- The `1 assertion = 1 expression` model aligns poorly with the concept of a checkpoint-level verification block.
- Makes it harder to express the mental model: "I am now asserting several things about this checkpoint in one go."

### Consecutive single-line assertions as implicit block

Treating consecutive `assert` lines without an intervening action as an implicit assertion block.

Rejected because:
- Whether an empty line or comment separates a block is ambiguous.
- Source-level proximity is a fragile basis for semantics in an indentation-insensitive, block-based syntax.
- Readers may expect fail-fast behavior for each separate `assert` line rather than grouped aggregation.

### Phased execution

Running all actions in a case first, then evaluating all assertions afterward.

Rejected because:
- Assertions that depend on intermediate state (state between two actions) would observe the wrong state.
- After an assertion failure, actions that should not run would already have run.
- Failure localization degrades: the relationship between a failing assertion and the action that produced the observable state is obscured.

### Implicit `$ true` at case start

Inserting a fake no-op action to satisfy the "assertion must follow action" constraint at case start.

Rejected because:
- Creates a spurious action result at the initial checkpoint.
- Process expectations (`exit 0`, `stdout empty`, `stderr empty`) would become valid at the initial checkpoint even when no actual action has run. This is misleading in artifacts, diagnostics, and reports.
- Violates the principle of explicit, simple, and direct test scenarios.
- The correct concept is an **initial checkpoint** with workspace state and no last action result, not an implicit action.

### `$ ... { ... }` action-attached assertion syntax

```reportage
$ rellog init {
  exit 0
  dir exists .rellog
}
```

A syntax where expectations are written inline inside the action step.

Explored because:
- The action and its expected outcomes appear in one visual unit.
- Expectations are co-located with the action that produces the observable state.

Rejected for v0 because:
- The boundary between POSIX shell syntax passed to `sh -c` and reportage assertion syntax is ambiguous. `{` and `}` are valid shell syntax and command arguments.
- Choosing a delimiter that is unambiguous in all shell contexts is difficult.
- Precondition assertions (before any action) would still require a standalone `assert { ... }` form. Having two forms for assertions (action-attached and standalone) creates syntactic inconsistency.
- An assertion block is a checkpoint-level construct, not an action-attached construct. Embedding it in the action step conflates two distinct concepts.
- If needed in a future version, action-attached syntax sugar can be designed with the full context of what reportage syntax would look like at that point.

### `assert` / `require` split

Splitting assertions into hard assertions (`require`, stops case immediately on failure) and soft assertions (`assert`, collects all failures).

Rejected for v0 because:
- Adds two concepts and two keywords where one is sufficient.
- The v0 model — assertion block failure stops the case, all expectations within a block are evaluated — is already a clear single model.
- If a hard vs. soft distinction is needed later, it can be introduced as a property of the block, not a new keyword.

## Consequences

### Positive

- Checkpoint-level verification is explicit. The assertion block is a named, bounded construct that corresponds directly to a checkpoint.
- Precondition assertions (before any action), post-action assertions, and multi-action intermediate assertions all use the same model.
- Multiple expectations per checkpoint can be evaluated in a single pass, with independent failure reporting. Assertion roulette is avoided.
- The source-order execution model is preserved. Phased execution cannot arise from the syntax.
- Rust domain model maps cleanly: `AssertionBlock`, `Expectation`, `Checkpoint`, `EvidenceRequirement` are distinct, explicit types.

### Negative

- A single expectation still requires the `assert { ... }` block form. `assert { exit 0 }` is slightly heavier than `assert exit 0`.
- The concepts of `expectation`, `checkpoint`, and `evidence requirement` need to be explained to users who are unfamiliar with the model.

### Neutral

- A single-line multiple expectation form such as `assert { exit 0; stderr empty }` is a future candidate. If adopted, `;` would be the explicit expectation separator. Not adopted in v0.
- The single-line `assert ${expectation}` form could be added later as syntactic sugar over a single-expectation block. Rejected for v0 to keep the syntax small and the model unambiguous.
