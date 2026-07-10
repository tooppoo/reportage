use std::path::PathBuf;

use crate::diagnostic::{DiagnosticCode, DiagnosticLocation};
use crate::model::{LogicalOperator, Script};
use crate::shim_event::ShimInvocationEvent;

/// The captured output of a single `$` action step.
///
/// Produced by the executor and stored in the checkpoint as the last action result.
/// Also recorded in `CaseResult` for artifact output.
#[derive(Debug, Clone)]
pub struct ActionResult {
    pub command: String,
    // i32 rather than u8: the OS returns None when a process is killed by a signal, which the executor maps to -1.
    // See executor::execute_action.
    pub exit_code: i32,
    // Raw process output bytes, not decoded text. See docs/semantics.md and the accompanying ADR
    // on raw byte semantics for stdout/stderr: non-UTF-8 output must survive unmodified through
    // capture and evaluation; lossy decoding is confined to display layers (CLI, artifact JSON's
    // optional `text` helper view).
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// Shim invocation events collected from the action-scoped event directory.
    /// Empty when no protocol-compliant shim was observed during this action.
    pub shim_invocations: Vec<ShimInvocationEvent>,
    /// Runner-level warnings about shim event file parsing. Surfaced as diagnostics.
    /// Does not affect exit_code, stdout, or stderr.
    pub shim_event_parse_warnings: Vec<String>,
}

/// The kind and actual vs. expected values of a single evaluated expectation.
#[derive(Debug)]
pub enum ExpectationKind {
    Exit {
        expected: u8,
        actual: i32,
    },
    StdoutContains {
        expected: String,
        actual: Vec<u8>,
    },
    StderrContains {
        expected: String,
        actual: Vec<u8>,
    },
    StdoutEmpty {
        actual: Vec<u8>,
    },
    StderrEmpty {
        actual: Vec<u8>,
    },
    FileExists {
        path: String,
        observation: FileExistsObservation,
    },
    FileContains {
        path: String,
        expected: String,
        observation: FileContentObservation,
    },
    FileContentsEquals {
        path: String,
        expected_source: ContentsEqualsExpectedSource,
        observation: ContentsEqualsObservation,
    },
    StdoutContentsEquals {
        expected_source: ContentsEqualsExpectedSource,
        comparison: ContentsEqualsComparison,
    },
    StderrContentsEquals {
        expected_source: ContentsEqualsExpectedSource,
        comparison: ContentsEqualsComparison,
    },
    FileTextEquals {
        path: String,
        expected_source: TextEqualsExpectedSource,
        observation: ContentsEqualsObservation,
    },
    StdoutTextEquals {
        expected_source: TextEqualsExpectedSource,
        comparison: ContentsEqualsComparison,
    },
    StderrTextEquals {
        expected_source: TextEqualsExpectedSource,
        comparison: ContentsEqualsComparison,
    },
    DirExists {
        path: String,
        observation: DirExistsObservation,
    },
    DirContains {
        path: String,
        expected_entry: String,
        observation: DirContainsObservation,
    },
    /// A `not` / `all` / `any` logical composition.
    /// `children` holds each nested expectation's own result (independently evaluated, never flipped by a `not`), so which child passed or failed is never lost — see docs/semantics.md — Logical composition.
    Logical {
        operator: LogicalOperator,
        children: Vec<ExpectationResult>,
    },
}

