# Testing

This directory documents how reportage tests itself. It exists because reportage is an E2E test runner, so its own test suite has to decide, for every behavior, whether that behavior is best verified by writing a `.repor` script or a Rust test.

The rule of thumb is that user-visible CLI behavior belongs in `.repor` self-testing, while execution-basis, structural, and fine-grained internal behavior belongs in Rust.

## Documents

- [self-testing.md](self-testing.md): the `.repor` self-testing layer. Covers what self-testing verifies, the command-resolution model it depends on, and how `e2e/` and `examples/` relate to each other.
- [rust-integration-tests.md](rust-integration-tests.md): the Rust integration test layer in `crates/reportage-cli/tests/`. Covers self-testing bootstrap, PATH shim resolution, coverage/subprocess routing, artifact JSON structure, and diagnostic/exit-code mapping.
- [rust-unit-tests.md](rust-unit-tests.md): the Rust unit / focused test layer inside `crates/reportage-core/src/` and `crates/reportage-core/tests/`. Covers the parser, AST, semantic evaluator, expectation evaluator, diagnostic construction, and artifact model.

## How the layers relate

```text
.repor self-testing         -> user-visible CLI-level E2E behavior
Rust integration tests      -> self-testing bootstrap, boundary conditions, structural validation
Rust unit / focused tests   -> parser, AST, semantic evaluator, diagnostic, artifact model
```

Self-testing is representative self-testing, not full self-hosting. It depends on an execution basis, PATH shim resolution, and Cargo-built binary invocation, that reportage cannot verify about itself without circularity. That basis, along with structural validation that is awkward to express as CLI-level assertions, stays in Rust integration tests. The internal models that CLI behavior is built on, the parser, AST, semantic evaluator, and diagnostic construction, are verified directly in Rust unit tests, independent of any CLI invocation.

## Deciding where a new test belongs

Ask these questions in order:

1. **Is the behavior visible through the CLI, expressible as a `$` action and an `assert` block, and does asserting it not require inspecting internal Rust types?** Write a `.repor` self-test. See [self-testing.md](self-testing.md).
2. **Does verifying the behavior require the self-testing harness itself, PATH shim resolution, the Cargo-built binary, coverage/subprocess routing, or exact artifact JSON / diagnostic structure?** Write a Rust integration test. See [rust-integration-tests.md](rust-integration-tests.md).
3. **Is the behavior internal to a single component, parser output, AST shape, semantic evaluation, expectation evaluation, diagnostic construction, or artifact model, and does it not need a full CLI invocation to observe?** Write a Rust unit / focused test. See [rust-unit-tests.md](rust-unit-tests.md).

When a scenario could be written at more than one layer, prefer the CLI-level `.repor` self-test for the user-visible outcome, and keep the Rust test only where it verifies something the self-test cannot, such as artifact JSON field structure or an internal error classification. Avoid maintaining two tests for the same observation; if a temporary duplicate is kept during a migration, say why in a comment.
