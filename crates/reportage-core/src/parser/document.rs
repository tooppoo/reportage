use super::heredoc::parse_heredoc_literal;
use super::literal::{RequiredKind, parse_value_literal};
use super::{ParseError, Rule};
use crate::source::{CaseDocumentation, DocumentationText, FileDocumentation};

/// The duplicate-field check shared by every document field arm, kept out of
/// line so each arm reads as "reject the duplicate, then parse the value".
fn reject_duplicate_documentation_field<T>(
    slot: &Option<T>,
    field: &'static str,
    line: usize,
) -> Result<(), ParseError> {
    if slot.is_some() {
        return Err(ParseError::DuplicateDocumentationField { line, field });
    }
    Ok(())
}

/// Parses a `document_title_field` pair's value, enforcing the string-literal
/// kind shared by both document scopes.
fn parse_document_title_field(field: pest::iterators::Pair<Rule>) -> Result<String, ParseError> {
    let literal_pair = field
        .into_inner()
        .next()
        .expect("document_title_field must have value_literal");
    parse_value_literal(literal_pair)
        .expect_kind(RequiredKind::StringLiteral, "`title` documentation field")
}

/// Parses a `document_description_string_field` pair's value, enforcing the
/// text-literal kind shared by both document scopes.
fn parse_document_description_string_field(
    field: pest::iterators::Pair<Rule>,
) -> Result<DocumentationText, ParseError> {
    let literal_pair = field
        .into_inner()
        .next()
        .expect("document_description_string_field must have value_literal");
    let text = parse_value_literal(literal_pair).expect_kind(
        RequiredKind::TextValueStringOrHeredoc,
        "`description` documentation field",
    )?;
    Ok(DocumentationText::new(text))
}

/// Parses a `document_description_heredoc_field` pair's value.
fn parse_document_description_heredoc_field(
    field: pest::iterators::Pair<Rule>,
) -> Result<DocumentationText, ParseError> {
    let literal_pair = field
        .into_inner()
        .next()
        .expect("document_description_heredoc_field must have heredoc_literal");
    Ok(DocumentationText::new(parse_heredoc_literal(literal_pair)?))
}

/// Parses a `document_file_block` pair into [`FileDocumentation`], enforcing
/// the body rules the grammar deliberately leaves open: at least one field,
/// and no field declared twice. Which fields can appear at all is the
/// grammar's whitelist (`document_file_field_line`), not this function's
/// concern.
pub(super) fn parse_document_file_block(
    pair: pest::iterators::Pair<Rule>,
) -> Result<FileDocumentation, ParseError> {
    let line = pair.line_col().0;

    let mut title: Option<String> = None;
    let mut group: Option<String> = None;
    let mut order: Option<u64> = None;
    let mut description: Option<DocumentationText> = None;

    for field in pair.into_inner() {
        let field_line = field.line_col().0;
        match field.as_rule() {
            Rule::document_title_field => {
                reject_duplicate_documentation_field(&title, "title", field_line)?;
                title = Some(parse_document_title_field(field)?);
            }
            Rule::document_group_field => {
                reject_duplicate_documentation_field(&group, "group", field_line)?;
                let literal_pair = field
                    .into_inner()
                    .next()
                    .expect("document_group_field must have value_literal");
                group = Some(
                    parse_value_literal(literal_pair)
                        .expect_kind(RequiredKind::StringLiteral, "`group` documentation field")?,
                );
            }
            Rule::document_order_field => {
                reject_duplicate_documentation_field(&order, "order", field_line)?;
                let value_pair = field
                    .into_inner()
                    .next()
                    .expect("document_order_field must have document_order_value");
                let value_str = value_pair.as_str();
                // The grammar guarantees a digit run, so the only possible
                // failure is overflow of the model's u64 range.
                order = Some(value_str.parse::<u64>().map_err(|_| {
                    ParseError::InvalidDocumentationOrder {
                        line: field_line,
                        value: value_str.to_string(),
                    }
                })?);
            }
            Rule::document_description_string_field => {
                reject_duplicate_documentation_field(&description, "description", field_line)?;
                description = Some(parse_document_description_string_field(field)?);
            }
            Rule::document_description_heredoc_field => {
                reject_duplicate_documentation_field(&description, "description", field_line)?;
                description = Some(parse_document_description_heredoc_field(field)?);
            }
            // When the closing brace line has no final newline, its `trail`
            // matches EOI, and pest surfaces that as an explicit EOI pair
            // inside the block (same as case_block).
            Rule::EOI => {}
            rule => unreachable!("unexpected rule in document_file_block: {rule:?}"),
        }
    }

    if title.is_none() && group.is_none() && order.is_none() && description.is_none() {
        return Err(ParseError::EmptyDocumentBlock { line });
    }

    Ok(FileDocumentation {
        title,
        group,
        order,
        description,
    })
}

/// Parses a `document_case_block` pair into [`CaseDocumentation`], enforcing
/// the same open body rules as the file scope: at least one field, and no
/// field declared twice. Association with the following case is the caller's
/// concern (see `parse`).
pub(super) fn parse_document_case_block(
    pair: pest::iterators::Pair<Rule>,
) -> Result<CaseDocumentation, ParseError> {
    let line = pair.line_col().0;

    let mut title: Option<String> = None;
    let mut description: Option<DocumentationText> = None;

    for field in pair.into_inner() {
        let field_line = field.line_col().0;
        match field.as_rule() {
            Rule::document_title_field => {
                reject_duplicate_documentation_field(&title, "title", field_line)?;
                title = Some(parse_document_title_field(field)?);
            }
            Rule::document_description_string_field => {
                reject_duplicate_documentation_field(&description, "description", field_line)?;
                description = Some(parse_document_description_string_field(field)?);
            }
            Rule::document_description_heredoc_field => {
                reject_duplicate_documentation_field(&description, "description", field_line)?;
                description = Some(parse_document_description_heredoc_field(field)?);
            }
            // When the closing brace line has no final newline, its `trail`
            // matches EOI, and pest surfaces that as an explicit EOI pair
            // inside the block (same as case_block).
            Rule::EOI => {}
            rule => unreachable!("unexpected rule in document_case_block: {rule:?}"),
        }
    }

    if title.is_none() && description.is_none() {
        return Err(ParseError::EmptyDocumentBlock { line });
    }

    Ok(CaseDocumentation { title, description })
}
