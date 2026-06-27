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
    pub status: CaseStatus,
    pub actions: Vec<ActionResult>,
    pub assertion_blocks: Vec<AssertionBlockResult>,
}

/// The result of a complete script run (all concrete cases).
#[derive(Debug)]
pub struct RunResult {
    pub cases: Vec<CaseResult>,
}

impl RunResult {
    /// Process exit code for the run.
    ///
    /// Severity order: 3 (runtime) > 2 (script error) > 1 (assertion failure) > 0 (pass).
    /// See docs/exit-codes.md for the full table and precedence rule.
    pub fn exit_code(&self) -> i32 {
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
