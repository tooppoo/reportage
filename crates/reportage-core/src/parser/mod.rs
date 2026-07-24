mod document;
mod error;
mod expectation;
mod heredoc;
mod literal;
mod step;

pub use error::ParseError;

use pest::Parser;
use pest_derive::Parser;

use self::document::{parse_document_case_block, parse_document_file_block};
use self::step::{parse_before_each_block, parse_case_block};
use crate::source::{SourceCase, SourceFile, SourceSpan, SourceText};

#[derive(Parser)]
#[grammar = "reportage.pest"]
struct ReportageParser;

#[cfg(test)]
mod tests;

/// Parses `source` into the source-level model.
///
/// The returned [`SourceFile`] owns a copy of `source` and associates each case
/// with its byte range in that text; run [`SourceFile::into_script`] to obtain
/// the execution-model `Script`.
/// Each case's span is exactly the pest `case_block` pair's matched range —
/// the grammar, not this function, defines where a case block starts and ends.
pub fn parse(source: &str) -> Result<SourceFile, ParseError> {
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
    // Call into_inner() to get its contents (document blocks, case_blocks, SOI, EOI).
    let script_pair = pairs.into_iter().next().expect("script always matches");
    let mut file_documentation: Option<FileDocumentation> = None;
    let mut before_each: Option<BeforeEach> = None;
    // A parsed `document case` block waiting for its target case, with the
    // block's start line for the orphan diagnostic.
    let mut pending_case_documentation: Option<(CaseDocumentation, usize)> = None;
    let mut cases: Vec<SourceCase> = Vec::new();
    for pair in script_pair.into_inner() {
        match pair.as_rule() {
            // The grammar accepts document blocks and the before_each block
            // anywhere at top level, any number of times; the canonical
            // top-level form `document file? before_each? (document case? case)*`
            // is enforced here so each violation gets its own actionable
            // diagnostic.
            Rule::document_file_block => {
                let line = pair.line_col().0;
                // `before_each.is_some()` is part of the placement check:
                // `document file` describes the whole file, so it must lead
                // the file, before `before_each` too, keeping the canonical
                // form strict now rather than tightening it later against
                // already-accepted sources.
                if !cases.is_empty()
                    || pending_case_documentation.is_some()
                    || before_each.is_some()
                {
                    return Err(ParseError::DocumentFileAfterCase { line });
                }
                if file_documentation.is_some() {
                    return Err(ParseError::DuplicateDocumentFile { line });
                }
                file_documentation = Some(parse_document_file_block(pair)?);
            }
            Rule::document_case_block => {
                let line = pair.line_col().0;
                // Checked before parsing the body so a second block is always
                // reported as the duplicate it is, even when the first block
                // would also end up orphaned (duplicate wins over orphan).
                if pending_case_documentation.is_some() {
                    return Err(ParseError::DuplicateDocumentCase { line });
                }
                pending_case_documentation = Some((parse_document_case_block(pair)?, line));
            }
            Rule::before_each_block => {
                let line = pair.line_col().0;
                // Rejected while a `document case` is pending, not only after
                // the first case: a `document case` block must stay adjacent
                // to the case it documents, so `before_each` cannot sit
                // between them. Mirrors the `document file` placement check
                // above.
                if !cases.is_empty() || pending_case_documentation.is_some() {
                    return Err(ParseError::BeforeEachAfterCase { line });
                }
                if before_each.is_some() {
                    return Err(ParseError::DuplicateBeforeEach { line });
                }
                before_each = Some(parse_before_each_block(pair)?);
            }
            Rule::case_block => {
                let pair_span = pair.as_span();
                let span = SourceSpan::new(pair_span.start(), pair_span.end());
                let documentation = pending_case_documentation
                    .take()
                    .map(|(documentation, _)| documentation);
                cases.push(SourceCase::new(
                    documentation,
                    parse_case_block(pair)?,
                    span,
                ));
            }
            // SOI, EOI, and silent blank/comment lines carry no content.
            _ => {}
        }
    }

    if let Some((_, line)) = pending_case_documentation {
        return Err(ParseError::OrphanDocumentCase { line });
    }

    Ok(SourceFile::new(
        SourceText::new(source.to_string()),
        file_documentation,
        before_each,
        cases,
    ))
}
