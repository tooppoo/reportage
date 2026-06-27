# Use POSIX Shell Execution and PATH Shims in v0

- Status: Accepted
- Created: 2026-06-27T10:05:00Z

## Context

reportage scripts use `$` steps to execute commands. The v0 design intentionally avoids full shell parsing or shell rewriting. reportage also needs a boundary where adapters can mediate command execution and connect runtime-specific coverage tools.

## Decision

Use POSIX shell execution for `$` steps and use case-local PATH shims for registered commands.

For each concrete case, the runner creates a case-local `bin` directory, writes executable shims for registered commands, and prepends that directory to `PATH`.

## Alternatives Considered

Parsing and rewriting shell commands was considered, but it would make the runner a shell parser. Requiring a dedicated command syntax was considered, but it would move reportage away from the shell-like execution model.

## Consequences

### Positive Consequences

- Shell semantics are delegated to the shell.
- Command mediation remains transparent in scripts.
- Coverage-aware execution can be implemented through adapters and shims.

### Negative Consequences

- Native Windows shell execution is out of scope for v0.
- Only commands resolved through PATH can be intercepted reliably.

### Neutral Consequences

- Timeout support is deferred, but the execution layer should be structured so timeout and cancellation can be added later.
