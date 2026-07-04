# Use Block-Form Logical Composition (`not` / `all` / `any`) for Expectations

- Status: Accepted
- Created: 2026-07-04T15:00:00Z

## Context

v0 assertion blocks only support an implicit `all` over the expectations listed inside `assert { ... }`. There is no way to express negation, alternative ("or at least one of these"), or an explicit grouped conjunction as a single expectation expression.

File assertions (#24) make this more pressing: once `file "path" exists` / `file "path" contains "text"` exist, users will want to write things like "this file must not exist" or "one of these two files must exist". The naive shape for that is an infix expression:

```reportage
assert {
  file "a.txt" exists and file "b.txt" contains "ok"
  file "tmp/error.log" not exists
}
```

Infix form (`A and B`, `A or B`) requires operator precedence, decisions about how it interacts with line breaks, and a more complex diagnostic location model (which operand failed, at what span). Predicate-level negation (`file "path" not exists`) pushes negation into every predicate's grammar individually rather than treating it as a general expectation-level concept, and would have to be re-invented for every future subject (`dir`, `file-count`, `jq`, ...).

This ADR defines how Reportage composes expectations instead: as a dedicated block-form construct, layered above individual expectation kinds, independent of what those expectations are.

## Decision

Reportage introduces three block-form logical composition operators as expectation expressions: `not { ... }`, `all { ... }`, `any { ... }`. A composition block's body accepts the same single-line or multi-line forms as `assert { ... }`, and may nest further `not` / `all` / `any` blocks alongside atomic expectations.

```reportage
assert {
  not {
    file "tmp/error.log" exists
  }

  all {
    exit 0
    stdout contains "PASS"
  }

  any {
    file "result.json" contains "\"status\":\"passed\""
    file "result.json" contains "\"status\":\"ok\""
  }
}
```

### 1. Block form, not infix form

Operator syntax introduces a block; the expectations it composes are the block's body, not left/right operands of an inline operator. Rejecting infix form (`A and B` / `A or B`) avoids operator precedence rules, avoids deciding how a boolean expression interacts with line breaks and comments, and keeps per-expectation diagnostic location simple: every expectation expression, atomic or composed, is still just "this span of source".

### 2. No predicate-level negation

`file "path" not exists` is not adopted. Negation is not a property of individual predicates; it is `not { ... }` wrapping any expectation expression. This keeps predicate grammars (file, dir, future subjects) free of a `not` keyword each would otherwise need to parse and diagnose independently, and lets one negation mechanism work uniformly across every present and future expectation kind.

### 3. `and` / `or` are not aliases for `all` / `any`

`and { ... }` / `or { ... }` are rejected, not accepted as spellings of `all` / `any`. v0's canonical logical composition vocabulary is exactly `not` / `all` / `any`. Not aliasing avoids two spellings reaching the AST, diagnostics, docs, and snapshot fixtures for the same construct.

### 4. Multiple expectations inside `not` group as an implicit `all`

`not { A B }` evaluates as `not(all(A, B))`, never as `not(A) and not(B)`. A composition block's body reuses the exact same implicit-`all` grouping `assert { ... }` already uses for its top-level expectations, so `not` stays an expectation-expression-level operator: it negates one grouped result, not each item in its body individually.

### 5. Composition affects assertion result only, never semantic errors

`not` / `all` / `any` combine assertion success / failure. They do not turn a semantic error (the expectation definition itself is invalid) into a success or failure, and `not` does not invert a semantic error into its opposite outcome. Semantic errors and assertion outcomes are different axes; composition operates on the latter only.

### 6. An empty composition block is a semantic error

`not { }` / `all { }` / `any { }` contain no expectation expression to evaluate, so there is no evidence comparison to perform. This is treated the same way an empty `assert { ... }` is: a defect in the script / semantic model, not an assertion failure. The grammar accepts an empty block (`empty_composition_body`) precisely so Reportage can distinguish this case — `semantic.expectation.empty_block` — from a generic `parse.syntax` rejection, following the `semantic.*` namespace defined by docs/semantic-diagnostics.md (#41).

### 7. Evaluation preserves child results

The evaluator evaluates every child expectation expression it can reach, even once a composition's overall outcome is already determined (e.g. one `any` candidate has already passed), and each child's own result — pass or fail — is retained in the composition's result structure rather than collapsed into a single boolean. This does not implement a full nested diagnostic model (not required by this issue), but keeps the door open for one: a later change can render `children` as nested diagnostics without changing the evaluator's shape.

## Alternatives Considered

### Infix `A and B` / `A or B`

Rejected. Requires operator precedence, complicates diagnostic location and partial-failure reporting, and blurs the boundary between an expectation predicate's own grammar and a general boolean combinator.

### Predicate-level negation (`file "path" not exists`)

Rejected. Ties negation to one predicate's grammar instead of treating it as a property of any expectation expression; would need reinventing per subject.

### `and { ... }` / `or { ... }` as aliases for `all` / `any`

Rejected for v0. Every alias is another spelling that AST, diagnostics, docs, and snapshot fixtures must carry indefinitely for no behavioral benefit.

### Item-wise negation for `not` with multiple children

Rejected. `not(A) and not(B)` for `not { A B }` would make `not`'s meaning depend on how many expectations happen to be listed inside it, and would require `not` to special-case its body's grouping instead of reusing the same implicit-`all` rule every other block uses.

## Consequences

### Positive Consequences

- Reportage gets negation, conjunction, and disjunction over expectations without touching any expectation predicate's own grammar (file, dir, process expectations all compose the same way).
- `parse.syntax` continues to reject unsupported forms (infix, `and` / `or` blocks, predicate-level negation) without new grammar surface area.
- The `semantic.*` diagnostic namespace (#41) now has a first real consumer (`semantic.expectation.empty_block`), validating that the namespace design holds up in practice.

### Negative Consequences

- Scripts that want "one of these must exist" now require a block (`any { ... }`) rather than a single inline expression; this is more verbose for the simplest cases.
- The evaluator's result structure for a composed expectation is recursive (`children: Vec<ExpectationResult>`), which is a small amount of additional complexity carried by every expectation kind's exhaustive matches (CLI rendering, JSON artifact rendering).

### Neutral Consequences

- This ADR does not implement a full nested diagnostic / child diagnostic model; docs/semantic-diagnostics.md already treats that as optional follow-up work.
- File assertion predicates (#24) are unaffected: this ADR does not add or change any predicate grammar, only the expectation-expression layer above it.
