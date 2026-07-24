use crate::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticDetails, DiagnosticLocation};
use crate::model::{
    FixtureReferenceError, LogicalOperator, RequiredLiteralKind, ValueLiteralKind,
    WorkspacePathError,
};

#[derive(Debug, PartialEq)]
pub enum ParseError {
    /// A syntax error produced by the pest grammar.
    Syntax {
        line: usize,
        column: usize,
        message: String,
        /// The full source line the error is on, for a caret-annotated display snippet.
        /// Not part of the stable diagnostic contract — see `DiagnosticDetails::pest_message`.
        source_line: String,
    },
    /// A case block must contain at least one step.
    EmptyCase { line: usize, name: String },
    /// A case block must contain at least one assertion block.
    MissingAssertionBlock { line: usize, name: String },
    /// An action step must contain a non-empty command after trimming whitespace.
    EmptyAction { line: usize },
    /// Exit code is outside the valid range 0..=255.
    InvalidExitCode { line: usize, value: String },
    /// A `not` / `all` / `any` logical composition block contains zero expectation expressions.
    EmptyLogicalCompositionBlock {
        line: usize,
        operator: LogicalOperator,
    },
    /// A `<"...">` workspace path literal failed `WorkspacePath::parse` validation — a `write`
    /// step's target path, or a `contents_equals` expected value.
    InvalidWorkspacePath {
        line: usize,
        raw: String,
        reason: WorkspacePathError,
        /// Human-readable name of the argument position, e.g. "`write` step path" or
        /// "`file contents_equals` expected value". Mirrors `LiteralKindMismatch::position`.
        position: &'static str,
    },
    /// An `@"<path>"` fixture reference literal failed `FixtureReference::parse`
    /// lexical validation (empty, absolute, or a `.` / `..` segment).
    InvalidFixtureReference {
        line: usize,
        raw: String,
        reason: FixtureReferenceError,
    },
    /// A literal of the wrong kind appeared in an argument position whose
    /// signature requires a different kind (e.g. `file "out.txt" exists`,
    /// whose subject requires a workspace path literal `<"out.txt">`).
    /// Grammar-wise the script parses; this is a semantic invalid case with
    /// an actionable diagnostic. See docs/reference/semantic-diagnostics.md.
    LiteralKindMismatch {
        line: usize,
        /// Human-readable name of the argument position, e.g. "`file` checkpoint subject".
        position: &'static str,
        expected: RequiredLiteralKind,
        actual: ValueLiteralKind,
        /// The offending literal as written in source, e.g. `"out.txt"` or `<"out.txt">`.
        source: String,
        /// The suggested replacement, e.g. `<"out.txt">`, or a description
        /// such as "a string literal or heredoc literal".
        suggestion: String,
    },
    /// A heredoc literal (in a `write` step or a `file ... contains`
    /// expectation) has a non-blank body line indented less than the
    /// closing fence's indentation.
    ShallowHeredocIndent { line: usize },
    /// A document block contains no documentation field.
    EmptyDocumentBlock { line: usize },
    /// A document block declares the same documentation field more than once.
    DuplicateDocumentationField { line: usize, field: &'static str },
    /// A document block's `order` value is a digit run that overflows the
    /// model's u64 range; the grammar guarantees it is otherwise a
    /// non-negative integer.
    InvalidDocumentationOrder { line: usize, value: String },
    /// A source contains more than one `document file` block.
    DuplicateDocumentFile { line: usize },
    /// A `document file` block appears after the source's first case block,
    /// after a `document case` block, or after a `before_each` block,
    /// violating the canonical top-level form
    /// `document file? before_each? (document case? case)*`.
    DocumentFileAfterCase { line: usize },
    /// A second `document case` block appears before the previous one's
    /// target case, which would associate two blocks with one case.
    DuplicateDocumentCase { line: usize },
    /// A `document case` block is not followed by a case to associate with.
    OrphanDocumentCase { line: usize },
    /// A source contains more than one `before_each` block.
    DuplicateBeforeEach { line: usize },
    /// A `before_each` block appears after the source's first case block or
    /// after a `document case` block, violating the canonical top-level form
    /// `document file? before_each? (document case? case)*`.
    BeforeEachAfterCase { line: usize },
    /// A `before_each` body contains a `$` action step. Actions are banned
    /// there regardless of the command; setup commands belong in each case
    /// body. See the `before_each` ADR.
    BeforeEachActionStep { line: usize },
    /// A `before_each` body contains an `assert` block. Setup results are
    /// verified at the start of each case body instead; see the `before_each`
    /// ADR and the deferred-topics record.
    BeforeEachAssertionBlock { line: usize },
    /// A `before_each` block contains no steps.
    EmptyBeforeEach { line: usize },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Syntax {
                line,
                column,
                message,
                source_line,
            } => {
                writeln!(f, "parse error at line {line}, column {column}: {message}")?;
                let indent: String = source_line
                    .chars()
                    .take(column.saturating_sub(1))
                    .map(|c| if c == '\t' { '\t' } else { ' ' })
                    .collect();
                write!(f, "  | {source_line}\n  | {indent}^")
            }
            ParseError::EmptyCase { line, name } => write!(
                f,
                "parse error at line {line}: case '{name}' must contain at least one step"
            ),
            ParseError::MissingAssertionBlock { line, name } => write!(
                f,
                "parse error at line {line}: case '{name}' must contain at least one assertion block"
            ),
            ParseError::EmptyAction { line } => write!(
                f,
                "parse error at line {line}: action command must not be empty"
            ),
            ParseError::InvalidExitCode { line, value } => write!(
                f,
                "parse error at line {line}: invalid exit code '{value}', expected integer in 0..=255"
            ),
            ParseError::EmptyLogicalCompositionBlock { line, operator } => write!(
                f,
                "parse error at line {line}: '{}' block must contain at least one expectation expression",
                operator.keyword()
            ),
            ParseError::InvalidWorkspacePath {
                line,
                raw,
                reason,
                position,
            } => {
                let reason_text = match reason {
                    WorkspacePathError::Empty => "must not be empty",
                    WorkspacePathError::Absolute => "must be relative; absolute paths are rejected",
                    WorkspacePathError::DotSegment => "must not contain '.' or '..' segments",
                };
                write!(
                    f,
                    "parse error at line {line}: {position} '{raw}' {reason_text}"
                )
            }
            ParseError::InvalidFixtureReference { line, raw, reason } => {
                let reason_text = match reason {
                    FixtureReferenceError::Empty => "must not be empty",
                    FixtureReferenceError::Absolute => {
                        "must be relative; absolute paths are rejected"
                    }
                    FixtureReferenceError::DotSegment => "must not contain '.' or '..' segments",
                };
                write!(
                    f,
                    "parse error at line {line}: fixture reference path '{raw}' {reason_text}"
                )
            }
            ParseError::LiteralKindMismatch {
                line,
                position,
                expected,
                actual,
                source,
                suggestion,
            } => write!(
                f,
                "parse error at line {line}: {position} requires a {expected}, but {source} is a {actual}; use {suggestion} instead",
                expected = expected.name(),
                actual = actual.name(),
            ),
            ParseError::ShallowHeredocIndent { line } => write!(
                f,
                "parse error at line {line}: heredoc literal body line is indented less than its closing fence"
            ),
            ParseError::EmptyDocumentBlock { line } => write!(
                f,
                "parse error at line {line}: document block must contain at least one documentation field"
            ),
            ParseError::DuplicateDocumentationField { line, field } => write!(
                f,
                "parse error at line {line}: documentation field '{field}' is declared more than once in the same document block"
            ),
            ParseError::InvalidDocumentationOrder { line, value } => write!(
                f,
                "parse error at line {line}: invalid order '{value}', expected a non-negative integer no greater than {}",
                u64::MAX
            ),
            ParseError::DuplicateDocumentFile { line } => write!(
                f,
                "parse error at line {line}: a source may contain at most one `document file` block"
            ),
            ParseError::DocumentFileAfterCase { line } => write!(
                f,
                "parse error at line {line}: `document file` must appear before `before_each`, all `document case` blocks, and cases"
            ),
            ParseError::DuplicateDocumentCase { line } => write!(
                f,
                "parse error at line {line}: at most one `document case` block may precede a case"
            ),
            ParseError::OrphanDocumentCase { line } => write!(
                f,
                "parse error at line {line}: `document case` must be followed by the case it documents"
            ),
            ParseError::DuplicateBeforeEach { line } => write!(
                f,
                "parse error at line {line}: a source may contain at most one `before_each` block"
            ),
            ParseError::BeforeEachAfterCase { line } => write!(
                f,
                "parse error at line {line}: `before_each` must appear before all `document case` blocks and cases"
            ),
            ParseError::BeforeEachActionStep { line } => write!(
                f,
                "parse error at line {line}: `before_each` must not contain a `$` action step; run setup commands in each case body instead"
            ),
            ParseError::BeforeEachAssertionBlock { line } => write!(
                f,
                "parse error at line {line}: `before_each` must not contain an `assert` block; verify setup results at the start of each case body instead"
            ),
            ParseError::EmptyBeforeEach { line } => write!(
                f,
                "parse error at line {line}: `before_each` block must contain at least one `write` step"
            ),
        }
    }
}

