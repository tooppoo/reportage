# Exit Codes

This document defines the reportage CLI exit code policy and the exit codes introduced in v0.

## Policy

- Exit code `0` means success: every case in the run passed.
- Non-zero exit codes must be as specific and distinguishable as reasonably possible.
- Test assertion failure, parse/validation error, and execution/runtime error must not share the same exit code.
- Exit codes are process-level severity signals for CI and shell callers.
- Artifact `result` values are run outcome categories for humans and tools reading reportage output.

## v0 Exit Code Table

| Code | Meaning |
|------|---------|
| `0`  | **Success** — all cases passed. |
| `1`  | **Test/assertion failure** — the run completed but at least one assertion did not pass. |
| `2`  | **Script/config validation error** — the selected reportage scripts or configuration could not be treated as valid test input. Examples include read errors, parse errors, unsupported syntax, invalid config, a case with no assertion, an assertion with no preceding action, or an exit code outside `0..=255`. |
| `3`  | **Action execution/runtime error** — the runner could not execute an action, write required artifacts, or perform required runtime infrastructure work. This is distinct from a non-zero action exit code, which is a normal execution outcome. |

## Artifact result categories

The top-level artifact `result` is not a boolean pass/fail field. It records the coarse run outcome category.

v0 result categories:

| `result` | Exit code | Meaning |
|----------|-----------|---------|
| `pass` | `0` | All selected cases executed and passed. |
| `test_failed` | `1` | The selected scripts were valid and execution completed, but one or more assertions failed. |
| `script_error` | `2` | One or more selected reportage script files could not be used as valid test definitions. Examples include `read_error`, `parse_error`, unsupported syntax, and invalid script structure. |
| `config_error` | `2` | The configuration itself is invalid, unsupported, or cannot be used for discovery. |
| `runtime_error` | `3` | The runner failed due to infrastructure/runtime conditions such as shell spawn failure or required artifact write failure. |

`script_error` is intentionally broader than `parse_error`. Read errors and parse errors for selected reportage files both produce top-level `result: "script_error"` and process exit code `2`.

Concrete causes must remain distinguishable in structured diagnostics, for example by using file-level error kinds such as `read_error` and `parse_error`.

## Precedence

When a run contains multiple cases with different error types, the exit code reflects the highest-severity result:

```text
3 (runtime error) > 2 (script/config error) > 1 (assertion failure) > 0 (success)
```

For artifact `result`, use the corresponding highest-severity run outcome category. If multiple categories share exit code `2`, prefer the category that identifies the failing layer:

```text
runtime_error > config_error > script_error > test_failed > pass
```

## Notes

- A non-zero exit code from an action (`$ false` exits with `1`) is not itself an error. It is captured as the action's result and evaluated by explicit `assert exit` assertions.
- Exit code `3` is reserved for infrastructure failures such as the POSIX shell not being found on `PATH`, or required artifact generation failing. It does not mean "the action exited non-zero".
