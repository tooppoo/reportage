# Why reportage? / Why not reportage?

## Overview

`reportage` is an explicit, runtime-agnostic, coverage-aware E2E scenario runner with shell-like actions.

Its first target is CLI testing, but its core value is not limited to CLI. The main claim of `reportage` is not the target domain, but the execution model.

`reportage` provides a lightweight E2E execution model where setup, shell-like actions, and explicit assertion blocks stay close together, while runtime-specific execution and coverage instrumentation are delegated to adapters and PATH shims.

More specifically, the primary field of `reportage` is verification of externally observable evidence at E2E boundaries: process results, files, generated artifacts, structured output, HTTP responses, logs, and coverage artifacts.

This model can naturally target CLI commands, HTTP APIs, services, and web frameworks. Some targets may support coverage integration. Others may only support runtime-independent E2E execution. `reportage` should support both cases.

`reportage` is not a coverage engine. It does not replace existing coverage tools. Instead, it provides a structured way to connect existing runtime coverage tools to E2E execution.

## Why reportage?

Use `reportage` when you want E2E tests that are explicit, lightweight, runtime-agnostic, and coverage-aware.

Many projects need to test behavior at the boundary where users or external systems interact with the software: command execution, files, environment variables, stdout, stderr, exit codes, HTTP responses, generated artifacts, or service behavior.

The important object of verification is observable evidence: the concrete inputs, actions, outputs, artifacts, and diagnostics that show what happened at that boundary.

Unit tests and integration tests often do not fully cover this boundary. Plain shell scripts can exercise it, but they tend to become difficult to assert, parameterize, structure, and connect to coverage reporting.

`reportage` aims to provide a middle ground:

- close to shell script
- more structured than shell script
- lighter than full BDD frameworks
- not tied to one language runtime
- designed to connect with existing coverage tooling
- extensible from CLI to services, HTTP APIs, and web framework E2E

A `reportage` test keeps the important parts of an E2E scenario close together:

- fixture files
- shell-like execution steps
- exit code assertions
- stdout / stderr assertions
- file assertions
- file count assertions
- JSON assertions through `jq`
- coverage-aware execution through adapters and PATH shims

Example:

```text
case "invalid config returns JSON diagnostic" {
  file ".app/config.kdl" <<'KDL'
kind "unknown"
KDL

  $ app check --json

  assert {
    exit 1
    stderr empty
    stdout jq '.ok == false'
    stdout jq '.diagnostics[0].code == "UNKNOWN_KIND"'
  }
}
```

The test remains readable as an execution scenario. The input, command, and expected observations are visible in one place.

## Execution model, not target domain

The central claim of `reportage` is not that it supports a particular domain such as CLI, HTTP API, or web framework testing.

The central claim is that E2E tests should be written against a stable, runtime-agnostic execution model.

In this model:

- test syntax is independent from the application runtime
- commands are executed through a POSIX shell-like interface
- registered commands can be intercepted through PATH shims
- runtime-specific execution details are handled by adapters
- coverage instrumentation is adapter-defined
- assertions verify externally observable evidence
- coverage-enabled and coverage-disabled runs should not require different test scripts

This distinction matters.

