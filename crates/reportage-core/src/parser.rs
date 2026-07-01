use pest::Parser;
use pest_derive::Parser;

use crate::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticDetails, DiagnosticLocation};
use crate::model::{
    ActionStep, AssertionBlock, Case, ExitExpectation, Expectation, OutputExpectation,
    OutputMatcher, Script, Step,
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
    },
    /// A case block must contain at least one step.
    EmptyCase { line: usize, name: String },
    /// A case block must contain at least one assertion block.
    MissingAssertionBlock { line: usize, name: String },
    /// An action step must contain a non-empty command after trimming whitespace.
    EmptyAction { line: usize },
    /// Exit code is outside the valid range 0..=255.
    InvalidExitCode { line: usize, value: String },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Syntax {
                line,
                column,
                message,
            } => write!(f, "parse error at line {line}, column {column}: {message}"),
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
        }
    }
}

impl std::error::Error for ParseError {}

impl ParseError {
    /// The stable, machine-readable diagnostic code for this error.
    ///
    /// This is independent of the enum variant name: downstream tests and tooling should
    /// depend on this code (or its string form) rather than on `Display` output.
    /// See docs/diagnostics.md.
    pub const fn code(&self) -> DiagnosticCode {
        match self {
            ParseError::Syntax { .. } => DiagnosticCode::ParseSyntax,
            ParseError::EmptyCase { .. } => DiagnosticCode::ParseEmptyCase,
            ParseError::MissingAssertionBlock { .. } => DiagnosticCode::ParseMissingAssertionBlock,
            ParseError::EmptyAction { .. } => DiagnosticCode::ParseEmptyAction,
            ParseError::InvalidExitCode { .. } => DiagnosticCode::ParseInvalidExitCode,
        }
    }

    /// Converts this error into the struct-based diagnostic model, separating
    /// the stable `code` from the improvable `message`, `location`, and the
    /// weaker-stability `details`.
    pub fn to_diagnostic(&self) -> Diagnostic {
        let (location, details) = match self {
            ParseError::Syntax {
                line,
                column,
                message,
            } => (
                Some(DiagnosticLocation {
                    line: *line,
                    column: Some(*column),
                }),
                DiagnosticDetails {
                    pest_message: Some(message.clone()),
                    raw_value: None,
                },
            ),
            ParseError::EmptyCase { line, name } => (
                Some(DiagnosticLocation {
                    line: *line,
                    column: None,
                }),
                DiagnosticDetails {
                    pest_message: None,
                    raw_value: Some(name.clone()),
                },
            ),
            ParseError::MissingAssertionBlock { line, name } => (
                Some(DiagnosticLocation {
                    line: *line,
                    column: None,
                }),
                DiagnosticDetails {
                    pest_message: None,
                    raw_value: Some(name.clone()),
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
                    pest_message: None,
                    raw_value: Some(value.clone()),
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
/// The grammar's `string_char` rule only accepts `\\`, `\"`, `\n`, and `\t` as
/// escape sequences, so every `\` in `raw` is guaranteed to be followed by one
/// of those four characters.
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

fn parse_action_step(pair: pest::iterators::Pair<Rule>) -> Result<Step, ParseError> {
    // action_step = { "$" ~ ws* ~ command }
    let line = pair.line_col().0;
    let command = pair
        .into_inner()
        .next()
        .expect("action_step must have command")
        .as_str()
        .trim()
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

    let expectations: Vec<Expectation> = match body.as_rule() {
        Rule::single_assert => {
            // single_assert = { ws* ~ expectation ~ ws* }
            let exp_pair = body
                .into_inner()
                .next()
                .expect("single_assert must have expectation");
            vec![parse_expectation(exp_pair)?]
        }
        Rule::multi_assert => {
            // multi_assert = { nl ~ assertion_line+ ~ ws* }
            // assertion_line is silent, so its child (expectation) is promoted.
            body.into_inner()
                .map(|p| parse_expectation(p))
                .collect::<Result<Vec<_>, _>>()?
        }
        rule => unreachable!("unexpected rule in assertion_block: {rule:?}"),
    };

    let block = AssertionBlock::new(expectations)
        .expect("grammar guarantees at least one expectation in assertion block");
    Ok(Step::AssertionBlock(block))
}

fn parse_expectation(pair: pest::iterators::Pair<Rule>) -> Result<Expectation, ParseError> {
    // expectation = { exit_exp | stdout_exp | stderr_exp }
    let inner = pair
        .into_inner()
        .next()
        .expect("expectation must have inner rule");

    match inner.as_rule() {
        Rule::exit_exp => parse_exit_exp(inner),
        Rule::stdout_exp => parse_output_exp(inner, true),
        Rule::stderr_exp => parse_output_exp(inner, false),
        rule => unreachable!("unexpected rule in expectation: {rule:?}"),
    }
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
            // output_contains = { "contains" ~ ws+ ~ quoted_string }
            let qs = inner
                .into_inner()
                .next()
                .expect("output_contains must have quoted_string");
            OutputMatcher::Contains(extract_string_inner(qs))
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
        // `\\n` is an escaped backslash followed by a literal `n`, not an
        // escaped newline. See docs/adr/20260701T214658Z_string-literal-escape-sequences.md.
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

    // Trailing whitespace on any line must be accepted (parity with the
    // hand-written parser, which called trim() on every line).
    #[test]
    fn trailing_whitespace_is_accepted() {
        // Trailing spaces on case opener, steps, assertion body, and closers.
        let src = "case \"x\" {   \n  $ true   \n  assert {   \n    exit 0   \n  }   \n}   \n";
        assert!(parse(src).is_ok());
    }

    // Diagnostic codes are the stable, external identifier of a ParseError.
    // These tests pin the string form directly, independent of the enum
    // variant name and of Display message text. See docs/diagnostics.md.
    #[test]
    fn syntax_error_has_stable_code() {
        let err = parse("$ true\n").unwrap_err();
        assert_eq!(err.code().as_str(), "parse.syntax");

        let diagnostic = err.to_diagnostic();
        assert_eq!(diagnostic.code.as_str(), "parse.syntax");
        assert!(diagnostic.details.pest_message.is_some());
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
}
