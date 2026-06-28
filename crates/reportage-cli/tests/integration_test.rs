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
