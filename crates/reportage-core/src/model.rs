//! Parsed representation of a reportage script.
//!
//! This module holds only the structure derived from source syntax.
//! Execution outputs and assertion results belong to the `result` module.
//! The checkpoint evidence context used during evaluation lives in the `evaluator` module.
//!
//! See docs/semantics.md for the conceptual model and the checkpoint-based assertion ADR.

/// A parsed reportage script (one test module file).
#[derive(Debug)]
pub struct Script {
    pub cases: Vec<Case>,
}

/// A test case with a name and an ordered sequence of steps.
///
/// Steps are executed in source order. Action steps and assertion blocks are not
/// separated into phases. See the checkpoint-based assertion ADR.
#[derive(Debug)]
pub struct Case {
    pub name: String,
    pub steps: Vec<Step>,
}

/// A step in a case body, executed in source order.
///
/// Source order is preserved. Action and assertion steps are never reordered
/// into phases. See docs/semantics.md — Action and Assertion block.
#[derive(Debug)]
pub enum Step {
    Action(ActionStep),
    AssertionBlock(AssertionBlock),
}

/// A shell-like action step (`$ ...`).
///
/// Executed by `sh -c`. On completion, produces an `ActionResult` that updates
/// the current checkpoint. See docs/semantics.md — Shell execution.
#[derive(Debug)]
pub struct ActionStep {
    pub command: String,
}

/// A checkpoint-level assertion block (`assert { ... }`).
///
/// This block verifies the current checkpoint. It is intentionally not modeled
/// as an assertion attached to the nearest action, so it can represent both
/// precondition assertions at the initial checkpoint and post-action assertions.
///
/// See docs/semantics.md — Assertion block and the checkpoint-based assertion ADR.
#[derive(Debug)]
pub struct AssertionBlock {
    expectations: Vec<Expectation>,
}

/// Error returned when constructing an `AssertionBlock` with invalid content.
#[derive(Debug, PartialEq)]
pub enum AssertionBlockError {
    /// An assertion block must contain at least one expectation.
    Empty,
}

impl AssertionBlock {
    /// Construct an `AssertionBlock`, rejecting empty blocks.
    ///
    /// An empty block (`assert { }`) is always a script error.
    pub fn new(expectations: Vec<Expectation>) -> Result<Self, AssertionBlockError> {
        if expectations.is_empty() {
            return Err(AssertionBlockError::Empty);
        }
        Ok(Self { expectations })
    }

    pub fn expectations(&self) -> &[Expectation] {
        &self.expectations
    }
}

/// An individual expected condition within an assertion block.
///
/// Each expectation is side-effect-free and declares its evidence requirement.
/// Evaluation result is reported per expectation, independently of other expectations.
///
/// See docs/semantics.md — Expectation and Evidence requirement.
#[derive(Debug)]
pub enum Expectation {
    Exit(ExitExpectation),
    // v0 parser produces only Exit, Stdout, Stderr, and Logical. The
    // remaining variants are defined for conceptual completeness; they are
    // not yet parsed or evaluated. See docs/TBD.md for planned additions.
    Stdout(OutputExpectation),
    Stderr(OutputExpectation),
    File(FileExpectation),
    Dir(DirExpectation),
    FileCount(FileCountExpectation),
    Jq(JqExpectation),
    /// Block-form logical composition (`not` / `all` / `any`) over nested
    /// expectation expressions. See docs/semantics.md — Logical composition
    /// and the accompanying ADR.
    Logical(LogicalExpectation),
}

impl Expectation {
    /// The evidence this expectation requires from the current checkpoint.
    ///
    /// Workspace evidence is available at the initial checkpoint.
    /// `LastActionResult`, `Stdout`, and `Stderr` are only available after
    /// a `$` action has run.
    ///
    /// For a logical composition, this is the evidence requirement of
    /// whichever child (if any) needs a last action result, so a composition
    /// wrapping a process expectation is still rejected at the initial
    /// checkpoint the same way a bare process expectation is.
    pub fn required_evidence(&self) -> EvidenceRequirement {
        match self {
            Expectation::Exit(_) => EvidenceRequirement::LastActionResult,
            Expectation::Stdout(_) => EvidenceRequirement::Stdout,
            Expectation::Stderr(_) => EvidenceRequirement::Stderr,
            Expectation::File(_) | Expectation::Dir(_) | Expectation::FileCount(_) => {
                EvidenceRequirement::Workspace
            }
            Expectation::Jq(j) => match j.source {
                OutputSource::Stdout => EvidenceRequirement::Stdout,
                OutputSource::Stderr => EvidenceRequirement::Stderr,
            },
            Expectation::Logical(l) => l
                .children()
                .iter()
                .map(Expectation::required_evidence)
                .find(EvidenceRequirement::needs_action_result)
                .unwrap_or(EvidenceRequirement::Workspace),
        }
    }
}

