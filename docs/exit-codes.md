# Exit Codes

This document defines the reportage CLI exit code policy and the exit codes introduced in v0.

## Policy

- Exit code `0` means success: every case in the run passed.
- Non-zero exit codes must be as specific and distinguishable as reasonably possible.
- Test assertion failure, parse/validation error, and execution/runtime error must not share the same exit code.

## v0 Exit Code Table

| Code | Meaning |
|------|---------|
| `0`  | **Success** — all cases passed. |
| `1`  | **Test/assertion failure** — the run completed but at least one assertion did not pass. |
| `2`  | **Parse or validation/spec error** — the script could not be parsed, or a spec rule was violated (e.g., a case with no assertion, an assertion with no preceding action, an exit code outside `0..=255`). |
| `3`  | **Action execution/runtime error** — the runner could not execute an action (e.g., the shell binary could not be spawned). This is distinct from a non-zero action exit code, which is a normal execution outcome. |

## Precedence

When a run contains multiple cases with different error types, the exit code reflects the highest-severity result:

```
3 (runtime error) > 2 (validation error) > 1 (assertion failure) > 0 (success)
```

## Notes

- A non-zero exit code from an action (`$ false` exits with `1`) is not itself an error. It is captured as the action's result and evaluated by explicit `assert exit` assertions.
- Exit code `3` is reserved for infrastructure failures such as the POSIX shell not being found on `PATH`. It does not mean "the action exited non-zero".
