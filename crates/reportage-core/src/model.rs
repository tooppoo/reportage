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
/// Holds the same [`Step`] as a case body, in source order, so setup and case
/// body share one step model and one executor. Which steps `before_each`
/// actually accepts is a parser rule rather than a property of this type: the
/// alternative — a narrower step type per block kind — makes every step added
/// to a case body a second, independent decision about `before_each`, and
/// forces two execution paths for identical semantics.
///
/// `before_each` is never shared state: each concrete case replays these steps
/// against its own fresh workspace.
/// See docs/reference/execution-model.md — `before_each`, and the
/// accompanying ADR.
#[derive(Debug)]
pub struct BeforeEach {
    steps: Vec<Step>,
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
    pub fn new(steps: Vec<Step>) -> Result<Self, BeforeEachError> {
        if steps.is_empty() {
            return Err(BeforeEachError::Empty);
        }
        Ok(Self { steps })
    }

    pub fn steps(&self) -> &[Step] {
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
    Binding(BindingDeclaration),
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

/// A write step writes a literal or resolved binding value to a file in the concrete case workspace.
///
/// Create-only: rejected at runtime if `path` already exists.
/// See docs/reference/semantics.md — Write step.
#[derive(Debug)]
pub struct WriteFileStep {
    pub path: WorkspacePath,
    pub content: TextValueExpression,
    /// The permission bits the created file ends up with, when the step named
    /// a `mode`. `None` leaves the mode alone, which is what every `write`
    /// step written before `mode` existed relies on.
    pub mode: Option<FileMode>,
}

#[derive(Debug)]
pub struct BindingDeclaration {
    pub name: String,
    pub source: RuntimeEvidenceSource,
    pub declaration_span: LocatedSpan,
}

/// A byte range in the original `*.repor` source, with the 1-based line and
/// column of its first character.
///
/// Distinct from [`crate::source::SourceSpan`], which is a bare byte range
/// tied to a `SourceFile`'s own text: this type carries the resolved position
/// a diagnostic prints, and is produced wherever the parser already knows it.
///
/// Spans always address the source a user wrote, never an intermediate
/// representation: an interpolated heredoc's binding reference is recorded
/// against the original body, not against the dedented text it is scanned in.
/// See [`InterpolatedText`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocatedSpan {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeEvidenceSource {
    StdoutExact,
    StderrExact,
    StdoutLine,
    StderrLine,
}

impl RuntimeEvidenceSource {
    pub const fn stream(self) -> OutputSource {
        match self {
            Self::StdoutExact | Self::StdoutLine => OutputSource::Stdout,
            Self::StderrExact | Self::StderrLine => OutputSource::Stderr,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingReference {
    pub name: String,
    pub reference_span: LocatedSpan,
}

/// A source-level text value expression: the one input type every `TextValue`
/// argument position takes.
///
/// The three forms are kept apart here, not collapsed to a value at parse
/// time, so diagnostics, AST snapshots, and provenance can still say which one
/// a script wrote. Runtime consumers must not branch on the variant: they
/// resolve the expression through [`crate::text_value::ResolveTextValue`] and
/// operate on the resulting [`TextValue`], so `write` and every text matcher
/// behave identically regardless of the form that produced the value.
/// See docs/adr/20260726T060000Z_interpolated-text-literal.md.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextValueExpression {
    /// A raw `"..."` string literal or raw heredoc literal. Never interpolated:
    /// `&{name}` inside it is literal text.
    Raw(TextLiteral),
    /// A direct `&name` reference to a case-local binding's whole value.
    Binding(BindingReference),
    /// An `&"..."` / `&` + heredoc interpolated text literal.
    Interpolated(InterpolatedText),
}

impl TextValueExpression {
    /// The expression's `TextValue` when it needs no binding environment to
    /// resolve, and `None` when it does.
    ///
    /// A parse-time helper with no runtime caller: every evaluation path
    /// resolves through [`crate::text_value::ResolveTextValue`] against the
    /// bindings in scope, so that a raw literal, a direct reference, and an
    /// interpolated literal share one resolution. Use this only where there is
    /// genuinely no binding environment to resolve against — asserting on a
    /// parsed expression's literal text, for instance.
    pub fn binding_free_text_value(&self) -> Option<TextValue> {
        match self {
            Self::Raw(literal) => Some(literal.to_text_value()),
            Self::Binding(_) => None,
            Self::Interpolated(text) => text.binding_free_text_value(),
        }
    }

    /// Every binding reference this expression makes, in source order.
    ///
    /// One traversal shared by scope validation and provenance collection, so
    /// a direct reference and a reference inside an interpolated literal are
    /// never validated by two different code paths.
    pub fn binding_references(&self) -> Vec<&BindingReference> {
        match self {
            Self::Raw(_) => Vec::new(),
            Self::Binding(reference) => vec![reference],
            Self::Interpolated(text) => text.binding_references().collect(),
        }
    }
}

impl PartialEq<str> for TextValueExpression {
    fn eq(&self, other: &str) -> bool {
        matches!(self, Self::Raw(literal) if literal.to_text_value().as_str() == other)
    }
}

/// Which surface literal an [`InterpolatedText`] was written as.
///
/// Retained for diagnostics, snapshots, and provenance only: both forms
/// produce the same `TextValue` and the same runtime behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterpolatedTextForm {
    /// An `&"..."` interpolated string literal.
    String,
    /// An `&` + heredoc interpolated heredoc literal.
    Heredoc,
}

/// An interpolated text literal, held as the alternating literal and binding
/// reference segments recognized in its source, never as a pre-evaluated
/// string.
///
/// Segments are already unescaped and (for the heredoc form) dedented, so
/// evaluation is a concatenation with each binding's exact value: binding
/// values are inserted verbatim, with no escaping, quoting, indentation,
/// newline normalization, or recursive interpolation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterpolatedText {
    form: InterpolatedTextForm,
    segments: Vec<InterpolatedTextSegment>,
    span: LocatedSpan,
}

impl InterpolatedText {
    pub fn new(
        form: InterpolatedTextForm,
        segments: Vec<InterpolatedTextSegment>,
        span: LocatedSpan,
    ) -> Self {
        Self {
            form,
            segments,
            span,
        }
    }