impl ExpectationKind {
    /// The stable diagnostic code for a failing expectation of this kind, if one is defined.
    /// Passing expectations, and expectation kinds without a dedicated diagnostic code, return `None`.
    ///
    /// `Exit` / `StdoutContains` / `StderrContains` / `StdoutEmpty` / `StderrEmpty` are not
    /// covered here: unlike `File*`/`Dir*`, they carry no independent "observation" enum that
    /// already encodes pass/fail, so determining their code requires the sibling
    /// `ExpectationResult.passed` computed during evaluation. See
    /// `ExpectationResult::failure_diagnostic_code`.
    ///
    /// See docs/semantic-diagnostics.md.
    pub fn failure_diagnostic_code(&self) -> Option<DiagnosticCode> {
        match self {
            ExpectationKind::FileExists { observation, .. } => match observation {
                FileExistsObservation::RegularFile => None,
                FileExistsObservation::Missing => Some(DiagnosticCode::AssertionFileExistsMissing),
                FileExistsObservation::NotRegularFile => {
                    Some(DiagnosticCode::AssertionFileExistsNotAFile)
                }
            },
            ExpectationKind::FileContains { observation, .. } => match observation {
                FileContentObservation::Found => None,
                FileContentObservation::NotFound => {
                    Some(DiagnosticCode::AssertionFileContainsMismatch)
                }
                FileContentObservation::Missing
                | FileContentObservation::NotRegularFile
                | FileContentObservation::Unreadable
                | FileContentObservation::NotUtf8 => {
                    Some(DiagnosticCode::AssertionFileContainsPreconditionUnmet)
                }
            },
            ExpectationKind::FileContentsEquals { observation, .. } => match observation {
                ContentsEqualsObservation::Compared(comparison) => match comparison.outcome {
                    ContentsEqualsOutcome::Match => None,
                    ContentsEqualsOutcome::Mismatch(_) => {
                        Some(DiagnosticCode::AssertionFileContentsEqualsMismatch)
                    }
                },
                ContentsEqualsObservation::ActualMissing => {
                    Some(DiagnosticCode::AssertionFileContentsEqualsActualMissing)
                }
                ContentsEqualsObservation::ActualNotRegularFile => {
                    Some(DiagnosticCode::AssertionFileContentsEqualsActualNotARegularFile)
                }
                ContentsEqualsObservation::ActualUnreadable => {
                    Some(DiagnosticCode::AssertionFileContentsEqualsActualUnreadable)
                }
            },
            ExpectationKind::StdoutContentsEquals { comparison, .. } => match comparison.outcome {
                ContentsEqualsOutcome::Match => None,
                ContentsEqualsOutcome::Mismatch(_) => {
                    Some(DiagnosticCode::AssertionStdoutContentsEqualsMismatch)
                }
            },
            ExpectationKind::StderrContentsEquals { comparison, .. } => match comparison.outcome {
                ContentsEqualsOutcome::Match => None,
                ContentsEqualsOutcome::Mismatch(_) => {
                    Some(DiagnosticCode::AssertionStderrContentsEqualsMismatch)
                }
            },
            ExpectationKind::FileTextEquals { observation, .. } => match observation {
                ContentsEqualsObservation::Compared(comparison) => match comparison.outcome {
                    ContentsEqualsOutcome::Match => None,
                    ContentsEqualsOutcome::Mismatch(_) => {
                        Some(DiagnosticCode::AssertionFileTextEqualsMismatch)
                    }
                },
                ContentsEqualsObservation::ActualMissing => {
                    Some(DiagnosticCode::AssertionFileTextEqualsActualMissing)
                }
                ContentsEqualsObservation::ActualNotRegularFile => {
                    Some(DiagnosticCode::AssertionFileTextEqualsActualNotARegularFile)
                }
                ContentsEqualsObservation::ActualUnreadable => {
                    Some(DiagnosticCode::AssertionFileTextEqualsActualUnreadable)
                }
            },
            ExpectationKind::StdoutTextEquals { comparison, .. } => match comparison.outcome {
                ContentsEqualsOutcome::Match => None,
                ContentsEqualsOutcome::Mismatch(_) => {
                    Some(DiagnosticCode::AssertionStdoutTextEqualsMismatch)
                }
            },
            ExpectationKind::StderrTextEquals { comparison, .. } => match comparison.outcome {
                ContentsEqualsOutcome::Match => None,
                ContentsEqualsOutcome::Mismatch(_) => {
                    Some(DiagnosticCode::AssertionStderrTextEqualsMismatch)
                }
            },
            ExpectationKind::DirExists { observation, .. } => match observation {
                DirExistsObservation::Directory => None,
                DirExistsObservation::Missing => Some(DiagnosticCode::AssertionDirExistsMissing),
                DirExistsObservation::NotADirectory => {
                    Some(DiagnosticCode::AssertionDirExistsNotADirectory)
                }
            },
            ExpectationKind::DirContains { observation, .. } => match observation {
                DirContainsObservation::Found => None,
                DirContainsObservation::EntryMissing => {
                    Some(DiagnosticCode::AssertionDirContainsEntryMissing)
                }
                DirContainsObservation::SubjectMissing => {
                    Some(DiagnosticCode::AssertionDirContainsSubjectMissing)
                }
                DirContainsObservation::SubjectNotADirectory => {
                    Some(DiagnosticCode::AssertionDirContainsSubjectNotADirectory)
                }
                DirContainsObservation::SubjectUnreadable => {
                    Some(DiagnosticCode::AssertionDirContainsSubjectUnreadable)
                }
            },
            _ => None,
        }
    }
}

