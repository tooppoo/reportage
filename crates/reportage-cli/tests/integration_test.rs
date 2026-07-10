use std::path::PathBuf;

use assert_cmd::Command;
use assert_fs::TempDir;
use assert_fs::prelude::*;

fn reportage(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("reportage").unwrap();
    cmd.current_dir(dir);
    cmd
}

fn write_script(dir: &TempDir, name: &str, content: &str) -> std::path::PathBuf {
    let child = dir.child(name);
    child.write_str(content).unwrap();
    child.path().to_path_buf()
}

fn write_config(dir: &TempDir, content: &str) {
    dir.child("reportage.kdl").write_str(content).unwrap();
}

fn read_single_result_json(dir: &TempDir) -> (serde_json::Value, PathBuf) {
    let runs_dir = dir.child(".reportage").child("runs");
    let entries: Vec<_> = std::fs::read_dir(runs_dir.path())
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(entries.len(), 1, "expected exactly one run directory");

    let run_dir = entries[0].path();
    let content = std::fs::read_to_string(run_dir.join("result.json")).unwrap();
    (serde_json::from_str(&content).unwrap(), run_dir)
}

const PASSING_CASE: &str = r#"
case "pass" {
  $ true
  assert {
    exit 0
  }
}
"#;

const FAILING_CASE: &str = r#"
case "fail" {
  $ false
  assert {
    exit 0
  }
}
"#;

// --- bootstrap / structural: no-op run artifact shape ---
//
// Representative passing/failing-assertion CLI scenarios live in
// e2e/cases/passing-and-failing.repor (#109). The tests below verify `result.json`
// structure for a no-op run, which a `.repor` self-test cannot express directly.

#[test]
fn empty_script_is_noop_success() {
    let dir = TempDir::new().unwrap();
    let script = write_script(&dir, "empty.repor", "");

    reportage(&dir)
        .arg(script)
        .assert()
        .code(0)
        .stdout(predicates::str::contains("NO-OP"))
        .stdout(predicates::str::contains("no cases found"));

    let (json, run_dir) = read_single_result_json(&dir);
    assert_eq!(json["status"], "passed");
    assert_eq!(json["noop"], true);
    assert_eq!(json["summary"]["scripts"], 0);
    assert_eq!(json["summary"]["actions"], 0);
    assert_eq!(json["summary"]["assertions"], 0);
    assert_eq!(json["summary"]["passed"], 0);
    assert_eq!(json["summary"]["failed"], 0);
    assert_eq!(json["summary"]["errors"], 0);
    assert_eq!(json["tests"].as_array().unwrap().len(), 0);
    assert!(
        !run_dir.join("cases").exists(),
        "no-op run must not create case/checkpoint/evidence artifacts"
    );
}

#[test]
fn whitespace_only_script_is_noop_success() {
    let dir = TempDir::new().unwrap();
    let script = write_script(&dir, "whitespace.repor", " \n\t\n  \n");

    reportage(&dir)
        .arg(script)
        .assert()
        .code(0)
        .stdout(predicates::str::contains("NO-OP"));

    let (json, _run_dir) = read_single_result_json(&dir);
    assert_eq!(json["status"], "passed");
    assert_eq!(json["noop"], true);
    assert_eq!(json["summary"]["actions"], 0);
    assert_eq!(json["summary"]["assertions"], 0);
}

// --- failing assertions ---
//
// Representative passing/failing-assertion CLI scenarios live in
// e2e/cases/passing-and-failing.repor (#109).

// --- multiple expectations in one block ---
//
// The representative multiple-expectations-in-one-block scenario lives in
// e2e/cases/assertion-blocks.repor (#109).

// --- logical composition (#25) ---
//
// Representative `all`/`any`/`not` scenarios, including the `not { A B }` = `not(all(A, B))`
// distinction, live in e2e/composition/logical-composition.repor (#109). The exhaustive
// pass/fail combinations for `all`/`any`/`not` are unit-tested directly against the semantic
// evaluator in `crates/reportage-core/src/evaluator.rs` (`all_passes_when_every_child_passes`
// and its neighbors), independent of any CLI invocation. The tests below verify what the CLI
// externalizes for a logical composition result, which the unit tests do not cover.

#[test]
fn nested_logical_composition_is_evaluated_and_recorded_in_artifact() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "nested composition" {
  $ false
  assert {
    all {
      not {
        exit 0
      }
      any {
        exit 1
        exit 2
      }
    }
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(0);

    let (json, _) = read_single_result_json(&dir);
    let expectation = &json["tests"][0]["assertions"][0]["expectation"];
    assert_eq!(expectation["kind"], "logical");
    assert_eq!(expectation["operator"], "all");
    assert_eq!(expectation["status"], "passed");
    assert_eq!(expectation["children"][0]["operator"], "not");
    assert_eq!(expectation["children"][1]["operator"], "any");
}

#[test]
fn empty_logical_composition_block_exits_with_code_two() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "empty composition" {
  $ true
  assert {
    all {
    }
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(2);
}

// --- multiple assertion blocks ---
//
// The representative precondition/postcondition assertion-block scenario lives in
// e2e/cases/assertion-blocks.repor (#109).

// --- validation/spec errors ---

#[test]
fn missing_assertion_block_exits_with_code_two() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "missing assertion" {
  $ true
}
"#,
    );
    reportage(&dir).arg(script).assert().code(2);
}

