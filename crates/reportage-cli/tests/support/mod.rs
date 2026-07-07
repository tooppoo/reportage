//! Shared PATH-overlay shim harness for suite self-tests.
//!
//! Suite runs must go through normal command resolution, not a direct `cargo_bin` path:
//! the outer suite invocation is started by command name (`reportage`) against a PATH whose
//! first entry is a runner-owned shim directory, and inner `$ reportage ...` steps inside the
//! `.repor` scripts resolve to the same shim via the inherited PATH. The shim delegates to the
//! cargo-built `reportage` binary, so both invocation layers land in `cargo llvm-cov` coverage
//! collection.
#![cfg(unix)]

use std::path::{Path, PathBuf};

use assert_fs::TempDir;
use reportage_core::shim::{CommandName, CommandShim, ExecutableInvocation};

/// Workspace root: two directories above this package's manifest.
pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// A materialized `reportage` shim plus the PATH overlay that resolves to it.
pub struct ShimHarness {
    shim_dir: TempDir,
    reportage_bin: PathBuf,
    path_env: String,
}

impl ShimHarness {
    /// Materialize a `reportage` shim delegating to the cargo-built binary and build the
    /// PATH overlay (shim dir prepended to the inherited PATH).
    pub fn new() -> ShimHarness {
        let reportage_bin = assert_cmd::cargo::cargo_bin("reportage");

        let shim_dir = TempDir::new().unwrap();
        let name = CommandName::new("reportage").unwrap();
        let invocation = ExecutableInvocation::new(reportage_bin.clone(), vec![]).unwrap();
        CommandShim::new(name, invocation)
            .materialize(shim_dir.path())
            .unwrap();

        let original = std::env::var("PATH").unwrap_or_default();
        let path_env = format!("{}:{}", shim_dir.path().display(), original);

        ShimHarness {
            shim_dir,
            reportage_bin,
            path_env,
        }
    }

    /// The directory holding the generated shim (the PATH overlay's first entry).
    pub fn shim_dir(&self) -> &Path {
        self.shim_dir.path()
    }

    /// The generated shim file itself.
    pub fn shim_path(&self) -> PathBuf {
        self.shim_dir.path().join("reportage")
    }

    /// The cargo-built `reportage` binary the shim delegates to.
    pub fn reportage_bin(&self) -> &Path {
        &self.reportage_bin
    }

    /// An outer suite invocation: `reportage` started by command name under the PATH overlay,
    /// with the workspace root as working directory.
    ///
    /// Deliberately `std::process::Command::new("reportage")`, not `Command::cargo_bin`:
    /// resolving the command through PATH is the behavior under test.
    pub fn suite_command(&self) -> std::process::Command {
        let mut cmd = std::process::Command::new("reportage");
        cmd.current_dir(workspace_root())
            .env("PATH", &self.path_env);
        cmd
    }
}
