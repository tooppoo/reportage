use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use serde_json::json;

use crate::result::{
    ActionResult, CaseResult, CaseStatus, ContentsEqualsExpectedSource, ContentsEqualsObservation,
    ExecutionReport, ExpectationKind, ExpectationResult, FileErrorKind, TextEqualsExpectedSource,
};

/// Error rejecting an unsafe run id value.
///
/// A run id becomes a single path component under `<artifact-root>/runs/`, so it must not be usable to escape or corrupt that layout.
#[derive(Debug, PartialEq, Eq)]
pub enum RunIdError {
    Empty,
    ContainsPathSeparator(String),
    ReservedSegment(String),
    ContainsControlChar(String),
}

impl std::fmt::Display for RunIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunIdError::Empty => write!(f, "run id must not be empty"),
            RunIdError::ContainsPathSeparator(id) => {
                write!(f, "run id '{id}' must not contain a path separator")
            }
            RunIdError::ReservedSegment(id) => {
                write!(f, "run id '{id}' must not be '.' or '..'")
            }
            RunIdError::ContainsControlChar(id) => {
                write!(f, "run id '{id}' must not contain control characters")
            }
        }
    }
}

impl std::error::Error for RunIdError {}

/// A validated run id: a single safe path component for `<artifact-root>/runs/<id>`.
///
/// This is an internal development / self-testing affordance (`--debug-run-id`), not a public stable interface.
/// See docs/TBD.md — "Self-test run ID control".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunId(String);