#[test]
fn process_expectation_at_initial_checkpoint_exits_with_code_two() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "process expectation before action" {
  assert {
    exit 0
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(2);
}

#[test]
fn invalid_exit_code_value_exits_with_code_two() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "invalid exit" {
  $ true
  assert {
    exit 999
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(2);
}

#[test]
fn top_level_action_exits_with_code_two() {
    let dir = TempDir::new().unwrap();
    let script = write_script(&dir, "test.repor", "$ true\n");
    reportage(&dir).arg(script).assert().code(2);
}

#[test]
fn unsupported_expectation_type_exits_with_code_two() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "unsupported" {
  $ true
  assert {
    unknown_assertion
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(2);
}

#[test]
fn bare_assert_without_block_exits_with_code_two() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "bare assert" {
  $ true
  assert exit 0
}
"#,
    );
    reportage(&dir).arg(script).assert().code(2);
}

#[test]
fn empty_assert_block_exits_with_code_two() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "empty block" {
  $ true
  assert {
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(2);
}

// --- artifacts ---

#[test]
fn artifacts_directory_is_created_on_passing_run() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "pass" {
  $ true
  assert {
    exit 0
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(0);
    dir.child(".reportage").assert(predicates::path::is_dir());
}

#[test]
fn artifacts_directory_is_created_on_failing_run() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "fail" {
  $ false
  assert {
    exit 0
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(1);
    dir.child(".reportage").assert(predicates::path::is_dir());
}

#[test]
fn result_json_is_written() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "pass" {
  $ true
  assert {
    exit 0
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(0);

    let (json, run_dir) = read_single_result_json(&dir);
    assert!(
        run_dir.join("result.json").exists(),
        "result.json should exist"
    );
    assert_eq!(json["status"], "passed");
}

// --- source order execution ---

#[test]
fn assertion_block_failure_stops_subsequent_action() {
    // assert { exit 1 } fails because true exits 0.
    // Source order execution must not run the second action after the block failure.
    // This is verified by checking that only one action appears in result.json.
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "source order" {
  $ true
  assert {
    exit 1
  }
  $ false
  assert {
    exit 0
  }
}
"#,
    );
    reportage(&dir).arg(&script).assert().code(1);

    let runs_dir = dir.child(".reportage").child("runs");
    let entries: Vec<_> = std::fs::read_dir(runs_dir.path())
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let content = std::fs::read_to_string(entries[0].path().join("result.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();

    let actions = json["tests"][0]["actions"].as_array().unwrap();
    assert_eq!(
        actions.len(),
        1,
        "only the first action should have run; source order execution stops on assertion block failure"
    );
}

// --- output content ---
//
// Representative pass/fail stdout markers live in e2e/output/pass-fail-markers.repor (#109).

// --- config-driven mode ---
//
// Representative config-driven discovery scenarios, including the `--config` flag, live in
// e2e/discovery/config-driven.repor and e2e/discovery/explicit-config.repor.

#[test]
fn config_and_scripts_combined_is_rejected() {
    let dir = TempDir::new().unwrap();
    let script = write_script(&dir, "test.repor", PASSING_CASE);
    write_config(
        &dir,
        r#"
reportage {
  config {
    version 1
  }
  tests {
    path "test.repor"
  }
}
"#,
    );
    reportage(&dir)
        .arg("--config")
        .arg("reportage.kdl")
        .arg(script)
        .assert()
        .code(3);
}

#[test]
fn config_pattern_matching_no_files_exits_two() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        r#"
reportage {
  config {
    version 1
  }
  tests {
    path "no_match/**/*.repor"
  }
}
"#,
    );
    reportage(&dir).assert().code(2);
}

#[test]
fn config_with_dot_segment_path_exits_nonzero() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        r#"
reportage {
  config {
    version 1
  }
  tests {
    path "./test.repor"
  }
}
"#,
    );
    // Config validation error → exit 3
    reportage(&dir).assert().code(3);
}

// The source path attribution this test verified (discovered file names appearing in stdout)
// is covered by e2e/discovery/config-driven.repor and e2e/discovery/aggregate-failure.repor,
// which both assert `stdout contains` the discovered file names.

#[test]
fn pre_execution_validation_blocks_all_execution_on_parse_error() {
    let dir = TempDir::new().unwrap();
    // valid.repor would pass, but broken.repor has a parse error.
    // Neither should have its $-actions executed.
    write_script(&dir, "valid.repor", PASSING_CASE);
    write_script(&dir, "broken.repor", "this is not valid syntax\n");
    write_config(
        &dir,
        r#"
reportage {
  config {
    version 1
  }
  tests {
    path "*.repor"
  }
}
"#,
    );
    // Parse error → exit 2; no cases should have run
    reportage(&dir).assert().code(2);
}

// The aggregate-failure scenario (overall exit 1 when one of several discovered files'
// cases fails) is covered by e2e/discovery/aggregate-failure.repor. The representative
// explicit-multiple-scripts scenario lives in e2e/discovery/multiple-scripts.repor (#109).

#[test]
fn file_read_error_exits_two_with_no_execution() {
    let dir = TempDir::new().unwrap();
    write_config(
        &dir,
        r#"
reportage {
  config {
    version 1
  }
  tests {
    path "*.repor"
  }
}
"#,
    );
    // Write a file that matches but is a directory, not a regular file.
    // Actually, let's create a file and then remove it so glob matched it... easier: point to a non-existent file via explicit script mode.
    let nonexistent = dir.path().join("nonexistent.repor");
    reportage(&dir).arg(&nonexistent).assert().code(2);
}

// --- shim invocation event integration ---

/// Resolve a standard binary by name using `which`.
#[cfg(unix)]
fn which_bin(name: &str) -> PathBuf {
    let output = std::process::Command::new("which")
        .arg(name)
        .output()
        .unwrap();
    PathBuf::from(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Return the effective PATH with `extra_dir` prepended.
#[cfg(unix)]
fn path_with_prefix(extra_dir: &std::path::Path) -> String {
    let original = std::env::var("PATH").unwrap_or_default();
    format!("{}:{original}", extra_dir.display())
}

/// Read and parse result.json from the single run directory inside `dir`.
#[cfg(unix)]
fn read_result_json(dir: &TempDir) -> serde_json::Value {
    let runs_dir = dir.child(".reportage").child("runs");
    let entries: Vec<_> = std::fs::read_dir(runs_dir.path())
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(entries.len(), 1, "expected exactly one run directory");
    let content = std::fs::read_to_string(entries[0].path().join("result.json")).unwrap();
    serde_json::from_str(&content).unwrap()
}

/// A runner-generated shim that fails to write its event file emits a prefixed stderr diagnostic.
/// That diagnostic is observable stderr: it is not filtered from `assert { stderr empty }`.
#[test]
#[cfg(unix)]
fn shim_stderr_warning_is_not_filtered_from_stderr_empty_assertion() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new().unwrap();

    // Create a hand-crafted shim that unconditionally writes a prefixed warning to stderr and then delegates to `true`.
    // This mimics the behavior of a real shim that cannot write its event file.
    let shim_dir = dir.child("shims");
    shim_dir.create_dir_all().unwrap();
    let true_path = which_bin("true");
    let shim_path = shim_dir.path().join("reportage-test-warn-shim");
    std::fs::write(
        &shim_path,
        format!(
            "#!/bin/sh\nprintf 'reportage shim warning: failed to write shim invocation event: /fake/path\\n' >&2\nexec '{}' \"$@\"\n",
            true_path.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&shim_path, std::fs::Permissions::from_mode(0o755)).unwrap();

    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "shim stderr not filtered" {
  $ reportage-test-warn-shim
  assert {
    stderr empty
  }
}
"#,
    );

    // The assertion must fail: the shim's stderr warning is observable stderr and is not automatically filtered from `assert { stderr empty }`.
    reportage(&dir)
        .arg(script)
        .env("PATH", path_with_prefix(shim_dir.path()))
        .assert()
        .code(1);
}

/// When a reportage-generated shim is invoked during an action, the result artifact (result.json) records the observed shim invocation metadata.
#[test]
#[cfg(unix)]
fn result_json_contains_shim_invocations_when_shim_is_used() {
    use reportage_core::shim::{CommandName, CommandShim, ExecutableInvocation};

    let dir = TempDir::new().unwrap();
    let shim_dir = dir.child("shims");
    shim_dir.create_dir_all().unwrap();

    let true_path = which_bin("true");
    let name = CommandName::new("reportage-test-artifact-shim").unwrap();
    let invocation = ExecutableInvocation::new(true_path, vec![]).unwrap();
    let shim = CommandShim::new(name, invocation);
    shim.materialize(shim_dir.path()).unwrap();

    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "shim artifact" {
  $ reportage-test-artifact-shim
  assert {
    exit 0
  }
}
"#,
    );

    reportage(&dir)
        .arg(script)
        .env("PATH", path_with_prefix(shim_dir.path()))
        .assert()
        .code(0);

    let json = read_result_json(&dir);
    let invocations = &json["tests"][0]["actions"][0]["shimInvocations"];
    assert!(
        invocations.is_array(),
        "shimInvocations must be an array in result.json"
    );
    let invocations = invocations.as_array().unwrap();
    assert_eq!(invocations.len(), 1, "one shim invocation expected");
    assert_eq!(
        invocations[0]["commandName"], "reportage-test-artifact-shim",
        "commandName in artifact must match the shim name"
    );
    assert!(
        invocations[0]["shimPath"]
            .as_str()
            .unwrap()
            .contains("reportage-test-artifact-shim"),
        "shimPath must reference the shim file"
    );
    assert_eq!(invocations[0]["forwardsCallerArgs"], true);
}

/// When a case fails and the action was resolved through a reportage-generated shim, the CLI diagnostics include the observed shim path and target invocation.
#[test]
#[cfg(unix)]
fn failing_case_with_shim_shows_shim_metadata_in_cli_output() {
    use reportage_core::shim::{CommandName, CommandShim, ExecutableInvocation};

    let dir = TempDir::new().unwrap();
    let shim_dir = dir.child("shims");
    shim_dir.create_dir_all().unwrap();

    let true_path = which_bin("true");
    let name = CommandName::new("reportage-test-diag-shim").unwrap();
    let invocation = ExecutableInvocation::new(true_path, vec![]).unwrap();
    let shim = CommandShim::new(name, invocation);
    shim.materialize(shim_dir.path()).unwrap();

    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "shim diagnostics" {
  $ reportage-test-diag-shim
  assert {
    exit 1
  }
}
"#,
    );

    // The assertion fails (true exits 0, not 1).
    // The CLI must include shim metadata in its diagnostics.
    reportage(&dir)
        .arg(script)
        .env("PATH", path_with_prefix(shim_dir.path()))
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "shim invoked for 'reportage-test-diag-shim'",
        ));
}

