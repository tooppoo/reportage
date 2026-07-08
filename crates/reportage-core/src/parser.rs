use pest::Parser;
use pest_derive::Parser;

use crate::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticDetails, DiagnosticLocation};
use crate::model::{
    ActionStep, AssertionBlock, Case, DirExpectation, DirMatcher, ExitExpectation, Expectation,
    FileContentsReference, FileExpectation, FileMatcher, FixtureReference, FixtureReferenceError,
    LogicalExpectation, LogicalOperator, OutputExpectation, OutputMatcher, RequiredLiteralKind,
    Script, SideEffectingStep, Step, TextLiteral, ValueLiteralKind, WorkspacePath,
    WorkspacePathError, WriteFileStep,
};

#[derive(Parser)]
#[grammar = "reportage.pest"]
struct ReportageParser;

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
    /// an actionable diagnostic. See docs/semantic-diagnostics.md.
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
        }
    }
}

impl std::error::Error for ParseError {}

impl ParseError {
    /// The stable, machine-readable diagnostic code for this error.
    ///
    /// This is independent of the enum variant name: downstream tests and tooling should depend on this code (or its string form) rather than on `Display` output.
    /// See docs/diagnostics.md.
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
        };

        Diagnostic {
            code: self.code(),
            message: self.to_string(),
            location,
            details,
        }
    }
}

pub fn parse(source: &str) -> Result<Script, ParseError> {
    let pairs = ReportageParser::parse(Rule::script, source).map_err(|e| {
        let (line, col) = match e.line_col {
            pest::error::LineColLocation::Pos((l, c)) => (l, c),
            pest::error::LineColLocation::Span((l, c), _) => (l, c),
        };
        ParseError::Syntax {
            line,
            column: col,
            message: e.variant.message().to_string(),
            source_line: e.line().to_string(),
        }
    })?;

    // `parse()` returns a Pairs that yields the top-level `script` pair.
    // Call into_inner() to get its contents (case_blocks, SOI, EOI).
    let script_pair = pairs.into_iter().next().expect("script always matches");
    let mut cases: Vec<Case> = Vec::new();
    for pair in script_pair.into_inner() {
        if pair.as_rule() == Rule::case_block {
            // SOI, EOI, and silent blank_lines are skipped via the else branch.
            cases.push(parse_case_block(pair)?);
        }
    }

    Ok(Script { cases })
}

fn parse_case_block(pair: pest::iterators::Pair<Rule>) -> Result<Case, ParseError> {
    let line = pair.line_col().0;
    let mut inner = pair.into_inner();

    let name_pair = inner.next().expect("case_block must have a name");
    let name = extract_string_inner(name_pair);

    let mut steps: Vec<Step> = Vec::new();
    let mut has_assertion_block = false;
    for pair in inner {
        match pair.as_rule() {
            Rule::action_step => steps.push(parse_action_step(pair)?),
            Rule::assertion_block => {
                has_assertion_block = true;
                steps.push(parse_assertion_block(pair)?);
            }
            Rule::write_step_string | Rule::write_step_heredoc => {
                steps.push(parse_write_step(pair)?)
            }
            rule => unreachable!("unexpected rule in case_block: {rule:?}"),
        }
    }

    if steps.is_empty() {
        return Err(ParseError::EmptyCase { line, name });
    }

    if !has_assertion_block {
        return Err(ParseError::MissingAssertionBlock { line, name });
    }

    Ok(Case { name, steps })
}

fn extract_string_inner(quoted: pest::iterators::Pair<Rule>) -> String {
    // quoted_string = { "\"" ~ string_inner ~ "\"" }
    let raw = quoted
        .into_inner()
        .next()
        .expect("quoted_string must have string_inner")
        .as_str();
    unescape_string(raw)
}

/// Unescapes a raw `string_inner` match into its AST value.
///
/// The grammar's `string_char` rule only accepts `\\`, `\"`, `\n`, and `\t` as escape sequences, so every `\` in `raw` is guaranteed to be followed by one of those four characters.
fn unescape_string(raw: &str) -> String {
    let mut result = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            result.push(c);
            continue;
        }
        match chars.next() {
            Some('\\') => result.push('\\'),
            Some('"') => result.push('"'),
            Some('n') => result.push('\n'),
            Some('t') => result.push('\t'),
            other => {
                unreachable!("grammar guarantees only \\\\, \\\", \\n, \\t escapes, got {other:?}")
            }
        }
    }
    result
}

/// A parsed `value_literal`: its surface kind, its unescaped inner value,
/// and enough source context to build an actionable kind-mismatch diagnostic.
struct ValueLiteral {
    kind: ValueLiteralKind,
    /// The unescaped inner string value.
    value: String,
    /// The inner quoted string exactly as written in source, including its
    /// surrounding quotes (e.g. `"out.txt"`), used to render suggestions.
    quoted_source: String,
    line: usize,
}

/// The literal kind an argument position requires, together with which
/// surface forms its grammar actually accepts — the extra bit
/// [`RequiredLiteralKind`] alone doesn't carry. A kind mismatch's suggested
/// replacement must only point at forms the position's grammar would accept,
/// or the suggestion would steer the author into the very `parse.syntax`
/// error the semantic diagnostic exists to avoid.
#[derive(Clone, Copy)]
enum RequiredKind {
    /// The position requires a `<"...">` workspace path literal.
    WorkspacePath,
    /// The position requires a TextValue and its grammar accepts both the
    /// string literal and heredoc literal forms (a `write` step's content,
    /// `file contains` expected text).
    TextValueStringOrHeredoc,
    /// The position requires a TextValue but its grammar only wires up the
    /// string literal form (`stdout contains` / `stderr contains` expected
    /// text — see #88's scope note in the grammar), so the suggestion must
    /// not mention a heredoc literal.
    TextValueStringOnly,
    /// The position requires a plain `"..."` string literal
    /// (`dir contains` entry name).
    StringLiteral,
    /// The position requires a `FileContentsReference`: a `<"...">`
    /// workspace path literal or an `@"..."` fixture reference literal
    /// (a `contents_equals` expected value). See #92.
    FileContentsReference,
}

impl RequiredKind {
    /// The user-facing requirement this maps to in the diagnostic contract.
    fn required_literal_kind(self) -> RequiredLiteralKind {
        match self {
            RequiredKind::WorkspacePath => RequiredLiteralKind::WorkspacePath,
            RequiredKind::TextValueStringOrHeredoc | RequiredKind::TextValueStringOnly => {
                RequiredLiteralKind::TextValue
            }
            RequiredKind::StringLiteral => RequiredLiteralKind::StringLiteral,
            RequiredKind::FileContentsReference => RequiredLiteralKind::FileContentsReference,
        }
    }
}

impl ValueLiteral {
    /// The literal exactly as written in source, e.g. `"out.txt"`,
    /// `<"out.txt">`, or `@"out.txt"`.
    fn rendered(&self) -> String {
        match self.kind {
            ValueLiteralKind::StringLiteral => self.quoted_source.clone(),
            ValueLiteralKind::WorkspacePath => format!("<{}>", self.quoted_source),
            ValueLiteralKind::FixtureReference => format!("@{}", self.quoted_source),
        }
    }

    /// Checks this literal against the kind `position` requires, returning
    /// the unescaped inner value on a match and an actionable
    /// `LiteralKindMismatch` (semantic.literal.kind_mismatch) otherwise.
    fn expect_kind(
        self,
        expected: RequiredKind,
        position: &'static str,
    ) -> Result<String, ParseError> {
        let matches = match expected {
            RequiredKind::WorkspacePath => self.kind == ValueLiteralKind::WorkspacePath,
            // TextValue's other form, the heredoc literal, is a distinct
            // grammar rule and never reaches this check.
            RequiredKind::TextValueStringOrHeredoc
            | RequiredKind::TextValueStringOnly
            | RequiredKind::StringLiteral => self.kind == ValueLiteralKind::StringLiteral,
            RequiredKind::FileContentsReference => {
                matches!(
                    self.kind,
                    ValueLiteralKind::WorkspacePath | ValueLiteralKind::FixtureReference
                )
            }
        };
        if matches {
            return Ok(self.value);
        }

        let suggestion = match expected {
            RequiredKind::WorkspacePath => format!("<{}>", self.quoted_source),
            RequiredKind::TextValueStringOrHeredoc => {
                format!(
                    "a string literal or heredoc literal (e.g. {})",
                    self.quoted_source
                )
            }
            RequiredKind::TextValueStringOnly | RequiredKind::StringLiteral => {
                self.quoted_source.clone()
            }
            RequiredKind::FileContentsReference => {
                format!(
                    "a workspace path literal or fixture reference literal (e.g. <{0}> or @{0})",
                    self.quoted_source
                )
            }
        };
        Err(ParseError::LiteralKindMismatch {
            line: self.line,
            position,
            expected: expected.required_literal_kind(),
            actual: self.kind,
            source: self.rendered(),
            suggestion,
        })
    }
}

/// Parses a `value_literal` pair into its kind, unescaped value, and source
/// rendering. Infallible: which kinds a position accepts is checked
/// separately via [`ValueLiteral::expect_kind`].
fn parse_value_literal(pair: pest::iterators::Pair<Rule>) -> ValueLiteral {
    // value_literal = { workspace_path_literal | fixture_reference_literal | quoted_string }
    debug_assert_eq!(pair.as_rule(), Rule::value_literal);
    let line = pair.line_col().0;
    let variant = pair
        .into_inner()
        .next()
        .expect("value_literal must have a variant");

    let (kind, quoted) = match variant.as_rule() {
        Rule::quoted_string => (ValueLiteralKind::StringLiteral, variant),
        Rule::workspace_path_literal | Rule::fixture_reference_literal => {
            let kind = if variant.as_rule() == Rule::workspace_path_literal {
                ValueLiteralKind::WorkspacePath
            } else {
                ValueLiteralKind::FixtureReference
            };
            let quoted = variant
                .into_inner()
                .next()
                .expect("path/fixture literal must wrap a quoted_string");
            (kind, quoted)
        }
        rule => unreachable!("unexpected rule in value_literal: {rule:?}"),
    };

    let quoted_source = quoted.as_str().to_string();
    ValueLiteral {
        kind,
        value: extract_string_inner(quoted),
        quoted_source,
        line,
    }
}

