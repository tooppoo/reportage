# reportage

[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![CI](https://github.com/tooppoo/reportage/actions/workflows/ci.yml/badge.svg)](https://github.com/tooppoo/reportage/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/tooppoo/reportage/graph/badge.svg?token=qdk2zXMPED)](https://codecov.io/gh/tooppoo/reportage)
[![CodeQL](https://github.com/tooppoo/reportage/actions/workflows/github-code-scanning/codeql/badge.svg)](https://github.com/tooppoo/reportage/actions/workflows/github-code-scanning/codeql)

`reportage` is an explicit, runtime-agnostic, coverage-aware E2E scenario runner with shell-like actions.

It starts with CLI E2E tests, but the execution model is intentionally built around adapters and PATH shims so that other runtime targets, such as web framework processes, can be supported later.

reportage is inspired by Go's `testscript`, especially its lightweight script-oriented approach to E2E testing. It is not intended to be a compatible implementation. The v0 design focuses on explicit, structured scenarios with shell-like actions, per-case isolation, assertion blocks with checkpoint-based verification, case-local parameterized tests, and coverage-aware command execution through adapters.

## Status

reportage is in early design. The documents in this repository describe the intended v0 direction and may change as implementation work validates or rejects specific choices.

## Documentation

- [Philosophy](docs/philosophy.md): design principles and scope boundaries.
- [Syntax](docs/syntax.md): script syntax, including `before_each`, `case`, `params`, `variant`, file heredocs, shell steps, and assertions.
- [Semantics](docs/semantics.md): execution model, workspace lifecycle, parameter expansion, shell execution, PATH shims, and coverage adapter responsibilities.
- [Why reportage? / Why not reportage?](docs/why-or-why-not.md): When does `reportage` work well, and when does it not?
- [Design Principles](docs/design-principle.md): thin core, transparent shims, opt-in adapters, and evidence-first boundaries.
- [v0 Technical Selection](docs/v0-technical-selection.md): implementation foundation and links to detailed technical specs.
- [Configuration](docs/configuration.md): KDL v2 config shape, commands, tests, and validation rules.
- [Path Matching](docs/path-matching.md): path-like config value rules and test glob semantics.
- [Artifacts](docs/artifacts.md): default artifact generation and early result layout.
- [ADR](docs/adr/README.md): durable architecture and design decisions.
- [TBD](TBD.md): intentionally deferred features and design topics.

## v0 Direction

The v0 design is intentionally narrow:

- POSIX shell execution for `$` steps.
- Native Windows shell execution is out of scope; use WSL, a devcontainer, or Linux CI on Windows.
- One test file may contain multiple `case` blocks.
- `before_each` is optional, at most one per file, and runs before every concrete case.
- `params` are case-local in v0.
- Each parameter `variant` expands into a concrete case.
- Each concrete case runs in an isolated workspace.
- Registered commands are resolved through PATH shims.
- Coverage integration belongs to adapters and shims; reportage does not implement a coverage engine.
- `assert ... jq ...` uses external `jq` in v0.

## Non-goals for v0

- No native Windows shell support.
- No `before_all` or `after_all`.
- No `after_each` unless a concrete need appears later.
- No module-scope parameters in v0.
- No embedded jq engine in v0.
- No full shell parser or shell rewriting.
- No hidden fixture namespace such as `@fixture` in v0.
