# Do not use ephemeral run artifacts as the primary drift baseline

- Status: Rejected
- Created: 2026-07-01T04:18:10Z

## Context

Reportage generates run artifacts for each run and each case. These artifacts are useful as evidence, diagnostics, and reproduction material, but ordinary run artifacts are expected to be ephemeral and git-ignored by default.

One considered option was to add evidence drift detection as a standalone feature that compares evidence from an arbitrary previous run with evidence from the current run.

```text
previous run evidence
vs
current run evidence
```

This option is aligned with Reportage's evidence-first direction at a conceptual level. It could detect changes in observed evidence even when ordinary assertions still pass.

However, arbitrary previous run comparison depends on storage and selection conditions outside the scenario itself:

- whether a previous local artifact directory still exists;
- whether CI artifacts were uploaded and are still retained;
- which previous run should be treated as the comparison target;
- whether environment, toolchain, instrumentation, or adapter changes are mixed into the comparison;
- whether the comparison target is reviewable by humans and agents.

Because these artifacts are not normally git-managed, they are weak as durable project policy or reviewable test baselines.

## Decision

Reportage will not adopt standalone evidence drift detection based on arbitrary ephemeral run artifacts as a primary feature.

Evidence drift must be treated, by default, as drift from an explicitly approved evidence baseline:

```text
current observed evidence
vs
approved evidence baseline
```

The approved baseline belongs to the evidence baseline / approval mode design in [#48](https://github.com/tooppoo/reportage/issues/48). It is a deliberate, reviewable comparison target rather than whichever previous run artifact happens to be available.

`evidence drift` should therefore mean approved-baseline drift unless a future command explicitly states that it is doing local debug comparison between run artifact directories.

## Non-Goals

This ADR does not reject all comparison between two run artifact directories.

A future local analysis or debug command may compare two explicit artifact directories. Such a command must not be treated as the main evidence testing model, and it must not replace approved baseline comparison.

## Alternatives Considered

### Compare against the most recent previous run automatically

This was rejected because the most recent previous run is not necessarily meaningful. It may come from a different branch, environment, toolchain, adapter configuration, coverage instrumentation mode, or local workspace state.

Automatic previous-run selection would make failures harder to review because the comparison target is implicit and may be unavailable to reviewers.

### Compare against a user-selected previous run artifact

This is more explicit than automatic selection, but it still relies on non-durable artifacts that may not be present in CI, may expire, and are not naturally reviewed in version control.

It remains useful as a possible debug command, but not as the primary evidence testing contract.

### Store all run artifacts as baselines

This was rejected because it would turn ordinary execution output into durable project state without explicit approval. It would also increase repository noise and encourage approving large evidence changes without a clear assertion-level specification.

## Consequences

### Positive Consequences

- Evidence testing has a durable and reviewable comparison target.
- Baseline changes can be reviewed through normal version-control workflows.
- Reportage avoids depending on local or CI artifact retention as its primary correctness model.
- The term `drift` remains tied to an explicit baseline rather than an arbitrary historical artifact.

### Negative Consequences

- Reportage will not provide automatic arbitrary previous-run drift detection as a main workflow.
- Users who want exploratory comparison between two local runs will need a separate debug/local analysis command if that use case is later adopted.
- Approval workflow design becomes important because the baseline carries semantic weight.

### Neutral Consequences

- Ordinary assertions remain the explicit specification mechanism.
- Evidence baselines supplement ordinary assertions by detecting unanticipated observed evidence changes.
- Baseline comparison must still account for unstable evidence such as timestamps, absolute paths, duration, coverage noise, and toolchain-dependent output.

## References

- [#48: evidence baseline / approval mode を設計する](https://github.com/tooppoo/reportage/issues/48)
- [#51: 不採用案の理由をADRとして記録する](https://github.com/tooppoo/reportage/issues/51)
