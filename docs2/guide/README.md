# User guide

This section is for people using reportage: deciding whether to adopt it, writing `.repor` scripts, configuring a project, and reading run results.

This guide is navigational and conceptual. Exact contracts — grammar, assertion semantics, configuration rules, exit codes, output schemas — live in [the reference section](../reference/README.md) and are linked from here rather than restated.

## Documents

- [Why reportage? / Why not reportage?](why-reportage.md): read this first when deciding whether reportage fits your project. It explains the execution model reportage is built around and names the cases where a more specialized tool is the better choice.

## Common tasks

### Learn the script syntax

Known-good runnable scripts are the fastest way in: see [`examples/`](../../examples/) in the repository root. The normative grammar is the generated [syntax reference](../../docs/syntax.md), and the behavior of each construct is defined in [the language semantics reference](../reference/semantics.md).

### Configure a project

Test discovery and command registration live in `reportage.kdl`. See [the configuration reference](../reference/configuration.md) for the format and [the path matching reference](../reference/path-matching.md) for how test file patterns resolve.

### Run tests and interpret results

- Process exit codes and their severity model: [the exit code reference](../reference/exit-codes.md).
- The artifact bundle written under `.reportage/runs/`: [the artifacts reference](../reference/artifacts.md).
- Diagnostic codes appearing in output: [the parse diagnostics reference](../reference/diagnostics.md) and [the semantic and assertion diagnostics reference](../reference/semantic-diagnostics.md).

### Connect coverage tooling

reportage delegates coverage to adapters and PATH shims rather than measuring coverage itself. See [the shims reference](../reference/shims.md) for the command-resolution model and [the shim scaffold reference](../reference/shim-scaffold.md) for generating a starting-point coverage shim with `reportage shim scaffold`.