/// What was observed on the filesystem for a `file <"path"> exists` expectation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileExistsObservation {
    /// `path` resolves (following symlinks) to a regular file.
    RegularFile,
    /// `path` resolves to something other than a regular file (e.g. a directory).
    NotRegularFile,
    /// `path` does not exist (including a broken symlink).
    Missing,
}

/// What was observed on the filesystem for a `file <"path"> contains "<text>"` expectation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileContentObservation {
    /// `path` is a readable UTF-8 regular file whose content contains the expected substring.
    Found,
    /// `path` is a readable UTF-8 regular file, but its content does not contain the expected substring.
    NotFound,
    /// `path` does not exist.
    Missing,
    /// `path` exists but is not a regular file (e.g. a directory).
    NotRegularFile,
    /// `path` is a regular file but could not be read.
    Unreadable,
    /// `path` is a regular file, but its content is not valid UTF-8.
    NotUtf8,
}

/// Where a `contents_equals` expected value came from, for display purposes only.
///
/// Carries the raw path/reference string as written in the script, not a resolved
/// filesystem path: a `Fixture` reference resolves relative to the `*.repor` file's
/// directory, which is not otherwise visible in an `ExpectationKind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentsEqualsExpectedSource {
    /// A `<"...">` workspace path literal.
    Workspace(String),
    /// An `@"..."` fixture reference literal.
    Fixture(String),
}

/// Where a `text_equals` expected value's `TextLiteral` source representation came from,
/// for display purposes only.
///
/// The runtime comparison always operates on the resolved `TextValue`'s UTF-8 bytes,
/// which are identical regardless of literal form; only diagnostic rendering may
/// differentiate on `Quoted` vs. `Heredoc` (e.g. to show heredoc body line numbers).
/// See docs/adr — text_equals evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextEqualsExpectedSource {
    /// A `"..."` string literal.
    Quoted(String),
    /// A ``` ... ``` heredoc literal, already dedented.
    Heredoc(String),
}

/// A completed byte-for-byte `contents_equals` comparison: the full actual and expected
/// byte buffers (kept for artifact evidence, exactly like `ExpectationKind::StdoutContains`
/// keeps full captured output), plus the resulting [`ContentsEqualsOutcome`].
///
/// CLI diagnostic rendering must never print `actual` / `expected` in full — only a
/// bounded, escaped window derived from [`ContentsMismatch`]. See docs/semantic-diagnostics.md.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentsEqualsComparison {
    pub actual: Vec<u8>,
    pub expected: Vec<u8>,
    pub outcome: ContentsEqualsOutcome,
}

impl ContentsEqualsComparison {
    /// Compares `actual` and `expected` byte-for-byte, with no normalization of any kind.
    pub fn compare(actual: Vec<u8>, expected: Vec<u8>) -> Self {
        let outcome = match first_byte_diff(&actual, &expected) {
            None => ContentsEqualsOutcome::Match,
            Some(offset) => ContentsEqualsOutcome::Mismatch(ContentsMismatch {
                actual_len: actual.len(),
                expected_len: expected.len(),
                first_diff_offset: offset,
            }),
        };
        ContentsEqualsComparison {
            actual,
            expected,
            outcome,
        }
    }
}

/// The outcome of a byte-for-byte `contents_equals` comparison. See docs/semantic-diagnostics.md.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentsEqualsOutcome {
    Match,
    Mismatch(ContentsMismatch),
}

/// Bounded facts about a `contents_equals` byte mismatch, sufficient to render a bounded,
/// escaped diagnostic without printing the full actual/expected byte buffers.
/// See docs/semantic-diagnostics.md — `contents_equals` mismatch diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentsMismatch {
    pub actual_len: usize,
    pub expected_len: usize,
    /// Byte offset of the first differing byte. When one buffer is a strict prefix of
    /// the other, this is the shorter buffer's length (the point truncation begins).
    pub first_diff_offset: usize,
}

/// The first byte offset at which `a` and `b` differ, or `None` if they are equal.
/// A length mismatch where one is a prefix of the other counts as differing at the
/// shorter buffer's length (e.g. a missing trailing newline).
fn first_byte_diff(a: &[u8], b: &[u8]) -> Option<usize> {
    let common_len = a.len().min(b.len());
    for i in 0..common_len {
        if a[i] != b[i] {
            return Some(i);
        }
    }
    if a.len() != b.len() {
        Some(common_len)
    } else {
        None
    }
}