/// Parses a `value_literal` pair into a [`FileContentsReference`] (a
/// `contents_equals` expected value): a `<"...">` workspace path literal or
/// an `@"..."` fixture reference literal, each validated against its own
/// lexical policy at construction time. Any other literal kind (a plain
/// `"..."` string literal) is rejected as a `LiteralKindMismatch` via
/// [`ValueLiteral::expect_kind`]. See #92 and
/// docs/adr/20260706T170000Z_fixture-reference-value-syntax.md.
fn parse_file_contents_reference(
    literal_pair: pest::iterators::Pair<Rule>,
    position: &'static str,
) -> Result<FileContentsReference, ParseError> {
    let literal = parse_value_literal(literal_pair);
    let kind = literal.kind;
    let line = literal.line;
    let raw = literal.expect_kind(RequiredKind::FileContentsReference, position)?;

    match kind {
        ValueLiteralKind::WorkspacePath => {
            let path =
                WorkspacePath::parse(&raw).map_err(|reason| ParseError::InvalidWorkspacePath {
                    line,
                    raw,
                    reason,
                    position,
                })?;
            Ok(FileContentsReference::Workspace(path))
        }
        ValueLiteralKind::FixtureReference => {
            let fixture = FixtureReference::parse(&raw)
                .map_err(|reason| ParseError::InvalidFixtureReference { line, raw, reason })?;
            Ok(FileContentsReference::Fixture(fixture))
        }
        ValueLiteralKind::StringLiteral => {
            unreachable!("expect_kind already rejected StringLiteral for FileContentsReference")
        }
    }
}

fn parse_action_step(pair: pest::iterators::Pair<Rule>) -> Result<Step, ParseError> {
    // action_step = { "$" ~ ws* ~ command }
    let line = pair.line_col().0;
    // Only space/tab are trimmed, never newlines: a continuation-preserving
    // command can legitimately end in a `\` + newline pair (see the grammar's
    // `command` rule), and trimming newlines would strip the newline half of
    // that pair while leaving the `\` behind.
    let command = pair
        .into_inner()
        .next()
        .expect("action_step must have command")
        .as_str()
        .trim_matches(|c: char| c == ' ' || c == '\t')
        .to_string();

    if command.is_empty() {
        return Err(ParseError::EmptyAction { line });
    }

    Ok(Step::Action(ActionStep { command }))
}

fn parse_assertion_block(pair: pest::iterators::Pair<Rule>) -> Result<Step, ParseError> {
    // assertion_block = { "assert" ~ ws* ~ "{" ~ (single_assert | multi_assert) ~ ws* ~ "}" }
    let body = pair
        .into_inner()
        .next()
        .expect("assertion_block must have body");

    let expectations = parse_expectation_body(body)?;

    let block = AssertionBlock::new(expectations)
        .expect("grammar guarantees at least one expectation in assertion block");
    Ok(Step::AssertionBlock(block))
}

/// Parses the shared body form used by both `assert { ... }` and `not` / `all` / `any` composition blocks: a single expectation on one line, one or more expectations each on their own line, or (composition blocks only) zero expectations.
///
/// `assert { ... }`'s grammar never actually produces `empty_composition_body` (its body form requires at least one expectation), so this is dead code for that caller; it exists purely so both callers can share one function.
fn parse_expectation_body(
    body: pest::iterators::Pair<Rule>,
) -> Result<Vec<Expectation>, ParseError> {
    match body.as_rule() {
        Rule::single_assert => {
            // single_assert = { ws* ~ expectation ~ ws* }
            let exp_pair = body
                .into_inner()
                .next()
                .expect("single_assert must have expectation");
            Ok(vec![parse_expectation(exp_pair)?])
        }
        Rule::multi_assert => {
            // multi_assert = { trail ~ assertion_or_heredoc_line+ ~ ws* }
            // assertion_line / heredoc_assertion_line are silent, so their
            // children (expectation / heredoc_expectation respectively) are
            // promoted directly here as a mix of the two rule kinds.
            body.into_inner()
                .map(|pair| match pair.as_rule() {
                    Rule::expectation => parse_expectation(pair),
                    Rule::heredoc_expectation => parse_heredoc_expectation(pair),
                    rule => unreachable!("unexpected rule in multi_assert: {rule:?}"),
                })
                .collect::<Result<Vec<_>, _>>()
        }
        Rule::empty_composition_body => Ok(vec![]),
        rule => unreachable!("unexpected rule in expectation body: {rule:?}"),
    }
}

fn parse_expectation(pair: pest::iterators::Pair<Rule>) -> Result<Expectation, ParseError> {
    // expectation = { exit_exp | stdout_exp | stderr_exp | file_exp | dir_exp | logical_composition }
    let inner = pair
        .into_inner()
        .next()
        .expect("expectation must have inner rule");

    match inner.as_rule() {
        Rule::exit_exp => parse_exit_exp(inner),
        Rule::stdout_exp => parse_output_exp(inner, true),
        Rule::stderr_exp => parse_output_exp(inner, false),
        Rule::file_exp => parse_file_exp(inner),
        Rule::dir_exp => parse_dir_exp(inner),
        Rule::logical_composition => parse_logical_composition(inner),
        rule => unreachable!("unexpected rule in expectation: {rule:?}"),
    }
}

fn parse_logical_composition(pair: pest::iterators::Pair<Rule>) -> Result<Expectation, ParseError> {
    // logical_composition = { not_block | all_block | any_block }
    let block = pair
        .into_inner()
        .next()
        .expect("logical_composition must have a variant");
    let line = block.line_col().0;

    let operator = match block.as_rule() {
        Rule::not_block => LogicalOperator::Not,
        Rule::all_block => LogicalOperator::All,
        Rule::any_block => LogicalOperator::Any,
        rule => unreachable!("unexpected rule in logical_composition: {rule:?}"),
    };

    // not_block / all_block / any_block = { "<kw>" ~ ws* ~ "{" ~ (single_assert | multi_assert | empty_composition_body) ~ ws* ~ "}" }
    let body = block
        .into_inner()
        .next()
        .expect("composition block must have a body");
    let children = parse_expectation_body(body)?;

    if children.is_empty() {
        return Err(ParseError::EmptyLogicalCompositionBlock { line, operator });
    }

    let logical =
        LogicalExpectation::new(operator, children).expect("checked non-empty children above");
    Ok(Expectation::Logical(logical))
}

fn parse_exit_exp(pair: pest::iterators::Pair<Rule>) -> Result<Expectation, ParseError> {
    // exit_exp = { "exit" ~ ws+ ~ exit_code }
    // exit_code = @{ ASCII_DIGIT+ }
    let code_pair = pair
        .into_inner()
        .next()
        .expect("exit_exp must have exit_code");
    let line = code_pair.line_col().0;
    let code_str = code_pair.as_str();

    let code = code_str.parse::<u64>().unwrap_or(u64::MAX);
    if code > 255 {
        return Err(ParseError::InvalidExitCode {
            line,
            value: code_str.to_string(),
        });
    }

    Ok(Expectation::Exit(ExitExpectation {
        expected: code as u8,
    }))
}

fn parse_output_exp(
    pair: pest::iterators::Pair<Rule>,
    is_stdout: bool,
) -> Result<Expectation, ParseError> {
    // stdout_exp = { "stdout" ~ ws+ ~ output_matcher }
    // stderr_exp = { "stderr" ~ ws+ ~ output_matcher }
    let matcher_pair = pair
        .into_inner()
        .next()
        .expect("output_exp must have output_matcher");

    let inner = matcher_pair
        .into_inner()
        .next()
        .expect("output_matcher must have a variant");

    let matcher = match inner.as_rule() {
        Rule::output_empty => OutputMatcher::Empty,
        Rule::output_contains => {
            // output_contains = { "contains" ~ ws+ ~ value_literal }
            let literal_pair = inner
                .into_inner()
                .next()
                .expect("output_contains must have value_literal");
            let position = if is_stdout {
                "`stdout contains` expected text"
            } else {
                "`stderr contains` expected text"
            };
            let expected = parse_value_literal(literal_pair)
                .expect_kind(RequiredKind::TextValueStringOnly, position)?;
            OutputMatcher::Contains(expected)
        }
        Rule::output_contents_equals => {
            // output_contents_equals = { "contents_equals" ~ ws+ ~ value_literal }
            let literal_pair = inner
                .into_inner()
                .next()
                .expect("output_contents_equals must have value_literal");
            let position = if is_stdout {
                "`stdout contents_equals` expected value"
            } else {
                "`stderr contents_equals` expected value"
            };
            OutputMatcher::ContentsEquals(parse_file_contents_reference(literal_pair, position)?)
        }
        rule => unreachable!("unexpected rule in output_matcher: {rule:?}"),
    };

    let exp = OutputExpectation { matcher };
    if is_stdout {
        Ok(Expectation::Stdout(exp))
    } else {
        Ok(Expectation::Stderr(exp))
    }
}

fn parse_file_exp(pair: pest::iterators::Pair<Rule>) -> Result<Expectation, ParseError> {
    // file_exp = { "file" ~ ws+ ~ value_literal ~ ws+ ~ file_predicate }
    let mut inner = pair.into_inner();
    let path_pair = inner.next().expect("file_exp must have a path");
    let path = parse_value_literal(path_pair)
        .expect_kind(RequiredKind::WorkspacePath, "`file` checkpoint subject")?;

    let predicate_pair = inner.next().expect("file_exp must have a predicate");
    // file_predicate = { file_contains | file_contents_equals | file_text_equals | file_exists }
    let predicate = predicate_pair
        .into_inner()
        .next()
        .expect("file_predicate must have a variant");

    let matcher = match predicate.as_rule() {
        Rule::file_exists => FileMatcher::Exists,
        Rule::file_contains => {
            // file_contains = { "contains" ~ ws+ ~ value_literal }
            let literal_pair = predicate
                .into_inner()
                .next()
                .expect("file_contains must have value_literal");
            let expected = parse_value_literal(literal_pair).expect_kind(
                RequiredKind::TextValueStringOrHeredoc,
                "`file contains` expected text",
            )?;
            FileMatcher::Contains(TextLiteral::Quoted(expected))
        }
        Rule::file_contents_equals => {
            // file_contents_equals = { "contents_equals" ~ ws+ ~ value_literal }
            let literal_pair = predicate
                .into_inner()
                .next()
                .expect("file_contents_equals must have value_literal");
            let expected = parse_file_contents_reference(
                literal_pair,
                "`file contents_equals` expected value",
            )?;
            FileMatcher::ContentsEquals(expected)
        }
        Rule::file_text_equals => {
            // file_text_equals = { "text_equals" ~ ws+ ~ value_literal }
            let literal_pair = predicate
                .into_inner()
                .next()
                .expect("file_text_equals must have value_literal");
            let expected = parse_value_literal(literal_pair).expect_kind(
                RequiredKind::TextValueStringOrHeredoc,
                "`file text_equals` expected text",
            )?;
            FileMatcher::TextEquals(TextLiteral::Quoted(expected))
        }
        rule => unreachable!("unexpected rule in file_predicate: {rule:?}"),
    };

    Ok(Expectation::File(FileExpectation { path, matcher }))
}

