use std::path::PathBuf;
use std::process::Command;

use crate::result::ActionResult;

#[derive(Debug)]
pub struct ExecutionError {
    pub message: String,
}

impl std::fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ExecutionError {}

/// Execution environment for action steps.
///
/// Owns PATH construction logic for action execution.
/// PATH prefixes are prepended before the inherited process PATH.
///
/// See docs/execution-model.md for the general shim injection model.
#[derive(Debug, Default)]
pub struct ExecutionEnvironment {
    /// Directories prepended to PATH before each action shell invocation.
    ///
    /// Multiple prefixes are prepended in the provided order.
    /// For example, `[A, B]` produces `PATH=A:B:<inherited PATH>`.
    pub path_prefixes: Vec<PathBuf>,
}

impl ExecutionEnvironment {
    pub fn with_path_prefixes(prefixes: Vec<PathBuf>) -> Self {
        Self {
            path_prefixes: prefixes,
        }
    }

    /// Compute the effective PATH to pass to the action shell.
    ///
    /// Returns `None` when no prefixes are configured, so the shell
    /// inherits PATH from the current process without modification.
    fn effective_path(&self) -> Option<String> {
        self.effective_path_with_base(std::env::var("PATH").ok().as_deref())
    }

    /// Compute the effective PATH given an explicit base PATH value.
    ///
    /// Prefixes are prepended in order; an absent or empty base is omitted.
    /// Used directly in tests to avoid dependence on the test-process PATH.
    pub(crate) fn effective_path_with_base(&self, inherited: Option<&str>) -> Option<String> {
        if self.path_prefixes.is_empty() {
            return None;
        }
        let mut parts: Vec<String> = self
            .path_prefixes
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        match inherited {
            Some(base) if !base.is_empty() => parts.push(base.to_string()),
            _ => {}
        }
        Some(parts.join(":"))
    }
}

pub fn execute_action(
    command: &str,
    env: &ExecutionEnvironment,
) -> Result<ActionResult, ExecutionError> {
    // Shell semantics are delegated to `sh -c` rather than parsed by the runner.
    // See ADR 20260627T100500Z_use-posix-shell-and-path-shims for the rationale.
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(command);

    // Prepend runner-owned PATH prefixes so the action shell resolves commands
    // through shims before falling through to the inherited PATH.
    // Shell selection remains separate: `sh` is chosen before the shim PATH applies.
    if let Some(path) = env.effective_path() {
        cmd.env("PATH", path);
    }

    let output = cmd.output().map_err(|e| ExecutionError {
        message: format!("failed to spawn shell for action '{command}': {e}"),
    })?;

    Ok(ActionResult {
        command: command.to_string(),
        // `status.code()` returns None when the process was terminated by a signal.
        // -1 is used as a sentinel; no valid expectation target should expect -1.
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_env() -> ExecutionEnvironment {
        ExecutionEnvironment::default()
    }

    // --- existing behaviour (no prefix) ---

    #[test]
    fn true_exits_zero() {
        let out = execute_action("true", &default_env()).unwrap();
        assert_eq!(out.exit_code, 0);
    }

    #[test]
    fn false_exits_one() {
        let out = execute_action("false", &default_env()).unwrap();
        assert_eq!(out.exit_code, 1);
    }

    #[test]
    fn stdout_is_captured() {
        let out = execute_action("echo hello", &default_env()).unwrap();
        assert_eq!(out.stdout.trim(), "hello");
    }

    #[test]
    fn stderr_is_captured() {
        let out = execute_action("echo error >&2", &default_env()).unwrap();
        assert_eq!(out.stderr.trim(), "error");
    }

    // --- PATH prefix logic (effective_path_with_base) ---

    #[test]
    fn no_prefix_returns_none() {
        let env = ExecutionEnvironment::default();
        assert_eq!(env.effective_path_with_base(Some("/usr/bin")), None);
    }

    #[test]
    fn single_prefix_prepended_before_base() {
        let env = ExecutionEnvironment::with_path_prefixes(vec![PathBuf::from("/a")]);
        assert_eq!(
            env.effective_path_with_base(Some("/usr/bin")),
            Some("/a:/usr/bin".to_string())
        );
    }

    #[test]
    fn multiple_prefixes_preserve_given_order() {
        let env = ExecutionEnvironment::with_path_prefixes(vec![
            PathBuf::from("/first"),
            PathBuf::from("/second"),
        ]);
        assert_eq!(
            env.effective_path_with_base(Some("/usr/bin")),
            Some("/first:/second:/usr/bin".to_string())
        );
    }

    #[test]
    fn absent_inherited_path_produces_prefixes_only() {
        let env = ExecutionEnvironment::with_path_prefixes(vec![PathBuf::from("/a")]);
        assert_eq!(env.effective_path_with_base(None), Some("/a".to_string()));
    }

    #[test]
    fn empty_inherited_path_produces_prefixes_only() {
        let env = ExecutionEnvironment::with_path_prefixes(vec![PathBuf::from("/a")]);
        assert_eq!(
            env.effective_path_with_base(Some("")),
            Some("/a".to_string())
        );
    }

    // --- PATH prefix integration through execute_action ---

    #[test]
    #[cfg(unix)]
    fn command_in_path_prefix_is_found() {
        use std::os::unix::fs::PermissionsExt;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let cmd_path = dir.path().join("reportage-test-custom-cmd");
        std::fs::write(&cmd_path, "#!/bin/sh\necho found-via-prefix\n").unwrap();
        std::fs::set_permissions(&cmd_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let env = ExecutionEnvironment::with_path_prefixes(vec![dir.path().to_path_buf()]);
        let out = execute_action("reportage-test-custom-cmd", &env).unwrap();
        assert_eq!(out.stdout.trim(), "found-via-prefix");
    }

    #[test]
    #[cfg(unix)]
    fn multiple_path_prefixes_first_wins() {
        use std::os::unix::fs::PermissionsExt;
        use tempfile::TempDir;

        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();

        for (dir, label) in [(&dir_a, "from-a"), (&dir_b, "from-b")] {
            let cmd = dir.path().join("reportage-test-precedence-cmd");
            std::fs::write(&cmd, format!("#!/bin/sh\necho {label}\n")).unwrap();
            std::fs::set_permissions(&cmd, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        // dir_a is first, so it should shadow dir_b.
        let env = ExecutionEnvironment::with_path_prefixes(vec![
            dir_a.path().to_path_buf(),
            dir_b.path().to_path_buf(),
        ]);
        let out = execute_action("reportage-test-precedence-cmd", &env).unwrap();
        assert_eq!(out.stdout.trim(), "from-a");
    }

    #[test]
    fn no_prefix_preserves_existing_behaviour() {
        let env = ExecutionEnvironment::default();
        let out = execute_action("true", &env).unwrap();
        assert_eq!(out.exit_code, 0);
    }
}