// --- file assertions (#24) ---
//
// Representative pass/fail scenarios for `file exists` and `file contains` live in
// e2e/assertions/file-exists.repor and e2e/assertions/file-contains.repor. The tests below
// verify filesystem boundary conditions and stable diagnostic codes.

#[test]
fn file_exists_fails_for_a_directory() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "directory is not a file" {
  $ mkdir -p a-directory
  assert {
    file <"a-directory"> exists
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "it is not a regular file (e.g. a directory)",
        ));
}

#[test]
fn file_exists_follows_symlink_to_regular_file() {
    #[cfg(unix)]
    {
        let dir = TempDir::new().unwrap();
        let script = write_script(
            &dir,
            "test.repor",
            r#"
case "symlink to file" {
  write <"real.txt"> ```
    hi
    ```
  $ ln -s real.txt link.txt
  assert {
    file <"link.txt"> exists
  }
}
"#,
        );
        reportage(&dir).arg(script).assert().code(0);
    }
}

#[test]
fn file_contains_fails_for_directory() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file contains directory" {
  $ mkdir -p a-directory
  assert {
    file <"a-directory"> contains "anything"
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "it is not a regular file (e.g. a directory)",
        ));
}

#[test]
#[cfg(unix)]
fn file_contains_fails_for_non_utf8_content() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file contains non-utf8" {
  $ printf '\377\376\000\377' > binary.dat
  assert {
    file <"binary.dat"> contains "anything"
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains("its content is not valid UTF-8"));
}

