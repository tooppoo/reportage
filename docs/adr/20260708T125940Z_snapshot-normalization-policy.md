# Snapshot Normalization Policy

- Status: Proposed
- Created: 2026-07-08T12:59:40Z

## Context

Reportage now has JSON output snapshot tests.

Those snapshots are useful for detecting output contract regressions.

However, some observed values are volatile across runs.

Examples include `schema_version`, `run_id`, `duration`, absolute artifact paths, temporary directories, timestamps, runtime metadata, and coverage-related data.

Comparing those values directly can make snapshot tests fail for reasons that are unrelated to the semantic contract being tested.

At the same time, normalizing too much can hide real contract breakage.

Reportage must therefore define a durable policy for stabilizing observed output before snapshot or baseline comparison.

This decision is not limited to JSON output.

The same problem can apply to stdout text, stderr text, artifact JSON, result JSON, shim event logs, filesystem evidence, coverage metadata, runtime metadata, and approved evidence baselines.

This ADR records the general policy only.

Related issues:

- #113 defines the general snapshot normalization policy.
- #114 defines JSON Schema annotation based JSON normalization.
- #115 defines JSON Schema bundling and metadata stripping tooling.
- #48 defines evidence baseline and approval mode workflow.
- #102 defines the relationship between artifact or result JSON documentation, schemas, fixtures, and snapshots.

## Decision

Reportage will use `snapshot normalization` as the formal term for this concept.

`data normalization` may be used as a broad explanatory phrase, but it is not the formal specification term.

`snapshot normalization` means a deterministic transformation applied to observed output or observed evidence before snapshot or baseline comparison.

Snapshot normalization belongs to the e2e, self-testing, and snapshot harness comparison policy.

It is not part of the Reportage DSL semantics.

Snapshot normalization must be used to remove execution-environment noise.

It must not be used to hide semantic contract changes.

Values without explicit normalization metadata or an explicit normalization rule must be preserved by default.

This is the default preserve policy.

A normalized snapshot must not be the only inspectable artifact when raw data is needed for failure investigation.

Raw observed output or raw observed artifacts should be kept when they are needed to understand what actually happened during a run.

Snapshot normalization and canonicalization are distinct concepts.

Normalization transforms volatile values into stable values according to policy.

Canonicalization stabilizes representation differences that do not change meaning.

Examples of normalization include replacing a run id, timestamp, duration, absolute path, temporary directory, or artifact output path with a stable placeholder.

Examples of canonicalization include stabilizing object key order, formatting, and line endings.

Both steps may appear in the same snapshot comparison pipeline.

They must not be treated as the same operation in the specification.

Result status, diagnostic kind, diagnostic code, severity, diagnostic message, assertion outcome, evidence structure, stdout representation, stderr representation, encoding, and contract-bearing location structure should not be normalized by default.

A contract field may be placeholderized in a case-specific snapshot only when that case is not responsible for verifying the concrete value.

When a contract field is placeholderized, another contract test must verify the concrete value.

`schema_version` is an example of such a field.

It may be placeholderized in a case snapshot when the case is not about schema versioning.

Its concrete value must still be verified by a separate contract test.

Normalization failure must be reported as a harness-level error before snapshot comparison.

It must not be reported as an ordinary snapshot mismatch.

Unsupported metadata, invalid metadata, type mismatch, parse failure, and invalid target handling are examples of normalization failures.

Missing normalization targets must not be ignored implicitly.

A required normalization target that is missing must be an error.

An optional normalization target may be missing only when it is explicitly marked as optional.

Concrete syntax for required and optional targets is delegated to format-specific follow-up issues.

E2E and self-testing snapshots should verify the stability of observable CLI output, evidence, and artifacts.

Rust integration tests and unit tests should verify individual normalization operations, failure handling, canonicalization steps, and metadata interpretation.

Snapshots are not a complete test suite for the normalizer implementation.

The internal normalizer behavior must be directly tested by Rust tests.

## Alternatives Considered

### Compare raw observed output directly

This is the simplest approach.

It was rejected because volatile values such as run ids, timestamps, durations, and absolute paths would create noisy snapshot churn.

Such churn reduces the review value of snapshot tests.

### Define normalization as data normalization

This term is broad.

It can be confused with database normalization, domain model normalization, JSON canonicalization, or DSL semantic normalization.

It was rejected because the decision concerns snapshot and baseline comparison only.

### Allow broad normalization of semantic contract fields

This would reduce snapshot churn.

It was rejected because it could hide changes in result status, diagnostics, messages, assertion outcomes, evidence structure, and other contract-bearing fields.

Reportage should keep evidence inspectable rather than make inconvenient differences disappear.

### Treat canonicalization as a normalization operation

This would reduce the number of terms.

It was rejected because value replacement and representation stabilization have different risks.

Replacing values can hide contract changes.

Canonicalizing object order or formatting usually preserves meaning.

The specification must keep those risks distinguishable.

## Consequences

### Positive Consequences

- Snapshot tests can become stable without ignoring the entire observed output.
- Execution-environment noise can be separated from semantic contract changes.
- The default preserve policy reduces the risk of accidental over-normalization.
- Raw observed output can remain available for failure investigation.
- JSON-specific mechanisms can be designed in #114 without making #113 depend on JSON Schema.
- Schema artifact tooling can be designed in #115 without making Reportage runtime a generic schema bundler.

### Negative Consequences

- The snapshot comparison pipeline becomes more complex than byte-for-byte comparison.
- Normalization metadata and normalization errors need explicit reporting.
- Placeholderized contract fields require separate contract tests.
- Raw and normalized artifacts require clearer artifact retention policy.

### Neutral Consequences

- JSON output normalization can be the first implementation target.
- The general policy remains applicable to non-JSON evidence later.
- Approval mode remains a separate workflow concern handled by #48.
