use super::*;

const FAILING_CASE: &str = r#"
case "fail" {
  $ false
  assert {
    exit 0
  }
}
"#;

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
fn json_format_before_each_write_failure_reports_a_runtime_diagnostic() {
    // The machine-readable channel must attribute the failure the same way
    // stderr does: a runtime diagnostic with the write step's code, whose
    // message names the position inside the module-level block — there is no
    // case body step index to point at.
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
before_each {
  write <"a.txt"> "first\n"
  write <"a.txt"> "second\n"
}

case "never runs its body" {
  $ true
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
    assert!(
        json["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .contains("before_each write step 2"),
        "diagnostic message must name the failing before_each step: {}",
        json["diagnostics"][0]["message"]
    );
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

// --- --format=json (#75) ---

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
