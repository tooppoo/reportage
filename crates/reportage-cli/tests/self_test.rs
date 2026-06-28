/// Self-test harness: runs the e2e `.repor` scripts through the cargo-built reportage binary.
///
/// `$ reportage ...` steps inside the scripts resolve to the same cargo-built binary
/// via a temporary PATH shim, not any binary installed on the system PATH.
/// This ensures coverage data is collected from both the runner invocation and every
/// subprocess the scripts spawn.
use std::path::PathBuf;

use assert_cmd::Command;
use assert_fs::TempDir;

/// Workspace root: two directories above this package's manifest.
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Write a `reportage` shim in `dir` that delegates to the binary named by `REPORTAGE_BIN`.
///
/// Using an env var in the shim body avoids embedding paths with special characters
/// and keeps the shim script reusable across invocations.
#[cfg(unix)]
fn write_reportage_shim(dir: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let shim = dir.join("reportage");
    std::fs::write(&shim, "#!/bin/sh\nexec \"$REPORTAGE_BIN\" \"$@\"\n").unwrap();
    std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// Run all e2e self-tests through the cargo-built binary and assert the suite passes.
///
/// The test runner (cargo-built `reportage`) reads `reportage.kdl` from the workspace root,
/// discovers `e2e/**/*.repor`, and executes each case. `$ reportage ...` steps inside the
/// scripts find the shim on PATH, which delegates to `REPORTAGE_BIN` — the same
/// cargo-built (and llvm-cov instrumented) binary.
#[test]
#[cfg(unix)]
fn self_tests_pass() {
    let reportage_bin = assert_cmd::cargo::cargo_bin("reportage");

    let shim_dir = TempDir::new().unwrap();
    write_reportage_shim(shim_dir.path());

    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", shim_dir.path().display(), original_path);

    Command::cargo_bin("reportage")
        .unwrap()
        .current_dir(workspace_root())
        .env("REPORTAGE_BIN", &reportage_bin)
        .env("PATH", &new_path)
        .assert()
        .success();
}