    pub fn form(&self) -> InterpolatedTextForm {
        self.form
    }

    pub fn segments(&self) -> &[InterpolatedTextSegment] {
        &self.segments
    }

    /// The whole literal's span in the original source.
    pub fn span(&self) -> LocatedSpan {
        self.span
    }

    pub fn binding_references(&self) -> impl Iterator<Item = &BindingReference> {
        self.segments.iter().filter_map(|segment| match segment {
            InterpolatedTextSegment::Literal(_) => None,
            InterpolatedTextSegment::Binding(reference) => Some(reference),
        })
    }

    /// The literal's `TextValue` when it references no binding, and `None`
    /// when it does. A reference-free interpolated literal is redundant but
    /// legal, so it stays usable where no binding environment exists.
    fn binding_free_text_value(&self) -> Option<TextValue> {
        let mut value = String::new();
        for segment in &self.segments {
            match segment {
                InterpolatedTextSegment::Literal(text) => value.push_str(text.as_str()),
                InterpolatedTextSegment::Binding(_) => return None,
            }
        }
        Some(TextValue::new(value))
    }
}

/// One piece of an [`InterpolatedText`]: literal text, or a binding reference
/// to substitute at step evaluation time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterpolatedTextSegment {
    Literal(TextValue),
    Binding(BindingReference),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundValue {
    Text(TextValue),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub name: String,
    pub value: BoundValue,
    pub declaration_span: LocatedSpan,
    pub source: BindingSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindingSource {
    pub action_index: usize,
    pub stream: OutputSource,
    pub capture_mode: CaptureMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMode {
    Exact,
    Line,
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

/// The POSIX permission bits a `write` step may request for the file it creates.
///
/// Constructed only via [`FileMode::from_bits`], which rejects anything above
/// `0o777`. Confining the type to the nine ordinary permission bits makes
/// setuid, setgid, and sticky — deliberately out of `write`'s scope —
/// unrepresentable rather than merely unused, so no caller can carry them as
/// far as the filesystem.
/// See docs/reference/semantics.md — Write step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileMode(u32);

/// Error returned when a raw value falls outside the permission bits
/// [`FileMode`] can represent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileModeError {
    /// The value had bits set above `0o777`: setuid, setgid, sticky, or a
    /// file type bit.
    OutOfRange,
}

impl FileMode {
    /// The permission bits a `write` step's file gets when the step names no
    /// `mode`: readable and writable by its owner, and nothing else — never
    /// executable.
    ///
    /// A stated default rather than whatever the platform happens to produce.
    /// It is applied the same way an explicit `mode` is, so a `write` step's
    /// result never depends on the umask the reportage process runs under,
    /// whether or not the step names a mode.
    pub const DEFAULT: Self = Self(0o600);

    /// Validates `bits` as a plain POSIX permission bit set and, if valid,
    /// returns the [`FileMode`] wrapping it.
    ///
    /// Rejects every value above `0o777`. The surface syntax already limits a
    /// `mode` to three octal digits, but validating here keeps the guarantee
    /// attached to the type instead of to one parser, so a value that reaches
    /// the filesystem is in range no matter which caller built it.
    pub fn from_bits(bits: u32) -> Result<Self, FileModeError> {
        if bits > 0o777 {
            return Err(FileModeError::OutOfRange);
        }
        Ok(Self(bits))
    }

    /// The permission bits, in the form `chmod(2)` expects.
    pub fn bits(self) -> u32 {
        self.0
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
    BindingReference,
}

impl ValueLiteralKind {
    /// The stable, user-facing name of this kind, as used in diagnostics.
    pub const fn name(self) -> &'static str {
        match self {
            ValueLiteralKind::StringLiteral => "StringLiteral",
            ValueLiteralKind::WorkspacePath => "WorkspacePath",
            ValueLiteralKind::FixtureReference => "FixtureReference",
            ValueLiteralKind::BindingReference => "BindingReference<TextValue>",
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
    pub fn new(value: String) -> Self {
        Self(value)
    }

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
    Contains(TextValueExpression),
    NotContains(String),
    Matches(String),
    /// `stdout` / `stderr contents_equals <FileContentsReference>`: byte-for-byte
    /// comparison against a workspace file or fixture file. See
    /// `evaluator/expectation.rs`.
    ContentsEquals(FileContentsReference),
    /// Byte-for-byte comparison against a literal or binding-backed `TextValue`, encoded as UTF-8 without normalization.
    TextEquals(TextValueExpression),
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
    /// Contains comparison against a literal or binding-backed `TextValue`.
    Contains(TextValueExpression),
    Matches(String),
    /// `file <"path"> contents_equals <FileContentsReference>`: byte-for-byte
    /// comparison against a workspace file or fixture file. See
    /// `evaluator/expectation.rs`.
    ContentsEquals(FileContentsReference),
    /// Byte-for-byte comparison against a literal or binding-backed `TextValue`, encoded as UTF-8 without normalization.
    TextEquals(TextValueExpression),
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputSource {
    Stdout,
    Stderr,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::closed(0o000)]
    #[case::owner_only(0o600)]
    #[case::executable(0o755)]
    #[case::open(0o777)]
    fn file_mode_accepts_a_permission_bit_set(#[case] bits: u32) {
        assert_eq!(FileMode::from_bits(bits).unwrap().bits(), bits);
    }

    // The bits just above the range are the ones `write` deliberately does not
    // offer, so they must be rejected rather than silently truncated into a
    // plausible-looking permission set.
    #[rstest]
    #[case::sticky(0o1000)]
    #[case::setgid(0o2000)]
    #[case::setuid(0o4000)]
    #[case::every_special_bit(0o7777)]
    fn file_mode_rejects_bits_above_the_permission_range(#[case] bits: u32) {
        assert_eq!(FileMode::from_bits(bits), Err(FileModeError::OutOfRange));
    }
}
