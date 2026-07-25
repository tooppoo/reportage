use super::expectation::parse_assertion_block;
use super::heredoc::parse_heredoc_literal;
use super::literal::{RequiredKind, extract_string_inner, parse_value_literal};
use super::{ParseError, Rule};
use crate::model::{
    ActionStep, BeforeEach, BeforeEachError, BindingDeclaration, BindingReference, BindingSpan,
    Case, Expectation, FileMatcher, OutputMatcher, RuntimeEvidenceSource, SideEffectingStep, Step,
    TextLiteral, TextSource, WorkspacePath, WriteFileStep,
};
use std::collections::HashSet;

/// Parses a `before_each_block` pair into the write-only [`BeforeEach`] model.
///
/// The grammar deliberately accepts the full case-body step surface here (see
/// the `before_each_block` rule), so the write-only policy is enforced in this
/// function: an action step or assertion block is rejected with a diagnostic
/// naming the ban and the allowed alternative, at the offending step's line.
pub(super) fn parse_before_each_block(
    pair: pest::iterators::Pair<Rule>,
) -> Result<BeforeEach, ParseError> {
    let line = pair.line_col().0;

    let mut steps: Vec<SideEffectingStep> = Vec::new();
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::action_step => {
                return Err(ParseError::BeforeEachActionStep {
                    line: pair.line_col().0,
                });
            }
            Rule::assertion_block => {
                return Err(ParseError::BeforeEachAssertionBlock {
                    line: pair.line_col().0,
                });
            }
            Rule::write_step_string | Rule::write_step_heredoc => {
                steps.push(parse_write_step(pair)?);
            }
            Rule::binding_step => {
                return Err(ParseError::BeforeEachBindingStep {
                    line: pair.line_col().0,
                });
            }
            // When the closing brace line has no final newline, its `trail`
            // matches EOI, and pest surfaces that as an explicit EOI pair,
            // exactly as in parse_case_block.
            Rule::EOI => {}
            rule => unreachable!("unexpected rule in before_each_block: {rule:?}"),
        }
    }

    BeforeEach::new(steps).map_err(|BeforeEachError::Empty| ParseError::EmptyBeforeEach { line })
}

pub(super) fn parse_case_block(pair: pest::iterators::Pair<Rule>) -> Result<Case, ParseError> {
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
                steps.push(Step::SideEffect(parse_write_step(pair)?))
            }
            Rule::binding_step => steps.push(parse_binding_step(pair)?),
            // When the closing brace line has no final newline, its `trail`
            // matches EOI, and pest surfaces that as an explicit EOI pair
            // inside the case_block.
            Rule::EOI => {}
            rule => unreachable!("unexpected rule in case_block: {rule:?}"),
        }
    }

    if steps.is_empty() {
        return Err(ParseError::EmptyCase { line, name });
    }

    if !has_assertion_block {
        return Err(ParseError::MissingAssertionBlock { line, name });
    }

    validate_bindings(&steps)?;
    Ok(Case { name, steps })
}

fn validate_bindings(steps: &[Step]) -> Result<(), ParseError> {
    let all_names: HashSet<String> = steps
        .iter()
        .filter_map(|step| match step {
            Step::Binding(binding) => Some(binding.name.clone()),
            _ => None,
        })
        .collect();
    let mut declared = HashSet::new();
    let mut action_seen = false;
    for step in steps {
        match step {
            Step::Action(_) => action_seen = true,
            Step::Binding(binding) => {
                if !action_seen {
                    return Err(ParseError::BindingRequiresAction {
                        name: binding.name.clone(),
                        span: binding.declaration_span,
                    });
                }
                if !declared.insert(binding.name.clone()) {
                    return Err(ParseError::DuplicateBinding {
                        name: binding.name.clone(),
                        span: binding.declaration_span,
                    });
                }
            }
            Step::AssertionBlock(block) => {
                for expectation in block.expectations() {
                    validate_expectation_bindings(expectation, &declared, &all_names)?;
                }
            }
            Step::SideEffect(SideEffectingStep::WriteFile(write)) => {
                validate_text_source(&write.content, &declared, &all_names)?;
            }
        }
    }
    Ok(())
}

