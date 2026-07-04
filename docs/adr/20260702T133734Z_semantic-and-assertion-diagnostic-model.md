# Define the Semantic and Assertion Failure Diagnostic Model and Code System

- Status: Accepted
- Created: 2026-07-02T13:37:34Z

## Context

[20260701T060000Z_stable-diagnostic-codes.md](20260701T060000Z_stable-diagnostic-codes.md) (#35) introduced stable diagnostic codes and a minimal diagnostic model for parser and validator errors, and reserved a `semantic.*` namespace without defining it. #30 ran semantic conformance cases against the semantic evaluator with pass/fail result verification only; expected diagnostic code verification was deferred until this contract existed.

Reportage must distinguish parse / validation errors, semantic errors, and assertion failures, while giving tests, CI, and tooling a machine-readable diagnostic identity to depend on.

Reportage is a testing DSL, so for source-derived diagnostics, a location / range that lets a user quickly find the failing test matters. At the same time, some input paths — semantic conformance cases — have no `.rpt` source text at all.

The full specification lives in [`docs/semantic-diagnostics.md`](../semantic-diagnostics.md). This ADR records the decisions behind it.

## Decision

### 1. Diagnostic code external representation

- Diagnostic codes are dot-separated namespace strings.
- Top-level namespaces are `parse.*` / `semantic.*` / `assertion.*`.
- Uppercase prefix forms such as `RPT-ASSERT-EXIT-MISMATCH` are not adopted.
- Code strings are stable identifiers independent of Rust internal error enum variant names.

### 2. Separation of semantic errors and assertion failures

- A semantic error means the script, normalized semantic model, or expectation definition is invalid, and evidence comparison cannot begin.
- An assertion failure means the script and expectation are valid, but evidence acquisition or comparison shows the expectation is not satisfied.
- Assertion failures are treated as diagnostics, in the same model as errors.

### 3. Severity classification

- Semantic errors have severity `error`.
- Assertion failures have severity `failure`.
- `warning` is reserved for future non-fatal diagnostics.
- Counting as a failed CI / test result and having severity `error` are distinct concepts and must not be conflated.

### 4. Message stability

- `message` is a human-readable view, not a stable contract.
- Tests and tooling must not depend on full-message matches.
- The stable contract is limited, in principle, to `code` and the stable details fields defined per code.

### 5. Location / source range model

- `location` is a model that expresses a source range, not a single line / column point.
- Source-derived diagnostics carry start / end line / column positions whenever a corresponding source node exists.
- `SourceRange.end` is an exclusive position.
- v0 does not require byte offsets in the stable contract.

### 6. Origin responsibility

- A diagnostic carries an `origin` in principle.
- A diagnostic without a `location` must carry an `origin`.
- Semantic conformance cases, which have no `.rpt` source text, are tracked through origin information such as spec id / rule id / case id instead of a source location.

### 7. Details stability

- The `details` field is part of the diagnostic model.
- `details` as a whole is not unconditionally stable API; stable fields are defined per diagnostic code.
- Full actual stdout / stderr / file content, debug dumps, and filesystem error messages are not part of the v0 stable details contract.

### 8. Room for logical composition

- The model keeps room for future nested / child diagnostics so that the logical composition diagnostics of #25 are not blocked.
- This decision does not require implementing nested / child diagnostics.

Full application of this model to the parser, evaluator, and CLI diagnostic rendering is handled by follow-up issues as needed.

## Alternatives Considered

### Uppercase prefix codes such as `RPT-ASSERT-EXIT-MISMATCH`

Rejected. #35 already established dot-separated namespace codes for `parse.*`; a second convention would split the code system into two incompatible styles and force tooling to handle both.

### Treating assertion failures as results rather than diagnostics

Rejected. Assertion failures need the same machine-readable identity (code), structured details, and source location as errors, so that conformance cases and tooling can verify them uniformly. The severity distinction (`failure` vs. `error`) preserves the difference in meaning without a second model.

### Making `message` or all of `details` a stable contract

Rejected. Stable message text makes every wording improvement a breaking change. Stable whole-`details` freezes volatile, potentially large or sensitive content (full stdout / stderr / file content) into the contract. Per-code stable details fields keep the contract small and intentional.

### Point-only location (line / column), deferring ranges

Rejected as the final policy. An earlier draft considered deferring range-capable locations to avoid colliding with the #35 `location` model, but a testing DSL benefits directly from ranges (CLI rendering, future editor integration), and an exclusive `SourceRange.end` aligns with Rust ranges, string slices, and LSP ranges. The #35 parse-side model is extended by follow-up work rather than redesigned here.

## Consequences

### Positive Consequences

- Tests and tooling can depend on diagnostic codes rather than message text, so user-facing messages can improve while diagnostic identity stays stable.
- Source-derived diagnostics get range-based locations, which benefits CLI rendering and future editor integration.
- Inputs without source text (semantic conformance cases) remain traceable through origin information (spec id / rule id / case id).
- `SourceRange.end` being exclusive aligns with Rust ranges, string slices, and LSP ranges.
- #30 or a follow-up issue can enable expected diagnostic code verification for conformance cases that specify one.

### Negative Consequences

- Renaming or removing a published code is a breaking change, so adding or changing codes requires compatibility care.
- Stable details fields must be defined per code, which is ongoing specification overhead; without that definition, `details` cannot be a primary dependency for tests or tooling.

### Neutral Consequences

- This ADR specifies the model; full application to the parser, evaluator, and CLI rendering is deferred to follow-up issues.
- Nested / child diagnostics remain possible but unimplemented.
