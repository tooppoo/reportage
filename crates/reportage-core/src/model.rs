//! Parsed representation of a reportage script.
//!
//! This module holds only the structure derived from source syntax.
//! Execution outputs and assertion results belong to the `result` module.
//! The checkpoint evidence context used during evaluation lives in the `evaluator` module.
//!
//! See docs/reference/execution-model.md for the conceptual model and the checkpoint-based assertion ADR.

/// A parsed reportage script (one test module file).
#[derive(Debug)]
pub struct Script {
    /// Module-level case-local setup, replayed inside each concrete case's
    /// isolated workspace before the case body runs; `None` when the module
    /// declares no `before_each` block.
    pub before_each: Option<BeforeEach>,
    pub cases: Vec<Case>,
}

/// A module-level `before_each { ... }` block: case-local setup replayed
/// inside each concrete case's isolated workspace, after the workspace is
/// created and before the case body's first step.
///
/// Holds [`SideEffectingStep`]s only, so an action step or assertion block
/// is unrepresentable here by construction — the write-only policy is
/// structural, not a validation pass a future caller could forget to run.
/// `before_each` is never shared state: each concrete case replays these
/// steps against its own fresh workspace.
/// See docs/reference/execution-model.md — `before_each`, and the
/// accompanying ADR.
#[derive(Debug)]
pub struct BeforeEach {
    steps: Vec<SideEffectingStep>,
}

/// Error returned when constructing a `BeforeEach` with invalid content.
#[derive(Debug, PartialEq)]
pub enum BeforeEachError {
    /// A `before_each` block must contain at least one step.
    /// The grammar accepts an empty body so Reportage can reject it as an
    /// actionable parse-domain error (`parse.before_each.empty`) rather than
    /// a generic syntax error; callers (the parser) are expected to have
    /// already turned this into a `ParseError` before reaching this
    /// constructor.
    Empty,
}

impl BeforeEach {
    /// Construct a `BeforeEach`, rejecting an empty step list.
    pub fn new(steps: Vec<SideEffectingStep>) -> Result<Self, BeforeEachError> {
        if steps.is_empty() {
            return Err(BeforeEachError::Empty);
        }
        Ok(Self { steps })
    }

    pub fn steps(&self) -> &[SideEffectingStep] {
        &self.steps
    }
}

/// A test case with a name and an ordered sequence of steps.
///
/// Steps are executed in source order.
/// Action steps and assertion blocks are not separated into phases.
/// See the checkpoint-based assertion ADR.
#[derive(Debug)]
pub struct Case {
    pub name: String,
    pub steps: Vec<Step>,
}

/// A step in a case body, executed in source order.
///
/// Source order is preserved.
/// Action and assertion steps are never reordered into phases.
/// See docs/reference/execution-model.md — Action, and docs/reference/semantics.md — Assertion block.
#[derive(Debug)]
pub enum Step {
    Action(ActionStep),
    AssertionBlock(AssertionBlock),
    /// A step that changes workspace state rather than executing an action
    /// or verifying a checkpoint. See docs/reference/semantics.md — Write step.
    SideEffect(SideEffectingStep),
}

/// A step that changes workspace state as a side effect, rather than
/// executing an action (`$ ...`) or verifying a checkpoint (`assert { ... }`).
///
/// A side-effecting step's failure is a runtime step error, never an
/// assertion failure: there is no expectation being compared against
/// evidence, only an operation that either succeeds or does not.
/// See docs/reference/semantics.md — Write step, and the accompanying ADR.
#[derive(Debug)]
pub enum SideEffectingStep {
    WriteFile(WriteFileStep),
}

/// A `write <"path"> <text_literal>` step: writes a text_literal's resolved
/// content (dedented, in the heredoc-literal case) to a file in the concrete
/// case workspace.
///
/// Create-only: rejected at runtime if `path` already exists.
/// See docs/reference/semantics.md — Write step.
#[derive(Debug)]
pub struct WriteFileStep {
    pub path: WorkspacePath,
    pub content: TextLiteral,
}

/// A path known to be safe to resolve against a concrete case workspace root.
///
/// Constructed only via [`WorkspacePath::parse`], which rejects empty paths,
/// absolute paths, and `.` / `..` path segments. A `WorkspacePath` never
/// refers to the repository root; it is always relative to the workspace
/// the current concrete case is running in.
/// See docs/adr — write step / workspace path domain type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePath(String);

/// Error returned when a raw path string fails `WorkspacePath` validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspacePathError {
    /// The path was empty.
    Empty,
    /// The path started with `/`.
    Absolute,
    /// The path contained a `.` or `..` segment.
    DotSegment,
}