// The combined-evidence pattern (a process expectation alongside `file exists` and
// `file contains` in one assertion block) is covered by
// e2e/artifacts/file-assertion-evidence.repor.

#[test]
fn absolute_file_assertion_path_is_a_script_error() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "absolute path rejected" {
  $ true
  assert {
    file <"/etc/passwd"> exists
  }
}
"#,
    );
    // The offending script's own path must be identifiable in the output, not just the diagnostic code, so a semantic error can be traced back to its source file.
    reportage(&dir)
        .arg(script)
        .assert()
        .code(2)
        .stdout(predicates::str::contains("test.repor"))
        .stderr(predicates::str::contains("semantic.file_path.absolute"));
}

#[test]
fn dot_segment_file_assertion_path_is_a_script_error() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "dot segment rejected" {
  $ true
  assert {
    file <"../secret.txt"> exists
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(2)
        .stdout(predicates::str::contains("test.repor"))
        .stderr(predicates::str::contains("semantic.file_path.dot_segment"));
}

#[test]
fn file_assertion_path_resolves_against_workspace_root_not_action_cd() {
    // A `cd` performed inside a `$` action must not change how the following file assertion's path is resolved.
    // See docs/semantics.md.
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "cd does not affect file assertion root" {
  $ mkdir -p subdir && cd subdir && echo hi > moved.txt
  assert {
    file <"subdir/moved.txt"> exists
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(0);
}

// --- `contents_equals` assertions (#87) ---
//
// The representative workspace-file pass scenario lives in
// e2e/assertions/contents-equals.repor ("file contents_equals passes against a workspace
// expected file").

#[test]
fn file_contents_equals_fails_on_byte_mismatch_against_workspace_expected_file() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file contents_equals workspace mismatch" {
  $ printf hello > expected.txt
  $ printf world > actual.txt
  assert {
    file <"actual.txt"> contents_equals <"expected.txt">
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "assertion.file.contents_equals.mismatch",
        ));
}

#[test]
fn file_contents_equals_passes_against_a_fixture_expected_file() {
    let dir = TempDir::new().unwrap();
    dir.child("expected.txt").write_str("hello").unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file contents_equals fixture pass" {
  $ printf hello > actual.txt
  assert {
    file <"actual.txt"> contents_equals @"expected.txt"
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(0);
}

#[test]
fn file_contents_equals_passes_against_a_fixture_expected_file_via_bare_relative_script_path() {
    // Regression test: `Path::parent()` returns `Some("")`, not `None`, for a bare relative
    // filename with no directory component (the common `cd examples && reportage foo.repor`
    // invocation shape). An earlier version of `evaluate_case`'s `repor_dir` computation only
    // substituted "." when `parent()` returned `None`, so this shape resolved `repor_dir` to an
    // empty path, and `fixture::resolve_fixture_source` failed to canonicalize it even though
    // the fixture file existed right next to the script.
    let dir = TempDir::new().unwrap();
    dir.child("expected.txt").write_str("hello").unwrap();
    write_script(
        &dir,
        "test.repor",
        r#"
case "file contents_equals fixture pass" {
  $ printf hello > actual.txt
  assert {
    file <"actual.txt"> contents_equals @"expected.txt"
  }
}
"#,
    );
    // Pass a bare filename, not the absolute path `write_script` returns, so `source_path` has
    // no directory component when the CLI resolves it.
    reportage(&dir).arg("test.repor").assert().code(0);
}

#[test]
fn file_contents_equals_fails_on_byte_mismatch_against_fixture_expected_file() {
    let dir = TempDir::new().unwrap();
    dir.child("expected.txt").write_str("hello").unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file contents_equals fixture mismatch" {
  $ printf world > actual.txt
  assert {
    file <"actual.txt"> contents_equals @"expected.txt"
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(1);
}

#[test]
fn file_contents_equals_missing_actual_is_assertion_failure() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file contents_equals missing actual" {
  $ printf hello > expected.txt
  assert {
    file <"does-not-exist.txt"> contents_equals <"expected.txt">
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "assertion.file.contents_equals.actual_missing",
        ));
}

// The missing-expected-workspace-path script error is covered by e2e/assertions/contents-equals.repor ("file contents_equals reports a script error for a missing expected workspace path"), which checks the same `semantic.file_contents_reference.missing` diagnostic code.

#[test]
fn file_contents_equals_missing_fixture_is_a_script_error() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file contents_equals missing fixture" {
  $ printf hello > actual.txt
  assert {
    file <"actual.txt"> contents_equals @"does-not-exist.txt"
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(2)
        .stderr(predicates::str::contains(
            "semantic.fixture_reference.missing",
        ));
}

// The stdout-vs-fixture pass scenario is covered by e2e/assertions/contents-equals.repor
// ("stdout contents_equals passes against a fixture reference").

#[test]
fn stderr_contents_equals_fails_on_mismatch_against_workspace_expected_file() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "stderr contents_equals workspace mismatch" {
  $ printf oops > expected.txt
  $ printf nope 1>&2
  assert {
    stderr contents_equals <"expected.txt">
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "assertion.stderr.contents_equals.mismatch",
        ));
}

