# Case-local immutable runtime evidence bindings

- Status: Accepted
- Created: 2026-07-25T18:00:00Z

## Context

Dynamic values produced by actions could previously be compared only inside the shell.
That compressed expected values, actual values, and mismatches into an exit status and prevented Reportage from representing the comparison as structured evidence.
Adding mutable variables, expressions, conditions, and loops would instead expand the DSL toward a general-purpose programming language.

The decision also needs stable boundaries with interpolated text literals in issue #71 and explicit action environment projection in issue #212.

## Decision

Reportage supports case-local immutable typed bindings captured with `let name <- source`.
The capture arrow denotes reading runtime evidence and is not a general assignment operator.
Version zero sources are `stdout`, `stderr`, `stdout_line`, and `stderr_line`; all produce `BoundValue::Text`.

`stdout` and `stderr` identify the same last-action evidence used by assertions.
Exact capture performs strict UTF-8 decoding and preserves content without trimming or normalization.
Single-line capture removes at most one final LF or CRLF, then rejects remaining line terminators.

Bindings are visible only after declaration in the same concrete case.
Names use the identifier grammar `[A-Za-z_][A-Za-z0-9_]*`; lower snake case is a documentation convention rather than a validity rule.
Redeclaration is forbidden.

`&name` is a typed direct reference to the complete bound value.
It is accepted in existing `TextValue` positions for write content and `contains` or `text_equals` expected values.
It is distinct from raw text, interpolation, and shell variable expansion.

Bindings retain their declaration span and provenance: source action, stream, and exact or line capture mode.
Artifacts keep action stdout and stderr as canonical evidence and do not duplicate bound plaintext.
Bindings are not implicitly exported to action environments.

## Alternatives Considered

### General mutable variables and expressions

This would require assignment rules, evaluation order beyond linear steps, control flow, and broader diagnostics.
Those capabilities are not necessary to move runtime comparisons into structured assertions.

### Shell-source interpolation

Inserting bound text into command source would combine Reportage parsing with shell quoting and injection concerns.
Issue #212 instead owns explicit environment projection for future action use.

### Trimming all captured output

Implicit trimming would destroy evidence and make exact comparisons depend on hidden normalization.
The explicit `_line` sources cover the common one-line command-output case without changing exact capture.

### Storing every binding value in the artifact manifest

This would duplicate action evidence and increase plaintext exposure.
The action stream artifact is sufficient as canonical provenance.

## Consequences

### Positive Consequences

- Runtime-derived expected values can participate in structured writes and assertions.
- Scope, immutability, type, and provenance remain explicit.
- Future bound value types can extend `BoundValue` without treating every binding as a string.

### Negative Consequences

- Literal and binding concatenation still requires issue #71.
- Action use still requires the separate environment-projection feature.
- Strict UTF-8 and single-line validation introduce runtime step errors for outputs that ordinary byte assertions can still inspect.

### Neutral Consequences

- A `let` step does not update the checkpoint or assert exit status.
- Uppercase letters remain valid in identifiers even though examples use lower snake case.
