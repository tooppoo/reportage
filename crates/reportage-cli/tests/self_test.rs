/// Self-test harness: runs the e2e `.repor` scripts through the cargo-built reportage binary.
///
/// `$ reportage ...` steps inside the scripts resolve to the same cargo-built binary
/// via a temporary PATH shim, not any binary installed on the system PATH.
/// This ensures coverage data is collected from both the runner invocation and every
/// subprocess the scripts spawn.
use std::path::PathBuf;

use assert_cmd::Command;
use assert_fs::TempDir;
use reportage_core::shim::{CommandName, CommandShim, ExecutableInvocation};

/// Workspace root: two directories above this package's manifest.
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Materialize a `reportage` shim in `dir` that delegates to `reportage_bin`.
///
/// The shim embeds the absolute path to the cargo-built binary directly, with
/// shell-safe escaping. This replaces the previous ad hoc wrapper that delegated
/// through the `REPORTAGE_BIN` environment variable.
#[cfg(unix)]
fn make_reportage_shim(dir: &std::path::Path, reportage_bin: PathBuf) {
    let name = CommandName::new("reportage").unwrap();
    let invocation = ExecutableInvocation::new(reportage_bin, vec![]).unwrap();
    let shim = CommandShim::new(name, invocation);
    shim.materialize(dir).unwrap();
}

/// Run all e2e self-tests through the cargo-built binary and assert the suite passes.
///
/// The test runner (cargo-built `reportage`) reads `reportage.kdl` from the workspace root,
/// discovers `e2e/**/*.repor`, and executes each case. `$ reportage ...` steps inside the
/// scripts find the shim on PATH, which delegates to the same cargo-built binary.
#[test]
#[cfg(unix)]
fn self_tests_pass() {
    let reportage_bin = assert_cmd::cargo::cargo_bin("reportage");

    let shim_dir = TempDir::new().unwrap();
    make_reportage_shim(shim_dir.path(), reportage_bin);

    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", shim_dir.path().display(), original_path);

    Command::cargo_bin("reportage")
        .unwrap()
        .current_dir(workspace_root())
        .env("PATH", &new_path)
        .assert()
        .success();
}
