//! Source-level model: the parser's output, before projection to the execution model.
//!
//! This model is a source-aware semantic model:
//! it associates the semantically interpreted `Case` structure with the original source text
//! and each case's byte range within that text.
//! It is not a lossless CST — whitespace, comments, and raw literal spellings are not
//! structurally preserved, and reconstructing the original source from this model is not
//! a supported operation.
//! A future CST / syntax tree can be added in front of the parser and lowered into this
//! model without changing its role.
//!
//! Execution (`executor` / `evaluator`) and reporting (`result` / `artifact`) must depend
//! only on the execution model in `model`; the one supported hand-off between the two
//! worlds is [`SourceFile::into_script`].
//!
//! See docs/adr/20260712T090000Z_parser-returns-source-level-model.md.

use crate::model::{Case, Script};

/// The UTF-8 source text of one parsed reportage file, owned by its [`SourceFile`].
///
/// Owning a copy of the parser input keeps `SourceFile` self-contained:
/// spans stay usable after parsing without borrowing from the caller's buffer,
/// and no self-referential structure is needed.
#[derive(Debug)]
pub struct SourceText(String);

impl SourceText {
    pub(crate) fn new(text: String) -> Self {
        Self(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Extracts the text covered by `span`.
    ///
    /// This is the only supported way to slice source text by span;
    /// callers must not index into `as_str()` with raw span offsets.
    /// A span must only be used against the `SourceText` of the `SourceFile` it came from;
    /// using one against a different text is a caller bug and panics (or returns garbage
    /// if the offsets happen to be valid there).
    pub fn slice(&self, span: SourceSpan) -> &str {
        &self.0[span.start..span.end]
    }
}

/// A byte range into the [`SourceText`] owned by the same [`SourceFile`].
///
/// Constructed only by the parser, which guarantees:
/// `start <= end`, `end` within the text, and both offsets on UTF-8 character boundaries.
/// External code can read the offsets but never fabricate a span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpan {
    start: usize,
    end: usize,
}

impl SourceSpan {
    pub(crate) fn new(start: usize, end: usize) -> Self {
        assert!(
            start <= end,
            "SourceSpan requires start <= end (got {start}..{end})"
        );
        Self { start, end }
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn end(&self) -> usize {
        self.end
    }
}

/// Documentation text, as opposed to the execution-model text (`TextLiteral` / `TextValue`).
///
/// The two are kept as distinct types because they answer to different rules:
/// execution text participates in assertion comparison and file writes,
/// while documentation text is display-only metadata that never reaches execution.
/// v0 defines documentation text as plain text; Markdown interpretation is out of scope.
/// The source-side literal kind (string literal vs. heredoc literal) is not preserved:
/// both forms resolve to the same plain text here.
#[derive(Debug)]
pub struct DocumentationText(String);

impl DocumentationText {
    pub(crate) fn new(text: String) -> Self {
        Self(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// File-scope documentation metadata from a `document file` block.
///
/// Holds only what the source explicitly states: every field is optional and
/// no fallback value (file stem as title, a default group, path-based order)
/// is materialized here. Display-time fallbacks are applied when the
/// Documentation Catalog is built (#170), where both the source path and this
/// model are available.
#[derive(Debug)]
pub struct FileDocumentation {
    pub title: Option<String>,
    pub group: Option<String>,
    pub order: Option<u64>,
    pub description: Option<DocumentationText>,
}

/// Case-scope documentation metadata from a `document case` block.
///
/// Holds only what the source explicitly states: every field is optional and
/// no fallback value (the case name as title) is materialized here. Display
/// fallbacks are applied when the Documentation Catalog is built (#170),
/// where both this model and the execution `Case::name` are available.
#[derive(Debug)]
pub struct CaseDocumentation {
    pub title: Option<String>,
    pub description: Option<DocumentationText>,
}

/// One case of a parsed file: the execution-model `Case`, the byte range
/// of the whole `case` block (matching the pest `case_block` pair) in the original source,
/// and the documentation from the `document case` block immediately preceding it, when present.
///
/// The span covers the `case` line's leading indentation through the closing brace line,
/// including that line's trailing whitespace / inline comment and its line ending when present.
/// It excludes blank lines and comment lines before or after the block — in particular,
/// an associated `document case` block and the lines separating it from the case
/// are never part of the span.
#[derive(Debug)]
pub struct SourceCase {
    documentation: Option<CaseDocumentation>,
    case: Case,
    span: SourceSpan,
}

impl SourceCase {
    pub(crate) fn new(
        documentation: Option<CaseDocumentation>,
        case: Case,
        span: SourceSpan,
    ) -> Self {
        Self {
            documentation,
            case,
            span,
        }
    }

    /// The `document case` metadata, or `None` when the source declares none
    /// for this case.
    ///
    /// `None` means exactly "no `document case` block precedes this case";
    /// it is never substituted with fallback values here (see [`CaseDocumentation`]).
    pub fn documentation(&self) -> Option<&CaseDocumentation> {
        self.documentation.as_ref()
    }

    pub fn case(&self) -> &Case {
        &self.case
    }

    pub fn span(&self) -> SourceSpan {
        self.span
    }
}

/// A parsed reportage file as the parser returns it:
/// the owned source text, the file-scope documentation when the source declares one,
/// and the cases with their source spans, in source order.
#[derive(Debug)]
pub struct SourceFile {
    source: SourceText,
    file_documentation: Option<FileDocumentation>,
    cases: Vec<SourceCase>,
}

impl SourceFile {
    /// Assembles a `SourceFile`, asserting the span invariants the parser is
    /// required to uphold: every span lies within `source` on UTF-8 character
    /// boundaries, and spans appear in source order without overlapping.
    /// A violation is a parser bug, not an input error, so it panics.
    pub(crate) fn new(
        source: SourceText,
        file_documentation: Option<FileDocumentation>,
        cases: Vec<SourceCase>,
    ) -> Self {
        let text = source.as_str();
        let mut previous_end = 0usize;
        for source_case in &cases {
            let span = source_case.span();
            assert!(
                span.end() <= text.len(),
                "case span {}..{} exceeds source length {}",
                span.start(),
                span.end(),
                text.len()
            );
            assert!(
                text.is_char_boundary(span.start()) && text.is_char_boundary(span.end()),
                "case span {}..{} is not on UTF-8 character boundaries",
                span.start(),
                span.end()
            );
            assert!(
                previous_end <= span.start(),
                "case spans must be in source order and non-overlapping"
            );
            previous_end = span.end();
        }
        Self {
            source,
            file_documentation,
            cases,
        }
    }

    pub fn source(&self) -> &SourceText {
        &self.source
    }

    /// The `document file` metadata, or `None` when the source declares none.
    ///
    /// `None` means exactly "no `document file` block in the source";
    /// it is never substituted with fallback values here (see [`FileDocumentation`]).
    pub fn file_documentation(&self) -> Option<&FileDocumentation> {
        self.file_documentation.as_ref()
    }

    pub fn cases(&self) -> &[SourceCase] {
        &self.cases
    }

    /// The original source text of `source_case`'s whole `case` block.
    pub fn case_source(&self, source_case: &SourceCase) -> &str {
        self.source.slice(source_case.span())
    }

    /// Projects this source-level model into the execution-model [`Script`].
    ///
    /// Consuming by design: the execution path has no use for source text or
    /// spans, and a non-consuming projection would force `Clone` onto the
    /// whole `Case` / `Step` tree for no current consumer.
    /// Source text, spans, and documentation metadata are dropped here.
    pub fn into_script(self) -> Script {
        Script {
            cases: self
                .cases
                .into_iter()
                .map(|source_case| source_case.case)
                .collect(),
        }
    }
}