[Cucumber](https://github.com/cucumber) separates specification text from step implementation.

`reportage` separates E2E scripts from runtime execution, coverage instrumentation, and evidence collection.

[Cypress](https://github.com/cypress-io/cypress) and [Playwright](https://github.com/microsoft/playwright) focus primarily on browser-oriented E2E workflows.

`reportage` focuses on a general E2E execution model that can include CLI, HTTP, services, and web frameworks when those targets can produce observable evidence for direct assertions.

[Hurl](https://github.com/Orange-OpenSource/hurl), [Bruno](https://github.com/usebruno/bruno), [Postman](https://github.com/postmanlabs), and similar tools focus on API-client or API-collection workflows.

`reportage` can test HTTP APIs, but it approaches them as E2E targets within a broader evidence-oriented execution model rather than as API collections.

[Bats](https://github.com/bats-core) and shell scripts are close to command-line execution.

`reportage` keeps the shell-like feel while adding structured cases, inline fixtures, richer assertions, and coverage-aware command resolution.

## Coverage-aware, not coverage-owning

`reportage` does not implement coverage instrumentation itself.

Coverage support depends on each runtime ecosystem. For example, different ecosystems may use different mechanisms:

- instrumented binaries
- runtime coverage environment variables
- coverage bootstrap files
- language-specific agents
- coverage report merging tools

`reportage` should not replace these tools. Instead, it should provide a common E2E execution model that lets adapters connect them to test execution.

The intended model is:

```text
reportage script
  -> shell-like execution
  -> PATH shim
  -> runtime adapter
  -> existing coverage tool
  -> coverage artifact collection
```

This makes E2E coverage explicit, repeatable, and adaptable.

The claim is not:

```text
Only reportage can collect coverage from E2E tests.
```

The claim is:

```text
reportage treats coverage-aware execution as part of the E2E execution model.
```

That distinction is important. Existing E2E tools can often be combined with coverage tools, but coverage integration is commonly external, framework-specific, or project-specific. `reportage` makes that integration a first-class design concern through adapters and PATH shims.

## CLI first, not CLI only

The first target for `reportage` is CLI E2E.

CLI is a good first target because it naturally exposes the concerns `reportage` wants to model:

- command execution
- exit codes
- stdout
- stderr
- files
- environment variables
- generated artifacts
- process boundaries
- coverage-aware command invocation

However, the design should not assume that all E2E targets are CLI commands.

The same model can extend to services and web frameworks:

```text
case "health endpoint returns ok" {
  start app

  http GET "/health"

  assert http status 200
  assert http body jq '.ok == true'
}
```

In this model, `start app` can be handled by an adapter. The adapter may start a Node, Ruby, PHP, JVM, Go, Rust, or other service with or without coverage instrumentation.

Coverage may be available for some adapters and unavailable for others. That should be represented as adapter capability, not as a global guarantee.

## Why not reportage?

Do not use `reportage` when a more specialized tool better matches the primary need.

### Use Cucumber when the goal is business-readable BDD

Cucumber is better when the primary goal is shared, business-readable executable specification.

If scenarios need to be written in a product language and read by product owners, QA, and non-developer stakeholders, Gherkin is a better fit.

`reportage` is more developer-facing. It favors explicit fixtures, commands, assertions, and externally observable evidence over natural-language specifications.

### Use Playwright or Cypress directly for rich browser interaction testing

If the main target is browser UI automation, cross-browser behavior, tracing, screenshots, selectors, browser contexts, and complex interaction flows, dedicated browser E2E tools are better.

`reportage` may eventually integrate with browser-oriented workflows, but it should not try to replace mature browser automation tools. Its primary browser-adjacent role is to assert evidence produced by a web system or by specialized browser tools, not to become the default interaction engine itself.

### Use API-client tools when API workflow management is the main need

Use tools such as Hurl, Bruno, Postman, or similar systems when the primary need is API collection management, interactive request editing, request sharing, or API-client workflow.

`reportage` can treat HTTP APIs as E2E targets, but it is not primarily an API client or API collection manager. HTTP API testing is not secondary in `reportage`; it is simply approached through a different model.

The difference is this:

```text
API-client tools organize API requests.

reportage organizes E2E execution and evidence verification.
```

If the main concern is request collections, environments, manual exploration, and API-client ergonomics, use an API-client tool.

If the main concern is shell-like setup, service orchestration, fixture locality, explicit assertions, observable evidence, and coverage-aware runtime execution, use `reportage`.

### Use Bats or plain shell scripts for simple command testing

If a project only needs a few command-line smoke tests, a shell script, Bats, or Cram may be enough.

`reportage` becomes useful when tests need more structure:

- multiple cases per file
- inline fixture definitions
- richer assertions
- jq assertions
- file assertions
- file count assertions
- coverage-aware command interception
- runtime adapters

For simple command checks, `reportage` may be unnecessary.

### Use native test runners for unit and integration tests

`reportage` should not replace native unit or integration tests.

Use the language ecosystem's native test runner when the test should call internal APIs, inspect in-memory structures, use mocks, or check small units of behavior.

`reportage` is for E2E boundaries: processes, services, files, HTTP, shell execution, externally observable behavior, and evidence generated at those boundaries.

### Do not use reportage when native Windows shell support is required

In v1, `$` steps are POSIX shell commands.

Native Windows shell execution is out of scope. Windows users should use WSL, devcontainers, or Linux-based CI.

This is an intentional trade-off to keep the shell-like execution model simple and predictable.

### Do not use reportage when universal coverage support is expected

`reportage` cannot guarantee coverage support for every language, runtime, framework, or deployment model.

Coverage depends on the ecosystem and adapter implementation. Some runtimes may support precise coverage. Some may support best-effort coverage. Some may not support coverage in a given execution mode.

`reportage` provides the adapter and shim model. It does not guarantee that every adapter can produce coverage.

## Summary

Use `reportage` when you need:

- explicit, structured E2E scenarios
- shell-like action syntax
- inline fixtures
- assertion blocks with checkpoint-based verification
- rich expectations per checkpoint
- jq-based JSON assertions
- externally observable evidence verification
- CLI-first testing
- extensibility toward HTTP APIs, services, and web frameworks
- runtime-independent test syntax
- coverage-aware execution through adapters and PATH shims

Do not use `reportage` merely because you need any E2E tool.

Use it when the execution model matters.

The core position is:

```text
reportage is not defined by the target domain.
reportage is defined by its E2E execution model.
that model verifies externally observable evidence.
```

Or, more compactly:

```text
reportage is an explicit, runtime-agnostic, coverage-aware E2E scenario runner for externally observable evidence.
```
