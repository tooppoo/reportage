use super::*;

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

// Representative `before_each` scenarios (setup visible at the initial
// checkpoint, per-concrete-case replay, and each placement/body parse error)
// live in e2e/cases/before-each.repor (#70). The tests below verify the
// stderr contracts not asserted there in full.

#[test]
fn before_each_action_step_is_rejected_with_guidance() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
before_each {
  write <"seed.txt"> "seed\n"
  $ mkdir -p fixtures
}

case "never runs" {
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
        .code(2)
        .stderr(predicates::str::contains("parse.before_each.action_step"))
        .stderr(predicates::str::contains(
            "run setup commands in each case body instead",
        ));
}
