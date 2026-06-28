use std::path::PathBuf;

use crate::model::Script;

/// The captured output of a single `$` action step.
///
/// Produced by the executor and stored in the checkpoint as the last action result.
/// Also recorded in `CaseResult` for artifact output.
#[derive(Debug, Clone)]
pub struct ActionResult {
    pub command: String,
    // i32 rather than u8: the OS returns None when a process is killed by a signal,
    // which the executor maps to -1. See executor::execute_action.
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// The kind and actual vs. expected values of a single evaluated expectation.
#[derive(Debug)]
pub enum ExpectationKind {
    Exit { expected: u8, actual: i32 },
    StdoutContains { expected: String, actual: String },
    StderrContains { expected: String, actual: String },
    StdoutEmpty { actual: String },
    StderrEmpty { actual: String },
}

/// The result of evaluating one expectation within an assertion block.
///
/// Failures are reported per expectation, independently of other expectations
/// in the same block. See docs/semantics.md — Expectation.
#[derive(Debug)]
pub struct ExpectationResult {
    pub kind: ExpectationKind,
    pub passed: bool,
}

/// The result of evaluating one assertion block.
///
/// All expectations within the block are evaluated; `has_failures` reflects
/// whether any of them failed. See docs/semantics.md — Assertion block.
#[derive(Debug)]
pub struct AssertionBlockResult {
    /// Index of this assertion block's step within the case body.
    pub step_index: usize,
    pub expectations: Vec<ExpectationResult>,
}

impl AssertionBlockResult {
    /// Returns true if one or more expectations in this block failed.
    pub fn has_failures(&self) -> bool {
        self.expectations.iter().any(|e| !e.passed)
    }
}

/// The outcome of a concrete case execution.
#[derive(Debug)]
pub enum CaseStatus {
    Pass,
    Fail,
    /// A structural problem with the test script itself: empty assertion block,
    /// process expectation at the initial checkpoint, etc.
    ScriptError(String),
    /// A runtime infrastructure failure: cannot spawn the shell, I/O error, etc.
    RuntimeError(String),
}

/// The full result of one concrete case.
#[derive(Debug)]
pub struct CaseResult {
    pub name: String,
    /// Source file this case was loaded from. Set by the caller after evaluation.
    pub source_path: Option<PathBuf>,
    pub status: CaseStatus,
    pub actions: Vec<ActionResult>,
    pub assertion_blocks: Vec<AssertionBlockResult>,
}

/// Kind of a file-level error encountered during the pre-execution validation phase.
#[derive(Debug)]
pub enum FileErrorKind {
    ReadError(String),
    ParseError(String),
}

/// A file-level error: a test file that could not be read or parsed.
///
/// Collected during the pre-execution validation phase. If any file errors exist,
/// no `$` actions execute from any file. See docs/semantics.md — Validation phase.
#[derive(Debug)]
pub struct FileError {
    pub source_path: PathBuf,
    pub kind: FileErrorKind,
}

/// A test file that has been read and parsed successfully.
///
/// Used to carry the script to the evaluation phase after pre-execution validation.
pub struct ValidatedFile {
    pub source_path: PathBuf,
    pub script: Script,
}

/// The result of a complete run (all concrete cases from all files).
#[derive(Debug)]
pub struct RunResult {
    pub cases: Vec<CaseResult>,
    /// File-level errors from the pre-execution validation phase.
    pub file_errors: Vec<FileError>,
}

impl RunResult {
    /// Process exit code for the run.
    ///
    /// File-level errors produce exit code 2. Severity order within cases:
    /// 3 (runtime) > 2 (script error) > 1 (assertion failure) > 0 (pass).
    /// See docs/exit-codes.md for the full table and precedence rule.
    pub fn exit_code(&self) -> i32 {
        if !self.file_errors.is_empty() {
            return 2;
        }
        self.cases.iter().fold(0i32, |max, case| {
            let code = match &case.status {
                CaseStatus::Pass => 0,
                CaseStatus::Fail => 1,
                CaseStatus::ScriptError(_) => 2,
                CaseStatus::RuntimeError(_) => 3,
            };
            max.max(code)
        })
    }
}
