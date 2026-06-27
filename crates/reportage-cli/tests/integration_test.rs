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
