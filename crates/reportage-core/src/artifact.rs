use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::result::ExecutionReport;
use crate::run_result::build_run_result_document;

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
/// See docs/planning/TBD.md — "Self-test run ID control".
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

    /// Writes the artifact bundle for `result`: the canonical `result.json` manifest plus the captured-output evidence files it references.
    /// See [`crate::run_result::build_run_result_document`] and `spec/artifacts/run-result/schema.json`.
    pub fn write(&self, result: &ExecutionReport) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.run_dir)?;
        let value = build_run_result_document(result);
        let json = serde_json::to_string_pretty(&value)
            .expect("result JSON serialization should not fail");
        std::fs::write(self.run_dir.join("result.json"), json)?;
        self.write_captured_output(result)?;
        Ok(())
    }

    /// Writes each action's captured stdout/stderr as raw bytes under
    /// `<run-dir>/<test_id>/<action_id>/{stdout,stderr}.bin`.
    ///
    /// This is the artifact-file side of the "captured stdout/stderr are never inlined" policy: the `result.json` manifest and the `--format=json` document reference these files by relative path (`artifactRef`) instead of embedding raw bytes.
    /// See `crate::run_result` and docs/adr/20260708T130500Z_artifact-run-result-canonical-manifest.md.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::result::{ActionResult, CaseResult, CaseStatus};
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
}
