//! Suite self-tests: run the `.repor` suites (`e2e/`, `examples/`,
//! `tests/fixtures/syntax/{valid,invalid}`) through the cargo-built `reportage` binary via a
//! PATH overlay shim.
//!
//! Both the outer suite invocation (`reportage --config ...`) and the inner `$ reportage ...`
//! steps inside the scripts resolve through the same shim (see `support`), so the whole run
//! exercises the command-resolution model users rely on while `cargo llvm-cov` collects
//! coverage from every delegated invocation.
//!
//! Coverage scope: what these tests put on the coverage-collection path is the shim-delegated
//! cargo-built `reportage` binary only. Arbitrary external subprocesses spawned by actions are
//! not claimed as coverage targets here; connecting other runtimes is a future coverage-adapter
//! responsibility.
#![cfg(unix)]

mod support;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Output;

use assert_fs::TempDir;

use support::{ShimHarness, workspace_root};

/// Render an `Output` for assertion failure messages.
fn describe(output: &Output) -> String {
    format!(
        "status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    )
}

/// Parse CLI stdout of a `--format=json` run as a single JSON document.
fn parse_json_stdout(output: &Output) -> serde_json::Value {
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!(
            "stdout was not a single valid JSON document: {e}\n{}",
            describe(output)
        )
    })
}

/// The `category == "parse"` entries of a JSON report's `diagnostics`.
fn parse_diagnostics(json: &serde_json::Value) -> Vec<&serde_json::Value> {
    json["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["category"] == "parse")
        .collect()
}

/// Confirm that under the overlay PATH, `reportage` resolves to the generated shim rather
/// than an ambient `reportage` that appears later on the PATH.
#[test]
fn shim_resolves_before_ambient_reportage() {
    use std::os::unix::fs::PermissionsExt;

    let harness = ShimHarness::new();

    // A decoy `reportage` placed on PATH after the shim dir stands in for an ambient
    // install; resolution must still pick the shim.
    let decoy_dir = TempDir::new().unwrap();
    let decoy = decoy_dir.path().join("reportage");
    std::fs::write(&decoy, "#!/bin/sh\nexit 97\n").unwrap();
    std::fs::set_permissions(&decoy, std::fs::Permissions::from_mode(0o755)).unwrap();

    let path = format!(
        "{}:{}:{}",
        harness.shim_dir().display(),
        decoy_dir.path().display(),
        std::env::var("PATH").unwrap_or_default(),
    );

    let output = std::process::Command::new("sh")
        .args(["-c", "command -v reportage"])
        .env("PATH", &path)
        .output()
        .unwrap();

    let resolved = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim().to_string());
    assert_eq!(
        resolved,
        harness.shim_path(),
        "reportage must resolve to the generated shim, not an ambient binary"
    );
}

/// Run the `e2e/` suite (discovered by the default `reportage.kdl`) through the shim and
/// assert the suite passes.
#[test]
fn e2e_suite_passes_through_reportage_shim() {
    let harness = ShimHarness::new();
    let output = harness.suite_command("self-test-e2e").output().unwrap();
    assert!(
        output.status.success(),
        "e2e suite failed\n{}",
        describe(&output)
    );
}

/// Run the `examples/` suite through the shim and assert it passes: the
/// documentation-oriented samples must stay a correct, runnable reference.
#[test]
fn examples_suite_passes_through_reportage_shim() {
    let harness = ShimHarness::new();
    let output = harness
        .suite_command("self-test-examples")
        .args(["--config", "reportage.examples.kdl"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "examples suite failed\n{}",
        describe(&output)
    );
}

/// Run the valid syntax fixtures through the shim and assert none produces a parse error.
///
/// This verifies syntax acceptance only: runtime/assertion outcomes of the valid fixtures
/// are deliberately out of scope (see issue #117), so the run's exit code is not asserted.
#[test]
fn valid_syntax_fixtures_have_no_parse_errors_through_reportage_shim() {
    let harness = ShimHarness::new();
    let output = harness
        .suite_command("self-test-fixtures-valid")
        .args([
            "--config",
            "reportage.fixtures.valid.kdl",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    let json = parse_json_stdout(&output);
    let parse_diags = parse_diagnostics(&json);
    assert!(
        parse_diags.is_empty(),
        "valid fixtures must not produce parse errors, got: {parse_diags:#?}"
    );
    assert!(
        !json["tests"].as_array().unwrap().is_empty(),
        "valid fixtures were expected to be discovered and parsed into cases\n{}",
        describe(&output)
    );
}

/// Run the invalid syntax fixtures through the shim and assert every fixture — and nothing
/// else — produces exactly one parse error, with the documented parse-error exit code 2.
#[test]
fn invalid_syntax_fixtures_all_produce_parse_errors_through_reportage_shim() {
    let harness = ShimHarness::new();
    let output = harness
        .suite_command("self-test-fixtures-invalid")
        .args([
            "--config",
            "reportage.fixtures.invalid.kdl",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(2),
        "parse errors must map to exit code 2\n{}",
        describe(&output)
    );

    let json = parse_json_stdout(&output);
    let parse_diags = parse_diagnostics(&json);

    let observed: BTreeSet<PathBuf> = parse_diags
        .iter()
        .map(|d| PathBuf::from(d["origin"]["source"].as_str().unwrap()))
        .collect();
    let fixtures_dir = PathBuf::from("tests/fixtures/syntax/invalid");
    let expected: BTreeSet<PathBuf> = std::fs::read_dir(workspace_root().join(&fixtures_dir))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .filter(|name| {
            PathBuf::from(name)
                .extension()
                .is_some_and(|e| e == "repor")
        })
        .map(|name| fixtures_dir.join(name))
        .collect();

    assert_eq!(
        observed, expected,
        "every invalid fixture must produce a parse error, and every parse error must come from an invalid fixture"
    );
    assert_eq!(
        parse_diags.len(),
        expected.len(),
        "expected exactly one parse error per invalid fixture"
    );
    assert!(
        json["tests"].as_array().unwrap().is_empty(),
        "pre-execution validation must block all execution on parse errors"
    );
}

/// Run a representative case whose action is an inner `$ reportage ...` invocation and
/// assert the `--format=json` report exposes the observed shim invocation metadata.
#[test]
fn json_output_exposes_observed_inner_shim_invocations() {
    let harness = ShimHarness::new();
    let output = harness
        .suite_command("self-test-json-shim-invocations")
        .args(["--format", "json", "e2e/options/help-show-usage.repor"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "representative case failed\n{}",
        describe(&output)
    );

    let json = parse_json_stdout(&output);
    let invocations: Vec<&serde_json::Value> = json["tests"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|test| test["actions"].as_array().unwrap())
        .flat_map(|action| {
            action["shimInvocations"]
                .as_array()
                .map(|v| v.as_slice())
                .unwrap_or_default()
        })
        .collect();

    let reportage_invocation = invocations
        .iter()
        .find(|inv| inv["commandName"] == "reportage")
        .unwrap_or_else(|| {
            panic!("no shim invocation for `reportage` was observed, got: {invocations:#?}")
        });
    assert_eq!(
        reportage_invocation["shimPath"],
        harness.shim_path().display().to_string(),
        "the inner invocation must have gone through the generated shim"
    );
    assert_eq!(
        reportage_invocation["target"]["program"],
        harness.reportage_bin().display().to_string(),
        "the shim must delegate to the cargo-built reportage binary"
    );
}
