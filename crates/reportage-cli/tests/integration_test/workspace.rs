use super::*;

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
fn before_each_write_failure_is_a_runtime_error_naming_the_block() {
    // The second before_each write violates the create-only overwrite policy.
    // The failure must be attributed to the module-level block with its
    // 1-based position, not to a case body step.
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
    reportage(&dir)
        .arg(script)
        .assert()
        .code(3)
        .stderr(predicates::str::contains("before_each write step 2"))
        .stderr(predicates::str::contains("step.write.target_exists"));
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