fn validate_expectation_bindings(
    expectation: &Expectation,
    declared: &HashSet<String>,
    all_names: &HashSet<String>,
) -> Result<(), ParseError> {
    match expectation {
        Expectation::Stdout(output) | Expectation::Stderr(output) => match &output.matcher {
            OutputMatcher::Contains(source) | OutputMatcher::TextEquals(source) => {
                validate_text_source(source, declared, all_names)
            }
            _ => Ok(()),
        },
        Expectation::File(file) => match &file.matcher {
            FileMatcher::Contains(source) | FileMatcher::TextEquals(source) => {
                validate_text_source(source, declared, all_names)
            }
            _ => Ok(()),
        },
        Expectation::Logical(logical) => {
            for child in logical.children() {
                validate_expectation_bindings(child, declared, all_names)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validate_text_source(
    source: &TextSource,
    declared: &HashSet<String>,
    all_names: &HashSet<String>,
) -> Result<(), ParseError> {
    if let TextSource::Binding(reference) = source
        && !declared.contains(&reference.name)
    {
        return if all_names.contains(&reference.name) {
            Err(ParseError::BindingUsedBeforeDeclaration {
                name: reference.name.clone(),
                span: reference.reference_span,
            })
        } else {
            Err(ParseError::UndefinedBinding {
                name: reference.name.clone(),
                span: reference.reference_span,
            })
        };
    }
    Ok(())
}

fn parse_binding_step(pair: pest::iterators::Pair<Rule>) -> Result<Step, ParseError> {
    let span = pair.as_span();
    let (line, column) = pair.line_col();
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .expect("binding_step must have an identifier")
        .as_str()
        .to_string();
    if !valid_binding_identifier(&name) {
        return Err(ParseError::InvalidBindingIdentifier { line, name });
    }
    let source = match inner
        .next()
        .expect("binding_step must have an evidence source")
        .as_str()
    {
        "stdout" => RuntimeEvidenceSource::StdoutExact,
        "stderr" => RuntimeEvidenceSource::StderrExact,
        "stdout_line" => RuntimeEvidenceSource::StdoutLine,
        "stderr_line" => RuntimeEvidenceSource::StderrLine,
        value => unreachable!("grammar produced unknown evidence source: {value}"),
    };
    Ok(Step::Binding(BindingDeclaration {
        name,
        source,
        declaration_span: BindingSpan {
            start: span.start(),
            end: span.end(),
            line,
            column,
        },
    }))
}

fn valid_binding_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(
        chars.next(),
        Some(first) if first.is_ascii_alphabetic() || first == '_'
    ) && chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
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

// Returns the [`SideEffectingStep`] itself rather than a [`Step`], because a
// `write` step is legal in two containers with different step models: a case
// body (which wraps it in `Step::SideEffect`) and a `before_each` body (which
// holds `SideEffectingStep`s only).
fn parse_write_step(pair: pest::iterators::Pair<Rule>) -> Result<SideEffectingStep, ParseError> {
    match pair.as_rule() {
        Rule::write_step_string => parse_write_step_string(pair),
        Rule::write_step_heredoc => parse_write_step_heredoc(pair),
        rule => unreachable!("unexpected rule in write step: {rule:?}"),
    }
}

fn parse_write_step_string(
    pair: pest::iterators::Pair<Rule>,
) -> Result<SideEffectingStep, ParseError> {
    // write_step_string = { "write" ~ ws+ ~ value_literal ~ ws+ ~ value_literal }
    let line = pair.line_col().0;
    let mut inner = pair.into_inner();

    let path_pair = inner.next().expect("write_step_string must have a path");
    let raw_path = parse_value_literal(path_pair)
        .expect_kind(RequiredKind::WorkspacePath, "`write` step path")?;

    let content_pair = inner
        .next()
        .expect("write_step_string must have content value_literal");
    let content = parse_text_source(content_pair, "`write` step content")?;

    let path =
        WorkspacePath::parse(&raw_path).map_err(|reason| ParseError::InvalidWorkspacePath {
            line,
            raw: raw_path,
            reason,
            position: "`write` step path",
        })?;

    Ok(SideEffectingStep::WriteFile(WriteFileStep {
        path,
        content,
    }))
}

fn parse_write_step_heredoc(
    pair: pest::iterators::Pair<Rule>,
) -> Result<SideEffectingStep, ParseError> {
    // write_step_heredoc = { "write" ~ ws+ ~ value_literal ~ ws* ~ heredoc_literal }
    let line = pair.line_col().0;
    let mut inner = pair.into_inner();

    let path_pair = inner.next().expect("write_step_heredoc must have a path");
    let raw_path = parse_value_literal(path_pair)
        .expect_kind(RequiredKind::WorkspacePath, "`write` step path")?;

    let literal_pair = inner
        .next()
        .expect("write_step_heredoc must have a heredoc_literal");
    let content = TextSource::Literal(TextLiteral::Heredoc(parse_heredoc_literal(literal_pair)?));

    let path =
        WorkspacePath::parse(&raw_path).map_err(|reason| ParseError::InvalidWorkspacePath {
            line,
            raw: raw_path,
            reason,
            position: "`write` step path",
        })?;

    Ok(SideEffectingStep::WriteFile(WriteFileStep {
        path,
        content,
    }))
}

pub(super) fn parse_text_source(
    pair: pest::iterators::Pair<Rule>,
    position: &'static str,
) -> Result<TextSource, ParseError> {
    let inner = pair
        .into_inner()
        .next()
        .expect("text_value_source must have a value");
    if inner.as_rule() == Rule::binding_reference {
        let span = inner.as_span();
        let (line, column) = inner.line_col();
        let name = inner
            .into_inner()
            .next()
            .expect("binding_reference must have an identifier")
            .as_str()
            .to_string();
        return Ok(TextSource::Binding(BindingReference {
            name,
            reference_span: BindingSpan {
                start: span.start(),
                end: span.end(),
                line,
                column,
            },
        }));
    }
    let required = if position.contains(" contains`") {
        RequiredKind::TextValueStringOnly
    } else {
        RequiredKind::TextValueStringOrHeredoc
    };
    let value = parse_value_literal(inner).expect_kind(required, position)?;
    Ok(TextSource::Literal(TextLiteral::Quoted(value)))
}
