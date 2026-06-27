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

pub fn execute_action(command: &str) -> Result<ActionResult, ExecutionError> {
    // Shell semantics are delegated to `sh -c` rather than parsed by the runner.
    // See ADR 20260627T100500Z_use-posix-shell-and-path-shims for the rationale.
    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()
        .map_err(|e| ExecutionError {
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

    #[test]
    fn true_exits_zero() {
        let out = execute_action("true").unwrap();
        assert_eq!(out.exit_code, 0);
    }

    #[test]
    fn false_exits_one() {
        let out = execute_action("false").unwrap();
        assert_eq!(out.exit_code, 1);
    }

    #[test]
    fn stdout_is_captured() {
        let out = execute_action("echo hello").unwrap();
        assert_eq!(out.stdout.trim(), "hello");
    }

    #[test]
    fn stderr_is_captured() {
        let out = execute_action("echo error >&2").unwrap();
        assert_eq!(out.stderr.trim(), "error");
    }
}
