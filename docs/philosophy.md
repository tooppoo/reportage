# Philosophy

reportage is a language-agnostic, coverage-aware E2E script runner.

It is inspired by Go's `testscript`: small text files, shell-like steps, and tests that exercise software from the outside. reportage takes that general direction and adapts it for a broader goal: runtime-independent E2E scripts whose command execution can be connected to language-specific coverage tooling through adapters.

This document defines the design principles for the v0 direction. It intentionally does not cover broader positioning such as "why reportage" or comparisons against other tools.

## E2E first, but lightweight

reportage treats E2E tests as a first-class testing layer.

E2E tests are often heavy because the system under test may require files, subprocesses, databases, services, containers, or framework bootstrapping. That cost may be unavoidable for the system under test. The runner itself should not add unnecessary weight.

For v0, reportage should stay small:

- plain text scripts;
- POSIX shell execution;
- isolated temporary workspaces;
- explicit assertions;
- PATH shims for command mediation;
- external coverage tools rather than a built-in coverage engine.

## Shell-like, not host-language test code

E2E tests should be readable as executable scenarios.

Host-language test code is useful for unit tests and many integration tests, but CLI E2E scenarios often read better as scripts:

```reportage
case "check json output" {
  $ mycli check --json

  assert exit 0
  assert stdout jq '.ok == true'
}
```

The DSL should preserve the directness of shell transcripts while adding test-specific structure: cases, setup, parameter variants, file heredocs, and rich assertions.

## Arrange, Act, Assert without forcing phases

reportage should make Arrange, Act, Assert easy to express:

- arrange with `before_each`, `file`, `copy`, and other setup steps;
- act with `$` shell steps;
- assert with explicit `assert` statements.

The DSL should not require formal `arrange`, `act`, or `assert` blocks in v0. CLI E2E tests often need repeated act/assert cycles in the same case, such as `init`, assert, then `check`, assert. A strict phase model would make those scenarios more awkward.

## Isolation by default

Each concrete case should run in its own workspace.

This includes cases expanded from parameter variants. Files created by `before_each` and files created inside a `case` are both written into that concrete case workspace. They are discarded when the case finishes, unless a debug option preserves them.

The v0 design deliberately avoids shared mutable test state:

- no `before_all`;
- no `after_all`;
- no shared workspace across cases;
- no module-global files that are implicitly visible to every case;
- no `after_each` unless a concrete need appears later.

This keeps test behavior deterministic and reduces order dependence.

## Language-agnostic scripts, adapter-specific execution

The script syntax should not depend on whether the system under test is written in Go, Rust, Ruby, JavaScript, PHP, Scala, Java, or another runtime.

Runtime-specific behavior belongs behind adapters and shims.

A reportage script should say:

```reportage
$ mycli check --json
```

It should not need to say whether `mycli` is implemented as:

- a Rust binary;
- a Node entrypoint;
- a Ruby executable with a coverage bootstrap;
- a PHP script with Xdebug or PCOV enabled;
- a JVM main class with a coverage agent;
- a Go coverage-instrumented binary.

The adapter decides how the registered command is executed.

## Coverage-aware, not a coverage engine

reportage should not implement coverage measurement itself.

Coverage semantics differ across runtimes: line coverage, statement coverage, branch coverage, region coverage, source-map remapping, JVM agents, process termination behavior, and report formats are all runtime-specific.

reportage should provide an execution boundary where coverage tools can be connected:

1. the adapter prepares coverage-aware command shims;
2. reportage runs the E2E script through those shims;
3. the adapter collects and finalizes coverage artifacts.

The runner orchestrates coverage-aware execution. The adapter and existing ecosystem tools measure coverage.

## PATH shims as the command boundary

v0 uses PATH shims to mediate command execution.

For each concrete case, reportage creates a case-local `bin` directory, places registered command shims in it, and prepends that directory to `PATH`. `$` steps are then executed by the POSIX shell.

This keeps `$` steps close to real shell scripts while still allowing adapters to intercept registered commands.

The runner should not parse and rewrite arbitrary shell pipelines in v0. Shell syntax should be handled by the shell. Command mediation should happen through PATH resolution.

## Explicit assertions

Assertions should be explicit and start with `assert`.

This avoids ambiguity between setup steps, shell steps, and checks. It also leaves room for richer assertions without turning the DSL into a shell dialect with hidden behavior.

Examples:

```reportage
assert exit 0
assert stderr empty
assert stdout contains "created"
assert stdout matches /release [0-9]+\.[0-9]+\.[0-9]+/
assert stdout jq '.ok == true'
assert file exists "CHANGELOG.md"
assert file-count ".rellog/entries/*.kdl" == 0
```

## Small syntax before expressive power

reportage should prefer a small, explicit syntax over a large convenience surface.

For v0, this means:

- case grouping is done by files, not nested `describe` blocks;
- cases do not nest;
- `params` are case-local;
- parameter variants use `variant`, not context-dependent `case` syntax;
- heredoc file content is raw unless `template` is explicitly requested;
- `jq` assertions depend on external `jq`;
- native Windows shell execution is out of scope.

Additional features should be introduced only when concrete examples show that the current model is insufficient.

## CLI first, not CLI only

CLI E2E is the first target because it is the simplest and clearest form of shell-like E2E testing.

The conceptual model should still leave room for non-CLI targets. A future adapter may start a web framework process, wait for readiness, issue HTTP requests, and collect coverage if the runtime allows it. If coverage cannot be collected, reportage should still be useful as a runtime-independent E2E script runner.

The v0 syntax and semantics should therefore avoid assumptions that only make sense for CLI tools.
