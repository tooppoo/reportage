# Design

This section is for maintainers of reportage. It preserves the knowledge the implementation cannot express: why the project exists, what constraints its architecture must keep, which technical directions were accepted, and how the project tests itself.

For the exact behavior contracts these decisions produced, see [the reference section](../reference/README.md). For the rationale behind individual decisions, see the ADRs under [`adr/`](../adr/README.md).

## Documents

- [Philosophy](philosophy.md): the design principles behind the v0 direction — what reportage is, what it deliberately is not, and the reasoning that shapes the DSL and runtime. Read this first when judging whether a proposed feature fits.
- [Design principles](design-principles.md): the structural cost constraints — thin core, transparent shims, opt-in adapters, post-processing analysis. Read this before adding a capability to the core runner.
- [v0 technical selection](v0-technical-selection.md): the accepted v0 technical direction at a glance, with links to the detailed specifications.

## Testing strategy

- [Testing overview](testing/README.md): how reportage tests itself, and how to decide whether a new test belongs in `.repor` self-testing, Rust integration tests, or Rust unit tests.
- [Self-testing](testing/self-testing.md): the `.repor` self-testing layer and the roles of the `e2e/`, `examples/`, and syntax fixture suites.
- [Rust integration tests](testing/rust-integration-tests.md): the execution basis, structural output, and boundary conditions verified in [`crates/reportage-cli/tests/`](../../crates/reportage-cli/tests/).
- [Rust unit / focused tests](testing/rust-unit-tests.md): the internal models verified directly in `reportage-core`.
- [Syntax conformance fixtures](testing/syntax-conformance.md): the known-valid and known-invalid `.repor` fixtures and their AST snapshots.

## Planned and undecided work

Deferred features and open design questions live in [the planning section](../planning/TBD.md). Nothing there is normative.