#[test]
fn stdout_contents_equals_fails_on_mismatch_against_workspace_expected_file() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "stdout contents_equals workspace mismatch" {
  $ printf hello > expected.txt
  $ printf world
  assert {
    stdout contents_equals <"expected.txt">
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "assertion.stdout.contents_equals.mismatch",
        ));
}

#[test]
fn file_contents_equals_actual_directory_is_assertion_failure() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file contents_equals actual is a directory" {
  $ mkdir -p a-dir
  $ printf hello > expected.txt
  assert {
    file <"a-dir"> contents_equals <"expected.txt">
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "assertion.file.contents_equals.actual_not_regular_file",
        ));
}

#[test]
fn not_block_wrapping_a_passing_file_contents_equals_prints_bytes_match_detail() {
    // A `not` composition recurses into every child regardless of the child's own pass/fail
    // state (see render::human::print_failed_expectation's doc comment), so a `contents_equals`
    // child that itself matched still has its "bytes match" detail printed when the
    // surrounding `not` fails because that child held.
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "not wrapping a passing contents_equals" {
  $ printf hello > expected.txt
  $ printf hello > actual.txt
  assert {
    not {
      file <"actual.txt"> contents_equals <"expected.txt">
    }
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains("bytes match"));
}

// --- `text_equals` assertions (#88) ---
//
// Representative pass scenarios (quoted-string and heredoc literals) and the quoted-string
// mismatch scenario live in e2e/assertions/text-equals.repor.

#[test]
fn file_text_equals_fails_on_heredoc_byte_mismatch() {
    // Mirrors `file_text_equals_fails_on_byte_mismatch`, but with a heredoc expected value: a
    // failing heredoc-form text_equals must report the same diagnostic code as the quoted-string
    // form, and its human-rendered subject description must use the heredoc literal label
    // instead of the compact quoted-literal rendering (see `format_text_equals_source`).
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file text_equals heredoc mismatch" {
  $ printf 'hello\nworld\n' > actual.txt
  assert {
    file <"actual.txt"> text_equals ```
    hello
    WORLD
    ```
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "assertion.file.text_equals.mismatch",
        ))
        .stderr(predicates::str::contains(
            "text_equals <heredoc literal> — bytes differ",
        ));
}

// The missing-actual-file assertion failure is covered by e2e/assertions/text-equals.repor
// ("file text_equals reports an assertion failure for a missing actual file"), which checks
// the same `assertion.file.text_equals.actual_missing` diagnostic code.

#[test]
fn file_text_equals_actual_directory_is_assertion_failure() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file text_equals actual is a directory" {
  $ mkdir -p a-dir
  assert {
    file <"a-dir"> text_equals "hello"
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "assertion.file.text_equals.actual_not_regular_file",
        ));
}

// Both `text_equals` kind-mismatch script errors (rejecting a fixture reference and rejecting
// a workspace path literal as the expected value) are covered by e2e/assertions/text-equals.repor,
// which checks the same `semantic.literal.kind_mismatch` diagnostic code for each case.

#[test]
fn not_block_wrapping_a_passing_file_text_equals_prints_bytes_match_detail() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "not wrapping a passing text_equals" {
  $ printf hello > actual.txt
  assert {
    not {
      file <"actual.txt"> text_equals "hello"
    }
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains("bytes match"));
}

// --- stdout / stderr `text_equals` assertions ---
//
// Representative pass scenarios (quoted-string and heredoc literals), the quoted-string
// mismatch scenarios, and both kind-mismatch script errors live in
// e2e/assertions/text-equals.repor.

#[test]
fn stdout_text_equals_fails_on_heredoc_byte_mismatch() {
    // Mirrors `file_text_equals_fails_on_heredoc_byte_mismatch` for a captured stream: a failing
    // heredoc-form stdout text_equals must report its own stream-scoped diagnostic code, and its
    // human-rendered subject description must use the `text_equals` operator keyword and the
    // heredoc literal label (see `format_text_equals_source` / `print_contents_equals_detail`).
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "stdout text_equals heredoc mismatch" {
  $ printf 'hello\nworld\n'
  assert {
    stdout text_equals ```
    hello
    WORLD
    ```
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "assertion.stdout.text_equals.mismatch",
        ))
        .stderr(predicates::str::contains(
            "stdout text_equals <heredoc literal> — bytes differ",
        ));
}

#[test]
fn stderr_text_equals_mismatch_reports_stream_scoped_code_and_quoted_source() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "stderr text_equals quoted mismatch" {
  $ sh -c 'printf "warn\n" >&2'
  assert {
    stderr text_equals "other\n"
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "assertion.stderr.text_equals.mismatch",
        ))
        .stderr(predicates::str::contains(
            "stderr text_equals \"other\\n\" — bytes differ",
        ));
}

#[test]
fn not_block_wrapping_a_passing_stdout_text_equals_prints_bytes_match_detail() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "not wrapping a passing stdout text_equals" {
  $ printf hello
  assert {
    not {
      stdout text_equals "hello"
    }
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "stdout text_equals \"hello\" — bytes match",
        ));
}

// --- dir assertions (#66) ---
//
// Representative pass/fail scenarios for `dir exists` and `dir contains` live in
// e2e/artifacts/dir-assertion-evidence.repor. The tests below verify diagnostic codes not
// covered there (missing path, not-a-directory, broken symlink), or additional source-path
// attribution alongside a diagnostic code (absolute/dot-segment rejection) that the self-test
// already checks without the source-path assertion.

#[test]
fn dir_exists_fails_against_a_regular_file() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "dir exists against a file" {
  $ touch marker
  assert {
    dir <"marker"> exists
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "it is not a directory (e.g. a regular file)",
        ))
        .stderr(predicates::str::contains(
            "assertion.dir.exists.not_directory",
        ));
}