/// Parses the heredoc-literal form of `file ... contains` / `file ...
/// text_equals`, reachable only through `multi_assert` (see
/// `heredoc_assertion_line` in the grammar).
fn parse_heredoc_expectation(pair: pest::iterators::Pair<Rule>) -> Result<Expectation, ParseError> {
    // heredoc_expectation = { file_exp_heredoc | file_text_equals_heredoc }
    let inner = pair
        .into_inner()
        .next()
        .expect("heredoc_expectation must have inner rule");

    match inner.as_rule() {
        Rule::file_exp_heredoc => {
            // file_exp_heredoc = { "file" ~ ws+ ~ value_literal ~ ws+ ~ "contains" ~ ws+ ~ heredoc_literal }
            let mut inner = inner.into_inner();
            let path_pair = inner.next().expect("file_exp_heredoc must have a path");
            let path = parse_value_literal(path_pair)
                .expect_kind(RequiredKind::WorkspacePath, "`file` checkpoint subject")?;

            let literal_pair = inner
                .next()
                .expect("file_exp_heredoc must have a heredoc_literal");
            let content = parse_heredoc_literal(literal_pair)?;

            Ok(Expectation::File(FileExpectation {
                path,
                matcher: FileMatcher::Contains(TextLiteral::Heredoc(content)),
            }))
        }
        Rule::file_text_equals_heredoc => {
            // file_text_equals_heredoc = { "file" ~ ws+ ~ value_literal ~ ws+ ~ "text_equals" ~ ws+ ~ heredoc_literal }
            let mut inner = inner.into_inner();
            let path_pair = inner
                .next()
                .expect("file_text_equals_heredoc must have a path");
            let path = parse_value_literal(path_pair)
                .expect_kind(RequiredKind::WorkspacePath, "`file` checkpoint subject")?;

            let literal_pair = inner
                .next()
                .expect("file_text_equals_heredoc must have a heredoc_literal");
            let content = parse_heredoc_literal(literal_pair)?;

            Ok(Expectation::File(FileExpectation {
                path,
                matcher: FileMatcher::TextEquals(TextLiteral::Heredoc(content)),
            }))
        }
        rule => unreachable!("unexpected rule in heredoc_expectation: {rule:?}"),
    }
}

fn parse_dir_exp(pair: pest::iterators::Pair<Rule>) -> Result<Expectation, ParseError> {
    // dir_exp = { "dir" ~ ws+ ~ value_literal ~ ws+ ~ dir_predicate }
    let mut inner = pair.into_inner();
    let path_pair = inner.next().expect("dir_exp must have a path");
    let path = parse_value_literal(path_pair)
        .expect_kind(RequiredKind::WorkspacePath, "`dir` checkpoint subject")?;

    let predicate_pair = inner.next().expect("dir_exp must have a predicate");
    // dir_predicate = { dir_contains | dir_exists }
    let predicate = predicate_pair
        .into_inner()
        .next()
        .expect("dir_predicate must have a variant");

    let matcher = match predicate.as_rule() {
        Rule::dir_exists => DirMatcher::Exists,
        Rule::dir_contains => {
            // dir_contains = { "contains" ~ ws+ ~ value_literal }
            let literal_pair = predicate
                .into_inner()
                .next()
                .expect("dir_contains must have value_literal");
            let name = parse_value_literal(literal_pair)
                .expect_kind(RequiredKind::StringLiteral, "`dir contains` entry name")?;
            DirMatcher::Contains(name)
        }
        rule => unreachable!("unexpected rule in dir_predicate: {rule:?}"),
    };

    Ok(Expectation::Dir(DirExpectation { path, matcher }))
}

fn parse_write_step(pair: pest::iterators::Pair<Rule>) -> Result<Step, ParseError> {
    match pair.as_rule() {
        Rule::write_step_string => parse_write_step_string(pair),
        Rule::write_step_heredoc => parse_write_step_heredoc(pair),
        rule => unreachable!("unexpected rule in write step: {rule:?}"),
    }
}

fn parse_write_step_string(pair: pest::iterators::Pair<Rule>) -> Result<Step, ParseError> {
    // write_step_string = { "write" ~ ws+ ~ value_literal ~ ws+ ~ value_literal }
    let line = pair.line_col().0;
    let mut inner = pair.into_inner();

    let path_pair = inner.next().expect("write_step_string must have a path");
    let raw_path = parse_value_literal(path_pair)
        .expect_kind(RequiredKind::WorkspacePath, "`write` step path")?;

    let content_pair = inner
        .next()
        .expect("write_step_string must have content value_literal");
    let content = TextLiteral::Quoted(parse_value_literal(content_pair).expect_kind(
        RequiredKind::TextValueStringOrHeredoc,
        "`write` step content",
    )?);

    let path =
        WorkspacePath::parse(&raw_path).map_err(|reason| ParseError::InvalidWorkspacePath {
            line,
            raw: raw_path,
            reason,
            position: "`write` step path",
        })?;

    Ok(Step::SideEffect(SideEffectingStep::WriteFile(
        WriteFileStep { path, content },
    )))
}

fn parse_write_step_heredoc(pair: pest::iterators::Pair<Rule>) -> Result<Step, ParseError> {
    // write_step_heredoc = { "write" ~ ws+ ~ value_literal ~ ws* ~ heredoc_literal }
    let line = pair.line_col().0;
    let mut inner = pair.into_inner();

    let path_pair = inner.next().expect("write_step_heredoc must have a path");
    let raw_path = parse_value_literal(path_pair)
        .expect_kind(RequiredKind::WorkspacePath, "`write` step path")?;

    let literal_pair = inner
        .next()
        .expect("write_step_heredoc must have a heredoc_literal");
    let content = TextLiteral::Heredoc(parse_heredoc_literal(literal_pair)?);

    let path =
        WorkspacePath::parse(&raw_path).map_err(|reason| ParseError::InvalidWorkspacePath {
            line,
            raw: raw_path,
            reason,
            position: "`write` step path",
        })?;

    Ok(Step::SideEffect(SideEffectingStep::WriteFile(
        WriteFileStep { path, content },
    )))
}

/// Parses a `heredoc_literal` pair into its dedented `String` content.
/// Shared by `write_step_heredoc` and `file_exp_heredoc` — the fence and
/// dedent rules are identical regardless of which construct the heredoc
/// literal appears in.
fn parse_heredoc_literal(pair: pest::iterators::Pair<Rule>) -> Result<String, ParseError> {
    // heredoc_literal = { PUSH(opening_fence) ~ ws* ~ nl ~ heredoc_body ~ closing_fence_line ~ DROP }
    let mut inner = pair.into_inner();

    let _opening_fence = inner
        .next()
        .expect("heredoc_literal must have an opening_fence (pushed onto the pest match stack)");

    let body_pair = inner
        .next()
        .expect("heredoc_literal must have heredoc_body");
    let body_start_line = body_pair.line_col().0;
    let body_text = body_pair.as_str();

    let closing_pair = inner
        .next()
        .expect("heredoc_literal must have closing_fence_line");
    // closing_fence_line = { closing_fence_indent ~ PEEK ~ "`"* ~ ws* ~ (nl | EOI) }
    let indent = closing_pair
        .into_inner()
        .next()
        .expect("closing_fence_line must have closing_fence_indent")
        .as_str();

    dedent_heredoc_body(body_text, indent, body_start_line)
}

/// Dedents a heredoc literal body against its closing fence's indentation.
///
/// Every non-blank line must start with `indent` as a literal string prefix
/// (no tab/space width normalization); that prefix is stripped. Blank and
/// whitespace-only lines are exempt from the prefix check and are dedented
/// to a genuinely empty line instead. Line endings (LF or CRLF) are
/// preserved exactly as they appeared in the source.
///
/// `body_start_line` is the source line number of `body`'s first line, used
/// to report the correct line for a shallow-indentation error.
fn dedent_heredoc_body(
    body: &str,
    indent: &str,
    body_start_line: usize,
) -> Result<String, ParseError> {
    let mut result = String::with_capacity(body.len());
    for (i, (content, ending)) in split_lines_keep_ending(body).into_iter().enumerate() {
        let is_blank = content.chars().all(|c| c == ' ' || c == '\t');
        if is_blank {
            result.push_str(ending);
            continue;
        }
        match content.strip_prefix(indent) {
            Some(stripped) => {
                result.push_str(stripped);
                result.push_str(ending);
            }
            None => {
                return Err(ParseError::ShallowHeredocIndent {
                    line: body_start_line + i,
                });
            }
        }
    }
    Ok(result)
}