impl std::error::Error for ParseError {}

impl ParseError {
    /// The stable, machine-readable diagnostic code for this error.
    ///
    /// This is independent of the enum variant name: downstream tests and tooling should depend on this code (or its string form) rather than on `Display` output.
    /// See docs/reference/diagnostics.md.
    pub const fn code(&self) -> DiagnosticCode {
        match self {
            ParseError::Syntax { .. } => DiagnosticCode::ParseSyntax,
            ParseError::EmptyCase { .. } => DiagnosticCode::ParseEmptyCase,
            ParseError::MissingAssertionBlock { .. } => DiagnosticCode::ParseMissingAssertionBlock,
            ParseError::EmptyAction { .. } => DiagnosticCode::ParseEmptyAction,
            ParseError::InvalidExitCode { .. } => DiagnosticCode::ParseInvalidExitCode,
            ParseError::EmptyLogicalCompositionBlock { .. } => {
                DiagnosticCode::SemanticExpectationEmptyBlock
            }
            ParseError::InvalidWorkspacePath { reason, .. } => match reason {
                WorkspacePathError::Empty => DiagnosticCode::SemanticWorkspacePathEmpty,
                WorkspacePathError::Absolute => DiagnosticCode::SemanticWorkspacePathAbsolute,
                WorkspacePathError::DotSegment => DiagnosticCode::SemanticWorkspacePathDotSegment,
            },
            ParseError::InvalidFixtureReference { reason, .. } => match reason {
                FixtureReferenceError::Empty => DiagnosticCode::SemanticFixtureReferenceEmpty,
                FixtureReferenceError::Absolute => DiagnosticCode::SemanticFixtureReferenceAbsolute,
                FixtureReferenceError::DotSegment => {
                    DiagnosticCode::SemanticFixtureReferenceDotSegment
                }
            },
            ParseError::LiteralKindMismatch { .. } => DiagnosticCode::SemanticLiteralKindMismatch,
            ParseError::ShallowHeredocIndent { .. } => {
                DiagnosticCode::ParseHeredocLiteralShallowIndent
            }
            ParseError::EmptyDocumentBlock { .. } => DiagnosticCode::ParseDocumentBlockEmpty,
            ParseError::DuplicateDocumentationField { .. } => {
                DiagnosticCode::ParseDocumentBlockDuplicateField
            }
            ParseError::InvalidDocumentationOrder { .. } => {
                DiagnosticCode::ParseDocumentBlockInvalidOrder
            }
            ParseError::DuplicateDocumentFile { .. } => DiagnosticCode::ParseDocumentFileDuplicate,
            ParseError::DocumentFileAfterCase { .. } => DiagnosticCode::ParseDocumentFileAfterCase,
            ParseError::DuplicateDocumentCase { .. } => DiagnosticCode::ParseDocumentCaseDuplicate,
            ParseError::OrphanDocumentCase { .. } => DiagnosticCode::ParseDocumentCaseOrphan,
            ParseError::DuplicateBeforeEach { .. } => DiagnosticCode::ParseBeforeEachDuplicate,
            ParseError::BeforeEachAfterCase { .. } => DiagnosticCode::ParseBeforeEachAfterCase,
            ParseError::BeforeEachActionStep { .. } => DiagnosticCode::ParseBeforeEachActionStep,
            ParseError::BeforeEachAssertionBlock { .. } => {
                DiagnosticCode::ParseBeforeEachAssertionBlock
            }
            ParseError::EmptyBeforeEach { .. } => DiagnosticCode::ParseBeforeEachEmpty,
        }
    }

