use super::*;

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
