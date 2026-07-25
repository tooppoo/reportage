use super::*;

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
