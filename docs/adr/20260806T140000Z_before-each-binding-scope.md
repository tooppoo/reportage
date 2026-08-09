# Runtime Evidence Binding Scope Across `before_each` and the Case Body

- Status: Accepted
- Created: 2026-08-06T14:00:00Z

Follows [ADR: `before_each` Is a Case-Local Setup Phase](20260806T090000Z_before-each-case-local-setup-phase.md), which deferred this decision.

## Context

`before_each` shares the case body's step surface, but `let` and binding references inside it stayed rejected (`semantic.binding.before_each_forbidden`) while the scope spanning the two phases was undecided.

The motivating shape from #227 needs them: setup runs `$ pwd`, captures the result with `let workspace <- stdout_line`, and writes a config file interpolating it — and the case body then asserts on that same value. That crosses the phase boundary in one direction, and raises what happens in the other.

Runtime evidence bindings are already per concrete case and immutable within a case body. The open questions were which declarations each phase can see, whether a case body may reuse a setup binding's name, and whether `before_each` running an action satisfies a case body `let`.

## Decision

### Scope flows forward only

A binding declared in `before_each` must be in scope for the rest of `before_each` and for the whole case body that follows.

A binding declared in a case body must not be in scope for `before_each`. A `before_each` reference to such a name must be reported as undefined (`semantic.binding.undefined`), not as a use-before-declaration: `before_each` runs first, so the name never becomes available to it, and `semantic.binding.use_before_declaration` would say the opposite.

Each phase's binding flow is validated in source order against the scope it starts with: `before_each` starts empty, and a case body starts with whatever `before_each` left declared.

### No shadowing

A case body must not redeclare a `before_each` binding's name (`semantic.binding.duplicate`).

Bindings are immutable, and `before_each` is not a separate lexical block the reader can see from the case body — it is a different part of the file. A name that means one thing at the top of a case body and another below it would have to be resolved by scrolling to a block the case never mentions.

### `let` requires an action in its own phase

A `let` must be preceded by an action **in the same phase**. A `before_each` action does not satisfy a case body `let`.

This follows from the body-entry checkpoint, which carries no process evidence: a case body `let` placed before that body's first action has nothing to capture from. Accepting it would either capture the last setup action's output — the coupling the checkpoint boundary exists to prevent — or fail at runtime on a rule the parser could have stated.

### Provenance names the action, not the phase

A binding's provenance records the action it captured from, using the concrete case's own action numbering. A `before_each` binding therefore points at a setup action, and no separate provenance shape is needed for setup bindings.

## Consequences

### Positive Consequences

- Setup can capture an action's output once per module and use it in every case body, which is the shape #227 exists to enable.
- Each phase's binding flow is validated by one function over one step list, so a reference is diagnosed the same way wherever it is written.
- The "declared in the case body" mistake reports the diagnostic that describes it, instead of one implying the name arrives later.

### Negative Consequences

- A case body's binding names are constrained by a block it does not mention: adding `let x` to `before_each` turns an existing case body `let x` into a duplicate-declaration error.
- Reading a case body no longer tells the whole binding story; a reference may resolve to a declaration in `before_each`.

### Neutral Consequences

- Removes the `semantic.binding.before_each_forbidden` diagnostic code, which existed only to reject `let` and binding references inside `before_each`. Per [Diagnostic Codes](../reference/diagnostics.md), v0 does not commit to a strict semver policy for codes; this ADR is the record of its removal.
- The `action seen` reset at the case body boundary is the parse-time counterpart of the body-entry checkpoint; the two must stay consistent.
