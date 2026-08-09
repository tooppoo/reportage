# Phase-Aware Step Origin in the JSON Contracts

- Status: Accepted
- Created: 2026-08-08T14:00:00Z

Follows [ADR: `before_each` Is a Case-Local Setup Phase](20260806T090000Z_before-each-case-local-setup-phase.md), which decided that results carry a phase-aware step origin, and revises two consequences of [ADR: Canonical Artifact Run Result Manifest](20260708T130500Z_artifact-run-result-canonical-manifest.md).

## Context

A concrete case now runs two step sequences: the module-level `before_each` block, replayed inside the case's workspace, and the case body. Each numbers its steps from zero, so a step index alone no longer identifies a step.

The internal result model already carries a `StepOrigin` on every step-attributed result. Neither external contract carried it, so `result.json` and `--format=json` could not say whether an action, an assertion, or a step-attributed diagnostic came from setup or from the case body.

[ADR: Canonical Artifact Run Result Manifest](20260708T130500Z_artifact-run-result-canonical-manifest.md) recorded, when the manifest was introduced, that "step indices and per-step counts of the old shape are no longer recorded in the manifest; failure positions are carried by diagnostics instead", and that the two `schemaVersion` fields "currently share the value `1` but version independently". The first is reversed here; the second's stated value is now `2`.

## Decision

### Both contracts carry a phase-aware step origin

`spec/artifacts/run-result/schema.json` and `spec/output/json-report/schema.json` each define a `StepOrigin` object:

```json
"step": { "phase": "before_each", "index": 0 }
```

`phase` must be `before_each` or `case`. `index` must be 0-based and local to that phase, counting every step kind — action, assertion block, binding, write. It is required on every `actions[]` and `assertions[]` entry, and present on a `diagnostics[]` entry only when the diagnostic is attributed to one step.

Actions keep a single case-global numbering in `actions[]`, in execution order, so a `before_each` action and a case body action share one sequence. `step.index` must not be read as a position in `actions[]`, and an index must not be compared across phases.

### `checkpoint: "initial"` is phase-relative

`"initial"` means the phase-entry checkpoint of the assertion's own `step.phase`, not that no action has run in the concrete case. A case body assertion placed before that body's first action reports `"initial"` even when `before_each` already ran actions, because the body-entry checkpoint does not carry their process evidence.

The two initial checkpoints are told apart by the assertion's `step.phase`.

### `schemaVersion` bump rule

Both contracts move to `2`, and this ADR records the rule that decision applied, which [ADR: JSON Output Schema and Validation Policy](20260707T050100Z_json-output-schema-and-validation-policy.md) left implicit.

`schemaVersion` must be bumped when a document produced by the new code would fail validation against the previous version's published schema, or when an existing value's meaning changes. Concretely, for the objects this contract declares `additionalProperties: false`:

- adding a property — required **or** optional — bumps it, because `additionalProperties: false` makes any new key invalid under the previous schema;
- removing a property bumps it;
- changing what an existing value means bumps it, even when the shape is unchanged. Both changes here qualify on their own: `step` is a new required property, and `checkpoint: "initial"` is redefined.

Adding a diagnostic code does not bump it. `DiagnosticCode` is declared an open, growing set, so a new code is already valid under the current schema.

The two contracts version independently and must each be bumped only when that contract changes. They move together here because both received `step`; a future change touching one alone must not bump the other.

## Consequences

### Positive Consequences

- A consumer can tell a setup step from a case body step, and can locate either within its own block.
- `checkpoint: "initial"` stops being ambiguous once a concrete case has two phase-entry checkpoints.
- The bump rule is written down, so the next contributor adding a field to a closed object does not have to re-derive it.

### Negative Consequences

- Breaking change for both contracts: a consumer validating against either published v1 schema fails on the version constant and on the new required property.
- Two numbers describe an action's position — its index in `actions[]` and its `step.index` — and they routinely differ. The schema descriptions and [`docs/reference/artifacts.md`](../reference/artifacts.md) state the distinction; nothing enforces that a consumer reads it.

### Neutral Consequences

- Revises two Neutral Consequences of [ADR: Canonical Artifact Run Result Manifest](20260708T130500Z_artifact-run-result-canonical-manifest.md): the manifest records a step position again, now as a phase-aware origin rather than the pre-#102 bare index, and the two `schemaVersion` fields now share the value `2`.
- `tests/fixtures/run_result/before_each_phase.repor` and `tests/fixtures/json_report/before_each_phase.repor` become required representative fixtures, so `phase: "before_each"` is present in the fixture corpus rather than only `case`.
