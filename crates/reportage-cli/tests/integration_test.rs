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

// --- passing cases ---

#[test]
fn passing_cases_with_explicit_exit_assertions() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "first pass" {
  $ true
  assert {
    exit 0
  }
}

case "second pass" {
  $ false
  assert {
    exit 1
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(0);
}

#[test]
fn false_with_assert_exit_one_is_a_pass() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "second pass" {
  $ false
  assert {
    exit 1
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(0);
}

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
    assert_eq!(json["result"], "pass");
    assert_eq!(json["noop"], true);
    assert_eq!(json["summary"]["noop"], true);
    assert_eq!(json["summary"]["cases"]["total"], 0);
    assert_eq!(json["summary"]["cases"]["passed"], 0);
    assert_eq!(json["summary"]["cases"]["failed"], 0);
    assert_eq!(json["summary"]["steps"]["executed"], 0);
    assert_eq!(json["summary"]["assertions"]["total"], 0);
    assert_eq!(json["cases"].as_array().unwrap().len(), 0);
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
    assert_eq!(json["result"], "pass");
    assert_eq!(json["noop"], true);
    assert_eq!(json["summary"]["cases"]["total"], 0);
    assert_eq!(json["summary"]["steps"]["executed"], 0);
    assert_eq!(json["summary"]["assertions"]["total"], 0);
}

// --- failing assertions ---

#[test]
fn failing_assertion_exits_with_code_one() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "failing assertion" {
  $ false
  assert {
    exit 0
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(1);
}

// --- multiple expectations in one block ---

#[test]
fn multiple_expectations_in_one_block() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "multiple expectations" {
  $ true
  assert {
    exit 0
    exit 0
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(0);
}

// --- multiple assertion blocks ---

#[test]
fn precondition_and_postcondition_assertion_blocks() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "pre and post" {
  $ true
  assert {
    exit 0
  }
  $ false
  assert {
    exit 1
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(0);
}

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

    // .reportage/runs/<timestamp>/result.json must exist
    let runs_dir = dir.child(".reportage").child("runs");
    runs_dir.assert(predicates::path::is_dir());

    // Find the single run directory
    let runs_path = runs_dir.path();
    let entries: Vec<_> = std::fs::read_dir(runs_path)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(entries.len(), 1, "expected exactly one run directory");

    let result_json = entries[0].path().join("result.json");
    assert!(result_json.exists(), "result.json should exist");

    let content = std::fs::read_to_string(&result_json).unwrap();
    assert!(
        content.contains("\"result\""),
        "result.json should contain result field"
    );
    assert!(content.contains("pass"), "result.json should indicate pass");
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

    let actions = json["cases"][0]["actions"].as_array().unwrap();
    assert_eq!(
        actions.len(),
        1,
        "only the first action should have run; source order execution stops on assertion block failure"
    );
}

// --- output content ---

#[test]
fn stdout_shows_pass_for_passing_case() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "my test" {
  $ true
  assert {
    exit 0
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(0)
        .stdout(predicates::str::contains("PASS"))
        .stdout(predicates::str::contains("my test"));
}

#[test]
fn stdout_shows_fail_for_failing_case() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "my test" {
  $ false
  assert {
    exit 0
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stdout(predicates::str::contains("FAIL"))
        .stdout(predicates::str::contains("my test"));
}

// --- config-driven mode ---

#[test]
fn config_driven_mode_discovers_and_runs_files() {
    let dir = TempDir::new().unwrap();
    write_script(&dir, "test.repor", PASSING_CASE);
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
    reportage(&dir).assert().code(0);
}

#[test]
fn config_driven_mode_with_glob_pattern() {
    let dir = TempDir::new().unwrap();
    write_script(&dir, "a.repor", PASSING_CASE);
    write_script(&dir, "b.repor", PASSING_CASE);
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
    reportage(&dir).assert().code(0);
}

#[test]
fn explicit_config_flag_uses_specified_file() {
    let dir = TempDir::new().unwrap();
    write_script(&dir, "test.repor", PASSING_CASE);
    let config_path = dir.child("custom.kdl");
    config_path
        .write_str(
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
        )
        .unwrap();
    reportage(&dir)
        .arg("--config")
        .arg(config_path.path())
        .assert()
        .code(0);
}

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

#[test]
fn source_path_appears_in_config_driven_output() {
    let dir = TempDir::new().unwrap();
    write_script(&dir, "mytest.repor", PASSING_CASE);
    write_config(
        &dir,
        r#"
reportage {
  config {
    version 1
  }
  tests {
    path "mytest.repor"
  }
}
"#,
    );
    reportage(&dir)
        .assert()
        .code(0)
        .stdout(predicates::str::contains("mytest.repor"));
}

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

#[test]
fn multiple_files_run_all_cases() {
    let dir = TempDir::new().unwrap();
    write_script(&dir, "a.repor", PASSING_CASE);
    write_script(&dir, "b.repor", FAILING_CASE);
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
    // One file fails → overall exit 1
    reportage(&dir).assert().code(1);
}

#[test]
fn explicit_multiple_scripts_run_all_cases() {
    let dir = TempDir::new().unwrap();
    let a = write_script(&dir, "a.repor", PASSING_CASE);
    let b = write_script(&dir, "b.repor", PASSING_CASE);
    reportage(&dir).arg(a).arg(b).assert().code(0);
}

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
    // Actually, let's create a file and then remove it so glob matched it...
    // easier: point to a non-existent file via explicit script mode.
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

/// A runner-generated shim that fails to write its event file emits a prefixed
/// stderr diagnostic. That diagnostic is observable stderr: it is not filtered
/// from `assert { stderr empty }`.
#[test]
#[cfg(unix)]
fn shim_stderr_warning_is_not_filtered_from_stderr_empty_assertion() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new().unwrap();

    // Create a hand-crafted shim that unconditionally writes a prefixed
    // warning to stderr and then delegates to `true`. This mimics the
    // behavior of a real shim that cannot write its event file.
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

    // The assertion must fail: the shim's stderr warning is observable stderr
    // and is not automatically filtered from `assert { stderr empty }`.
    reportage(&dir)
        .arg(script)
        .env("PATH", path_with_prefix(shim_dir.path()))
        .assert()
        .code(1);
}

/// When a reportage-generated shim is invoked during an action, the result
/// artifact (result.json) records the observed shim invocation metadata.
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
    let invocations = &json["cases"][0]["actions"][0]["shim_invocations"];
    assert!(
        invocations.is_array(),
        "shim_invocations must be an array in result.json"
    );
    let invocations = invocations.as_array().unwrap();
    assert_eq!(invocations.len(), 1, "one shim invocation expected");
    assert_eq!(
        invocations[0]["command_name"], "reportage-test-artifact-shim",
        "command_name in artifact must match the shim name"
    );
    assert!(
        invocations[0]["shim_path"]
            .as_str()
            .unwrap()
            .contains("reportage-test-artifact-shim"),
        "shim_path must reference the shim file"
    );
    assert_eq!(invocations[0]["forwards_caller_args"], true);
}

/// When a case fails and the action was resolved through a reportage-generated
/// shim, the CLI diagnostics include the observed shim path and target invocation.
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

#[test]
fn file_exists_passes_for_a_regular_file() {
    let dir = TempDir::new().unwrap();
    dir.child("evidence.txt").write_str("hello").unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file exists" {
  $ true
  assert {
    file "evidence.txt" exists
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(0);
}

#[test]
fn file_exists_fails_for_a_missing_file() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file missing" {
  $ true
  assert {
    file "does-not-exist.txt" exists
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(1);
}

#[test]
fn file_exists_fails_for_a_directory() {
    let dir = TempDir::new().unwrap();
    dir.child("a-directory").create_dir_all().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "directory is not a file" {
  $ true
  assert {
    file "a-directory" exists
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(1);
}

#[test]
fn file_exists_follows_symlink_to_regular_file() {
    #[cfg(unix)]
    {
        let dir = TempDir::new().unwrap();
        dir.child("real.txt").write_str("hi").unwrap();
        std::os::unix::fs::symlink(dir.child("real.txt").path(), dir.child("link.txt").path())
            .unwrap();
        let script = write_script(
            &dir,
            "test.repor",
            r#"
case "symlink to file" {
  $ true
  assert {
    file "link.txt" exists
  }
}
"#,
        );
        reportage(&dir).arg(script).assert().code(0);
    }
}

#[test]
fn file_contains_passes_when_substring_present() {
    let dir = TempDir::new().unwrap();
    dir.child("result.json")
        .write_str("{\"status\":\"passed\"}")
        .unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file contains" {
  $ true
  assert {
    file "result.json" contains "\"status\":\"passed\""
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(0);
}

#[test]
fn file_contains_fails_when_substring_absent() {
    let dir = TempDir::new().unwrap();
    dir.child("result.json")
        .write_str("{\"status\":\"fail\"}")
        .unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file contains mismatch" {
  $ true
  assert {
    file "result.json" contains "passed"
  }
}
"#,
    );
    reportage(&dir)
        .arg(script)
        .assert()
        .code(1)
        .stderr(predicates::str::contains("does not contain"));
}

#[test]
fn file_contains_fails_for_missing_file() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file contains missing" {
  $ true
  assert {
    file "missing.txt" contains "anything"
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(1);
}

#[test]
fn file_contains_fails_for_directory() {
    let dir = TempDir::new().unwrap();
    dir.child("a-directory").create_dir_all().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file contains directory" {
  $ true
  assert {
    file "a-directory" contains "anything"
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(1);
}

#[test]
#[cfg(unix)]
fn file_contains_fails_for_non_utf8_content() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.child("binary.dat").path(), [0xff, 0xfe, 0x00, 0xff]).unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "file contains non-utf8" {
  $ true
  assert {
    file "binary.dat" contains "anything"
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(1);
}

#[test]
fn file_assertion_combines_with_process_expectations_in_one_block() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "combined evidence" {
  $ sh -c 'echo done > out.txt'
  assert {
    exit 0
    file "out.txt" exists
    file "out.txt" contains "done"
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(0);
}

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
    file "/etc/passwd" exists
  }
}
"#,
    );
    // The offending script's own path must be identifiable in the output, not just
    // the diagnostic code, so a semantic error can be traced back to its source file.
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
    file "../secret.txt" exists
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
    // A `cd` performed inside a `$` action must not change how the following
    // file assertion's path is resolved. See docs/semantics.md.
    let dir = TempDir::new().unwrap();
    dir.child("subdir").create_dir_all().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "cd does not affect file assertion root" {
  $ cd subdir && echo hi > moved.txt
  assert {
    file "subdir/moved.txt" exists
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
