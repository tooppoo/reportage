# Semantics Documentation Strategy and Semantic Rule Coverage

- Status: Accepted
- Created: 2026-07-08T06:15:00Z

## Context

Reportage already treats syntax and semantic rules as executable or machine-readable specification sources. The pest grammar defines syntax, semantic specs define rule-level behavior, and generated documentation is checked for freshness.

However, the documentation boundary around semantics had become ambiguous.

`docs/semantics.md` had grown into a broad, hand-written document covering language semantics, runner behavior, workspace behavior, checkpoints, shims, evidence, artifacts, and other execution concerns. At the same time, `docs/language/semantics.md` was generated from `spec/language/semantics/*.json`, but its name made it look like it described all language semantics rather than a generated semantic rule catalog.

This created several drift risks:

- hand-written prose could become the effective normative source instead of generated or executable sources;
- generated semantic rule documentation could omit implemented rules such as file assertions, directory assertions, output empty assertions, or logical composition;
- path-like values could be misclassified after fixture reference literals were introduced;
- runner lifecycle, artifact JSON contracts, diagnostics, and assertion semantics could be pushed into one schema or one document even though they have different consumers and validation mechanisms.

Issue [#85](https://github.com/tooppoo/reportage/issues/85) decided the documentation and coverage strategy for these concerns. This ADR records that durable project policy.

Related follow-up issues:

- [#98](https://github.com/tooppoo/reportage/issues/98)
- [#99](https://github.com/tooppoo/reportage/issues/99)
- [#100](https://github.com/tooppoo/reportage/issues/100)
- [#101](https://github.com/tooppoo/reportage/issues/101)
- [#102](https://github.com/tooppoo/reportage/issues/102)

## Decision

Reportage must split semantics documentation by responsibility and keep generated semantic rule documentation connected to machine-readable sources and CI checks.

### Semantics document set

`docs/semantics.md` must be an overview and entry point for the semantics document set.

It must not grow into the single hand-written normative source for all language, runner, artifact, diagnostic, and execution behavior. It should link to more specific documents and generated references.

Execution model and runtime semantics must be separated into `docs/execution-model.md`.

The generated semantic rule catalog must be named `docs/language/semantic-rules.md`. The previous name, `docs/language/semantics.md`, is too broad for a generated catalog of semantic rules and conformance cases.

### Generated semantic rule catalog

The generated semantic rule catalog must be generated from machine-readable semantic specs.

It should include semantic rule identifiers, categories, syntax forms, structured normative fields, conformance cases, expected outcomes, and expected diagnostic information where applicable.

Generated semantic rule documentation must be stale-checked in CI.

### Semantic rule coverage registry

Semantic rule coverage must use a Rust const registry as its initial source of truth.

The registry is not the runtime implementation. It is a spec coverage inventory that maps stable semantic rule identifiers to coverage requirements.

The registry should record only structured inventory data, such as:

- rule id;
- category;
- implementation status;
- whether a semantic spec is required;
- whether conformance cases are required;
- whether generated documentation is required;
- related syntax rule names, when useful;
- related diagnostic codes, when useful.

The registry must not contain long normative prose. Normative semantics belong in semantic specs. Rationale belongs in ADRs. Open questions belong in `docs/TBD.md`.

### Semantic rule categories

The v0 semantic rule categories are:

- `assertion`
- `logical-composition`
- `value-reference`

The following rules must be covered by the semantic rule coverage mechanism:

- file assertions;
- directory assertions;
- `stdout empty`;
- `stderr empty`;
- logical composition.

`logical-composition` must not be forced into the same schema shape as checkpoint field assertions. Logical composition rules combine expectations rather than directly comparing a checkpoint field.

`value-reference` must cover value literal resolution and reference policy. At minimum, it must include:

- `value-reference.workspace-path`
- `value-reference.fixture-reference`
- `value-reference.file-contents-reference`
- `value-reference.literal-kind-mismatch`

`fixture-reference` must not be modeled as a child of `workspace-path`. A workspace path refers to the concrete case workspace. A fixture reference refers to a test-definition-side file. They may share lexical validation policy, but they have different roots and different allowed positions.

`FileContentsReference` is the semantic domain accepted by `contents_equals`-style rules and must be modeled as:

```text
FileContentsReference = WorkspacePath | FixtureReference
```

### Boundaries with other documentation areas

`runner-lifecycle` must not be part of the semantic rule catalog. Runner lifecycle, including preconditions and postconditions, is handled by [#55](https://github.com/tooppoo/reportage/issues/55) and belongs in execution-model documentation. Lifecycle phases affect execution order and result classification. They must not change the evaluation semantics of cases, actions, assertions, expectations, or probes.

`artifact` and result JSON behavior must not be part of the semantic rule catalog. Artifact and result JSON documentation must be handled from JSON Schema, golden fixtures, snapshots, or equivalent machine-readable and executable sources. This is tracked by [#102](https://github.com/tooppoo/reportage/issues/102).

`diagnostic` behavior must not be forced into the semantic rule catalog. Diagnostic model and code policy should remain under diagnostic-specific design work.

Examples and E2E tests are executable evidence. They are not the sole source of truth for language semantics.

### Documentation management priority

Documentation should follow the priority order defined by [#85](https://github.com/tooppoo/reportage/issues/85):

1. Prefer documentation generated from implementation or machine-readable sources, where the generated content is itself verifiable.
2. If full verification is not practical, prefer generation tied to implementation or machine-readable sources.
3. If generation is not practical, keep manually updated documentation close to the implementation or source it follows and make update drift detectable.
4. If the documentation and implementation are separated, update drift must still be detectable.
5. Manual, nearby, undetected documentation is only a temporary compromise.
6. Manual, distant, undetected documentation should be eliminated.

In short, Reportage should prefer documentation states equivalent to #85 priorities 1 through 3, tolerate priority 4 when necessary, keep priority 5 temporary, and eliminate priority 6 over time.

## Alternatives Considered

### Keep `docs/semantics.md` as the complete hand-written semantics document

Rejected.

A large hand-written semantics document is easy to read initially, but it becomes difficult to prove against parser behavior, evaluator behavior, generated docs, runtime artifacts, and examples. It also encourages prose to become a competing source of truth.

### Rename nothing and clarify intent only in introductions

Rejected.

Introductory notes help, but the names themselves would still be misleading. `docs/language/semantics.md` sounds like the full language semantics document even though it is a generated catalog of semantic rules and conformance cases.

### Use implementation enums or AST variants as the semantic coverage source of truth

Rejected for the initial coverage mechanism.

Implementation enums and AST variants are useful drift-detection inputs, but making them the coverage source of truth would make the implementation shape too normative. The project needs an explicit inventory of which semantic rules require specs, conformance cases, and generated documentation.

### Use JSON as the initial semantic rule coverage registry format

Deferred.

JSON may become appropriate if the registry must be consumed by external generators, editor tooling, AI tooling, or schema validators outside Rust. For now, Rust const data is easier to connect to parser, evaluator, and semantic test code.

### Put fixture references under workspace paths

Rejected.

Workspace paths and fixture references may look similar because both refer to files, but they refer to different roots. A workspace path is resolved inside the concrete case workspace. A fixture reference is resolved on the test-definition side. Treating fixture references as children of workspace paths would blur that semantic boundary.

### Include runner lifecycle and artifact JSON in the semantic rule catalog

Rejected.

Runner lifecycle and artifact JSON are semantic in a broad sense, but they have different validation mechanisms and different consumers. Including them in the semantic rule catalog would make the catalog less precise and would force unrelated concepts into one schema.

## Consequences

### Positive Consequences

- `docs/semantics.md` remains useful as a navigational overview without becoming a competing normative source.
- Execution model documentation has a stable home in `docs/execution-model.md`.
- The generated semantic rule catalog has a more precise name: `docs/language/semantic-rules.md`.
- Missing semantic rules can be detected through an explicit coverage inventory.
- Fixture reference semantics are represented without incorrectly treating fixtures as workspace paths.
- Logical composition can use its own category instead of being forced into assertion-field comparison semantics.
- Artifact/result JSON, runner lifecycle, diagnostics, and semantic rules remain separated by responsibility.

### Negative Consequences

- The documentation set becomes more distributed and requires good cross-links.
- Contributors must learn the distinction between overview docs, execution model docs, generated semantic rule docs, semantic specs, coverage registry, ADRs, and TBD entries.
- A Rust const registry is not directly consumable by non-Rust tools without additional export support.
- Some decisions are now split across an older language-specification ADR and this more specific semantics-documentation ADR.

### Neutral Consequences

- `docs/language/semantics.md` references must be migrated to `docs/language/semantic-rules.md`.
- JSON registry export remains available as a future option if tooling requirements change.
- Some broad semantics topics remain outside the semantic rule catalog by design and will be handled by specific documents or issues.

## Non-Goals

This decision does not implement the document split, rename, registry, schema changes, or generated-doc checks. Those are handled by follow-up issues.

This decision does not define every semantic rule. It defines how semantic rules should be categorized, inventoried, and connected to documentation.

This decision does not supersede the ADR that established executable and machine-readable language specification sources. It refines the semantics documentation and coverage strategy within that policy.

## Summary

Reportage will treat `docs/semantics.md` as an overview, move execution model details to `docs/execution-model.md`, rename generated semantic rule documentation to `docs/language/semantic-rules.md`, and use a Rust const registry as the initial semantic rule coverage inventory. Semantic rule categories are `assertion`, `logical-composition`, and `value-reference`. Runner lifecycle, artifact/result JSON, and diagnostics remain outside the semantic rule catalog and are handled by their own documentation or design tracks.
