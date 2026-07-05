use std::path::{Path, PathBuf};
use std::process::Command;

use crate::result::ActionResult;
use crate::shim_event::{SHIM_EVENT_DIR_VAR, collect_from_dir};

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

    /// Returns the PATH string to set on the action shell, or `None` when no override is needed.
    ///
    /// `None` means "no prefixes configured — let the shell inherit PATH from the current process without modification".
    /// It is a control signal, not an empty path value.
    fn effective_path(&self) -> Option<String> {
        if self.path_prefixes.is_empty() {
            return None;
        }
        Some(self.build_path_string(std::env::var("PATH").ok().as_deref()))
    }

    /// Build the PATH string by prepending `path_prefixes` before `inherited`.
    ///
    /// Only called when `path_prefixes` is non-empty.
    /// Exposed for testing so tests can verify path construction without relying on the process PATH.
    pub(crate) fn build_path_string(&self, inherited: Option<&str>) -> String {
        let mut parts: Vec<String> = self
            .path_prefixes
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        match inherited {
            Some(base) if !base.is_empty() => parts.push(base.to_string()),
            _ => {}
        }
        parts.join(":")
    }
}

pub fn execute_action(
    command: &str,
    env: &ExecutionEnvironment,
    workspace_root: &Path,
) -> Result<ActionResult, ExecutionError> {
    // Create a fresh, action-scoped event directory so shim events from this action are isolated from events produced by any other action.
    // See ADR 20260628T210000Z_shim-invocation-event-side-channel.
    let event_dir = tempfile::TempDir::new().map_err(|e| ExecutionError {
        message: format!("failed to create shim event directory for action '{command}': {e}"),
    })?;

    // Shell semantics are delegated to `sh -c` rather than parsed by the runner.
    // See ADR 20260627T100500Z_use-posix-shell-and-path-shims for the rationale.
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(command);

    // Run in the concrete case's isolated workspace. A `cd` performed inside
    // the action's own shell never escapes this for the *next* action or
    // for file expectations, because each action spawns a fresh child shell.
    // See docs/semantics.md — Workspace lifecycle.
    cmd.current_dir(workspace_root);

    // Prepend runner-owned PATH prefixes so the action shell resolves commands through shims before falling through to the inherited PATH.
    // Shell selection remains separate: `sh` is chosen before the shim PATH applies.
    if let Some(path) = env.effective_path() {
        cmd.env("PATH", path);
    }

    // Expose the event directory so protocol-compliant shims can write invocation events before exec-ing their target.
    // The runner reads these events after the action completes; the shim must not write post-execution data because POSIX exec replaces the shim process.
    cmd.env(SHIM_EVENT_DIR_VAR, event_dir.path());

    let output = cmd.output().map_err(|e| ExecutionError {
        message: format!("failed to spawn shell for action '{command}': {e}"),
    })?;

    // Collect shim invocation events written by protocol-compliant shims.
    // Malformed event files are returned as warnings, not hard errors, so they do not silently corrupt the action result.
    let (shim_invocations, shim_event_parse_warnings) = collect_from_dir(event_dir.path());

    Ok(ActionResult {
        command: command.to_string(),
        // `status.code()` returns None when the process was terminated by a signal.
        // -1 is used as a sentinel; no valid expectation target should expect -1.
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        shim_invocations,
        shim_event_parse_warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_env() -> ExecutionEnvironment {
        ExecutionEnvironment::default()
    }

    fn tmp_workspace() -> tempfile::TempDir {
        tempfile::TempDir::new().unwrap()
    }

    // --- existing behaviour (no prefix) ---

    #[test]
    fn true_exits_zero() {
        let out = execute_action("true", &default_env(), tmp_workspace().path()).unwrap();
        assert_eq!(out.exit_code, 0);
    }

    #[test]
    fn false_exits_one() {
        let out = execute_action("false", &default_env(), tmp_workspace().path()).unwrap();
        assert_eq!(out.exit_code, 1);
    }

    #[test]
    fn stdout_is_captured() {
        let out = execute_action("echo hello", &default_env(), tmp_workspace().path()).unwrap();
        assert_eq!(out.stdout.trim(), "hello");
    }

    #[test]
    fn stderr_is_captured() {
        let out = execute_action("echo error >&2", &default_env(), tmp_workspace().path()).unwrap();
        assert_eq!(out.stderr.trim(), "error");
    }

    // --- effective_path: None/Some decision ---

    #[test]
    fn no_prefix_does_not_override_path() {
        // None means "no override needed — inherit PATH from the current process".
        let env = ExecutionEnvironment::default();
        assert!(env.effective_path().is_none());
    }

    #[test]
    fn with_prefix_effective_path_returns_some() {
        let env = ExecutionEnvironment::with_path_prefixes(vec![PathBuf::from("/a")]);
        assert!(env.effective_path().is_some());
    }

    // --- build_path_string: PATH string construction (always called with prefixes present) ---

    #[test]
    fn single_prefix_prepended_before_base() {
        let env = ExecutionEnvironment::with_path_prefixes(vec![PathBuf::from("/a")]);
        assert_eq!(env.build_path_string(Some("/usr/bin")), "/a:/usr/bin");
    }

    #[test]
    fn multiple_prefixes_preserve_given_order() {
        let env = ExecutionEnvironment::with_path_prefixes(vec![
            PathBuf::from("/first"),
            PathBuf::from("/second"),
        ]);
        assert_eq!(
            env.build_path_string(Some("/usr/bin")),
            "/first:/second:/usr/bin"
        );
    }

    #[test]
    fn absent_inherited_path_produces_prefixes_only() {
        let env = ExecutionEnvironment::with_path_prefixes(vec![PathBuf::from("/a")]);
        assert_eq!(env.build_path_string(None), "/a");
    }

    #[test]
    fn empty_inherited_path_produces_prefixes_only() {
        let env = ExecutionEnvironment::with_path_prefixes(vec![PathBuf::from("/a")]);
        assert_eq!(env.build_path_string(Some("")), "/a");
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
        let out =
            execute_action("reportage-test-custom-cmd", &env, tmp_workspace().path()).unwrap();
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
        let out = execute_action(
            "reportage-test-precedence-cmd",
            &env,
            tmp_workspace().path(),
        )
        .unwrap();
        assert_eq!(out.stdout.trim(), "from-a");
    }

    #[test]
    fn no_prefix_preserves_existing_behaviour() {
        let env = ExecutionEnvironment::default();
        let out = execute_action("true", &env, tmp_workspace().path()).unwrap();
        assert_eq!(out.exit_code, 0);
    }

    // --- shim invocation event collection ---

    #[test]
    fn non_shim_action_has_no_shim_invocations() {
        let out = execute_action("true", &default_env(), tmp_workspace().path()).unwrap();
        assert!(
            out.shim_invocations.is_empty(),
            "plain action must not produce shim invocations"
        );
        assert!(out.shim_event_parse_warnings.is_empty());
    }

    #[test]
    #[cfg(unix)]
    fn shim_invocation_is_recorded_in_action_result() {
        use crate::shim::{CommandName, CommandShim, ExecutableInvocation};
        use tempfile::TempDir;

        let shim_dir = TempDir::new().unwrap();
        let name = CommandName::new("reportage-test-shim-cmd").unwrap();
        let true_path = PathBuf::from(
            String::from_utf8_lossy(
                &std::process::Command::new("which")
                    .arg("true")
                    .output()
                    .unwrap()
                    .stdout,
            )
            .trim()
            .to_string(),
        );
        let invocation = ExecutableInvocation::new(true_path.clone(), vec![]).unwrap();
        let shim = CommandShim::new(name, invocation);
        shim.materialize(shim_dir.path()).unwrap();

        let env = ExecutionEnvironment::with_path_prefixes(vec![shim_dir.path().to_path_buf()]);
        let out = execute_action("reportage-test-shim-cmd", &env, tmp_workspace().path()).unwrap();

        assert_eq!(out.exit_code, 0);
        assert_eq!(out.shim_invocations.len(), 1, "one shim event expected");
        assert_eq!(
            out.shim_invocations[0].command_name,
            "reportage-test-shim-cmd"
        );
        assert_eq!(out.shim_invocations[0].target.program, true_path);
        assert!(out.shim_event_parse_warnings.is_empty());
    }

    #[test]
    #[cfg(unix)]
    fn events_from_one_action_are_not_attached_to_another_action() {
        use crate::shim::{CommandName, CommandShim, ExecutableInvocation};
        use tempfile::TempDir;

        let shim_dir = TempDir::new().unwrap();
        let name = CommandName::new("reportage-test-isolation-cmd").unwrap();
        let true_path = PathBuf::from(
            String::from_utf8_lossy(
                &std::process::Command::new("which")
                    .arg("true")
                    .output()
                    .unwrap()
                    .stdout,
            )
            .trim()
            .to_string(),
        );
        let invocation = ExecutableInvocation::new(true_path, vec![]).unwrap();
        let shim = CommandShim::new(name, invocation);
        shim.materialize(shim_dir.path()).unwrap();

        let env = ExecutionEnvironment::with_path_prefixes(vec![shim_dir.path().to_path_buf()]);

        // First action: invokes the shim.
        let first =
            execute_action("reportage-test-isolation-cmd", &env, tmp_workspace().path()).unwrap();
        assert_eq!(first.shim_invocations.len(), 1);

        // Second action: does not invoke the shim — must not see the first action's events.
        let second = execute_action("true", &env, tmp_workspace().path()).unwrap();
        assert!(
            second.shim_invocations.is_empty(),
            "second action must not inherit events from the first"
        );
    }

    #[test]
    #[cfg(unix)]
    fn multiple_shim_invocations_in_one_action_are_all_collected() {
        use crate::shim::{CommandName, CommandShim, ExecutableInvocation};
        use tempfile::TempDir;

        let shim_dir = TempDir::new().unwrap();
        let true_path = PathBuf::from(
            String::from_utf8_lossy(
                &std::process::Command::new("which")
                    .arg("true")
                    .output()
                    .unwrap()
                    .stdout,
            )
            .trim()
            .to_string(),
        );

        // Materialize two distinct shims in the same directory.
        for cmd_name in ["reportage-test-multi-a", "reportage-test-multi-b"] {
            let name = CommandName::new(cmd_name).unwrap();
            let invocation = ExecutableInvocation::new(true_path.clone(), vec![]).unwrap();
            CommandShim::new(name, invocation)
                .materialize(shim_dir.path())
                .unwrap();
        }

        let env = ExecutionEnvironment::with_path_prefixes(vec![shim_dir.path().to_path_buf()]);
        // Invoke both shims in a single action.
        let out = execute_action(
            "reportage-test-multi-a && reportage-test-multi-b",
            &env,
            tmp_workspace().path(),
        )
        .unwrap();

        assert_eq!(
            out.shim_invocations.len(),
            2,
            "both shim invocations must be collected"
        );
    }

    #[test]
    fn malformed_event_file_produces_warning_not_error() {
        use tempfile::TempDir;

        let event_dir = TempDir::new().unwrap();
        std::fs::write(event_dir.path().join("bad.json"), "not valid json").unwrap();

        // Run an action that will see the pre-existing malformed event file.
        // We inject it by using a custom action that writes the bad file to the event dir directly — but since execute_action creates a fresh dir per action, we test collect_from_dir indirectly via the shim_event module.
        // Here we test that the runner handles parse failures gracefully.
        let (events, warnings) = crate::shim_event::collect_from_dir(event_dir.path());
        assert!(events.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("malformed shim event file"));
    }
}