impl RunId {
    pub fn new(raw: impl Into<String>) -> Result<Self, RunIdError> {
        let raw = raw.into();
        if raw.is_empty() {
            return Err(RunIdError::Empty);
        }
        if raw.contains('/') || raw.contains('\\') {
            return Err(RunIdError::ContainsPathSeparator(raw));
        }
        if raw == "." || raw == ".." {
            return Err(RunIdError::ReservedSegment(raw));
        }
        if raw.chars().any(|c| c.is_control()) {
            return Err(RunIdError::ContainsControlChar(raw));
        }
        Ok(Self(raw))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Error constructing an `ArtifactWriter` for a fixed run id.
#[derive(Debug)]
pub enum ArtifactWriterError {
    /// The target run directory already exists.
    /// A fixed run id must not silently overwrite a previous run's artifacts.
    RunDirectoryExists(PathBuf),
}

impl std::fmt::Display for ArtifactWriterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArtifactWriterError::RunDirectoryExists(path) => write!(
                f,
                "run directory '{}' already exists; refusing to overwrite a previous run",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ArtifactWriterError {}

#[derive(Debug)]
pub struct ArtifactWriter {
    run_dir: PathBuf,
}

impl ArtifactWriter {
    pub fn for_run(base_dir: &Path) -> Self {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        ArtifactWriter {
            run_dir: base_dir.join("runs").join(millis.to_string()),
        }
    }

    /// Construct a writer for a fixed, caller-chosen run id.
    ///
    /// Internal development / self-testing affordance behind `--debug-run-id`; not a public stable interface.
    /// Rejects with `RunDirectoryExists` rather than silently overwriting a run directory that already exists.
    pub fn for_fixed_run(base_dir: &Path, run_id: &RunId) -> Result<Self, ArtifactWriterError> {
        let run_dir = base_dir.join("runs").join(run_id.as_str());
        if run_dir.exists() {
            return Err(ArtifactWriterError::RunDirectoryExists(run_dir));
        }
        Ok(ArtifactWriter { run_dir })
    }

    /// The run directory this writer writes into (e.g. `.reportage/runs/<id>`).
    ///
    /// This is the `artifactRoot` that `--format=json` output resolves `artifactRef` values
    /// against. See [`test_id`] / [`action_id`] for the path segments used underneath it.
    pub fn run_dir(&self) -> &Path {
        &self.run_dir
    }

    pub fn write(&self, result: &ExecutionReport) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.run_dir)?;
        let value = build_json(result);
        let json = serde_json::to_string_pretty(&value)
            .expect("result JSON serialization should not fail");
        std::fs::write(self.run_dir.join("result.json"), json)?;
        self.write_captured_output(result)?;
        Ok(())
    }

    /// Writes each action's captured stdout/stderr as raw bytes under
    /// `<run-dir>/<test_id>/<action_id>/{stdout,stderr}.bin`.
    ///
    /// This is the artifact-file side of the "captured stdout/stderr are not inlined in
    /// `--format=json`" policy: the JSON renderer references these files by relative path
    /// (`artifactRef`) instead of embedding raw bytes. See `reportage-cli::render::json` and
    /// docs/adr candidate "Captured stdout/stderr are v0 artifact references, not inline data".
    fn write_captured_output(&self, result: &ExecutionReport) -> std::io::Result<()> {
        for (case_index, case) in result.cases.iter().enumerate() {
            for (action_index, action) in case.actions.iter().enumerate() {
                let dir = self
                    .run_dir
                    .join(test_id(case_index))
                    .join(action_id(action_index));
                std::fs::create_dir_all(&dir)?;
                std::fs::write(dir.join("stdout.bin"), &action.stdout)?;
                std::fs::write(dir.join("stderr.bin"), &action.stderr)?;
            }
        }
        Ok(())
    }
}

/// The document-local id for the case at `case_index` (0-based) within an `ExecutionReport`.
///
/// Shared between artifact file paths (`<run-dir>/<test_id>/...`) and the `--format=json`
/// renderer's `tests[].id`, so a JSON `artifactRef` and the file it names always agree.
pub fn test_id(case_index: usize) -> String {
    format!("test-{}", case_index + 1)
}

/// The document-local id for the action at `action_index` (0-based) within one case.
///
/// See [`test_id`].
pub fn action_id(action_index: usize) -> String {
    format!("action-{}", action_index + 1)
}

fn build_json(result: &ExecutionReport) -> serde_json::Value {
    let summary = result.summary();
    let overall = if !result.file_errors.is_empty() {
        "script_error"
    } else if result.exit_code() == 0 {
        "pass"
    } else {
        "fail"
    };

    let mut obj = json!({
        "result": overall,
        "noop": summary.noop,
        "summary": {
            "noop": summary.noop,
            "cases": {
                "total": summary.cases_total,
                "passed": summary.cases_passed,
                "failed": summary.cases_failed
            },
            "steps": {
                "executed": summary.steps_executed
            },
            "assertions": {
                "total": summary.assertions_total
            }
        },
        "cases": result.cases.iter().map(case_json).collect::<Vec<_>>()
    });

    if !result.file_errors.is_empty() {
        obj["file_errors"] = json!(
            result
                .file_errors
                .iter()
                .map(|e| {
                    let (kind_str, message, diagnostic_code) = match &e.kind {
                        FileErrorKind::ReadError(msg) => ("read_error", msg.as_str(), None),
                        FileErrorKind::ParseError {
                            message,
                            diagnostic_code,
                            location: _,
                        } => ("parse_error", message.as_str(), Some(*diagnostic_code)),
                    };
                    let mut entry = json!({
                        "source_path": e.source_path.display().to_string(),
                        "kind": kind_str,
                        "message": message
                    });
                    if let Some(code) = diagnostic_code {
                        entry["diagnostic_code"] = json!(code.as_str());
                    }
                    entry
                })
                .collect::<Vec<_>>()
        );
    }

    obj
}

/// Canonical JSON representation of raw process output bytes: base64 `data` (always present) plus
/// an optional `text` helper view for display, present only when `bytes` is valid UTF-8.
///
/// `text` is never used for semantic comparison — see docs/semantics.md and the raw byte
/// semantics ADR. Consumers that need the canonical value must decode `data`.
fn stream_json(bytes: &[u8]) -> serde_json::Value {
    let mut obj = json!({
        "data": base64::engine::general_purpose::STANDARD.encode(bytes),
        "encoding": "base64",
    });
    if let Ok(text) = std::str::from_utf8(bytes) {
        obj["text"] = json!(text);
    }
    obj
}

/// JSON representation of a `contents_equals` expected value's source, for evidence purposes.
fn expected_source_json(source: &ContentsEqualsExpectedSource) -> serde_json::Value {
    match source {
        ContentsEqualsExpectedSource::Workspace(path) => json!({
            "kind": "workspace",
            "path": path,
        }),
        ContentsEqualsExpectedSource::Fixture(path) => json!({
            "kind": "fixture",
            "path": path,
        }),
    }
}

/// JSON representation of a `text_equals` expected value's source, for evidence purposes.
fn text_equals_expected_source_json(source: &TextEqualsExpectedSource) -> serde_json::Value {
    match source {
        TextEqualsExpectedSource::Quoted(value) => json!({
            "kind": "quoted",
            "value": value,
        }),
        TextEqualsExpectedSource::Heredoc(value) => json!({
            "kind": "heredoc",
            "value": value,
        }),
    }
}

fn action_json(index: usize, action: &ActionResult) -> serde_json::Value {
    let mut obj = json!({
        "index": index,
        "kind": "action",
        "command": action.command,
        "exit_code": action.exit_code,
        "stdout": stream_json(&action.stdout),
        "stderr": stream_json(&action.stderr)
    });

    if !action.shim_invocations.is_empty() {
        obj["shim_invocations"] = json!(
            action
                .shim_invocations
                .iter()
                .map(|ev| {
                    json!({
                        "schema_version": ev.schema_version,
                        "event": "shim_invoked",
                        "command_name": ev.command_name,
                        "shim_path": ev.shim_path.display().to_string(),
                        "target": {
                            "program": ev.target.program.display().to_string(),
                            "args": ev.target.args
                        },
                        "forwards_caller_args": ev.forwards_caller_args
                    })
                })
                .collect::<Vec<_>>()
        );
    }

    if !action.shim_event_parse_warnings.is_empty() {
        obj["shim_event_parse_warnings"] = json!(action.shim_event_parse_warnings);
    }

    obj
}

/// Renders one evaluated expectation, recursing into a `not` / `all` / `any` composition's own children so nested results are never lost — see docs/semantics.md — Logical composition.
fn expectation_result_json(e: &ExpectationResult) -> serde_json::Value {
    let result_str = if e.passed { "pass" } else { "fail" };
    let mut value = match &e.kind {
        ExpectationKind::Exit { expected, actual } => json!({
            "kind": "exit",
            "expected": expected,
            "actual": actual,
            "result": result_str,
        }),
        ExpectationKind::StdoutContains { expected, actual } => json!({
            "kind": "stdout_contains",
            "expected": expected,
            "actual": stream_json(actual),
            "result": result_str,
        }),
        ExpectationKind::StderrContains { expected, actual } => json!({
            "kind": "stderr_contains",
            "expected": expected,
            "actual": stream_json(actual),
            "result": result_str,
        }),
        ExpectationKind::StdoutEmpty { actual } => json!({
            "kind": "stdout_empty",
            "actual": stream_json(actual),
            "result": result_str,
        }),
        ExpectationKind::StderrEmpty { actual } => json!({
            "kind": "stderr_empty",
            "actual": stream_json(actual),
            "result": result_str,
        }),
        ExpectationKind::FileExists { path, .. } => json!({
            "kind": "file_exists",
            "path": path,
            "result": result_str,
        }),
        ExpectationKind::FileContains { path, expected, .. } => json!({
            "kind": "file_contains",
            "path": path,
            "expected": expected,
            "result": result_str,
        }),
        ExpectationKind::FileContentsEquals {
            path,
            expected_source,
            observation,
        } => {
            let mut obj = json!({
                "kind": "file_contents_equals",
                "path": path,
                "expected_source": expected_source_json(expected_source),
                "result": result_str,
            });
            if let ContentsEqualsObservation::Compared(comparison) = observation {
                obj["actual"] = stream_json(&comparison.actual);
                obj["expected"] = stream_json(&comparison.expected);
            }
            obj
        }
        ExpectationKind::FileTextEquals {
            path,
            expected_source,
            observation,
        } => {
            let mut obj = json!({
                "kind": "file_text_equals",
                "path": path,
                "expected_source": text_equals_expected_source_json(expected_source),
                "result": result_str,
            });
            if let ContentsEqualsObservation::Compared(comparison) = observation {
                obj["actual"] = stream_json(&comparison.actual);
                obj["expected"] = stream_json(&comparison.expected);
            }
            obj
        }
        ExpectationKind::StdoutContentsEquals {
            expected_source,
            comparison,
        } => json!({
            "kind": "stdout_contents_equals",
            "expected_source": expected_source_json(expected_source),
            "actual": stream_json(&comparison.actual),
            "expected": stream_json(&comparison.expected),
            "result": result_str,
        }),
        ExpectationKind::StderrContentsEquals {
            expected_source,
            comparison,
        } => json!({
            "kind": "stderr_contents_equals",
            "expected_source": expected_source_json(expected_source),
            "actual": stream_json(&comparison.actual),
            "expected": stream_json(&comparison.expected),
            "result": result_str,
        }),
        ExpectationKind::DirExists { path, .. } => json!({
            "kind": "dir_exists",
            "path": path,
            "result": result_str,
        }),
        ExpectationKind::DirContains {
            path,
            expected_entry,
            ..
        } => json!({
            "kind": "dir_contains",
            "path": path,
            "expected_entry": expected_entry,
            "result": result_str,
        }),
        ExpectationKind::Logical { operator, children } => json!({
            "kind": "logical",
            "operator": operator.keyword(),
            "children": children.iter().map(expectation_result_json).collect::<Vec<_>>(),
            "result": result_str,
        }),
    };
    if let Some(code) = e.failure_diagnostic_code() {
        value["diagnostic_code"] = json!(code.as_str());
    }
    value
}

fn case_json(case: &CaseResult) -> serde_json::Value {
    let (status, message, diagnostic_code, error_step_index): (
        &str,
        Option<&str>,
        Option<&str>,
        Option<usize>,
    ) = match &case.status {
        CaseStatus::Pass => ("pass", None, None, None),
        CaseStatus::Fail => ("fail", None, None, None),
        CaseStatus::ScriptError(err) => (
            "script_error",
            Some(err.message.as_str()),
            err.diagnostic_code.map(|c| c.as_str()),
            err.step_index,
        ),
        CaseStatus::RuntimeError(err) => (
            "runtime_error",
            Some(err.message.as_str()),
            err.diagnostic_code.map(|c| c.as_str()),
            err.step_index,
        ),
    };

    let actions: Vec<serde_json::Value> = case
        .actions
        .iter()
        .enumerate()
        .map(|(i, a)| action_json(i, a))
        .collect();

    let assertion_blocks: Vec<serde_json::Value> = case
        .assertion_blocks
        .iter()
        .map(|block| {
            let expectations: Vec<serde_json::Value> = block
                .expectations
                .iter()
                .map(expectation_result_json)
                .collect();
            json!({
                "step_index": block.step_index,
                "expectations": expectations,
                "result": if block.has_failures() { "fail" } else { "pass" }
            })
        })
        .collect();

    let mut obj = json!({
        "name": case.name,
        "status": status,
        "actions": actions,
        "assertion_blocks": assertion_blocks
    });

    if let Some(path) = &case.source_path {
        obj["source_path"] = json!(path.display().to_string());
    }

    if let Some(msg) = message {
        obj["message"] = json!(msg);
    }

    if let Some(code) = diagnostic_code {
        obj["diagnostic_code"] = json!(code);
    }

    if let Some(idx) = error_step_index {
        obj["step_index"] = json!(idx);
    }

    obj
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn empty_result() -> ExecutionReport {
        ExecutionReport {
            cases: vec![],
            file_errors: vec![],
        }
    }

    #[test]
    fn run_id_rejects_empty() {
        assert_eq!(RunId::new("").unwrap_err(), RunIdError::Empty);
    }

    #[test]
    fn run_id_rejects_path_separator() {
        assert!(matches!(
            RunId::new("a/b").unwrap_err(),
            RunIdError::ContainsPathSeparator(_)
        ));
        assert!(matches!(
            RunId::new("a\\b").unwrap_err(),
            RunIdError::ContainsPathSeparator(_)
        ));
    }

    #[test]
    fn run_id_rejects_dot_segments() {
        assert!(matches!(
            RunId::new(".").unwrap_err(),
            RunIdError::ReservedSegment(_)
        ));
        assert!(matches!(
            RunId::new("..").unwrap_err(),
            RunIdError::ReservedSegment(_)
        ));
    }

    #[test]
    fn run_id_rejects_control_characters() {
        assert!(matches!(
            RunId::new("a\nb").unwrap_err(),
            RunIdError::ContainsControlChar(_)
        ));
    }

    #[test]
    fn run_id_accepts_ordinary_name() {
        let id = RunId::new("file-assertion-selftest").unwrap();
        assert_eq!(id.as_str(), "file-assertion-selftest");
    }

    #[test]
    fn for_fixed_run_writes_to_named_run_directory() {
        let base = TempDir::new().unwrap();
        let run_id = RunId::new("fixed-run").unwrap();
        let writer = ArtifactWriter::for_fixed_run(base.path(), &run_id).unwrap();
        writer.write(&empty_result()).unwrap();

        assert!(base.path().join("runs/fixed-run/result.json").is_file());
    }

    #[test]
    fn for_fixed_run_rejects_existing_run_directory() {
        let base = TempDir::new().unwrap();
        let run_id = RunId::new("fixed-run").unwrap();
        ArtifactWriter::for_fixed_run(base.path(), &run_id)
            .unwrap()
            .write(&empty_result())
            .unwrap();

        let err = ArtifactWriter::for_fixed_run(base.path(), &run_id).unwrap_err();
        assert!(matches!(err, ArtifactWriterError::RunDirectoryExists(_)));
    }

    #[test]
    fn write_captures_action_stdout_and_stderr_as_artifact_files() {
        let base = TempDir::new().unwrap();
        let run_id = RunId::new("captured-output").unwrap();
        let writer = ArtifactWriter::for_fixed_run(base.path(), &run_id).unwrap();

        let case = CaseResult {
            name: "one action".to_string(),
            source_path: None,
            status: CaseStatus::Pass,
            actions: vec![ActionResult {
                command: "echo hello".to_string(),
                exit_code: 0,
                stdout: b"hello\n".to_vec(),
                stderr: b"".to_vec(),
                shim_invocations: vec![],
                shim_event_parse_warnings: vec![],
            }],
            assertion_blocks: vec![],
            side_effects_executed: 0,
        };
        writer
            .write(&ExecutionReport {
                cases: vec![case],
                file_errors: vec![],
            })
            .unwrap();

        let action_dir = writer.run_dir().join(test_id(0)).join(action_id(0));
        assert_eq!(
            std::fs::read(action_dir.join("stdout.bin")).unwrap(),
            b"hello\n"
        );
        assert_eq!(std::fs::read(action_dir.join("stderr.bin")).unwrap(), b"");
    }

    use crate::model::LogicalOperator;
    use crate::result::AssertionBlockResult;

    #[test]
    fn logical_expectation_result_renders_operator_and_children() {
        let block = AssertionBlockResult {
            step_index: 0,
            checkpoint_action_index: Some(0),
            expectations: vec![ExpectationResult {
                kind: ExpectationKind::Logical {
                    operator: LogicalOperator::Any,
                    children: vec![
                        ExpectationResult {
                            kind: ExpectationKind::Exit {
                                expected: 1,
                                actual: 0,
                            },
                            passed: false,
                        },
                        ExpectationResult {
                            kind: ExpectationKind::Exit {
                                expected: 0,
                                actual: 0,
                            },
                            passed: true,
                        },
                    ],
                },
                passed: true,
            }],
        };
        let case = CaseResult {
            name: "any exit".to_string(),
            source_path: None,
            status: CaseStatus::Pass,
            actions: vec![],
            assertion_blocks: vec![block],
            side_effects_executed: 0,
        };

        let json = case_json(&case);
        let expectation = &json["assertion_blocks"][0]["expectations"][0];
        assert_eq!(expectation["kind"], "logical");
        assert_eq!(expectation["operator"], "any");
        assert_eq!(expectation["result"], "pass");
        assert_eq!(expectation["children"][0]["kind"], "exit");
        assert_eq!(expectation["children"][0]["result"], "fail");
        assert_eq!(expectation["children"][1]["result"], "pass");
    }
}