/// Splits `s` into `(line_content, line_ending)` pairs without normalizing
/// line endings. `line_ending` is `"\n"`, `"\r\n"`, or `""` for a trailing
/// line with no terminator (not produced by the grammar, which requires
/// every heredoc body line to end in an actual newline, but handled here
/// defensively).
fn split_lines_keep_ending(s: &str) -> Vec<(&str, &str)> {
    let mut result = Vec::new();
    let mut rest = s;
    while !rest.is_empty() {
        match rest.find('\n') {
            Some(idx) => {
                let line = &rest[..idx];
                match line.strip_suffix('\r') {
                    Some(stripped) => result.push((stripped, "\r\n")),
                    None => result.push((line, "\n")),
                }
                rest = &rest[idx + 1..];
            }
            None => {
                result.push((rest, ""));
                rest = "";
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_passing_case() {
        let src = r#"
case "first pass" {
  $ true
  assert {
    exit 0
  }
}
"#;
        let script = parse(src).unwrap();
        assert_eq!(script.cases.len(), 1);
        assert_eq!(script.cases[0].name, "first pass");
        assert_eq!(script.cases[0].steps.len(), 2);
    }

    #[test]
    fn parse_two_cases() {
        let src = r#"
case "first" {
  $ true
  assert {
    exit 0
  }
}

case "second" {
  $ false
  assert {
    exit 1
  }
}
"#;
        let script = parse(src).unwrap();
        assert_eq!(script.cases.len(), 2);
        assert_eq!(script.cases[0].name, "first");
        assert_eq!(script.cases[1].name, "second");
    }

    #[test]
    fn parse_single_line_assert_block() {
        let src = r#"
case "inline" {
  $ true
  assert { exit 0 }
}
"#;
        let script = parse(src).unwrap();
        assert_eq!(script.cases[0].steps.len(), 2);
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert_eq!(block.expectations().len(), 1);
    }

    #[test]
    fn parse_multiple_expectations_in_one_block() {
        let src = r#"
case "multi" {
  $ true
  assert {
    exit 0
    exit 0
  }
}
"#;
        let script = parse(src).unwrap();
        assert_eq!(script.cases[0].steps.len(), 2);
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert_eq!(block.expectations().len(), 2);
    }

    #[test]
    fn top_level_action_is_error() {
        let src = "$ true\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn top_level_assert_is_error() {
        let src = "assert { exit 0 }\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn bare_assert_without_block_is_error() {
        let src = r#"
case "x" {
  $ true
  assert exit 0
}
"#;
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn exit_code_999_is_error() {
        let src = r#"
case "x" {
  $ true
  assert {
    exit 999
  }
}
"#;
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::InvalidExitCode { .. }));
    }

    #[test]
    fn exit_code_255_is_valid() {
        let src = r#"
case "x" {
  $ true
  assert {
    exit 255
  }
}
"#;
        assert!(parse(src).is_ok());
    }

    #[test]
    fn unclosed_case_is_error() {
        let src = "case \"x\" {\n  $ true\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn unclosed_assert_block_is_error() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    exit 0\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn empty_assert_block_multi_line_is_error() {
        let src = r#"
case "x" {
  $ true
  assert {
  }
}
"#;
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn empty_assert_block_single_line_is_error() {
        let src = "case \"x\" {\n  $ true\n  assert { }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn exit_without_code_is_error() {
        let src = r#"
case "x" {
  $ true
  assert {
    exit
  }
}
"#;
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn unsupported_expectation_is_error() {
        let src = r#"
case "x" {
  $ true
  assert {
    unknown_assertion
  }
}
"#;
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn action_command_is_stripped_of_dollar_and_trimmed() {
        let src = r#"
case "x" {
  $   echo hello
  assert { exit 0 }
}
"#;
        let script = parse(src).unwrap();
        if let Step::Action(a) = &script.cases[0].steps[0] {
            assert_eq!(a.command, "echo hello");
        } else {
            panic!("expected Action step");
        }
    }

    #[test]
    fn action_inside_assert_block_is_error() {
        let src = r#"
case "x" {
  $ true
  assert {
    $ false
  }
}
"#;
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn two_assertion_blocks_in_one_case_parses_ok() {
        let src = r#"
case "two blocks" {
  assert {
    exit 0
  }
  $ true
  assert {
    exit 0
  }
}
"#;
        let script = parse(src).unwrap();
        assert_eq!(script.cases[0].steps.len(), 3);
    }

    #[test]
    fn parse_stdout_empty() {
        let src = r#"
case "out" {
  $ true
  assert {
    stdout empty
  }
}
"#;
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert!(matches!(
            &block.expectations()[0],
            Expectation::Stdout(e) if matches!(e.matcher, OutputMatcher::Empty)
        ));
    }

    #[test]
    fn parse_stderr_empty() {
        let src = r#"
case "err" {
  $ true
  assert {
    stderr empty
  }
}
"#;
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert!(matches!(
            &block.expectations()[0],
            Expectation::Stderr(e) if matches!(e.matcher, OutputMatcher::Empty)
        ));
    }

    #[test]
    fn parse_stdout_contains() {
        let src = r#"
case "out" {
  $ echo hello
  assert {
    stdout contains "hello"
  }
}
"#;
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert!(matches!(
            &block.expectations()[0],
            Expectation::Stdout(e) if matches!(&e.matcher, OutputMatcher::Contains(s) if s == "hello")
        ));
    }

    #[test]
    fn parse_stderr_contains() {
        let src = r#"
case "err" {
  $ echo err >&2
  assert {
    stderr contains "err"
  }
}
"#;
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert!(matches!(
            &block.expectations()[0],
            Expectation::Stderr(e) if matches!(&e.matcher, OutputMatcher::Contains(s) if s == "err")
        ));
    }

    #[test]
    fn escaped_newline_unescapes_to_actual_newline() {
        let src = r#"
case "x" {
  $ true
  assert {
    stdout contains "a\nb"
  }
}
"#;
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert!(matches!(
            &block.expectations()[0],
            Expectation::Stdout(e) if matches!(&e.matcher, OutputMatcher::Contains(s) if s == "a\nb")
        ));
    }

    #[test]
    fn escaped_backslash_then_n_stays_literal_backslash_n() {
        // `\\n` is an escaped backslash followed by a literal `n`, not an escaped newline.
        // See docs/adr/20260701T214658Z_string-literal-escape-sequences.md.
        let src = r#"
case "x" {
  $ true
  assert {
    stdout contains "a\\nb"
  }
}
"#;
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert!(matches!(
            &block.expectations()[0],
            Expectation::Stdout(e) if matches!(&e.matcher, OutputMatcher::Contains(s) if s == "a\\nb")
        ));
    }

    #[test]
    fn escaped_tab_unescapes_to_actual_tab() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    stdout contains \"a\\tb\"\n  }\n}\n";
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert!(matches!(
            &block.expectations()[0],
            Expectation::Stdout(e) if matches!(&e.matcher, OutputMatcher::Contains(s) if s == "a\tb")
        ));
    }

    #[test]
    fn escaped_quote_does_not_terminate_string() {
        let src =
            "case \"x\" {\n  $ true\n  assert {\n    stdout contains \"say \\\"hi\\\"\"\n  }\n}\n";
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert!(matches!(
            &block.expectations()[0],
            Expectation::Stdout(e) if matches!(&e.matcher, OutputMatcher::Contains(s) if s == "say \"hi\"")
        ));
    }

    #[test]
    fn escaped_quote_in_case_name_does_not_terminate_string() {
        let src = "case \"a \\\"b\\\" c\" {\n  $ true\n  assert {\n    exit 0\n  }\n}\n";
        let script = parse(src).unwrap();
        assert_eq!(script.cases[0].name, "a \"b\" c");

        let err =
            parse("case \"a\\xb\" {\n  $ true\n  assert {\n    exit 0\n  }\n}\n").unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));

        let err = parse("case \"a\nb\" {\n  $ true\n  assert {\n    exit 0\n  }\n}\n").unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn raw_newline_in_string_is_rejected() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    stdout contains \"a\nb\"\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn crlf_raw_newline_in_string_is_rejected() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    stdout contains \"a\r\nb\"\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn bare_cr_in_string_is_rejected() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    stdout contains \"a\rb\"\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn unclosed_string_literal_is_rejected() {
        let src =
            "case \"x\" {\n  $ true\n  assert {\n    stdout contains \"never closed\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn undefined_escape_sequence_is_rejected() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    stdout contains \"a\\xb\"\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn undefined_escape_r_is_rejected() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    stdout contains \"a\\rb\"\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn undefined_unicode_escape_is_rejected() {
        let src =
            "case \"x\" {\n  $ true\n  assert {\n    stdout contains \"a\\u{1245}b\"\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn single_line_assert_multiple_expectations_is_error() {
        let src = r#"
case "x" {
  $ true
  assert { exit 0 exit 1 }
}
"#;
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    // Trailing whitespace on any line must be accepted (parity with the hand-written parser, which called trim() on every line).
    #[test]
    fn trailing_whitespace_is_accepted() {
        // Trailing spaces on case opener, steps, assertion body, and closers.
        let src = "case \"x\" {   \n  $ true   \n  assert {   \n    exit 0   \n  }   \n}   \n";
        assert!(parse(src).is_ok());
    }

    // See #77 / docs/adr/20260705T184047Z_use-hash-comment-marker.md.
    #[test]
    fn line_comment_before_case_block_is_ignored() {
        let src = "# leading comment\ncase \"x\" {\n  $ true\n  assert { exit 0 }\n}\n";
        let script = parse(src).unwrap();
        assert_eq!(script.cases.len(), 1);
    }

    #[test]
    fn comment_only_line_inside_case_block_is_ignored() {
        let src = "case \"x\" {\n  # comment\n  $ true\n  assert { exit 0 }\n}\n";
        let script = parse(src).unwrap();
        assert_eq!(script.cases[0].steps.len(), 2);
    }

    #[test]
    fn comment_only_line_inside_assertion_block_is_ignored() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    # comment\n    exit 0\n  }\n}\n";
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert_eq!(block.expectations().len(), 1);
    }

    #[test]
    fn inline_comment_after_case_and_assertion_block_boundaries_is_ignored() {
        let src = r#"
case "x" { # case open
  assert { # assert open
    exit 0 # expectation
  } # assert close
} # case close
"#;
        let script = parse(src).unwrap();
        assert_eq!(script.cases[0].steps.len(), 1);
    }

    #[test]
    fn inline_comment_after_single_line_assertion_block_is_ignored() {
        let src = "case \"x\" {\n  $ true\n  assert { exit 0 } # trailing\n}\n";
        assert!(parse(src).is_ok());
    }

    #[test]
    fn hash_in_string_literal_is_not_treated_as_comment() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    stdout contains \"hello # world\" # trailing comment\n  }\n}\n";
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert!(matches!(
            &block.expectations()[0],
            Expectation::Stdout(e) if matches!(&e.matcher, OutputMatcher::Contains(s) if s == "hello # world")
        ));
    }

    #[test]
    fn hash_in_action_command_is_preserved_as_command_text() {
        let src = "case \"x\" {\n  $ echo hello # passed to shell\n  assert { exit 0 }\n}\n";
        let script = parse(src).unwrap();
        let Step::Action(action) = &script.cases[0].steps[0] else {
            panic!("expected Action step");
        };
        assert_eq!(action.command, "echo hello # passed to shell");
    }

    // ─── Action line continuation (#80) ────────────────────────────────────

    #[test]
    fn action_continuation_joins_two_physical_lines() {
        let src = "case \"x\" {\n  $ echo one \\\n  two\n  assert { exit 0 }\n}\n";
        let script = parse(src).unwrap();
        assert_eq!(script.cases[0].steps.len(), 2);
        let Step::Action(action) = &script.cases[0].steps[0] else {
            panic!("expected Action step");
        };
        // The marker and the line break are preserved verbatim, as is the
        // continued line's own indentation.
        assert_eq!(action.command, "echo one \\\n  two");
    }

    #[test]
    fn action_continuation_marker_only_line_continues_further() {
        let src = "case \"x\" {\n  $ echo one \\\n\\\ntwo\n  assert { exit 0 }\n}\n";
        let script = parse(src).unwrap();
        assert_eq!(script.cases[0].steps.len(), 2);
        let Step::Action(action) = &script.cases[0].steps[0] else {
            panic!("expected Action step");
        };
        assert_eq!(action.command, "echo one \\\n\\\ntwo");
    }

    #[test]
    fn action_continuation_includes_blank_line_then_resumes_normal_syntax() {
        let src = "case \"x\" {\n  $ echo one \\\n\n  assert { exit 0 }\n}\n";
        let script = parse(src).unwrap();
        assert_eq!(script.cases[0].steps.len(), 2);
        let Step::Action(action) = &script.cases[0].steps[0] else {
            panic!("expected Action step");
        };
        // The blank line itself is consumed as part of this action step (its
        // own newline ends the step, not a further continuation), and the
        // next line resumes as an ordinary case_step.
        assert_eq!(action.command, "echo one \\\n");
    }

    // Per the review note on #80, an action line immediately followed by an
    // `assert {` line is a caution/invalid example, not a valid one: only the
    // `assert {` line is swallowed into the action body (it does not itself
    // end in a marker), so the block's real contents are left dangling as
    // bare `Reportage` syntax and fail to parse.
    #[test]
    fn action_continuation_swallows_only_the_next_line_before_ending() {
        let src = "case \"x\" {\n  $ true \\\n  assert {\n    exit 0\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn action_continuation_marker_followed_by_space_is_not_continuation() {
        let src = "case \"x\" {\n  $ echo hi \\ \n  assert { exit 0 }\n}\n";
        let script = parse(src).unwrap();
        assert_eq!(script.cases[0].steps.len(), 2);
        let Step::Action(action) = &script.cases[0].steps[0] else {
            panic!("expected Action step");
        };
        // The trailing space after `\` is trimmed like any other trailing
        // whitespace, but the `\` itself is ordinary command text, not a
        // continuation marker, since it wasn't the line's last character.
        assert_eq!(action.command, "echo hi \\");
    }

    #[test]
    fn action_continuation_marker_followed_by_hash_is_not_continuation() {
        let src = "case \"x\" {\n  $ echo hi \\# comment\n  assert { exit 0 }\n}\n";
        let script = parse(src).unwrap();
        assert_eq!(script.cases[0].steps.len(), 2);
        let Step::Action(action) = &script.cases[0].steps[0] else {
            panic!("expected Action step");
        };
        assert_eq!(action.command, "echo hi \\# comment");
    }

    #[test]
    fn action_continuation_marker_immediately_before_eof_is_plain_syntax_error() {
        // No dedicated "unterminated continuation" error: EOF right after a
        // marker just leaves the enclosing case block unclosed, the same
        // generic `parse.syntax` failure as any other unclosed block.
        let src = "case \"x\" {\n  $ echo hi \\";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
        assert_eq!(err.code().as_str(), "parse.syntax");
    }

    #[test]
    fn action_continuation_marker_is_a_literal_last_char_check_not_shell_unescaping() {
        // Reportage does not reinterpret shell escaping: only the physical
        // character immediately before the line break decides continuation,
        // so two consecutive backslashes still continue (the first is
        // ordinary command text; the second is the marker).
        let src = "case \"x\" {\n  $ echo hi \\\\\ntwo\n  assert { exit 0 }\n}\n";
        let script = parse(src).unwrap();
        let Step::Action(action) = &script.cases[0].steps[0] else {
            panic!("expected Action step");
        };
        assert_eq!(action.command, "echo hi \\\\\ntwo");
    }

    #[test]
    fn inline_comment_glued_to_token_is_error() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    exit 0#comment\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn double_slash_is_not_a_comment_marker() {
        let src = "// not a comment\ncase \"x\" {\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn comment_presence_does_not_change_ast_shape() {
        let with_comments = r#"
# leading comment
case "x" { # case open
  # standalone comment
  $ true
  assert { # assert open
    # standalone comment
    exit 0 # expectation
  } # assert close
} # case close
"#;
        let without_comments = r#"
case "x" {
  $ true
  assert {
    exit 0
  }
}
"#;
        let with = parse(with_comments).unwrap();
        let without = parse(without_comments).unwrap();
        assert_eq!(format!("{with:?}"), format!("{without:?}"));
    }

    #[test]
    fn comment_splitting_case_header_before_open_brace_is_error() {
        let src = "case \"x\" # comment\n{\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn comment_swallowing_single_line_assertion_close_brace_is_error() {
        let src = "case \"x\" {\n  $ true\n  assert { exit 0 # comment\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn comment_splitting_expectation_tokens_is_error() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    exit # comment\n    0\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    // A comment-only assertion block has no real expectation and must be rejected the same way an empty assertion block is, not accepted with an empty expectations list (which would panic in parse_assertion_block).
    #[test]
    fn comment_only_assertion_block_is_error() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    # comment only\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    // Diagnostic codes are the stable, external identifier of a ParseError.
    // These tests pin the string form directly, independent of the enum variant name and of Display message text.
    // See docs/diagnostics.md.
    #[test]
    fn syntax_error_has_stable_code() {
        let err = parse("$ true\n").unwrap_err();
        assert_eq!(err.code().as_str(), "parse.syntax");

        let diagnostic = err.to_diagnostic();
        assert_eq!(diagnostic.code.as_str(), "parse.syntax");
        assert!(diagnostic.details.pest_message.is_some());
    }

    // A bare `expected X` message doesn't show what's actually on the offending
    // line, which is often the only way to tell a syntax error apart from a
    // deeper structural issue (e.g. a stray fence line closing the wrong block).
    // The Display impl now echoes the source line with a caret under the column.
    #[test]
    fn syntax_error_display_includes_source_line_and_caret() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    exit 0\n  }\n} extra\n";
        let err = parse(src).unwrap_err();
        let rendered = err.to_string();

        assert!(rendered.contains("} extra"));
        assert!(rendered.contains("  |   ^"));
    }

    #[test]
    fn empty_case_has_stable_code() {
        let src = "case \"x\" {\n}\n";
        let err = parse(src).unwrap_err();
        assert_eq!(err.code().as_str(), "parse.empty_case");

        let diagnostic = err.to_diagnostic();
        assert_eq!(diagnostic.code.as_str(), "parse.empty_case");
        assert_eq!(diagnostic.details.raw_value.as_deref(), Some("x"));
    }

    #[test]
    fn missing_assertion_block_has_stable_code() {
        let src = "case \"x\" {\n  $ true\n}\n";
        let err = parse(src).unwrap_err();
        assert_eq!(err.code().as_str(), "parse.missing_assertion_block");

        let diagnostic = err.to_diagnostic();
        assert_eq!(diagnostic.code.as_str(), "parse.missing_assertion_block");
        assert_eq!(diagnostic.details.raw_value.as_deref(), Some("x"));
    }

    #[test]
    fn empty_action_has_stable_code() {
        let src = "case \"x\" {\n  $\n  assert {\n    exit 0\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert_eq!(err.code().as_str(), "parse.empty_action");

        let diagnostic = err.to_diagnostic();
        assert_eq!(diagnostic.code.as_str(), "parse.empty_action");
        assert_eq!(diagnostic.details, DiagnosticDetails::default());
    }

    #[test]
    fn invalid_exit_code_has_stable_code() {
        let src = r#"
case "x" {
  $ true
  assert {
    exit 999
  }
}
"#;
        let err = parse(src).unwrap_err();
        assert_eq!(err.code().as_str(), "parse.invalid_exit_code");

        let diagnostic = err.to_diagnostic();
        assert_eq!(diagnostic.code.as_str(), "parse.invalid_exit_code");
        assert_eq!(diagnostic.details.raw_value.as_deref(), Some("999"));
    }

    #[test]
    fn to_diagnostic_separates_code_message_and_location() {
        let src = r#"
case "x" {
  $ true
  assert {
    exit 999
  }
}
"#;
        let err = parse(src).unwrap_err();
        let diagnostic = err.to_diagnostic();

        assert_eq!(diagnostic.code.as_str(), "parse.invalid_exit_code");
        assert_eq!(diagnostic.message, err.to_string());
        assert_eq!(
            diagnostic.location.expect("location must be present").line,
            5
        );
        assert_eq!(diagnostic.details.raw_value.as_deref(), Some("999"));
    }

    #[test]
    fn parse_file_exists() {
        let src = r#"
case "x" {
  $ true
  assert {
    file <"out/result.json"> exists
  }
}
"#;
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert!(matches!(
            &block.expectations()[0],
            Expectation::File(f) if f.path == "out/result.json" && matches!(f.matcher, FileMatcher::Exists)
        ));
    }

    #[test]
    fn parse_file_contains() {
        let src = r#"
case "x" {
  $ true
  assert {
    file <"out/result.json"> contains "\"status\":\"passed\""
  }
}
"#;
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert!(matches!(
            &block.expectations()[0],
            Expectation::File(f) if f.path == "out/result.json"
                && matches!(&f.matcher, FileMatcher::Contains(s)
                    if s.to_text_value().as_str() == "\"status\":\"passed\"")
        ));
    }

    #[test]
    fn file_exists_and_contains_combine_with_process_expectations() {
        let src = r#"
case "x" {
  $ true
  assert {
    exit 0
    file <"a.txt"> exists
    file <"a.txt"> contains "hi"
  }
}
"#;
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert_eq!(block.expectations().len(), 3);
    }

    // `file <expectation> <path> <...args>` (expectation-first) is not the v0 syntax; only the subject-first `file <"path"> <predicate>` form parses.
    // See docs/adr/20260704T112155Z_subject-first-file-assertion-syntax.md.
    #[test]
    fn expectation_first_file_form_is_rejected() {
        let src = r#"
case "x" {
  $ true
  assert {
    file exists "a.txt"
  }
}
"#;
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn file_predicate_without_path_is_rejected() {
        let src = r#"
case "x" {
  $ true
  assert {
    file exists
  }
}
"#;
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn file_contains_without_text_is_rejected() {
        let src = r#"
case "x" {
  $ true
  assert {
    file <"a.txt"> contains
  }
}
"#;
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    // ─── dir assertions (#66) ───────────────────────────────────────────────

    #[test]
    fn parse_dir_exists() {
        let src = r#"
case "x" {
  $ true
  assert {
    dir <"out"> exists
  }
}
"#;
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert!(matches!(
            &block.expectations()[0],
            Expectation::Dir(d) if d.path == "out" && matches!(d.matcher, DirMatcher::Exists)
        ));
    }

    #[test]
    fn parse_dir_contains() {
        let src = r#"
case "x" {
  $ true
  assert {
    dir <"artifacts"> contains "result.json"
  }
}
"#;
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert!(matches!(
            &block.expectations()[0],
            Expectation::Dir(d) if d.path == "artifacts"
                && matches!(&d.matcher, DirMatcher::Contains(s) if s == "result.json")
        ));
    }

    #[test]
    fn dir_exists_and_contains_combine_with_process_expectations() {
        let src = r#"
case "x" {
  $ true
  assert {
    exit 0
    dir <"a"> exists
    dir <"a"> contains "b"
  }
}
"#;
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert_eq!(block.expectations().len(), 3);
    }

    // `dir <expectation> <path> <...args>` (expectation-first) is not the v0 syntax; only the subject-first `dir <"path"> <predicate>` form parses.
    // See docs/adr/20260706T000000Z_subject-first-directory-assertion-syntax.md.
    #[test]
    fn expectation_first_dir_form_is_rejected() {
        let src = r#"
case "x" {
  $ true
  assert {
    dir exists "a"
  }
}
"#;
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn dir_predicate_without_path_is_rejected() {
        let src = r#"
case "x" {
  $ true
  assert {
    dir exists
  }
}
"#;
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn dir_contains_without_name_is_rejected() {
        let src = r#"
case "x" {
  $ true
  assert {
    dir <"a"> contains
  }
}
"#;
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    // ─── Logical composition (#25) ────────────────────────────────────────

    fn logical_children(expectation: &Expectation) -> &[Expectation] {
        match expectation {
            Expectation::Logical(l) => l.children(),
            other => panic!("expected Expectation::Logical, got {other:?}"),
        }
    }

    #[test]
    fn parse_not_block_single_line() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    not { exit 1 }\n  }\n}\n";
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        let Expectation::Logical(l) = &block.expectations()[0] else {
            panic!("expected Logical expectation");
        };
        assert!(matches!(l.operator(), LogicalOperator::Not));
        assert_eq!(l.children().len(), 1);
        assert!(matches!(l.children()[0], Expectation::Exit(_)));
    }

    #[test]
    fn parse_all_block_multi_line() {
        let src = r#"
case "x" {
  $ true
  assert {
    all {
      exit 0
      stdout empty
    }
  }
}
"#;
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        let Expectation::Logical(l) = &block.expectations()[0] else {
            panic!("expected Logical expectation");
        };
        assert!(matches!(l.operator(), LogicalOperator::All));
        assert_eq!(l.children().len(), 2);
    }

    #[test]
    fn parse_any_block_multi_line() {
        let src = r#"
case "x" {
  $ true
  assert {
    any {
      exit 0
      exit 1
    }
  }
}
"#;
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert!(matches!(
            &block.expectations()[0],
            Expectation::Logical(l) if matches!(l.operator(), LogicalOperator::Any)
        ));
    }

    #[test]
    fn parse_nested_logical_composition() {
        let src = r#"
case "x" {
  $ true
  assert {
    all {
      not {
        exit 1
      }
      any {
        exit 0
        exit 2
      }
    }
  }
}
"#;
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        let outer_children = logical_children(&block.expectations()[0]);
        assert_eq!(outer_children.len(), 2);
        assert!(matches!(
            &outer_children[0],
            Expectation::Logical(l) if matches!(l.operator(), LogicalOperator::Not)
        ));
        assert!(matches!(
            &outer_children[1],
            Expectation::Logical(l) if matches!(l.operator(), LogicalOperator::Any)
        ));
    }

    #[test]
    fn empty_not_block_is_semantic_empty_block_error() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    not { }\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::EmptyLogicalCompositionBlock {
                operator: LogicalOperator::Not,
                ..
            }
        ));
        assert_eq!(err.code().as_str(), "semantic.expectation.empty_block");
    }

    #[test]
    fn empty_all_block_multi_line_is_semantic_empty_block_error() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    all {\n    }\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::EmptyLogicalCompositionBlock {
                operator: LogicalOperator::All,
                ..
            }
        ));
        assert_eq!(err.code().as_str(), "semantic.expectation.empty_block");
    }

    #[test]
    fn empty_any_block_with_comment_only_is_semantic_empty_block_error() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    any {\n      # no expectations here\n    }\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::EmptyLogicalCompositionBlock {
                operator: LogicalOperator::Any,
                ..
            }
        ));
    }

    #[test]
    fn empty_logical_composition_block_diagnostic_details_record_operator() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    all { }\n  }\n}\n";
        let err = parse(src).unwrap_err();
        let diagnostic = err.to_diagnostic();
        assert_eq!(diagnostic.code.as_str(), "semantic.expectation.empty_block");
        assert_eq!(diagnostic.details.raw_value.as_deref(), Some("all"));
    }

    #[test]
    fn and_block_is_not_accepted_as_logical_composition() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    and { exit 0 }\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn or_block_is_not_accepted_as_logical_composition() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    or { exit 0 }\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn infix_and_between_expectations_is_rejected() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    exit 0 and exit 0\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn infix_or_between_expectations_is_rejected() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    exit 0 or exit 1\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn single_line_composition_block_multiple_expectations_is_error() {
        // Mirrors single_line_assert_multiple_expectations_is_error: a composition block's single-line form accepts exactly one expectation, same as assert { ... }'s.
        let src = "case \"x\" {\n  $ true\n  assert {\n    all { exit 0 exit 1 }\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    // ─── Write step: string literal / heredoc literal (#67, #86) ──────────

    fn write_file_step(script: &Script) -> &WriteFileStep {
        let Step::SideEffect(SideEffectingStep::WriteFile(step)) = &script.cases[0].steps[0] else {
            panic!("expected first step to be a write step");
        };
        step
    }

    #[test]
    fn parse_basic_write_step() {
        let src = "case \"x\" {\n  write <\"a.txt\"> ```\n    hello\n    ```\n  $ true\n  assert {\n    exit 0\n  }\n}\n";
        let script = parse(src).unwrap();
        let step = write_file_step(&script);
        assert_eq!(step.path.as_str(), "a.txt");
        assert_eq!(step.content.to_text_value().as_str(), "hello\n");
        assert_eq!(script.cases[0].steps.len(), 3);
    }

    #[test]
    fn write_step_can_follow_an_action_in_source_order() {
        let src = "case \"x\" {\n  $ true\n  write <\"a.txt\"> ```\n    hello\n    ```\n  assert { exit 0 }\n}\n";
        let script = parse(src).unwrap();
        let Step::SideEffect(SideEffectingStep::WriteFile(step)) = &script.cases[0].steps[1] else {
            panic!("expected second step to be a write step");
        };
        assert_eq!(step.path.as_str(), "a.txt");
        assert_eq!(step.content.to_text_value().as_str(), "hello\n");
    }

    #[test]
    fn write_step_empty_block_content_is_empty_string() {
        let src = "case \"x\" {\n  write <\"empty.txt\"> ```\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
        let script = parse(src).unwrap();
        let step = write_file_step(&script);
        assert_eq!(step.content.to_text_value().as_str(), "");
    }

    #[test]
    fn write_step_blank_line_is_preserved_as_empty_line_after_dedent() {
        let src = "case \"x\" {\n  write <\"a.txt\"> ```\n    first\n\n    third\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
        let script = parse(src).unwrap();
        let step = write_file_step(&script);
        assert_eq!(step.content.to_text_value().as_str(), "first\n\nthird\n");
    }

    #[test]
    fn write_step_whitespace_only_line_is_dedented_to_empty_line() {
        // The blank line has trailing spaces shallower than the closing fence's indent;
        // it must still be exempt from the shallow-indent check and dedent to empty.
        let src = "case \"x\" {\n  write <\"a.txt\"> ```\n    first\n  \n    third\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
        let script = parse(src).unwrap();
        let step = write_file_step(&script);
        assert_eq!(step.content.to_text_value().as_str(), "first\n\nthird\n");
    }

    #[test]
    fn write_step_tab_indent_is_treated_as_literal_prefix_not_width() {
        // Closing fence indented with a tab; body lines must match that exact
        // tab character as a string prefix, not a width-equivalent number of spaces.
        let src = "case \"x\" {\n  write <\"a.txt\"> ```\n\thello\n\t```\n  $ true\n  assert { exit 0 }\n}\n";
        let script = parse(src).unwrap();
        let step = write_file_step(&script);
        assert_eq!(step.content.to_text_value().as_str(), "hello\n");
    }

    #[test]
    fn write_step_crlf_line_endings_are_preserved() {
        let src = "case \"x\" {\r\n  write <\"a.txt\"> ```\r\n    hello\r\n    ```\r\n  $ true\r\n  assert { exit 0 }\r\n}\r\n";
        let script = parse(src).unwrap();
        let step = write_file_step(&script);
        assert_eq!(step.content.to_text_value().as_str(), "hello\r\n");
    }

    #[test]
    fn write_step_content_preserves_variable_looking_text_literally() {
        let src = "case \"x\" {\n  write <\"a.txt\"> ```\n    ${ENTRY_KIND}\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
        let script = parse(src).unwrap();
        let step = write_file_step(&script);
        assert_eq!(step.content.to_text_value().as_str(), "${ENTRY_KIND}\n");
    }

    #[test]
    fn write_step_closing_fence_longer_than_opening_is_accepted() {
        let src = "case \"x\" {\n  write <\"a.txt\"> ```\n    hello\n    ````\n  $ true\n  assert { exit 0 }\n}\n";
        let script = parse(src).unwrap();
        let step = write_file_step(&script);
        assert_eq!(step.content.to_text_value().as_str(), "hello\n");
    }

    #[test]
    fn write_step_longer_opening_fence_allows_embedded_triple_backticks() {
        let src = "case \"x\" {\n  write <\"a.md\"> ````\n    ```ts\n    console.log(1)\n    ```\n    ````\n  $ true\n  assert { exit 0 }\n}\n";
        let script = parse(src).unwrap();
        let step = write_file_step(&script);
        assert_eq!(
            step.content.to_text_value().as_str(),
            "```ts\nconsole.log(1)\n```\n"
        );
    }

    #[test]
    fn write_step_shallow_indent_is_rejected() {
        // "mid" is indented less than the closing fence's 4 spaces.
        let src = "case \"x\" {\n  write <\"a.txt\"> ```\n    first\n  mid\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::ShallowHeredocIndent { .. }));
        assert_eq!(err.code().as_str(), "parse.heredoc_literal.shallow_indent");
    }

    #[test]
    fn write_step_unterminated_fence_is_a_syntax_error() {
        let src =
            "case \"x\" {\n  write <\"a.txt\"> ```\n    hello\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn write_step_opening_fence_inline_comment_is_rejected() {
        let src = "case \"x\" {\n  write <\"a.txt\"> ``` # comment\n    hello\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn write_step_absolute_path_is_rejected() {
        let src = "case \"x\" {\n  write <\"/etc/passwd\"> ```\n    x\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidWorkspacePath {
                reason: WorkspacePathError::Absolute,
                ..
            }
        ));
        assert_eq!(err.code().as_str(), "semantic.workspace_path.absolute");
    }

    #[test]
    fn write_step_dot_segment_path_is_rejected() {
        let src = "case \"x\" {\n  write <\"../a.txt\"> ```\n    x\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidWorkspacePath {
                reason: WorkspacePathError::DotSegment,
                ..
            }
        ));
        assert_eq!(err.code().as_str(), "semantic.workspace_path.dot_segment");
    }

    #[test]
    fn invalid_workspace_path_message_names_its_own_position_not_write_step() {
        // `WorkspacePath::parse` backs both a `write` step's target path and a `contents_equals`
        // expected `<"...">` value. The Display message must name whichever position the raw
        // path actually came from, not hardcode "write step path" regardless of origin.
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"actual.txt\"> contents_equals <\"../expected.txt\">\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidWorkspacePath {
                reason: WorkspacePathError::DotSegment,
                position: "`file contents_equals` expected value",
                ..
            }
        ));
        let message = err.to_string();
        assert!(message.contains("`file contents_equals` expected value"));
        assert!(!message.contains("write step"));
    }

    #[test]
    fn write_step_empty_path_is_rejected() {
        let src =
            "case \"x\" {\n  write <\"\"> ```\n    x\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidWorkspacePath {
                reason: WorkspacePathError::Empty,
                ..
            }
        ));
        assert_eq!(err.code().as_str(), "semantic.workspace_path.empty");
    }

    #[test]
    fn multiple_write_steps_are_kept_in_source_order() {
        let src = "case \"x\" {\n  write <\"a.txt\"> ```\n    a\n    ```\n  write <\"b.txt\"> ```\n    b\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
        let script = parse(src).unwrap();
        assert_eq!(script.cases[0].steps.len(), 4);
        let Step::SideEffect(SideEffectingStep::WriteFile(first)) = &script.cases[0].steps[0]
        else {
            panic!("expected write step");
        };
        let Step::SideEffect(SideEffectingStep::WriteFile(second)) = &script.cases[0].steps[1]
        else {
            panic!("expected write step");
        };
        assert_eq!(first.path.as_str(), "a.txt");
        assert_eq!(second.path.as_str(), "b.txt");
    }

    // Known limitation (documented in docs/semantics.md and the ADR): a
    // `write` step missing its own closing fence does not always produce a
    // syntax error. The grammar scans forward for the next line shaped like
    // a valid closing fence, which here belongs to what the author intended
    // as a *separate* `write <"b.txt">` step. That step's opening line is
    // silently absorbed as literal content of `a.txt`, and `b.txt`'s write
    // step disappears from the AST entirely — this test pins that exact
    // behavior so a future grammar change cannot silently alter it further
    // without a test failure calling it out.
    #[test]
    fn missing_closing_fence_silently_absorbs_a_later_write_step_as_content() {
        let src = concat!(
            "case \"x\" {\n",
            "  write <\"a.txt\"> ```\n",
            "    first\n",
            "    write <\"b.txt\"> ```\n",
            "    second\n",
            "    ```\n",
            "  $ true\n",
            "  assert { exit 0 }\n",
            "}\n",
        );
        let script = parse(src).unwrap();

        // Only 3 steps: the intended `write <"b.txt">` step never materializes.
        assert_eq!(script.cases[0].steps.len(), 3);

        let step = write_file_step(&script);
        assert_eq!(step.path.as_str(), "a.txt");
        assert_eq!(
            step.content.to_text_value().as_str(),
            "first\nwrite <\"b.txt\"> ```\nsecond\n"
        );

        assert!(matches!(script.cases[0].steps[1], Step::Action(_)));
        assert!(matches!(script.cases[0].steps[2], Step::AssertionBlock(_)));
    }

    // ─── Workspace path literal / literal kind mismatch (#93) ──────────────

    #[test]
    fn workspace_path_literal_reuses_string_literal_escape_rules() {
        // The inner quoted content of <"..."> shares quoted_string's escape
        // rules; the unescaped value is what reaches the AST.
        let src =
            "case \"x\" {\n  write <\"a\\tb.txt\"> \"content\"\n  $ true\n  assert { exit 0 }\n}\n";
        let script = parse(src).unwrap();
        let step = write_file_step(&script);
        assert_eq!(step.path.as_str(), "a\tb.txt");
    }

    #[test]
    fn file_subject_string_literal_is_kind_mismatch() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file \"out.txt\" exists\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::WorkspacePath,
                actual: ValueLiteralKind::StringLiteral,
                ..
            }
        ));
        assert_eq!(err.code().as_str(), "semantic.literal.kind_mismatch");

        // The message must be actionable: expected kind, actual kind, and
        // the suggested replacement.
        let message = err.to_string();
        assert!(message.contains("`file` checkpoint subject"));
        assert!(message.contains("WorkspacePath"));
        assert!(message.contains("StringLiteral"));
        assert!(message.contains("<\"out.txt\">"));
    }

    #[test]
    fn file_subject_fixture_reference_is_kind_mismatch() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file @\"out.txt\" exists\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::WorkspacePath,
                actual: ValueLiteralKind::FixtureReference,
                ..
            }
        ));

        let message = err.to_string();
        assert!(message.contains("FixtureReference"));
        assert!(message.contains("@\"out.txt\""));
        assert!(message.contains("<\"out.txt\">"));
    }

    #[test]
    fn heredoc_file_contains_subject_string_literal_is_kind_mismatch() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file \"out.txt\" contains ```\n    hi\n    ```\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::WorkspacePath,
                ..
            }
        ));
    }

    #[test]
    fn dir_subject_string_literal_is_kind_mismatch() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    dir \"out\" exists\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::WorkspacePath,
                actual: ValueLiteralKind::StringLiteral,
                ..
            }
        ));
        let message = err.to_string();
        assert!(message.contains("`dir` checkpoint subject"));
        assert!(message.contains("<\"out\">"));
    }

    #[test]
    fn write_path_string_literal_is_kind_mismatch() {
        let src = "case \"x\" {\n  write \"a.txt\" \"content\"\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::WorkspacePath,
                actual: ValueLiteralKind::StringLiteral,
                ..
            }
        ));
        let message = err.to_string();
        assert!(message.contains("`write` step path"));
    }

    #[test]
    fn write_heredoc_path_string_literal_is_kind_mismatch() {
        let src = "case \"x\" {\n  write \"a.txt\" ```\n    hello\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::WorkspacePath,
                ..
            }
        ));
    }

    #[test]
    fn write_content_workspace_path_literal_is_kind_mismatch() {
        let src =
            "case \"x\" {\n  write <\"a.txt\"> <\"b.txt\">\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::TextValue,
                actual: ValueLiteralKind::WorkspacePath,
                ..
            }
        ));
        let message = err.to_string();
        assert!(message.contains("`write` step content"));
        assert!(message.contains("string literal or heredoc literal"));
    }

    #[test]
    fn stdout_contains_workspace_path_literal_is_kind_mismatch() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    stdout contains <\"expected.stdout\">\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::TextValue,
                actual: ValueLiteralKind::WorkspacePath,
                ..
            }
        ));
        let message = err.to_string();
        assert!(message.contains("`stdout contains` expected text"));
        assert!(message.contains("TextValue"));
        // v0's grammar only wires the heredoc TextValue form into `write`
        // content and `file contains`; the suggestion here must not steer
        // the author toward a heredoc literal the grammar would reject.
        assert!(message.contains("use \"expected.stdout\" instead"));
        assert!(!message.contains("heredoc"));
    }

    #[test]
    fn stderr_contains_fixture_reference_is_kind_mismatch() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    stderr contains @\"expected.stderr\"\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::TextValue,
                actual: ValueLiteralKind::FixtureReference,
                ..
            }
        ));
        let message = err.to_string();
        assert!(message.contains("`stderr contains` expected text"));
        assert!(message.contains("use \"expected.stderr\" instead"));
        assert!(!message.contains("heredoc"));
    }

    #[test]
    fn file_contains_expected_workspace_path_literal_is_kind_mismatch() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> contains <\"expected.txt\">\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::TextValue,
                actual: ValueLiteralKind::WorkspacePath,
                ..
            }
        ));
        let message = err.to_string();
        assert!(message.contains("`file contains` expected text"));
    }

    #[test]
    fn dir_contains_entry_workspace_path_literal_is_kind_mismatch() {
        let src =
            "case \"x\" {\n  $ true\n  assert {\n    dir <\"out\"> contains <\"entry\">\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::StringLiteral,
                actual: ValueLiteralKind::WorkspacePath,
                ..
            }
        ));
        // The suggestion for a StringLiteral requirement is the same quoted
        // content without the workspace path wrapper.
        let message = err.to_string();
        assert!(message.contains("`dir contains` entry name"));
        assert!(message.contains("use \"entry\" instead"));
    }

    #[test]
    fn literal_kind_mismatch_diagnostic_carries_expected_actual_and_suggestion() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file \"out.txt\" exists\n  }\n}\n";
        let err = parse(src).unwrap_err();
        let diagnostic = err.to_diagnostic();

        assert_eq!(diagnostic.code.as_str(), "semantic.literal.kind_mismatch");
        assert_eq!(
            diagnostic.location.expect("location must be present").line,
            4
        );
        assert_eq!(diagnostic.details.raw_value.as_deref(), Some("\"out.txt\""));
        assert_eq!(
            diagnostic.details.expected_kind.as_deref(),
            Some("WorkspacePath")
        );
        assert_eq!(
            diagnostic.details.actual_kind.as_deref(),
            Some("StringLiteral")
        );
        assert_eq!(
            diagnostic.details.suggestion.as_deref(),
            Some("<\"out.txt\">")
        );
    }

    #[test]
    fn whitespace_between_path_marker_and_quote_is_syntax_error() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file < \"out.txt\"> exists\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));

        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\" > exists\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    // ─── Fixture reference literal / contents_equals / text_equals (#92) ───

    #[test]
    fn file_contents_equals_accepts_workspace_path_literal() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> contents_equals <\"expected.txt\">\n  }\n}\n";
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected assertion block");
        };
        let Expectation::File(file_exp) = &block.expectations()[0] else {
            panic!("expected file expectation");
        };
        assert!(matches!(
            file_exp.matcher,
            FileMatcher::ContentsEquals(FileContentsReference::Workspace(_))
        ));
    }

    #[test]
    fn file_contents_equals_accepts_fixture_reference_literal() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> contents_equals @\"expected.txt\"\n  }\n}\n";
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected assertion block");
        };
        let Expectation::File(file_exp) = &block.expectations()[0] else {
            panic!("expected file expectation");
        };
        match &file_exp.matcher {
            FileMatcher::ContentsEquals(FileContentsReference::Fixture(fixture)) => {
                assert_eq!(fixture.as_str(), "expected.txt");
            }
            other => panic!("expected fixture contents_equals, got {other:?}"),
        }
    }

    #[test]
    fn file_contents_equals_rejects_string_literal() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> contents_equals \"expected.txt\"\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::FileContentsReference,
                actual: ValueLiteralKind::StringLiteral,
                ..
            }
        ));
        let message = err.to_string();
        assert!(message.contains("`file contents_equals` expected value"));
        assert!(message.contains("workspace path literal or fixture reference literal"));
    }

    #[test]
    fn file_contents_equals_subject_fixture_reference_is_kind_mismatch() {
        // The `file` checkpoint subject requires a WorkspacePath, never a
        // FixtureReference, regardless of which predicate follows it.
        let src = "case \"x\" {\n  $ true\n  assert {\n    file @\"actual.txt\" contents_equals @\"expected.txt\"\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::WorkspacePath,
                actual: ValueLiteralKind::FixtureReference,
                ..
            }
        ));
    }

    #[test]
    fn file_text_equals_accepts_string_literal() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> text_equals \"expected\"\n  }\n}\n";
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected assertion block");
        };
        let Expectation::File(file_exp) = &block.expectations()[0] else {
            panic!("expected file expectation");
        };
        match &file_exp.matcher {
            FileMatcher::TextEquals(TextLiteral::Quoted(value)) => {
                assert_eq!(value, "expected");
            }
            other => panic!("expected quoted text_equals, got {other:?}"),
        }
    }

    #[test]
    fn file_text_equals_rejects_fixture_reference() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> text_equals @\"expected.txt\"\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::TextValue,
                actual: ValueLiteralKind::FixtureReference,
                ..
            }
        ));
        let message = err.to_string();
        assert!(message.contains("`file text_equals` expected text"));
    }

    #[test]
    fn file_text_equals_rejects_workspace_path_literal() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> text_equals <\"expected.txt\">\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::TextValue,
                actual: ValueLiteralKind::WorkspacePath,
                ..
            }
        ));
        let message = err.to_string();
        assert!(message.contains("`file text_equals` expected text"));
        assert!(message.contains("string literal or heredoc literal"));
    }

    #[test]
    fn file_text_equals_accepts_heredoc_literal() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> text_equals ```\n    hello\n    world\n    ```\n  }\n}\n";
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected assertion block");
        };
        let Expectation::File(file_exp) = &block.expectations()[0] else {
            panic!("expected file expectation");
        };
        match &file_exp.matcher {
            FileMatcher::TextEquals(TextLiteral::Heredoc(value)) => {
                assert_eq!(value, "hello\nworld\n");
            }
            other => panic!("expected heredoc text_equals, got {other:?}"),
        }
    }

    #[test]
    fn heredoc_file_text_equals_subject_string_literal_is_kind_mismatch() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file \"out.txt\" text_equals ```\n    hi\n    ```\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::WorkspacePath,
                ..
            }
        ));
    }

    #[test]
    fn stdout_contents_equals_accepts_fixture_reference_literal() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    stdout contents_equals @\"stdout.snapshot\"\n  }\n}\n";
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected assertion block");
        };
        assert!(matches!(
            &block.expectations()[0],
            Expectation::Stdout(OutputExpectation {
                matcher: OutputMatcher::ContentsEquals(FileContentsReference::Fixture(_)),
            })
        ));
    }

    #[test]
    fn stderr_contents_equals_accepts_workspace_path_literal() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    stderr contents_equals <\"expected.txt\">\n  }\n}\n";
        let script = parse(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected assertion block");
        };
        assert!(matches!(
            &block.expectations()[0],
            Expectation::Stderr(OutputExpectation {
                matcher: OutputMatcher::ContentsEquals(FileContentsReference::Workspace(_)),
            })
        ));
    }

    #[test]
    fn fixture_reference_empty_path_is_rejected() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> contents_equals @\"\"\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidFixtureReference {
                reason: FixtureReferenceError::Empty,
                ..
            }
        ));
        assert_eq!(err.code().as_str(), "semantic.fixture_reference.empty");
    }

    #[test]
    fn fixture_reference_absolute_path_is_rejected() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> contents_equals @\"/etc/passwd\"\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidFixtureReference {
                reason: FixtureReferenceError::Absolute,
                ..
            }
        ));
        assert_eq!(err.code().as_str(), "semantic.fixture_reference.absolute");
    }

    #[test]
    fn fixture_reference_dot_segment_leading_parent_path_is_rejected() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> contents_equals @\"../escape.txt\"\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidFixtureReference {
                reason: FixtureReferenceError::DotSegment,
                ..
            }
        ));
        assert_eq!(
            err.code().as_str(),
            "semantic.fixture_reference.dot_segment"
        );
    }

    #[test]
    fn fixture_reference_dot_segment_leading_current_path_is_rejected() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> contents_equals @\"./escape.txt\"\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidFixtureReference {
                reason: FixtureReferenceError::DotSegment,
                ..
            }
        ));
        assert_eq!(
            err.code().as_str(),
            "semantic.fixture_reference.dot_segment"
        );
    }

    #[test]
    fn fixture_reference_dot_segment_middle_current_path_is_rejected() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> contents_equals @\"snapshots/./stdout.json\"\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidFixtureReference {
                reason: FixtureReferenceError::DotSegment,
                ..
            }
        ));
        assert_eq!(
            err.code().as_str(),
            "semantic.fixture_reference.dot_segment"
        );
    }

    #[test]
    fn fixture_reference_dot_segment_middle_parent_path_is_rejected() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> contents_equals @\"snapshots/../stdout.json\"\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidFixtureReference {
                reason: FixtureReferenceError::DotSegment,
                ..
            }
        ));
        assert_eq!(
            err.code().as_str(),
            "semantic.fixture_reference.dot_segment"
        );
    }

    #[test]
    fn write_step_content_fixture_reference_is_kind_mismatch() {
        // Outside an assertion block, a fixture reference literal is still
        // just a value_literal whose kind never matches a write step's
        // TextValue content requirement: fixture references are only valid
        // in a FileContentsReference expected position (#92).
        let src = "case \"x\" {\n  write <\"out.txt\"> @\"expected.txt\"\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::TextValue,
                actual: ValueLiteralKind::FixtureReference,
                ..
            }
        ));
    }

    #[test]
    fn write_step_path_fixture_reference_is_kind_mismatch() {
        let src =
            "case \"x\" {\n  write @\"out.txt\" \"content\"\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::WorkspacePath,
                actual: ValueLiteralKind::FixtureReference,
                ..
            }
        ));
    }

    #[test]
    fn workspace_path_literal_value_validation_still_applies_to_write_path() {
        // Kind and value validation are separate layers: a correctly-kinded
        // workspace path literal whose unescaped value violates the
        // workspace path policy still fails with the existing
        // semantic.workspace_path.* diagnostics.
        let src =
            "case \"x\" {\n  write <\"/etc/passwd\"> \"x\"\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidWorkspacePath {
                reason: WorkspacePathError::Absolute,
                ..
            }
        ));
    }
}
