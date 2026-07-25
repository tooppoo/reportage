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

#[path = "integration_test/artifacts.rs"]
mod artifacts;
#[path = "integration_test/assertions.rs"]
mod assertions;
#[path = "integration_test/config.rs"]
mod config;
#[path = "integration_test/execution.rs"]
mod execution;
#[path = "integration_test/output.rs"]
mod output;
#[path = "integration_test/registered_commands.rs"]
mod registered_commands;
#[path = "integration_test/shim.rs"]
mod shim;
#[path = "integration_test/validation.rs"]
mod validation;
#[path = "integration_test/workspace.rs"]
mod workspace;
