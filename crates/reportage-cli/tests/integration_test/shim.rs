use super::*;

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
