# `before_each` Is a Case-Local Setup Phase

- Status: Accepted
- Created: 2026-08-06T09:00:00Z

Supersedes [ADR: `before_each` Is Write-Only Case-Local Setup](20260723T120000Z_before-each-case-local-setup.md).

## Context

[ADR: `before_each` Is Write-Only Case-Local Setup](20260723T120000Z_before-each-case-local-setup.md) shaped `before_each` as a declarative fixture declaration: a `write`-only body, with the ban enforced by the block's own step type so an action or assertion was unrepresentable downstream of the parser.

That shape holds only while shared setup is expressible as literal file content. #227 records the case where it is not: in [enozunu#42](https://github.com/tooppoo/enozunu/pull/42#discussion_r3719222482), several cases shared a setup sequence that initialized a Git repository, ran `pwd`, and wrote a config file interpolating that result. None of it is expressible as a `write`, so every case body repeated it verbatim.

The earlier ADR deferred assertions inside `before_each` on three open questions: how a setup assertion failure is classified, how it is attributed when `before_each` has no case name or step index of its own, and whether a second assertion context is worth having at all. The third question was posed about a `write`-only block, where there is almost nothing to assert. Once setup can run commands, it stops being rhetorical: an action updates the checkpoint without failing on a non-zero exit, so setup that cannot assert is setup that cannot tell whether it worked.

## Decision

### `before_each` is a setup phase of each concrete case, not a fixture declaration

`before_each` is declared at module level, but it executes as part of each concrete case, inside that case's isolated workspace, before the case body's first step. It must not be executed once into a shared location, and no state produced by one concrete case's setup may reach another.

### `before_each` shares the case body's step surface

`before_each` must hold the same step model as a case body, in source order. A step kind added to a case body should become available in `before_each` without a second decision; an exception must be justified by a reason specific to that step kind.

`$` action steps, `assert` blocks, and `write` steps are accepted. `let` runtime evidence bindings, and binding references inside `write` content, are rejected for now — not because a binding is unsuited to setup, but because the binding scope that spans the two phases is a separate decision, recorded when it is made. It is made in [ADR: Runtime Evidence Binding Scope Across `before_each` and the Case Body](20260806T140000Z_before-each-binding-scope.md).

Which steps `before_each` accepts is therefore a parser rule, not a property of its step type. This gives up the structural guarantee the superseded ADR valued, in exchange for one step model and one executor across both phases.

### Action non-determinism is not a reason to ban actions

The superseded ADR banned actions because a replayed command may be non-deterministic, which would give concrete cases different starting states.

That risk is real and unchanged, but it is the script author's to manage, exactly as it is inside a case body: reportage already runs arbitrary commands there, and does not attempt to prove any of them deterministic. Enforcing determinism only in `before_each` does not make a suite deterministic — it moves the same commands into case bodies, where they are duplicated and equally unproven. The cost of the ban is paid on every script; the protection it offers is partial.

Guaranteeing setup determinism remains a non-goal.

### Actions and assertions are permitted together

Actions must not be permitted in `before_each` unless assertion blocks are permitted alongside them. This constrains the language design, not each individual `before_each` body: a block containing an action but no assertion is accepted, exactly as a case body's action needs no assertion immediately after it.

The reason the two travel together is that a `$` step only updates the checkpoint; a non-zero exit is not by itself a failure. Setup that can run a command but cannot verify it fails silently and surfaces later as a confusing case body failure.

### A setup failure belongs to the concrete case being set up

`before_each` runs for a specific concrete case, so its failures are that case's:

- an assertion failure must fail that concrete case, and its case body must not run;
- a script or runtime error must give that concrete case the corresponding error status, and its case body must not run;
- one concrete case's setup failure must not stop any other concrete case from being set up and run.

This answers the superseded ADR's first two deferred questions. The failure is one failure per concrete case, not a module-level abort, and it is attributed to that case by name and to its step by a phase-aware step origin.

### The case body starts at its own initial checkpoint

Two checkpoint boundaries exist per concrete case:

