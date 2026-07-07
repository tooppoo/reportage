# Config-Driven Command Registration and Exec Path Resolution

- Status: Accepted
- Created: 2026-07-07T17:38:09Z

## Context

`docs/configuration.md` already documented a `reportage.commands` block for registering a
command id and an `exec` target, and `docs/semantics.md` / `docs/shims.md` already documented
that a registered command resolves through a case-local PATH shim. Neither was implemented:
`ReportageConfig` only held `tests`, and no code path connected config-driven runs to
`ExecutionEnvironment` or shim generation. Without this, reportage could not be recommended for
its primary intended use — registering an application's real entrypoint (or adapter-provided
entrypoint) as a `$`-callable command name in a `.repor` script.

This ADR records the decisions needed to close that gap:

- where `commands.command.exec` resolution happens, and whether it uses filesystem-touching
  canonicalization or purely lexical absolutization;
- which component owns case-local shim materialization;
- how config-driven command registration composes with the existing `ExecutionEnvironment` /
  `evaluator::evaluate` execution path;
- how explicit script mode is affected.

These affect the config schema's runtime contract and a public CLI behavior (which command names
resolve to what, and when), so they belong in an ADR rather than only in the issue/PR discussion.

## Decision

### `exec` resolution: lexical absolutization, not canonicalization

`commands.command.exec` is parsed as a config-file-relative string (`config::CommandConfig::exec`)
and is resolved to an absolute path at run setup time (`reportage-cli`'s `run_with_config`), not
during `config::parse_config`.

Resolution joins `exec` onto the config file's directory and lexically absolutizes the result
(equivalent to Rust's `std::path::absolute`): no filesystem access, no symlink resolution, and no
requirement that the target already exists at config-load time.

This must not use `std::fs::canonicalize` (or an equivalent that touches the filesystem), because
the target executable commonly does not exist yet when the config is parsed — e.g. a
not-yet-built `target/debug/myapp` in a workflow where the user builds the project and then runs
`reportage`. Requiring existence at config-parse time would make config loading fail before the
user has had a chance to build anything, for a check that is redundant anyway: an `exec` that does
not point to a real executable will simply fail when the shim is actually invoked, with the
ordinary shell "command not found" / exec failure behavior.

### Case-local shim materialization, not global setup

Config parsing produces a `CommandRegistry` (`reportage-core`'s `shim::CommandRegistry`): a
resolved list of `(CommandName, ExecutableInvocation)` pairs. This registry is not materialized to
disk once at startup. Instead, `evaluator::evaluate_case` materializes a fresh copy of every
registered command into `<case-workspace>/bin` after `Workspace::new()` creates that case's
isolated workspace, and prepends that directory to the case's `ExecutionEnvironment` PATH
prefixes.

This matches the existing per-case isolation model (each concrete case already gets its own
workspace, destroyed on drop) and keeps a case's shim directory lifetime tied to that case's
workspace lifetime, with no cross-case shared mutable state.

### Threading the registry through `evaluate`

`evaluator::evaluate` and `evaluator::evaluate_case` take a `&CommandRegistry` parameter.
`CommandRegistry` implements `Default` as an empty registry, which is what non-config-driven
runs pass. This was chosen over adding a second `evaluate_with_commands` entry point (which would
duplicate the case-execution loop) or folding the registry into `ExecutionEnvironment` (which
would conflate a static, run-wide PATH-prefix list with something that must be materialized fresh
per case).

### Explicit script mode never registers commands

`reportage <script>...` (explicit script mode) does not read a config file at all in v0, so it
never has a `CommandRegistry` to build. `reportage-cli`'s explicit-script code path always passes
`CommandRegistry::default()`. A config file registering commands may exist in the working
directory; explicit script mode does not consult it. Command registration requires `--config
<path>` or the default config mode (`reportage` with no script arguments). No option is added to
explicit script mode for opting into config-driven command registration — see Non-Goals.

## Alternatives Considered

### Canonicalize `exec` at config-parse time

Considered requiring `exec` to resolve to an existing file via `std::fs::canonicalize` during
`config::parse_config`, mirroring the existing "each `tests.path` pattern must match at least one
file" validation.

Rejected: `tests.path` validates that test *scripts* already exist, which is reasonable because
scripts are checked into the repository before a run. `exec` targets are frequently build
artifacts that do not exist until a build step runs after the config is written. Canonicalizing at
parse time would force build-then-configure ordering onto every project and would produce a
confusing "config error" for what is actually a "nothing has been built yet" state.

### Materialize shims once at config-load time into a single shared directory

Considered generating all registered command shims once, into one directory shared across every
case, instead of per case.

Rejected: it would work for the common case where every command's target is static for the whole
run, but it breaks the existing invariant that a case's `bin` directory (and everything under its
workspace) is destroyed with that case's workspace, and it introduces shared mutable state across
concurrently-evaluated cases. Per-case materialization is more filesystem I/O (one `write` +
`chmod` per command per case) but keeps every case fully isolated, which matches
`docs/semantics.md`'s existing workspace lifecycle model.

## Consequences

### Positive Consequences

- `reportage.kdl`'s documented `commands` block now has a working implementation; the
  previously-aspirational docs in `docs/configuration.md` / `docs/semantics.md` / `docs/shims.md`
  are now accurate.
- `exec` targets can be configured before they are built, matching normal project workflows.
- Case isolation is preserved: a registered command's shim lives and dies with its case's
  workspace, exactly like every other case-local artifact.

### Negative Consequences

- Every concrete case now performs one `write` + `chmod` per registered command, even when the
  case never invokes that command. For a large `commands` block and a large test suite this is a
  fixed per-case cost. It was judged acceptable because shim materialization is inexpensive
  relative to actually running a case's `$` actions.
- `exec` resolution error handling now spans two phases (structural/id validation at config parse
  time; path resolution and `ExecutableInvocation` validation at run setup time), so a caller
  needs to look in `reportage-cli::resolve_command_registry` as well as
  `reportage-core::config::parse_config` to see every way command registration can fail.

### Neutral Consequences

- v0's `commands.command.exec` remains a single program path with no fixed arguments; interpreter
  invocations (e.g. `ruby tool.rb`) and a fixed-args config syntax remain shim concepts
  (`shim::ExecutableInvocation` already supports fixed `args`) that are not yet exposed through
  `reportage.kdl`.

## Non-Goals

- Adding a command-registration option to explicit script mode (`reportage <script>...`).
- A config syntax for fixed arguments or interpreter-plus-script invocations.
- Any change to the shim invocation event protocol or its observability contract.
