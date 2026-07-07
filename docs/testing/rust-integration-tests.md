# Rust integration tests

This document describes the responsibility of Rust integration tests in `crates/reportage-cli/tests/`. For how this layer relates to `.repor` self-testing and Rust unit tests, see [README.md](README.md).

## Role

Rust integration tests verify the execution basis, structural output, and boundary conditions that self-testing cannot verify about itself without circularity. Self-testing depends on a Cargo-built binary, PATH shim resolution, and coverage/subprocess wiring already working; those preconditions have to be verified from outside the self-testing layer.

Rust integration tests are not the primary place to describe CLI-level user scenarios. Where a scenario is fully expressible as a `$` action and an `assert` block with no need to inspect internal Rust types, prefer a `.repor` self-test; see [self-testing.md](self-testing.md). `crates/reportage-cli/tests/integration_test.rs` currently contains many such CLI-level scenarios; these are expected to shrink over time as representative cases move to `e2e/` self-tests, leaving Rust integration tests to the responsibilities below.

## What stays in Rust integration tests

- **Self-testing harness bootstrap.** That the harness resolves and invokes the Cargo-built `reportage` binary, not an installed one; see `crates/reportage-cli/tests/self_test.rs`.
- **PATH shim resolution.** That a shim placed ahead of the inherited `PATH` resolves before any ambient binary of the same name, and that nested `reportage` invocations inside self-tests connect to the expected binary. See `shim_resolves_before_ambient_reportage` in `crates/reportage-cli/tests/self_test.rs`.
- **Coverage / subprocess routing.** The preconditions coverage collection depends on, that a shim correctly routes a subprocess invocation so coverage data attaches to the right binary. For the general shim/coverage distinction, see [../shims.md](../shims.md).
- **Artifact JSON structure.** Field-level and structure-level validation of `result.json` and related artifacts, for example `result_json_is_written`, `artifacts_directory_is_created_on_passing_run`, and `result_json_contains_shim_invocations_when_shim_is_used` in `crates/reportage-cli/tests/integration_test.rs`. A `.repor` self-test can assert that a stable field/value marker exists in an artifact file (see [self-testing.md](self-testing.md)), but exhaustive schema shape belongs here.
- **Diagnostic / exit-code mapping.** That specific failure categories, malformed config, parse errors, semantic errors, unsupported constructs, map to the documented exit codes; see tests such as `invalid_exit_code_value_exits_with_code_two`, `unsupported_expectation_type_exits_with_code_two`, and `pre_execution_validation_blocks_all_execution_on_parse_error`.
- **Config / parse / semantic pre-execution validation.** That invalid configuration or scripts are rejected before any case executes, including combined config-and-scripts rejection and dot-segment path rejection.
- **Filesystem / run-directory boundary conditions that are awkward to express in `.repor`.** For example, fixed run id collision handling (`debug_run_id_does_not_silently_overwrite_existing_run_directory`), which depends on pre-seeding a run directory before invocation.
- **Shim invocation events and shim stderr warnings.** That shim-emitted invocation events attach to results/artifacts, and that a shim's failure-to-write warning is observable stderr and is not filtered from stderr assertions; see `shim_stderr_warning_is_not_filtered_from_stderr_empty_assertion` and `failing_case_with_shim_shows_shim_metadata_in_cli_output`.

## What does not stay here

- Representative passing/failing CLI scenarios whose only assertions are process-level (`exit`, `stdout`, `stderr`) and that a `.repor` self-test already covers, or is planned to cover as part of migrating representative cases into `e2e/`.
- Internal model behavior, parser output shape, semantic evaluation of a single expression, diagnostic construction, that does not require a full CLI invocation to observe; see [rust-unit-tests.md](rust-unit-tests.md).

## Internal model vs. externalized result

A useful boundary when deciding whether a test belongs here or in Rust unit tests: does it verify an internal model directly, or does it verify what the CLI externalizes for that model?

```text
internal model (parser output, AST, semantic evaluation result, diagnostic value)
  -> verified directly              => Rust unit / focused test

CLI-externalized result (process exit code, stdout/stderr text, result.json, diagnostic
  rendering as seen by a caller of the binary)
  -> verified through a full CLI invocation => Rust integration test
```

For example, that a semantic evaluator produces a given diagnostic value is a unit-test concern. That the CLI maps a category of failure to a specific process exit code, or renders that diagnostic in `result.json`, is an integration-test concern.
