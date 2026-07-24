use super::heredoc::parse_heredoc_literal;
use super::literal::{RequiredKind, parse_file_contents_reference, parse_value_literal};
use super::{ParseError, Rule};
use crate::model::{
    AssertionBlock, DirExpectation, DirMatcher, ExitExpectation, Expectation, FileExpectation,
    FileMatcher, LogicalExpectation, LogicalOperator, OutputExpectation, OutputMatcher, Step,
    TextLiteral,
};

pub(super) fn parse_assertion_block(pair: pest::iterators::Pair<Rule>) -> Result<Step, ParseError> {
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
        Rule::output_text_equals => {
            // output_text_equals = { "text_equals" ~ ws+ ~ value_literal }
            let literal_pair = inner
                .into_inner()
                .next()
                .expect("output_text_equals must have value_literal");
            let position = if is_stdout {
                "`stdout text_equals` expected text"
            } else {
                "`stderr text_equals` expected text"
            };
            let expected = parse_value_literal(literal_pair)
                .expect_kind(RequiredKind::TextValueStringOrHeredoc, position)?;
            OutputMatcher::TextEquals(TextLiteral::Quoted(expected))
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
/// text_equals` / `stdout text_equals` / `stderr text_equals`, reachable only
/// through `multi_assert` (see `heredoc_assertion_line` in the grammar).
fn parse_heredoc_expectation(pair: pest::iterators::Pair<Rule>) -> Result<Expectation, ParseError> {
    // heredoc_expectation = { file_exp_heredoc | file_text_equals_heredoc
    //                         | stdout_text_equals_heredoc | stderr_text_equals_heredoc }
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
        Rule::stdout_text_equals_heredoc | Rule::stderr_text_equals_heredoc => {
            // stdout_text_equals_heredoc = { "stdout" ~ ws+ ~ "text_equals" ~ ws+ ~ heredoc_literal }
            // stderr_text_equals_heredoc = { "stderr" ~ ws+ ~ "text_equals" ~ ws+ ~ heredoc_literal }
            let rule = inner.as_rule();
            let literal_pair = inner
                .into_inner()
                .next()
                .expect("output text_equals heredoc rule must have a heredoc_literal");
            let content = parse_heredoc_literal(literal_pair)?;

            let exp = OutputExpectation {
                matcher: OutputMatcher::TextEquals(TextLiteral::Heredoc(content)),
            };
            if rule == Rule::stdout_text_equals_heredoc {
                Ok(Expectation::Stdout(exp))
            } else {
                Ok(Expectation::Stderr(exp))
            }
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
