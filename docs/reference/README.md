# Reference

This section holds the exact, normative contracts of reportage: what a script may contain, how it executes, what the CLI emits, and which identifiers are stable. Read a reference document when you need to know precisely what reportage does; read [the user guide](../guide/README.md) for when and why to use a feature, and [the design section](../design/README.md) for why the contracts are shaped this way.

Documents here are either generated from an executable source, mechanically checked against fixtures, or hand-written specifications. Each entry below says which.

## Language

- [Syntax reference](syntax.md) — **generated** from the normative syntax source [`crates/reportage-core/src/reportage.pest`](../../crates/reportage-core/src/reportage.pest); anything not expressible there is not part of v0. Regenerate with `just lang-docs-gen`.
- [Language semantics](semantics.md): overview and entry point for the semantics document set, plus the language semantic rules that have not yet migrated to the generated catalog. Read this before writing assertions.
- [Semantic rule catalog](semantic-rules.md) — **generated** from the JSON specs under [`spec/language/semantics/`](../../spec/language/semantics/README.md). Regenerate with `just semantic-docs-gen`.

## Runtime

- [Execution model](execution-model.md): how concrete cases are planned and run, the workspace and checkpoint lifecycle, `before_each`, shell execution, and the coverage adapter lifecycle. Read this before writing assertions that depend on checkpoints.
- [Shims](shims.md): the PATH overlay shim model for command resolution, interception limits, and shim invocation observability.
- [Shim event protocol](shim-event-protocol.md): the JSON event protocol compliant shims use to report invocations to the runner.
- [Shim scaffold](shim-scaffold.md): the `reportage shim scaffold` command, its builtin templates, and the output path policy.

## CLI input and output

- [Configuration](configuration.md): the `reportage.kdl` config format, loading rules, and validation errors.
- [Path matching](path-matching.md): the rules for path-like config values and test file glob discovery.
- [Documentation generation](docs-generation.md): the `reportage docs` subcommand, its input pattern rules, the plain text serialization contract, and the output replacement guarantees. Its representative outputs are mechanically checked against the generated example documents under `tests/fixtures/docs/`.
- [Exit codes](exit-codes.md): the process exit code policy and per-subcommand tables.
- [Artifacts](artifacts.md): the `.reportage/runs/<run-id>/` bundle, the `result.json` canonical manifest, and the raw evidence policy. Its JSON examples are mechanically checked against fixture snapshots.
- The stdout JSON contract for `--format=json` is owned by [`spec/output/json-report/`](../../spec/output/json-report/README.md), and the run result artifact contract by [`spec/artifacts/run-result/`](../../spec/artifacts/run-result/README.md).

## Diagnostics

- [Parse diagnostics](diagnostics.md): the stable `parse.*` diagnostic code system and its compatibility policy.
- [Semantic and assertion diagnostics](semantic-diagnostics.md): the `semantic.*`, `assertion.*`, and `step.*` namespaces, severity, locations, and details stability. This is a specification; parts may not yet be applied to CLI rendering.

## Decision rationale

The reference documents state what holds; the reason a contract was chosen lives in the ADRs under [`adr/`](../adr/README.md), linked from each document where relevant.