/// What was observed for the actual side of a `file <"path"> contents_equals <expected>`
/// expectation, once the expected side is already known to be resolvable.
///
/// Unlike the expected side (a test-definition error when unavailable), a missing /
/// non-regular / unreadable actual `file` is the subject under test failing to produce
/// the expected output, so it is always an assertion failure. See docs/semantic-diagnostics.md.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentsEqualsObservation {
    Compared(ContentsEqualsComparison),
    /// The actual path does not exist.
    ActualMissing,
    /// The actual path exists but is not a regular file (e.g. a directory).
    ActualNotRegularFile,
    /// The actual path is a regular file but could not be read.
    ActualUnreadable,
}

/// What was observed on the filesystem for a `dir <"path"> exists` expectation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirExistsObservation {
    /// `path` resolves (following symlinks) to a directory.
    Directory,
    /// `path` resolves to something other than a directory (e.g. a regular file).
    NotADirectory,
    /// `path` does not exist (including a broken symlink).
    Missing,
}

/// What was observed on the filesystem for a `dir <"path"> contains "<name>"` expectation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirContainsObservation {
    /// `path` is a directory containing an entry named `name`, directly under `path`.
    Found,
    /// `path` is a directory, but it has no entry named `name` directly under it.
    EntryMissing,
    /// `path` does not exist.
    SubjectMissing,
    /// `path` exists but is not a directory (e.g. a regular file).
    SubjectNotADirectory,
    /// `path` is a directory, but its entries could not be read (e.g. a permission error).
    SubjectUnreadable,
}

/// The result of evaluating one expectation within an assertion block.
///
/// Failures are reported per expectation, independently of other expectations in the same block.
/// See docs/semantics.md — Expectation.
#[derive(Debug)]
pub struct ExpectationResult {
    pub kind: ExpectationKind,
    pub passed: bool,
}

impl ExpectationResult {
    /// The stable diagnostic code for this expectation, if it failed and one is defined.
    ///
    /// Delegates to [`ExpectationKind::failure_diagnostic_code`] for kinds whose own
    /// observation already encodes pass/fail (`File*`/`Dir*`). For `Exit` / `StdoutContains` /
    /// `StderrContains` / `StdoutEmpty` / `StderrEmpty`, which carry no such observation, this
    /// gates on `passed` directly instead of re-deriving pass/fail from `expected`/`actual`
    /// (that comparison already happened once, in the evaluator).
    pub fn failure_diagnostic_code(&self) -> Option<DiagnosticCode> {
        if self.passed {
            return None;
        }
        match &self.kind {
            ExpectationKind::Exit { .. } => Some(DiagnosticCode::AssertionExitMismatch),
            ExpectationKind::StdoutContains { .. } => {
                Some(DiagnosticCode::AssertionStdoutContainsMismatch)
            }
            ExpectationKind::StderrContains { .. } => {
                Some(DiagnosticCode::AssertionStderrContainsMismatch)
            }
            ExpectationKind::StdoutEmpty { .. } => Some(DiagnosticCode::AssertionStdoutNotEmpty),
            ExpectationKind::StderrEmpty { .. } => Some(DiagnosticCode::AssertionStderrNotEmpty),
            other => other.failure_diagnostic_code(),
        }
    }
}

/// The result of evaluating one assertion block.
///
/// All expectations within the block are evaluated; `has_failures` reflects whether any of them failed.
/// See docs/semantics.md — Assertion block.
#[derive(Debug)]
pub struct AssertionBlockResult {
    /// Index of this assertion block's step within the case body.
    pub step_index: usize,
    pub expectations: Vec<ExpectationResult>,
    /// Index into the case's `actions` of the checkpoint this block evaluated process
    /// expectations against, i.e. how many `$` actions had run by this point minus one.
    /// `None` means the block ran at the initial checkpoint (no action has run yet), which
    /// is only possible when the block has no process expectations (`exit`/`stdout`/`stderr`)
    /// — see `evaluate_case`'s "assertion block ... uses a process expectation ... but no '$'
    /// action has run yet" check.
    pub checkpoint_action_index: Option<usize>,
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
    /// A structural problem with the test script itself: empty assertion block, process expectation at the initial checkpoint, etc.
    ScriptError(ScriptError),
    /// A runtime infrastructure failure: cannot spawn the shell, cannot create the case workspace, a side-effecting step (`write`) failed at runtime, etc.
    RuntimeError(RuntimeError),
}

