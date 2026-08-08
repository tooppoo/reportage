use super::expectation::parse_assertion_block;
use super::literal::{RequiredKind, extract_string_inner, parse_value_literal};
use super::text_expression::{
    TextSurface, TextValuePosition, parse_heredoc_text_value_expression,
    parse_inline_text_value_expression,
};
use super::{ParseError, Rule};
use crate::model::{
    ActionStep, BeforeEach, BeforeEachError, BindingDeclaration, Case, Expectation, FileMatcher,
    LocatedSpan, OutputMatcher, RuntimeEvidenceSource, SideEffectingStep, Step,
    TextValueExpression, WorkspacePath, WriteFileStep,
};
use std::collections::HashSet;

/// The `write` step's content position: the same `TextValueExpression` model,
/// grammar, and parser in a case body and in `before_each`. The two differ only
/// in the binding scope their references are validated against, which
/// [`validate_bindings`] applies per phase.
pub(super) const WRITE_CONTENT_POSITION: TextValuePosition =
    TextValuePosition::new("`write` step content", TextSurface::InlineAndHeredoc);

/// Parses a `before_each_block` pair into the [`BeforeEach`] model.
///
/// `BeforeEach` holds the same [`Step`] as a case body, and accepts the same
/// step kinds, parsed by the same functions. Its binding flow is validated
/// against an empty starting scope: a `before_each` step sees only the
/// declarations before it in `before_each`, never one a case body makes later.
pub(super) fn parse_before_each_block(
    pair: pest::iterators::Pair<Rule>,
) -> Result<BeforeEach, ParseError> {
    let line = pair.line_col().0;

    let mut steps: Vec<Step> = Vec::new();
    for pair in pair.into_inner() {
        match pair.as_rule() {
            Rule::action_step => steps.push(parse_action_step(pair)?),
            Rule::assertion_block => steps.push(parse_assertion_block(pair)?),
            Rule::write_step_string | Rule::write_step_heredoc => {
                steps.push(Step::SideEffect(parse_write_step(pair)?))
            }
            Rule::binding_step => steps.push(parse_binding_step(pair)?),
            // When the closing brace line has no final newline, its `trail`
            // matches EOI, and pest surfaces that as an explicit EOI pair,
            // exactly as in parse_case_block.
            Rule::EOI => {}
            rule => unreachable!("unexpected rule in before_each_block: {rule:?}"),
        }
    }

    validate_bindings(&steps, &BindingScope::empty())?;
    BeforeEach::new(steps).map_err(|BeforeEachError::Empty| ParseError::EmptyBeforeEach { line })
}

/// The runtime evidence bindings a step sequence starts with.
///
/// Empty for `before_each`, which is the first sequence a concrete case runs.
/// A case body starts with whatever `before_each` left declared, so a setup
/// binding is in scope for the whole body — but a case body declaration is not
/// in scope for `before_each`, which ran before it.
pub(super) struct BindingScope {
    declared: HashSet<String>,
}

impl BindingScope {
    pub(super) fn empty() -> Self {
        Self {
            declared: HashSet::new(),
        }
    }

    /// The scope a case body starts with, given the `before_each` that ran
    /// first. Every name here is already declared, since `before_each`
    /// completed before the case body's first step.
    pub(super) fn after(before_each: Option<&BeforeEach>) -> Self {
        Self {
            declared: before_each
                .map(|before_each| declared_names(before_each.steps()))
                .unwrap_or_default(),
        }
    }
}

fn declared_names(steps: &[Step]) -> HashSet<String> {
    steps
        .iter()
        .filter_map(|step| match step {
            Step::Binding(binding) => Some(binding.name.clone()),
            _ => None,
        })
        .collect()
}

pub(super) fn parse_case_block(
    pair: pest::iterators::Pair<Rule>,
    scope: &BindingScope,
) -> Result<Case, ParseError> {
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

    validate_bindings(&steps, scope)?;
    Ok(Case { name, steps })
}

