# Rust unit / focused tests

This document describes the responsibility of Rust unit and focused tests inside `crates/reportage-core/`. For how this layer relates to Rust integration tests and `.repor` self-testing, see [the testing overview](README.md).

## Role

Rust unit and focused tests verify reportage's internal models directly, without going through a full CLI invocation. They exist so a failure in the parser, AST, semantic evaluator, expectation evaluator, diagnostic construction, or artifact model can be localized to the component that produced it, rather than only observed as a CLI-level symptom.

## What belongs here

- **Parser.** Grammar acceptance/rejection for inline script fragments, in [`crates/reportage-core/src/parser.rs`](../../../crates/reportage-core/src/parser.rs).
- **AST / model.** Structural shape of parsed constructs (`Case`, `Step`, `Expectation`, matchers, literals) as produced by the parser, also in `parser.rs` and [`crates/reportage-core/src/model.rs`](../../../crates/reportage-core/src/model.rs).
- **Semantic evaluator.** Evaluation of expectations and logical composition (`all`/`any`/`not`) against a workspace/process state, in [`crates/reportage-core/src/semantic.rs`](../../../crates/reportage-core/src/semantic.rs) and [`crates/reportage-core/src/evaluator/mod.rs`](../../../crates/reportage-core/src/evaluator/mod.rs).
- **Expectation evaluator.** Checkpoint-based evaluation of individual expectations (exit, stdout/stderr, file/dir, contents-equals), in [`crates/reportage-core/src/evaluator/expectation.rs`](../../../crates/reportage-core/src/evaluator/expectation.rs) and [`crates/reportage-core/src/contents_diagnostic.rs`](../../../crates/reportage-core/src/contents_diagnostic.rs).
- **Diagnostic construction.** Building diagnostic values and stable diagnostic codes from parse/semantic/config failures, in [`crates/reportage-core/src/diagnostic.rs`](../../../crates/reportage-core/src/diagnostic.rs).
- **Artifact model.** Run-directory and result construction rules, such as fixed run id collision rejection (`for_fixed_run_rejects_existing_run_directory`), in [`crates/reportage-core/src/artifact.rs`](../../../crates/reportage-core/src/artifact.rs).
- **Config / fixture / shim models.** Config parsing and validation (`config.rs`), fixture resolution (`fixture.rs`), and shim/command-name/executable-invocation construction (`shim.rs`, `shim_event.rs`), independent of any CLI invocation.

## Focused tests in `crates/reportage-core/tests/`

Alongside `src/`-local unit tests, [`crates/reportage-core/tests/`](../../../crates/reportage-core/tests/) holds focused tests that exercise the same internal models against checked-in fixtures or specs, still without a CLI invocation:

- `grammar_fixtures.rs`: parses every `.repor` file under `examples/` and `e2e/` through `reportage_core::parser::parse`, guarding the grammar against drifting away from real scripts.
- `syntax_conformance.rs`: locks down which checked-in syntax fixtures are accepted or rejected by the production `parse()` entrypoint.
- `semantic_specs.rs`: loads `spec/language/semantics/*.json` conformance cases and runs them directly against the production semantic evaluator.

These are unit-adjacent: they call into `reportage_core` directly and assert on its internal types, they do not spawn the `reportage` binary or go through the CLI. That is what distinguishes them from Rust integration tests; see [Rust integration tests](rust-integration-tests.md).

## What does not stay here

- Anything that requires spawning the `reportage` binary, a shell action, or a subprocess. That is a Rust integration test or `.repor` self-test concern; see [Rust integration tests](rust-integration-tests.md) and [Self-testing](self-testing.md).
- Exact `result.json` structure as externalized by the CLI. The internal artifact model's construction rules are verified here; how the CLI serializes and exposes that model is verified in Rust integration tests.

## Internal model vs. externalized result

See [rust-integration-tests.md — Internal model vs. externalized result](rust-integration-tests.md#internal-model-vs-externalized-result) for the boundary between verifying a model directly (here) and verifying what the CLI externalizes for that model (Rust integration tests).