/// Structured detail for a [`CaseStatus::ScriptError`].
///
/// Covers both parse-domain problems detected at evaluation time (e.g. a missing
/// assertion block, which reuses `DiagnosticCode::ParseMissingAssertionBlock`) and
/// semantic errors (e.g. a path policy violation, or a process expectation used
/// before any action has run). `diagnostic_code` and `step_index` let callers
/// (CLI rendering, the `--format=json` renderer) surface the failure structurally
/// instead of parsing it back out of `message`.
#[derive(Debug)]
pub struct ScriptError {
    pub message: String,
    /// The stable diagnostic code for this failure, when one is defined.
    pub diagnostic_code: Option<DiagnosticCode>,
    /// The case-body step index this failure occurred at, when applicable.
    /// `None` for case-level problems not tied to one step (e.g. a missing assertion block).
    pub step_index: Option<usize>,
}

/// Structured detail for a [`CaseStatus::RuntimeError`].
///
/// A runtime error can originate from a side-effecting step with its own stable
/// diagnostic code (e.g. `step.write.target_exists`); `diagnostic_code` and
/// `step_index` let callers (CLI rendering, the `result.json` artifact) surface
/// that structurally instead of parsing it back out of `message`.
#[derive(Debug)]
pub struct RuntimeError {
    pub message: String,
    /// The stable diagnostic code for this failure, when one is defined.
    /// `None` for infrastructure failures that predate a diagnostic code
    /// (e.g. shell spawn failure, workspace creation failure).
    pub diagnostic_code: Option<DiagnosticCode>,
    /// The case-body step index this failure occurred at, when applicable.
    pub step_index: Option<usize>,
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
    /// Number of side-effecting steps (`write`, etc.) that ran to completion
    /// before this case finished. See [`RunSummary::steps_executed`].
    pub side_effects_executed: usize,
}

/// Kind of a file-level error encountered during the pre-execution validation phase.
#[derive(Debug)]
pub enum FileErrorKind {
    /// The file could not be read (e.g. missing, permission denied). Carries no diagnostic
    /// code: this predates parsing, so no `parse.*` / `semantic.*` code applies.
    ReadError(String),
    /// The file was read but failed to parse. `diagnostic_code` is the stable code the parser
    /// attached to this failure (see `parser::ParseError::code`). `location` is the parser's own
    /// line/column for the failure (see `parser::ParseError::to_diagnostic`); every `ParseError`
    /// variant carries one, unlike the evaluator-side `ScriptError` / `RuntimeError` below, whose
    /// source ranges are not yet tracked.
    ParseError {
        message: String,
        diagnostic_code: DiagnosticCode,
        location: Option<DiagnosticLocation>,
    },
}

/// A file-level error: a test file that could not be read or parsed.
///
/// Collected during the pre-execution validation phase.
/// If any file errors exist, no `$` actions execute from any file.
/// See docs/execution-model.md — Suite pre-execution validation.
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
pub struct ExecutionReport {
    pub cases: Vec<CaseResult>,
    /// File-level errors from the pre-execution validation phase.
    pub file_errors: Vec<FileError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunSummary {
    pub noop: bool,
    pub cases_total: usize,
    pub cases_passed: usize,
    pub cases_failed: usize,
    pub steps_executed: usize,
    pub assertions_total: usize,
}

impl ExecutionReport {
    /// A no-op run has valid selected input but no concrete cases to execute.
    pub fn is_noop(&self) -> bool {
        self.file_errors.is_empty() && self.cases.is_empty()
    }

    pub fn summary(&self) -> RunSummary {
        let cases_total = self.cases.len();
        let cases_passed = self
            .cases
            .iter()
            .filter(|case| matches!(case.status, CaseStatus::Pass))
            .count();
        let cases_failed = cases_total.saturating_sub(cases_passed);
        let steps_executed = self
            .cases
            .iter()
            .map(|case| case.actions.len() + case.side_effects_executed)
            .sum();
        let assertions_total = self
            .cases
            .iter()
            .flat_map(|case| &case.assertion_blocks)
            .map(|block| block.expectations.len())
            .sum();

        RunSummary {
            noop: self.is_noop(),
            cases_total,
            cases_passed,
            cases_failed,
            steps_executed,
            assertions_total,
        }
    }

    /// Process exit code for the run.
    ///
    /// File-level errors produce exit code 2.
    /// Severity order within cases: 3 (runtime) > 2 (script error) > 1 (assertion failure) > 0 (pass).
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
