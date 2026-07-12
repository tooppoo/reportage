# Treat empty and whitespace-only scripts as no-op success

- Status: Accepted
- Created: 2026-07-03T00:00:00Z

## Context

Reportage accepts empty and whitespace-only scripts as syntax-valid inputs. That parser policy keeps editor integrations, generated files, and partially-written scripts simple: an empty module can still be a valid module.

Syntax validity and execution policy are separate concerns. A file can parse successfully while producing zero concrete cases. The runner still needs a clear result for CI, CLI output, and artifacts when valid selected input has nothing to execute.

## Decision

In v0, an empty script, whitespace-only script, or valid suite with zero concrete cases is a no-op success.

No-op execution has these properties:

- the CLI exits with code `0`;
- the run status is represented as `result: "pass"` plus a machine-readable no-op marker;
- no `$` command is executed;
- no checkpoint is generated;
- no assertion is evaluated;
- no case, checkpoint, or evidence artifacts are generated;
- human-readable CLI output states that no cases were found and nothing was executed;
- run-level result summary records `noop: true` and zero counts for cases, executed steps, and assertions.

The no-op marker and zero counts are part of the run summary so tools can distinguish "all executed cases passed" from "nothing ran" without treating the run as a failure.

## Alternatives Considered

### Treat empty input as a script error

This would make accidental empty CI test files fail early. It was not selected for v0 because it would contradict the decision to make empty and whitespace-only scripts syntax-valid, and it would conflate parse validity with runner policy.

### Warning with success

A warning could make no-op runs more visible while preserving exit code `0`. It was not selected for v0 because reportage does not yet have a stable warning severity model. The CLI output and structured no-op marker provide visibility without introducing warning diagnostics in this issue.

### Separate `noop` run status

A distinct status such as `result: "noop"` would make no-op state explicit in one field. It was not selected for v0 because it expands the top-level result state space. `result: "pass"` plus `noop: true` preserves success semantics while keeping no-op machine-detectable.

### Strict mode or `--fail-on-empty`

Some CI workflows may want empty selected input to fail. That policy is deferred to a future strict mode, lint command, or option such as `--fail-on-empty`. It is not part of the default v0 runner behavior.

## Consequences

### Positive Consequences

- Empty and whitespace-only script handling stays consistent across parser and runner entry points.
- CI receives a successful process exit for valid zero-case input.
- Humans and tools can still detect no-op runs through CLI output and result summary fields.
- The runner avoids inventing fake actions, checkpoints, or assertions to represent nothing happening.

### Negative Consequences

- A mistakenly empty test file can pass CI under the default v0 runner policy.
- Projects that require empty selections to fail need a future strict or lint-oriented feature.

### Neutral Consequences

- Comment-only script behavior is left to the later comment/trivia execution policy. If comments are pure syntax trivia, comment-only scripts are likely to follow the same no-op policy.
- Result artifacts continue to be written for no-op runs, but case/checkpoint/evidence artifacts are not generated.
