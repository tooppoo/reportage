# Philosophy

reportage is an explicit, runtime-agnostic, coverage-aware E2E scenario runner with shell-like actions.

It is inspired by Go's [`testscript`](https://pkg.go.dev/github.com/rogpeppe/go-internal/testscript): small text files, shell-like steps, and tests that exercise software from the outside. reportage takes that general direction and adapts it for a broader goal: runtime-independent E2E scripts whose command execution can be connected to language-specific coverage tooling through adapters.

This document defines the design principles for the v0 direction. It intentionally does not cover broader positioning such as "why reportage" or detailed comparisons against other tools.

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

## Shell scripts are the natural substrate for black-box testing

When developers test a CLI or a black-box system manually, they usually do not start by writing host-language test code. They create files, run commands, inspect stdout and stderr, check exit codes, and compare generated artifacts.

That workflow is already close to a shell script.

reportage starts from this observation. For developer-facing E2E tests, shell-like action syntax is often the most direct way to express a test step. The goal is not to make tests read like prose. The goal is to make executable behavior easy to read, write, review, and run.

```reportage
case "check json output" {
  $ mycli check --json

  assert {
    exit 0
    stdout jq '.ok == true'
  }
}
```

## More than shell scripts

Raw shell scripts are simple, but they are weak as a test format.

They do not provide a consistent assertion model, parameterized cases, structured test results, per-case isolation, rich diagnostics, or a natural way to connect E2E execution to language-specific coverage tooling.

Bash-based testing frameworks can improve some of these problems, but they still usually leave coverage integration to project-specific glue. They also tend to inherit shell-level complexity when the goal is often simpler: describe files, run commands, and assert observable behavior.

reportage keeps the directness of shell scripts, but adds the missing test structure:

- `case` blocks;
- `before_each` setup;
- case-local `params` and `variant`s;
- file heredocs;
- explicit assertions;
- jq-based JSON checks;
- isolated workspaces;
- PATH-shim based command mediation;
- adapter-based coverage integration.

Ordinary filesystem operations should remain ordinary shell operations in v0. If a test needs to create a directory, copy fixture files, move files, or remove temporary files, it can use `$ mkdir`, `$ cp`, `$ mv`, or `$ rm`. reportage should add syntax where the shell is weak as a test format, not duplicate shell commands unnecessarily.

## Developer-readable, not prose-first

reportage lives near BDD and acceptance-testing tools in the broad sense that it describes externally observable behavior.

However, reportage does not optimize for prose-like scenarios. It optimizes for operation-first specifications: files, commands, exit codes, stdout, stderr, JSON output, generated artifacts, and coverage-aware command execution.

This makes reportage different from prose-first tools such as Cucumber. Cucumber-style frameworks are often useful when the primary goal is to express behavior in a form that is readable by a wider group of stakeholders. reportage focuses on a narrower but more direct audience: developers who need to understand and execute the concrete behavior of a system.

These tools may not be strict competitors. A prose-first BDD framework could describe the human-readable scenario, while reportage could provide the developer-readable, shell-like, coverage-aware verification behind a step definition. This is a hypothesis for future integration, not a v0 design requirement. It suggests that reportage should remain easy to invoke from other tools, but it does not require reportage to become a Cucumber plugin or a BDD framework.

## Executable specifications in AI-assisted development

AI-assisted development makes it easier to generate large amounts of implementation code and test code. That increases the value of tests that describe intended behavior at the system boundary.

Unit tests remain important for local correctness, but they can become too detailed to explain whether the system as a whole behaves as intended. E2E tests can serve as executable specifications: concrete files, commands, outputs, and artifacts that describe externally visible behavior.

reportage is designed for that role. It should be simple enough for developers and AI assistants to produce many useful E2E scenarios, while still being structured enough for humans to review and for CI to execute.

## Unit tests and E2E tests are complementary

reportage is not a replacement for unit tests.

Unit tests are effective for local invariants, edge cases, and small pieces of logic. reportage focuses on externally observable behavior: how the system behaves when invoked through commands, files, services, and adapters.

The goal is not to move all testing to E2E. The goal is to make E2E tests cheap, readable, and coverage-aware enough that they can represent high-level intent instead of being treated only as a small number of slow smoke tests.

## Arrange, Act, Assert without forcing phases

reportage should make Arrange, Act, Assert easy to express:

- arrange with `before_each`, `file`, and ordinary shell setup steps;
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

## Explicit assertion blocks

Assertion blocks are explicit and start with `assert { ... }`.

This avoids ambiguity between setup steps, shell steps, and verification. Each `assert { ... }` block is a checkpoint-level verification construct: it names an explicit boundary where observable state is checked against a set of expectations.

Using a block rather than individual assertion lines makes the grouping and failure aggregation unit visible in the source.

Examples:

```reportage
assert {
  exit 0
}

assert {
  exit 0
  stderr empty
  stdout contains "created"
  stdout matches /release [0-9]+\.[0-9]+\.[0-9]+/
  stdout jq '.ok == true'
  file exists "CHANGELOG.md"
  file-count ".rellog/entries/*.kdl" == 0
}
```

## Small syntax before expressive power

reportage should prefer a small, explicit syntax over a large convenience surface.

For v0, this means:

- case grouping is done by files, not nested `describe` blocks;
- cases do not nest;
- `params` are case-local;
- parameter variants use `variant`, not context-dependent `case` syntax;
- heredoc file content is raw unless `template` is explicitly requested;
- ordinary filesystem operations use `$` shell steps instead of dedicated syntax;
- `jq` assertions depend on external `jq`;
- native Windows shell execution is out of scope.

Additional features should be introduced only when concrete examples show that the current model is insufficient.

## CLI first, not CLI only

CLI E2E is the first target because it is the simplest and clearest form of shell-like E2E testing.

The conceptual model should still leave room for non-CLI targets. A future adapter may start a web framework process, wait for readiness, issue HTTP requests, and collect coverage if the runtime allows it. If coverage cannot be collected, reportage should still be useful as a runtime-independent E2E script runner.

The v0 syntax and semantics should therefore avoid assumptions that only make sense for CLI tools.