/// The `not` / `all` / `any` operator of a logical composition expectation.
///
/// `and` / `or` are deliberately not defined as aliases for `all` / `any`;
/// v0's canonical logical composition syntax is limited to these three. See
/// the accompanying ADR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalOperator {
    Not,
    All,
    Any,
}

impl LogicalOperator {
    /// The block keyword that introduces this operator in source syntax.
    pub const fn keyword(self) -> &'static str {
        match self {
            LogicalOperator::Not => "not",
            LogicalOperator::All => "all",
            LogicalOperator::Any => "any",
        }
    }
}

/// A block-form logical composition expectation: `not { ... }`, `all { ... }`,
/// or `any { ... }`.
///
/// `children` holds the expectation expressions inside the block in source
/// order, and may nest further `Logical` expectations. A `not` block with
/// multiple children negates their implicit-`all` grouping, not each child
/// individually: `not { A B }` evaluates as `not(all(A, B))`, never as
/// `not(A) and not(B)`. See docs/semantics.md — Logical composition.
#[derive(Debug)]
pub struct LogicalExpectation {
    operator: LogicalOperator,
    children: Vec<Expectation>,
}

/// Error returned when constructing a `LogicalExpectation` with invalid content.
#[derive(Debug, PartialEq)]
pub enum LogicalExpectationError {
    /// A `not` / `all` / `any` block must contain at least one expectation
    /// expression. The grammar accepts an empty body so Reportage can reject
    /// it as a semantic error rather than a generic syntax error; callers
    /// (the parser) are expected to have already turned this into a
    /// `ParseError` before reaching this constructor. See
    /// docs/semantic-diagnostics.md.
    Empty,
}

impl LogicalExpectation {
    /// Construct a `LogicalExpectation`, rejecting an empty child list.
    pub fn new(
        operator: LogicalOperator,
        children: Vec<Expectation>,
    ) -> Result<Self, LogicalExpectationError> {
        if children.is_empty() {
            return Err(LogicalExpectationError::Empty);
        }
        Ok(Self { operator, children })
    }

    pub fn operator(&self) -> LogicalOperator {
        self.operator
    }

    pub fn children(&self) -> &[Expectation] {
        &self.children
    }
}

/// The evidence an expectation needs from the current checkpoint.
///
/// `Workspace` is available at the initial checkpoint. `LastActionResult`,
/// `Stdout`, and `Stderr` require a preceding `$` action in the same case.
#[derive(Debug, PartialEq)]
pub enum EvidenceRequirement {
    /// Requires only the current workspace state (valid at the initial checkpoint).
    Workspace,
    /// Requires the last action result (exit code). Script error if no action has run.
    LastActionResult,
    /// Requires stdout from the last action. Script error if no action has run.
    Stdout,
    /// Requires stderr from the last action. Script error if no action has run.
    Stderr,
}

impl EvidenceRequirement {
    /// Returns true if this requirement needs a preceding `$` action result.
    pub fn needs_action_result(&self) -> bool {
        matches!(
            self,
            EvidenceRequirement::LastActionResult
                | EvidenceRequirement::Stdout
                | EvidenceRequirement::Stderr
        )
    }
}

/// Exit status expectation: `exit <code>`.
#[derive(Debug)]
pub struct ExitExpectation {
    pub expected: u8,
}

/// stdout / stderr matcher expectation.
#[derive(Debug)]
pub struct OutputExpectation {
    pub matcher: OutputMatcher,
}

/// Matcher for stdout or stderr output.
#[derive(Debug)]
pub enum OutputMatcher {
    Empty,
    Contains(String),
    NotContains(String),
    Matches(String),
}

/// File existence / content expectation.
#[derive(Debug)]
pub struct FileExpectation {
    pub path: String,
    pub matcher: FileMatcher,
}

/// Matcher for file expectations.
#[derive(Debug)]
pub enum FileMatcher {
    Exists,
    NotExists,
    Contains(String),
    Matches(String),
}

/// Directory existence expectation.
#[derive(Debug)]
pub struct DirExpectation {
    pub path: String,
    pub matcher: DirMatcher,
}

/// Matcher for directory expectations.
#[derive(Debug)]
pub enum DirMatcher {
    Exists,
    NotExists,
}

/// File count expectation: `file-count <glob> <op> <n>`.
#[derive(Debug)]
pub struct FileCountExpectation {
    pub glob: String,
    pub op: CountOp,
    pub count: usize,
}

/// Comparison operator for file count expectations.
#[derive(Debug)]
pub enum CountOp {
    Eq,
    Gte,
}

/// jq-based structured output expectation.
#[derive(Debug)]
pub struct JqExpectation {
    pub source: OutputSource,
    pub expression: String,
}

/// Which output stream a jq expectation evaluates.
#[derive(Debug)]
pub enum OutputSource {
    Stdout,
    Stderr,
}