/// Validates one step sequence's binding flow in source order, starting from
/// `scope`.
///
/// `action_seen` starts false for every sequence, including a case body that
/// follows a `before_each` containing actions: the body-entry checkpoint drops
/// the last setup action's process evidence, so a case body `let` needs an
/// action of its own to capture from.
fn validate_bindings(steps: &[Step], scope: &BindingScope) -> Result<(), ParseError> {
    // Names declared anywhere in this sequence, plus those already in scope.
    // A reference to a name in here but not yet declared is a use-before-
    // declaration; a reference to a name absent from it is undefined.
    let mut all_names = scope.declared.clone();
    all_names.extend(declared_names(steps));
    let mut declared = scope.declared.clone();
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
                validate_text_value_expression(&write.content, &declared, &all_names)?;
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
            OutputMatcher::Contains(expression) | OutputMatcher::TextEquals(expression) => {
                validate_text_value_expression(expression, declared, all_names)
            }
            _ => Ok(()),
        },
        Expectation::File(file) => match &file.matcher {
            FileMatcher::Contains(expression) | FileMatcher::TextEquals(expression) => {
                validate_text_value_expression(expression, declared, all_names)
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

/// Checks every binding a text value expression references against the scope
/// visible at its position.
///
/// A direct `&name` reference and a `&{name}` reference inside an interpolated
/// literal are the same check on the same traversal, so the two forms can never
/// diverge on which references are accepted or which diagnostic they produce.
fn validate_text_value_expression(
    expression: &TextValueExpression,
    declared: &HashSet<String>,
    all_names: &HashSet<String>,
) -> Result<(), ParseError> {
    for reference in expression.binding_references() {
        if declared.contains(&reference.name) {
            continue;
        }
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
    let name_pair = inner.next().expect("binding_step must have an identifier");
    let name = name_pair.as_str().to_string();
    if !valid_binding_identifier(&name) {
        let name_span = name_pair.as_span();
        let (name_line, name_column) = name_pair.line_col();
        return Err(ParseError::InvalidBindingIdentifier {
            name,
            span: LocatedSpan {
                start: name_span.start(),
                end: name_span.end(),
                line: name_line,
                column: name_column,
            },
        });
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
        declaration_span: LocatedSpan {
            start: span.start(),
            end: span.end(),
            line,
            column,
        },
    }))
}

pub(super) fn valid_binding_identifier(name: &str) -> bool {
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

// Returns the [`SideEffectingStep`] itself rather than a [`Step`], so the
// `SideEffectingStep`-specific write-step parsers below stay reachable without
// unwrapping a `Step`. Both callers wrap the result in `Step::SideEffect`.
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
    // write_step_string = { "write" ~ ws+ ~ value_literal ~ ws+ ~ inline_text_value_expression }
    let mut inner = pair.into_inner();
    let path_pair = inner.next().expect("write_step_string must have a path");
    // The path is validated before the content is parsed, so a step with both
    // an invalid path and invalid content reports the path — only one
    // diagnostic is emitted per parse, and the path is what the step names
    // first.
    let path = parse_write_path(path_pair)?;
    let content_pair = inner
        .next()
        .expect("write_step_string must have an inline_text_value_expression");
    let content = parse_inline_text_value_expression(content_pair, WRITE_CONTENT_POSITION)?;
    Ok(SideEffectingStep::WriteFile(WriteFileStep {
        path,
        content,
    }))
}

fn parse_write_step_heredoc(
    pair: pest::iterators::Pair<Rule>,
) -> Result<SideEffectingStep, ParseError> {
    // write_step_heredoc = { "write" ~ ws+ ~ value_literal ~ ws* ~ heredoc_text_value_expression }
    let mut inner = pair.into_inner();
    let path_pair = inner.next().expect("write_step_heredoc must have a path");
    let path = parse_write_path(path_pair)?;
    let content_pair = inner
        .next()
        .expect("write_step_heredoc must have a heredoc_text_value_expression");
    let content = parse_heredoc_text_value_expression(content_pair)?;
    Ok(SideEffectingStep::WriteFile(WriteFileStep {
        path,
        content,
    }))
}

/// Validates a `write` step's path literal, shared by both content forms so
/// the kind check and the path policy are applied in exactly one place.
fn parse_write_path(path_pair: pest::iterators::Pair<Rule>) -> Result<WorkspacePath, ParseError> {
    let line = path_pair.line_col().0;
    let raw_path = parse_value_literal(path_pair)
        .expect_kind(RequiredKind::WorkspacePath, WRITE_PATH_POSITION)?;
    WorkspacePath::parse(&raw_path).map_err(|reason| ParseError::InvalidWorkspacePath {
        line,
        raw: raw_path,
        reason,
        position: WRITE_PATH_POSITION,
    })
}

const WRITE_PATH_POSITION: &str = "`write` step path";
