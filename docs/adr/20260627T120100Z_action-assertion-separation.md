# Action and Assertion Separation

- Status: Accepted
- Created: 2026-06-27T12:01:00Z

## Context

A core design question for reportage is whether a `$` action's exit code should imply a test result on its own. Many shell-based test runners treat a non-zero exit from the command under test as an implicit failure. reportage takes a different approach.

## Decision

Actions and assertions are distinct concepts. Inside a `case` block:

- A line whose first token after optional leading whitespace is `$` is an **action**.
- A line whose first token after optional leading whitespace is `assert` is an **assertion**.
- Action exit code `0` is **not** an implicit assertion success.
- Every executable case **must** contain at least one explicit assertion.
- A case with actions but no assertions is a validation/spec error, not a passing case.

Assertion target resolution works as follows:

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

Requiring an assertion before the action (pre-condition style) was considered but rejected. post-condition assertions after the action are more natural for E2E tests.

## Consequences

### Positive Consequences

- Every passing case has at least one explicit claim about the outcome.
- The runner cannot optimize around action-only execution without breaking the evaluation model.
- The assertion model extends naturally to stdout, stderr, and file assertions in future versions.

### Negative Consequences

- Authors must write an `assert exit` even for cases where the exit code is the only relevant outcome.

### Neutral Consequences

- This separation is visible to users: forgetting `assert exit` is a detected error, not a silent pass.