    /// Converts this error into the struct-based diagnostic model, separating the stable `code` from the improvable `message`, `location`, and the weaker-stability `details`.
    pub fn to_diagnostic(&self) -> Diagnostic {
        let (location, details) = match self {
            ParseError::Syntax {
                line,
                column,
                message,
                source_line: _,
            } => (
                Some(DiagnosticLocation {
                    line: *line,
                    column: Some(*column),
                }),
                DiagnosticDetails {
                    pest_message: Some(message.clone()),
                    ..Default::default()
                },
            ),
            ParseError::EmptyCase { line, name } => (
                Some(DiagnosticLocation {
                    line: *line,
                    column: None,
                }),
                DiagnosticDetails {
                    raw_value: Some(name.clone()),
                    ..Default::default()
                },
            ),
            ParseError::MissingAssertionBlock { line, name } => (
                Some(DiagnosticLocation {
                    line: *line,
                    column: None,
                }),
                DiagnosticDetails {
                    raw_value: Some(name.clone()),
                    ..Default::default()
                },
            ),
            ParseError::EmptyAction { line } => (
                Some(DiagnosticLocation {
                    line: *line,
                    column: None,
                }),
                DiagnosticDetails::default(),
            ),
            ParseError::InvalidExitCode { line, value } => (
                Some(DiagnosticLocation {
                    line: *line,
                    column: None,
                }),
                DiagnosticDetails {
                    raw_value: Some(value.clone()),
                    ..Default::default()
                },
            ),
            ParseError::EmptyLogicalCompositionBlock { line, operator } => (
                Some(DiagnosticLocation {
                    line: *line,
                    column: None,
                }),
                DiagnosticDetails {
                    raw_value: Some(operator.keyword().to_string()),
                    ..Default::default()
                },
            ),
            ParseError::InvalidWorkspacePath { line, raw, .. } => (
                Some(DiagnosticLocation {
                    line: *line,
                    column: None,
                }),
                DiagnosticDetails {
                    raw_value: Some(raw.clone()),
                    ..Default::default()
                },
            ),
            ParseError::InvalidFixtureReference { line, raw, .. } => (
                Some(DiagnosticLocation {
                    line: *line,
                    column: None,
                }),
                DiagnosticDetails {
                    raw_value: Some(raw.clone()),
                    ..Default::default()
                },
            ),
            ParseError::LiteralKindMismatch {
                line,
                expected,
                actual,
                source,
                suggestion,
                ..
            } => (
                Some(DiagnosticLocation {
                    line: *line,
                    column: None,
                }),
                DiagnosticDetails {
                    raw_value: Some(source.clone()),
                    expected_kind: Some(expected.name().to_string()),
                    actual_kind: Some(actual.name().to_string()),
                    suggestion: Some(suggestion.clone()),
                    ..Default::default()
                },
            ),
            ParseError::ShallowHeredocIndent { line } => (
                Some(DiagnosticLocation {
                    line: *line,
                    column: None,
                }),
                DiagnosticDetails::default(),
            ),
            ParseError::EmptyDocumentBlock { line }
            | ParseError::DuplicateDocumentFile { line }
            | ParseError::DocumentFileAfterCase { line }
            | ParseError::DuplicateDocumentCase { line }
            | ParseError::OrphanDocumentCase { line }
            | ParseError::DuplicateBeforeEach { line }
            | ParseError::BeforeEachAfterCase { line }
            | ParseError::BeforeEachActionStep { line }
            | ParseError::BeforeEachAssertionBlock { line }
            | ParseError::EmptyBeforeEach { line } => (
                Some(DiagnosticLocation {
                    line: *line,
                    column: None,
                }),
                DiagnosticDetails::default(),
            ),
            ParseError::DuplicateDocumentationField { line, field } => (
                Some(DiagnosticLocation {
                    line: *line,
                    column: None,
                }),
                DiagnosticDetails {
                    raw_value: Some(field.to_string()),
                    ..Default::default()
                },
            ),
            ParseError::InvalidDocumentationOrder { line, value } => (
                Some(DiagnosticLocation {
                    line: *line,
                    column: None,
                }),
                DiagnosticDetails {
                    raw_value: Some(value.clone()),
                    ..Default::default()
                },
            ),
        };

        Diagnostic {
            code: self.code(),
            message: self.to_string(),
            location,
            details,
        }
    }
}
