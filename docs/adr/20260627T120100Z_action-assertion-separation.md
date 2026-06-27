# Action and Assertion Separation

- Status: Superseded by [20260627T120200Z use-assertion-blocks-and-checkpoint-based-assertion-model](20260627T120200Z_use-assertion-blocks-and-checkpoint-based-assertion-model.md)
- Created: 2026-06-27T12:01:00Z

## Context

A core design question for reportage is whether a `$` action's exit code should imply a test result on its own. Many shell-based test runners treat a non-zero exit from the command under test as an implicit failure. reportage takes a different approach.

## Actions and assertions

Reportage distinguishes actions from assertions as semantic step kinds.

An action performs an operation against the target system or its surrounding environment. In the first slice, actions are executed through a POSIX-compatible shell.

An assertion observes and validates state, output, exit status, files, or other artifacts available at that point in the scenario.

This distinction does not imply phased execution. Reportage preserves source order. Actions and assertions are executed or evaluated at the position where they appear in the scenario.

In particular, Reportage must not execute all actions first and then evaluate all assertions afterward, because assertions may depend on intermediate state.

## Decision

Actions and assertions are distinct concepts. Inside a `case` block:

- A line whose first token after optional leading whitespace is `$` is an **action**.
- A line whose first token after optional leading whitespace is `assert` is an **assertion**.
- Action exit code `0` is **not** an implicit assertion success.
- Every executable case **must** contain at least one explicit assertion.
- A case with actions but no assertions is a validation/spec error, not a passing case.

## Execution semantics

Steps are processed in source order. Actions and assertions are executed or evaluated at the position where they appear in the case.

An assertion evaluates the state or output available at that point in the scenario — primarily the exit status, stdout, and stderr of the immediately preceding action. Multiple consecutive assertions target the same preceding action and are evaluated as a group against the same state.

Assertion failure stops execution before the next action. The runner must not proceed to subsequent actions after an assertion in the current block has failed.

Example of intermediate-state validation:

```reportage
case "intermediate state" {
  $ echo first > state.txt
  assert exit 0

  $ echo second > state.txt
  assert exit 0
}
```

The first assertion is evaluated before `echo second` runs. Evaluating it afterward would observe the final state, not the intermediate state, which would make the assertion semantically incorrect.

## Assertion target resolution

- `assert exit <code>` evaluates the exit code of the **preceding action** in the same case.
- Assertion steps are ignored while resolving the preceding action, so multiple consecutive assertions may all target the same action.
- If there is no preceding action in the same case, the assertion is a validation/spec error.

Example of multiple assertions targeting the same action:

```reportage
case "same action" {
  $ true
  assert exit 0
  assert exit 0
}
```

Both `assert exit 0` lines target the `$ true` action.

## Alternatives Considered

Treating a zero exit code as an implicit pass was considered. This would allow `case` blocks without assertions to silently pass. However, it would make it easy to write cases that test nothing: if the command under test crashes before the real work, the test would still pass. Explicit assertions force the author to state what outcome they expect.

Requiring an assertion before the action (pre-condition style) was considered but rejected. Post-condition assertions after the action are more natural for E2E tests.

Phased execution — running all actions first and then evaluating all assertions — was considered and rejected. Assertions may depend on intermediate state that subsequent actions would overwrite. Phased execution would make such assertions observe the wrong state, which is incorrect per the source order semantics defined above.

## Consequences

### Positive Consequences

- Every passing case has at least one explicit claim about the outcome.
- The runner cannot optimize around action-only execution without breaking the evaluation model.
- The assertion model extends naturally to stdout, stderr, and file assertions in future versions.
- Intermediate state can be validated correctly within a single case.

### Negative Consequences

- Authors must write an `assert exit` even for cases where the exit code is the only relevant outcome.

### Neutral Consequences

- This separation is visible to users: forgetting `assert exit` is a detected error, not a silent pass.