- `before_each` starts at a **setup-entry checkpoint**: workspace state, no last action result. A process expectation before the block's first action is an initial-checkpoint error.
- The case body starts at a **body-entry checkpoint**, which carries the workspace state `before_each` produced but **must not** carry the last setup action's process evidence.

Workspace state carries over because producing it is why setup exists. Process evidence must not, because a case body's first `exit` / `stdout` / `stderr` would otherwise describe whichever command the module-level setup happened to end with — a coupling the case body never expressed and cannot see. A setup action's own result is verified inside `before_each`, where it is written.

### A setup assertion does not satisfy the case body's assertion requirement

Each case body must still contain at least one `assert` block. A `before_each` assertion verifies setup, and must not count toward that requirement: otherwise a case that verifies nothing about its own subject is accepted because the shared setup checked itself.

### Results carry a phase-aware step origin

`before_each` and a case body are separate source blocks that each number their steps from zero, so a step index alone cannot locate a step. Every step-attributed result — action result, assertion block result, and step-attributed script or runtime error — must carry both the phase and a phase-local, 0-based step index that counts every step kind.

`checkpoint: "initial"` on an assertion means the phase-entry checkpoint of that assertion's own phase, not "no action has run in this concrete case". The two initial checkpoints are told apart by the assertion's phase.

## Alternatives Considered

### Keeping the `write`-only body and adding a separate setup-command block

A second block kind (`before_each_exec`, or similar) permitted to run commands, leaving `before_each` declarative.

Rejected: it splits one concept across two blocks whose only difference is which steps they accept, and forces authors to decide which block a given setup step belongs to before knowing whether the next step will need the other. The ordering between the two blocks would then be a third decision.

### An allowlist of setup-oriented commands

Permitting `$ mkdir`, `$ cp`, and similar, while rejecting arbitrary commands.

Rejected, as in the superseded ADR: no mechanical line separates a setup command from any other, so the allowlist either grows without end or blocks legitimate setup. Nothing about this alternative improved when actions were permitted.

### Carrying the last setup action's process evidence into the case body

Leaving the checkpoint untouched across the phase boundary, so a case body's first `exit` describes the last setup action.

Rejected: it makes every case body's opening implicitly depend on how the shared setup happens to end, and a change to setup's last step would silently change what an unrelated case body's first assertion asserts.

### Counting a `before_each` assertion toward the case body's assertion requirement

Rejected: a case would then pass its structural check without asserting anything about its own subject, which is exactly the failure mode the requirement exists to prevent.

## Non-Goals

- `before_all`, `after_each`, `after_all`.
- Repository-level shared fixtures.
- Guaranteeing that setup is deterministic, or proving that a shell action has no effect outside the workspace.
- Module-scope parameter declarations.
- Reusable step blocks. `before_each` extracts a common prefix of case bodies; extracting a common suffix after case-specific work is a different mechanism.

## Consequences

### Positive Consequences

- Setup that needs a command is expressible once per module instead of once per case.
- Setup can verify itself where it is written, instead of failing silently and surfacing later.
- A step kind added to a case body is available in `before_each` without a second design decision, and runs through the same executor with the same semantics.
- A setup failure names the concrete case it belongs to, and the phase and step within it.

### Negative Consequences

- The write-only rule is no longer structural: `BeforeEach` can hold any `Step`, so the parser is the only thing that keeps a banned step out.
- `before_each` can now be as non-deterministic as any case body, and reportage does not detect it.
- Two initial checkpoints exist per concrete case, so `checkpoint: "initial"` must always be read together with the phase.

### Neutral Consequences

- Removes the `parse.before_each.action_step` and `parse.before_each.assertion_block` diagnostic codes. Per [Diagnostic Codes](../reference/diagnostics.md), v0 does not commit to a strict semver policy for codes; this ADR is the record of their removal.
- `let` inside `before_each` remains rejected (`semantic.binding.before_each_forbidden`) until the cross-phase binding scope is decided. See [ADR: Runtime Evidence Binding Scope Across `before_each` and the Case Body](20260806T140000Z_before-each-binding-scope.md).