impl WorkspacePath {
    /// Validates `raw` against the workspace path safety policy and, if
    /// valid, returns a `WorkspacePath` wrapping it.
    ///
    /// Rejects: empty paths, absolute paths (leading `/`), and `.` / `..`
    /// path segments. This centralizes path safety validation so every
    /// caller (today, only the `write` step) shares the same rejection
    /// rule, and future callers cannot bypass it by holding a raw `String`.
    pub fn parse(raw: &str) -> Result<Self, WorkspacePathError> {
        if raw.is_empty() {
            return Err(WorkspacePathError::Empty);
        }
        if raw.starts_with('/') {
            return Err(WorkspacePathError::Absolute);
        }
        for segment in raw.split('/') {
            if segment == "." || segment == ".." {
                return Err(WorkspacePathError::DotSegment);
            }
        }
        Ok(Self(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A fixture path known to be lexically safe to resolve against the directory
/// containing the referencing `*.repor` source file.
///
/// Constructed only via [`FixtureReference::parse`], which rejects empty
/// paths, absolute paths, and `.` / `..` path segments — the same lexical
/// policy as [`WorkspacePath`]. Lexical safety alone cannot prevent an escape
/// via a symlink, so a `FixtureReference` additionally requires a
/// filesystem-aware containment check before its target is read; see
/// `fixture::resolve_fixture_source`.
/// See docs/adr/20260706T170000Z_fixture-reference-value-syntax.md.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureReference(String);

/// Error returned when a raw path string fails `FixtureReference` lexical validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureReferenceError {
    /// The path was empty.
    Empty,
    /// The path started with `/`.
    Absolute,
    /// The path contained a `.` or `..` segment.
    DotSegment,
}

impl FixtureReference {
    /// Validates `raw` against the fixture reference lexical safety policy
    /// and, if valid, returns a `FixtureReference` wrapping it.
    ///
    /// Rejects: empty paths, absolute paths (leading `/`), and `.` / `..`
    /// path segments. Mirrors [`WorkspacePath::parse`] exactly; the two types
    /// share the same lexical policy but are never interchangeable, since
    /// they resolve against different base directories (the case workspace
    /// root vs. the `*.repor` source directory).
    pub fn parse(raw: &str) -> Result<Self, FixtureReferenceError> {
        if raw.is_empty() {
            return Err(FixtureReferenceError::Empty);
        }
        if raw.starts_with('/') {
            return Err(FixtureReferenceError::Absolute);
        }
        for segment in raw.split('/') {
            if segment == "." || segment == ".." {
                return Err(FixtureReferenceError::DotSegment);
            }
        }
        Ok(Self(raw.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The union of ways an assertion's expected file contents may be sourced: a
/// file already inside the case workspace, or a static fixture file kept
/// near the `*.repor` source.
///
/// `FileContentsReference` is not a `TextValue`; there is no implicit
/// conversion between the two. It is the expected-value category for the
/// `contents_equals` family (#87), never for `text_equals` (#88), which
/// takes a `TextValue` instead.
/// See docs/adr/20260706T170000Z_fixture-reference-value-syntax.md.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileContentsReference {
    /// A `<"...">` workspace path literal: a file inside the case workspace.
    Workspace(WorkspacePath),
    /// An `@"..."` fixture reference literal: a static file near the
    /// `*.repor` source.
    Fixture(FixtureReference),
}

/// The surface kind of a parsed `value_literal`: which of the three
/// single-line literal syntaxes a script actually wrote.
///
/// Each kind maps to exactly one semantic domain, independent of context:
/// `"..."` is always a text-domain value, `<"...">` is always a case-workspace
/// filesystem reference, and `@"..."` is always a fixture reference (reserved
/// for #92; no argument position accepts it yet). The parser keeps this kind
/// so an argument position can check it against its signature and reject a
/// mismatch as an actionable semantic diagnostic
/// (`semantic.literal.kind_mismatch`) instead of a bare syntax error.
/// See docs/adr/20260706T160000Z_workspace-path-literal-syntax.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueLiteralKind {
    /// An ordinary `"..."` string literal (text domain).
    StringLiteral,
    /// A `<"...">` workspace path literal (case-workspace filesystem reference).
    WorkspacePath,
    /// An `@"..."` fixture reference literal (test-definition-side file reference).
    FixtureReference,
}

impl ValueLiteralKind {
    /// The stable, user-facing name of this kind, as used in diagnostics.
    pub const fn name(self) -> &'static str {
        match self {
            ValueLiteralKind::StringLiteral => "StringLiteral",
            ValueLiteralKind::WorkspacePath => "WorkspacePath",
            ValueLiteralKind::FixtureReference => "FixtureReference",
        }
    }
}

/// The literal kind an argument position's signature requires.
///
/// Unlike [`ValueLiteralKind`], which names what a script actually wrote,
/// this names what a position accepts — so `TextValue` exists as a
/// requirement (satisfied by a string literal or a heredoc literal) even
/// though it is not itself a surface literal kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequiredLiteralKind {
    /// The position requires a `<"...">` workspace path literal.
    WorkspacePath,
    /// The position requires a text-domain value: a `"..."` string literal
    /// or a heredoc literal.
    TextValue,
    /// The position requires a plain `"..."` string literal specifically
    /// (e.g. a `dir contains` entry name, which is a single entry name, not
    /// general text content).
    StringLiteral,
    /// The position requires a [`FileContentsReference`]: a `<"...">`
    /// workspace path literal or an `@"..."` fixture reference literal (e.g.
    /// a `contents_equals` expected value).
    FileContentsReference,
}

impl RequiredLiteralKind {
    /// The stable, user-facing name of this requirement, as used in diagnostics.
    pub const fn name(self) -> &'static str {
        match self {
            RequiredLiteralKind::WorkspacePath => "WorkspacePath",
            RequiredLiteralKind::TextValue => "TextValue",
            RequiredLiteralKind::StringLiteral => "StringLiteral",
            RequiredLiteralKind::FileContentsReference => "FileContentsReference",
        }
    }
}

/// A `text_literal`: the syntax category `string literal | heredoc literal`,
/// accepted by `write` and `file ... contains`. Kept as a syntax-preserving
/// enum in the AST — rather than resolved to a plain value immediately at
/// parse time — purely so diagnostics, AST snapshots, and docs generation
/// can still tell which surface form a script used.
///
/// Runtime evaluation must never match on this enum's variants: it should
/// always go through [`TextLiteral::to_text_value`] and operate on the
/// resulting [`TextValue`] instead, so that `write` and `file contains`
/// behave identically regardless of which literal form produced the value.
/// See docs/reference/semantics.md — Text literal, and the accompanying ADR.
///
/// No parameter expansion or variable expansion is ever performed on either
/// form's content: `${VAR}`-shaped text is preserved verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextLiteral {
    /// An ordinary `"..."` string literal, already unescaped.
    Quoted(String),
    /// A ``` ... ``` heredoc literal, already dedented against its closing
    /// fence's indentation.
    Heredoc(String),
}

impl TextLiteral {
    /// Resolves this text_literal to its runtime [`TextValue`], erasing
    /// which surface form (`Quoted` or `Heredoc`) produced it.
    pub fn to_text_value(&self) -> TextValue {
        match self {
            TextLiteral::Quoted(value) | TextLiteral::Heredoc(value) => TextValue(value.clone()),
        }
    }
}

/// The resolved runtime value of a `text_literal`, with its syntactic origin
/// (string literal vs. heredoc literal) erased.
///
/// `TextValue` is not a display- or view-only wrapper: it is the actual
/// value passed into runtime evaluation. `write` writes its UTF-8 bytes to
/// the target file; `file ... contains` checks whether its UTF-8 bytes occur
/// as a substring of the target file's bytes. Every text-consuming action or
/// expectation is meant to share this one type as its input, rather than
/// each defining its own representation of "the text the script wrote."
///
/// A `TextValue` is not, itself, an assertion-only comparison value: for
/// `write` it is the content being written, and for `file contains` it is
/// the expected content being compared against, and a future `file
/// text_equals` or `stdout contains` could reuse the same type as either
/// role requires. See docs/reference/semantics.md — Text literal, and the
/// accompanying ADR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextValue(String);

impl TextValue {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A shell-like action step (`$ ...`).
///
/// Executed by `sh -c`.
/// On completion, produces an `ActionResult` that updates the current checkpoint.
/// See docs/reference/execution-model.md — Shell execution.
#[derive(Debug)]
pub struct ActionStep {
    pub command: String,
}

/// A checkpoint-level assertion block (`assert { ... }`).
///
/// This block verifies the current checkpoint.
/// It is intentionally not modeled as an assertion attached to the nearest action, so it can represent both precondition assertions at the initial checkpoint and post-action assertions.
///
/// See docs/reference/semantics.md — Assertion block and the checkpoint-based assertion ADR.
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
/// See docs/reference/semantics.md — Expectation and Evidence requirement.
#[derive(Debug)]
pub enum Expectation {
    Exit(ExitExpectation),
    // v0 parser produces Exit, Stdout, Stderr, File, Dir, and Logical.
    // FileCount and Jq (jq expression form) are defined for conceptual completeness; they are not yet parsed or evaluated.
    // See docs/planning/TBD.md for planned additions.
    Stdout(OutputExpectation),
    Stderr(OutputExpectation),
    File(FileExpectation),
    Dir(DirExpectation),
    FileCount(FileCountExpectation),
    Jq(JqExpectation),
    /// Block-form logical composition (`not` / `all` / `any`) over nested expectation expressions.
    /// See docs/reference/semantics.md — Logical composition and the accompanying ADR.
    Logical(LogicalExpectation),
}

impl Expectation {
    /// The evidence this expectation requires from the current checkpoint.
    ///
    /// Workspace evidence is available at the initial checkpoint.
    /// `LastActionResult`, `Stdout`, and `Stderr` are only available after a `$` action has run.
    ///
    /// For a logical composition, this is the requirement of whichever (possibly nested) child needs a preceding `$` action — covering `LastActionResult`, `Stdout`, and `Stderr` alike, not just exit code — so a composition wrapping any process expectation is still rejected at the initial checkpoint the same way a bare process expectation is.
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
/// `and` / `or` are deliberately not defined as aliases for `all` / `any`; v0's canonical logical composition syntax is limited to these three.
/// See the accompanying ADR.
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

/// A block-form logical composition expectation: `not { ... }`, `all { ... }`, or `any { ... }`.
///
/// `children` holds the expectation expressions inside the block in source order, and may nest further `Logical` expectations.
/// A `not` block with multiple children negates their implicit-`all` grouping, not each child individually: `not { A B }` evaluates as `not(all(A, B))`, never as `not(A) and not(B)`.
/// See docs/reference/semantics.md — Logical composition.
#[derive(Debug)]
pub struct LogicalExpectation {
    operator: LogicalOperator,
    children: Vec<Expectation>,
}

/// Error returned when constructing a `LogicalExpectation` with invalid content.
#[derive(Debug, PartialEq)]
pub enum LogicalExpectationError {
    /// A `not` / `all` / `any` block must contain at least one expectation expression.
    /// The grammar accepts an empty body so Reportage can reject it as a semantic error rather than a generic syntax error; callers (the parser) are expected to have already turned this into a `ParseError` before reaching this constructor.
    /// See docs/reference/semantic-diagnostics.md.
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
/// `Workspace` is available at the initial checkpoint.
/// `LastActionResult`, `Stdout`, and `Stderr` require a preceding `$` action in the same case.
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
    /// `stdout` / `stderr contents_equals <FileContentsReference>`: byte-for-byte
    /// comparison against a workspace file or fixture file. See
    /// `evaluator::evaluate_expectation_at_checkpoint`.
    ContentsEquals(FileContentsReference),
    /// `stdout` / `stderr text_equals <text_literal>`: byte-for-byte comparison
    /// of the captured stream's bytes against the `TextLiteral`'s `TextValue`
    /// encoded as UTF-8, with no normalization, exactly like
    /// [`FileMatcher::TextEquals`]. `text_literal` may be either a string
    /// literal or a heredoc literal. See [`TextLiteral`] and
    /// docs/adr — output text_equals evaluation.
    TextEquals(TextLiteral),
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
    /// `file <"path"> contains <text_literal>`: `text_literal` may be either
    /// a string literal or a heredoc literal. See [`TextLiteral`].
    Contains(TextLiteral),
    Matches(String),
    /// `file <"path"> contents_equals <FileContentsReference>`: byte-for-byte
    /// comparison against a workspace file or fixture file. See
    /// `evaluator::evaluate_file_expectation`.
    ContentsEquals(FileContentsReference),
    /// `file <"path"> text_equals <text_literal>`: byte-for-byte comparison
    /// of the actual file's bytes against the `TextLiteral`'s `TextValue`
    /// encoded as UTF-8, with no normalization. `text_literal` may be either
    /// a string literal or a heredoc literal. See [`TextLiteral`], #88, and
    /// docs/adr — text_equals evaluation.
    TextEquals(TextLiteral),
}

/// Directory existence / entry expectation.
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
    /// `dir <"path"> contains "<name>"`: `name` is a single directory entry
    /// name checked for exact match directly under `path`, never a nested
    /// path, a glob, or a recursive search.
    Contains(String),
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
