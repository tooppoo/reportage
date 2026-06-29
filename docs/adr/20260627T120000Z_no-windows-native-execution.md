# No Native Windows Execution in v0

- Status: Accepted
- Created: 2026-06-27T12:00:00Z

## Context

reportage uses shell-like scripting as a core part of its design. `$` steps are passed to a POSIX-compatible shell. Supporting native Windows execution would require either a separate execution backend, shell syntax translation, or limiting the script language to a lowest-common-denominator subset.

v0 aims to keep the specification and implementation small and well-defined.

## Decision

Native Windows shell execution is out of scope for v0.

Actions in `$` steps are executed through a POSIX-compatible shell, equivalent to:

```sh
sh -c '<action-text>'
```

The raw text after the leading `$` token is passed to the shell as the action text.

Windows users who want to run reportage must use a POSIX-compatible environment such as:

- WSL (Windows Subsystem for Linux)
- Dev Containers
- Another POSIX-like environment

This policy must be communicated clearly in user-facing documentation.

## Alternatives Considered

Supporting a Windows-native shell (`cmd.exe`, PowerShell) was considered. It would require maintaining separate execution semantics, testing on Windows CI, and either restricting the script language or translating it. The added complexity is not justified for v0.

Cross-platform shell abstraction libraries were considered. They would add a dependency and still cannot cover all POSIX shell idioms used in real-world E2E scripts.

## Consequences

### Positive Consequences

- Shell semantics are delegated to a single, well-specified shell (`sh`).
- The execution model stays simple and predictable.
- No Windows-specific CI infrastructure is required for v0.

### Negative Consequences

- Windows users cannot run reportage natively without WSL or a container.

### Neutral Consequences

- This decision is scoped to v0. Native Windows support may be considered in a future version once the POSIX execution model is stable.

## Compatibility

This decision is recorded in [TBD.md](../TBD.md) under "Native Windows shell execution".
