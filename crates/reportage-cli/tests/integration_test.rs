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
  assert exit 0
}

case "second pass" {
  $ false
  assert exit 1
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
  assert exit 1
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
  assert exit 0
}
"#,
    );
    reportage(&dir).arg(script).assert().code(1);
}

// --- multiple assertions ---

#[test]
fn multiple_assertions_for_one_action() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "multiple assertions" {
  $ true
  assert exit 0
  assert exit 0
}
"#,
    );
    reportage(&dir).arg(script).assert().code(0);
}

// --- validation/spec errors ---

#[test]
fn missing_assertion_exits_with_code_two() {
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
fn assertion_before_action_exits_with_code_two() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "assertion before action" {
  assert exit 0
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
  assert exit 999
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
fn unsupported_assertion_type_exits_with_code_two() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "unsupported" {
  $ true
  assert unknown_assertion
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
  assert exit 0
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
  assert exit 0
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
  assert exit 0
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
  assert exit 0
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
  assert exit 0
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