#[test]
fn dir_exists_fails_for_a_missing_path() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "dir exists missing" {
  $ true
  assert {
    dir <"nope"> exists
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains("assertion.dir.exists.missing"));
}

#[test]
fn absolute_dir_assertion_path_is_a_script_error() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "absolute dir path rejected" {
  $ true
  assert {
    dir <"/etc"> exists
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(2)
        .stdout(predicates::str::contains("test.repor"))
        .stderr(predicates::str::contains(
            "semantic.workspace_path.absolute",
        ));
}

#[test]
fn dot_segment_dir_assertion_path_is_a_script_error() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "dot segment dir path rejected" {
  $ true
  assert {
    dir <"../secret"> exists
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(2)
        .stdout(predicates::str::contains("test.repor"))
        .stderr(predicates::str::contains(
            "semantic.workspace_path.dot_segment",
        ));
}

#[test]
fn dir_assertion_path_resolves_against_workspace_root_not_action_cd() {
    // A `cd` performed inside a `$` action must not change how the following dir assertion's path is resolved.
    // See docs/semantics.md.
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "cd does not affect dir assertion root" {
  $ mkdir -p subdir && cd subdir && mkdir moved
  assert {
    dir <"subdir/moved"> exists
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(0);
}

#[test]
#[cfg(unix)]
fn dir_exists_fails_for_a_broken_symlink() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "broken symlink" {
  $ ln -s does-not-exist link
  assert {
    dir <"link"> exists
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains("assertion.dir.exists.missing"));
}

#[test]
fn dir_assertion_nested_in_not_block_with_invalid_path_is_still_a_script_error() {
    // A `not { ... }` (or `all`/`any`) block combines assertion *outcomes*; it must not let an
    // invalid subject path bypass semantic validation and reach the real filesystem just because
    // it is nested. Regression test: this previously reported an ordinary assertion pass/fail
    // (having actually stat'd the escaped path) instead of a script error.
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "nested invalid dir path is still rejected" {
  $ true
  assert {
    not {
      dir <"../escape"> exists
    }
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(2)
        .stderr(predicates::str::contains(
            "semantic.workspace_path.dot_segment",
        ));
}

#[test]
fn file_assertion_nested_in_not_block_with_invalid_path_is_still_a_script_error() {
    // Same regression as above, for the `file` subject.
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "nested invalid file path is still rejected" {
  $ true
  assert {
    not {
      file <"/etc/passwd"> exists
    }
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(2)
        .stderr(predicates::str::contains("semantic.file_path.absolute"));
}

// --- write step (#67) ---

#[test]
fn write_step_creates_file_seen_by_subsequent_file_assertion() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "write then assert" {
  write <"config.yml"> ```
    key: value
    ```
  assert {
    file <"config.yml"> contains "key: value"
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(0);
}

#[test]
fn write_step_creates_parent_directories_automatically() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "write into nested directory" {
  write <"expected/nested/stdout.txt"> ```
    ok
    ```
  assert {
    file <"expected/nested/stdout.txt"> exists
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(0);
}

#[test]
fn write_step_target_already_exists_is_a_runtime_step_error() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "write twice to same path" {
  write <"a.txt"> ```
    first
    ```
  write <"a.txt"> ```
    second
    ```
  assert {
    exit 0
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(3)
        .stderr(predicates::str::contains("step.write.target_exists"));
}

#[test]
fn write_step_parent_path_has_regular_file_is_a_runtime_step_error() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "parent is a regular file" {
  write <"blocker"> ```
    i am a file
    ```
  write <"blocker/child.txt"> ```
    unreachable
    ```
  assert {
    exit 0
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(3)
        .stderr(predicates::str::contains(
            "step.write.parent_not_a_directory",
        ));
}

#[test]
#[cfg(unix)]
fn write_step_rejects_symlink_parent_instead_of_escaping_the_workspace() {
    // A `$` action plants a symlink to a directory *outside* the workspace
    // before a later `write` step targets a path through it. The write must
    // be rejected as a runtime step error, and nothing must actually be
    // written outside the isolated workspace through the symlink.
    let dir = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        &format!(
            r#"
case "escape via symlink parent" {{
  $ ln -s {outside} escape
  write <"escape/leaked.txt"> ```
    leaked
    ```
  assert {{
    exit 0
  }}
}}
"#,
            outside = outside.path().display(),
        ),
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(3)
        .stderr(predicates::str::contains(
            "step.write.parent_not_a_directory",
        ));

    outside
        .child("leaked.txt")
        .assert(predicates::path::missing());
}

#[test]
fn write_step_failure_stops_subsequent_steps_in_the_same_case() {
    // The second write step fails (create-only, target already exists). The
    // case must stop there: the trailing `$` action's exit code, which
    // would otherwise satisfy `assert { exit 1 }`, must never be reached.
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "write error stops the case" {
  write <"a.txt"> ```
    first
    ```
  write <"a.txt"> ```
    second
    ```
  $ false
  assert {
    exit 1
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(3);
}

#[test]
fn write_step_absolute_path_is_a_script_error() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "write step absolute path" {
  write <"/etc/passwd"> ```
    x
    ```
  assert {
    exit 0
  }
}
"#,
    );
    // A `write` step's path is validated at parse time via `WorkspacePath::parse`
    // (a `ParseError`), unlike the checkpoint-time `file <"path"> ...` path
    // policy. Both now render their stable diagnostic code inline in CLI output.
    reportage(&dir)
        .arg(script)
        .assert()
        .code(2)
        .stderr(predicates::str::contains("`write` step path"))
        .stderr(predicates::str::contains(
            "semantic.workspace_path.absolute",
        ));
}

#[test]
fn concrete_cases_have_isolated_workspaces_and_do_not_collide_on_the_same_write_path() {
    // Two cases in the same script both `write` the same relative path.
    // If workspaces were shared across cases (rather than isolated per
    // concrete case), the second case's create-only write would fail
    // because the first case already created that path.
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "first case writes a.txt" {
  write <"a.txt"> ```
    from first case
    ```
  assert {
    file <"a.txt"> contains "from first case"
  }
}

case "second case writes a.txt" {
  write <"a.txt"> ```
    from second case
    ```
  assert {
    file <"a.txt"> contains "from second case"
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(0);
}

// --- --debug-run-id (#24) ---

#[test]
fn debug_run_id_writes_to_named_run_directory() {
    let dir = TempDir::new().unwrap();
    let script = write_script(&dir, "test.repor", PASSING_CASE);
    reportage(&dir)
        .args(["--debug-run-id", "fixed-id"])
        .arg(script)
        .assert()
        .code(0);

    dir.child(".reportage/runs/fixed-id/result.json")
        .assert(predicates::path::is_file());
}

#[test]
fn debug_run_id_does_not_silently_overwrite_existing_run_directory() {
    let dir = TempDir::new().unwrap();
    let script = write_script(&dir, "test.repor", PASSING_CASE);
    reportage(&dir)
        .args(["--debug-run-id", "fixed-id"])
        .arg(&script)
        .assert()
        .code(0);

    // Same run id again: must fail rather than silently overwrite.
    reportage(&dir)
        .args(["--debug-run-id", "fixed-id"])
        .arg(&script)
        .assert()
        .code(3)
        .stderr(predicates::str::contains("already exists"));
}

#[test]
fn debug_run_id_rejects_unsafe_values() {
    let dir = TempDir::new().unwrap();
    let script = write_script(&dir, "test.repor", PASSING_CASE);
    reportage(&dir)
        .args(["--debug-run-id", "../escape"])
        .arg(script)
        .assert()
        .code(3)
        .stderr(predicates::str::contains("invalid --debug-run-id"));
}

#[test]
fn debug_run_id_is_hidden_from_help() {
    use predicates::prelude::PredicateBooleanExt;

    reportage(&TempDir::new().unwrap())
        .arg("--help")
        .assert()
        .code(0)
        .stdout(predicates::str::contains("--debug-run-id").not());
}

// --- --format=json (#75) ---

/// Runs `reportage --format json` and parses CLI stdout as a single JSON document.
/// `serde_json::from_str` only succeeds when the entire input (aside from surrounding
/// whitespace) is one value, so this doubles as the "single valid JSON document" check.
fn run_json(dir: &TempDir, script: &std::path::Path) -> (serde_json::Value, i32) {
    let output = reportage(dir)
        .arg("--format")
        .arg("json")
        .arg(script)
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout was not a single valid JSON document: {e}\n{stdout}"));
    (json, output.status.code().unwrap())
}

#[test]
fn json_format_passed_case_has_passed_status_and_zero_exit_code() {
    let dir = TempDir::new().unwrap();
    let script = write_script(&dir, "test.repor", PASSING_CASE);

    let (json, actual_exit_code) = run_json(&dir, &script);

    assert_eq!(json["status"], "passed");
    assert_eq!(json["processExitCode"], 0);
    assert_eq!(actual_exit_code, 0);
    assert_eq!(json["processExitCode"], actual_exit_code);
}

#[test]
fn json_format_assertion_failure_has_failed_status_and_matching_exit_code() {
    let dir = TempDir::new().unwrap();
    let script = write_script(&dir, "test.repor", FAILING_CASE);

    let (json, actual_exit_code) = run_json(&dir, &script);

    assert_eq!(json["status"], "failed");
    assert_eq!(actual_exit_code, 1);
    assert_eq!(json["processExitCode"], actual_exit_code);
    assert_eq!(json["diagnostics"][0]["category"], "assertion");
    assert_eq!(json["diagnostics"][0]["severity"], "failure");
}

#[test]
fn json_format_parse_error_has_error_status_and_matching_exit_code() {
    let dir = TempDir::new().unwrap();
    let script = write_script(&dir, "broken.repor", "this is not valid syntax\n");

    let (json, actual_exit_code) = run_json(&dir, &script);

    assert_eq!(json["status"], "error");
    assert_eq!(actual_exit_code, 2);
    assert_eq!(json["processExitCode"], actual_exit_code);
    assert_eq!(json["diagnostics"][0]["category"], "parse");
    assert_eq!(json["tests"].as_array().unwrap().len(), 0);
}

#[test]
fn json_format_runtime_error_has_error_status_and_matching_exit_code() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "write twice to same path" {
  write <"a.txt"> ```
    first
    ```
  write <"a.txt"> ```
    second
    ```
  assert {
    exit 0
  }
}
"#,
    );

    let (json, actual_exit_code) = run_json(&dir, &script);

    assert_eq!(json["status"], "error");
    assert_eq!(actual_exit_code, 3);
    assert_eq!(json["processExitCode"], actual_exit_code);
    assert_eq!(json["diagnostics"][0]["category"], "runtime");
    assert_eq!(json["diagnostics"][0]["code"], "step.write.target_exists");
}

/// `--format=json`'s CLI stdout must contain only the JSON document: no human-readable
/// `PASS`/`FAIL` labels or diagnostic lines, which the default (human) renderer prints instead.
#[test]
fn json_format_stdout_has_no_human_readable_output_mixed_in() {
    use predicates::prelude::PredicateBooleanExt;

    let dir = TempDir::new().unwrap();
    let script = write_script(&dir, "test.repor", FAILING_CASE);

    reportage(&dir)
        .arg("--format")
        .arg("json")
        .arg(script)
        .assert()
        .code(1)
        .stdout(predicates::str::contains("PASS").not())
        .stdout(predicates::str::contains("FAIL").not());
}

/// Captured stdout is referenced via `artifactRef` (relative to `artifactRoot`), never
/// inlined; the referenced file must actually exist and contain the captured bytes.
#[test]
fn json_format_artifact_ref_resolves_to_a_real_file_with_captured_bytes() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "greets" {
  $ echo hello
  assert {
    exit 0
    stdout contains "hello"
  }
}
"#,
    );

    let (json, _) = run_json(&dir, &script);

    let artifact_root = json["artifactRoot"].as_str().unwrap();
    let artifact_ref = json["tests"][0]["actions"][0]["stdout"]["artifactRef"]
        .as_str()
        .unwrap();
    assert_eq!(artifact_ref, "test-1/action-1/stdout.bin");

    let resolved = dir.path().join(artifact_root).join(artifact_ref);
    let content = std::fs::read_to_string(&resolved)
        .unwrap_or_else(|e| panic!("artifactRef did not resolve to a real file: {e}"));
    assert_eq!(content, "hello\n");

    // Never inlined alongside the reference.
    assert!(
        json["tests"][0]["actions"][0]["stdout"]
            .get("data")
            .is_none()
    );
}

// --- config-driven commands (#119) ---

#[cfg(unix)]
fn write_executable(dir: &TempDir, name: &str, script_body: &str) -> PathBuf {
    let child = dir.child(name);
    child
        .write_str(&format!("#!/bin/sh\n{script_body}\n"))
        .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(child.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    child.path().to_path_buf()
}

#[test]
#[cfg(unix)]
fn config_registered_command_is_callable_by_name_from_repor() {
    let dir = TempDir::new().unwrap();
    write_executable(&dir, "real-tool", "echo real-output");
    write_script(
        &dir,
        "test.repor",
        r#"
case "calls registered command" {
  $ mytool
  assert {
    exit 0
    stdout contains "real-output"
  }
}
"#,
    );
    write_config(
        &dir,
        r#"
reportage {
  config {
    version 1
  }
  commands {
    command "mytool" {
      exec "real-tool"
    }
  }
  tests {
    path "test.repor"
  }
}
"#,
    );
    reportage(&dir).assert().code(0);
}

#[test]
#[cfg(unix)]
fn config_registered_command_shim_takes_priority_over_ambient_path() {
    let dir = TempDir::new().unwrap();
    write_executable(&dir, "real-tool", "echo real-output");

    // Ambient PATH decoy: a same-named executable that must NOT win.
    let decoy_dir = TempDir::new().unwrap();
    write_executable(&decoy_dir, "mytool", "echo decoy-output");

    write_script(
        &dir,
        "test.repor",
        r#"
case "shim wins over ambient path" {
  $ mytool
  assert {
    exit 0
    stdout contains "real-output"
  }
}
"#,
    );
    write_config(
        &dir,
        r#"
reportage {
  config {
    version 1
  }
  commands {
    command "mytool" {
      exec "real-tool"
    }
  }
  tests {
    path "test.repor"
  }
}
"#,
    );

    let original_path = std::env::var("PATH").unwrap_or_default();
    let path_with_decoy = format!("{}:{}", decoy_dir.path().display(), original_path);
    reportage(&dir)
        .env("PATH", path_with_decoy)
        .assert()
        .code(0);
}

#[test]
#[cfg(unix)]
fn config_registered_command_shim_is_materialized_in_case_local_bin_dir() {
    let dir = TempDir::new().unwrap();
    let real_tool = write_executable(&dir, "real-tool", "echo real-output");
    write_script(
        &dir,
        "test.repor",
        r#"
case "shim invocation is observable" {
  $ mytool
  assert {
    exit 0
  }
}
"#,
    );
    write_config(
        &dir,
        r#"
reportage {
  config {
    version 1
  }
  commands {
    command "mytool" {
      exec "real-tool"
    }
  }
  tests {
    path "test.repor"
  }
}
"#,
    );

    // Config-driven mode (no explicit script argument), so registered commands apply.
    let output = reportage(&dir)
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    let shim_invocations = &json["tests"][0]["actions"][0]["shimInvocations"];
    assert_eq!(shim_invocations[0]["commandName"], "mytool");
    let shim_path = shim_invocations[0]["shimPath"].as_str().unwrap();
    assert!(
        shim_path.ends_with("/bin/mytool"),
        "shim path {shim_path} must live in a case-local 'bin' directory"
    );
    // The case-local shim directory must not be the config-driven run's own directory: each
    // concrete case gets its own isolated workspace `bin` directory.
    assert!(!shim_path.starts_with(dir.path().to_str().unwrap()));

    let target_program = shim_invocations[0]["target"]["program"].as_str().unwrap();
    assert_eq!(
        std::fs::canonicalize(target_program).unwrap(),
        std::fs::canonicalize(&real_tool).unwrap()
    );
}

#[test]
#[cfg(unix)]
fn explicit_script_mode_does_not_register_config_commands() {
    let dir = TempDir::new().unwrap();
    write_executable(&dir, "real-tool", "echo real-output");
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "no config commands in explicit mode" {
  $ mytool
  assert {
    exit 0
  }
}
"#,
    );
    // A config file that registers "mytool" exists in the working directory, but explicit
    // script mode must never read it.
    write_config(
        &dir,
        r#"
reportage {
  config {
    version 1
  }
  commands {
    command "mytool" {
      exec "real-tool"
    }
  }
  tests {
    path "test.repor"
  }
}
"#,
    );

    // Explicit script argument selects explicit script mode: no config commands, so `mytool`
    // is not found on the ambient PATH and the case fails.
    reportage(&dir).arg(script).assert().code(1);
}
