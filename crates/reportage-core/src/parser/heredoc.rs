use super::{ParseError, Rule};
use crate::model::LocatedSpan;

/// A heredoc literal body after dedenting, together with the mapping back to
/// the original source.
///
/// The mapping exists because an interpolated heredoc's `&{name}` markers are
/// recognized in the dedented text, while every diagnostic and provenance span
/// must address the source the user wrote. Keeping the two together makes it
/// impossible to hand a dedented offset to a diagnostic by accident.
pub(super) struct DedentedHeredoc {
    text: String,
    lines: Vec<DedentedLine>,
}

/// One dedented body line's origin in the source.
struct DedentedLine {
    /// Byte offset of this line's first character within [`DedentedHeredoc::text`].
    dedented_start: usize,
    /// Byte offset, in the whole source, of the same character.
    source_start: usize,
    /// 1-based source line number.
    line: usize,
    /// How many columns the dedent stripped from this line, so a column in the
    /// dedented text can be shifted back onto the source line.
    stripped_columns: usize,
}

impl DedentedHeredoc {
    pub(super) fn text(&self) -> &str {
        &self.text
    }

    pub(super) fn into_text(self) -> String {
        self.text
    }

    /// Maps a `start..end` byte range of [`Self::text`] onto the source.
    ///
    /// `start` and `end` must lie on the same dedented line — every caller
    /// scans for `&{name}` markers, which the scanner never lets straddle a
    /// line ending.
    pub(super) fn source_span(&self, start: usize, end: usize) -> LocatedSpan {
        let origin = self
            .lines
            .iter()
            .rev()
            .find(|line| line.dedented_start <= start)
            .expect("a dedented offset always belongs to a recorded body line");
        let column_offset = self.text[origin.dedented_start..start].chars().count();
        LocatedSpan {
            start: origin.source_start + (start - origin.dedented_start),
            end: origin.source_start + (end - origin.dedented_start),
            line: origin.line,
            column: origin.stripped_columns + column_offset + 1,
        }
    }
}

/// Parses a `heredoc_literal` pair into its dedented content and source
/// mapping.
///
/// Shared by every heredoc position, raw and interpolated alike — the fence
/// and dedent rules are identical regardless of which construct the heredoc
/// literal appears in, and interpolation is applied afterwards, to the already
/// dedented text (see docs/reference/semantics.md — Interpolated text literal).
pub(super) fn parse_heredoc_literal(
    pair: pest::iterators::Pair<Rule>,
) -> Result<DedentedHeredoc, ParseError> {
    // heredoc_literal = { PUSH(opening_fence) ~ ws* ~ nl ~ heredoc_body ~ closing_fence_line ~ DROP }
    let mut inner = pair.into_inner();

    let _opening_fence = inner
        .next()
        .expect("heredoc_literal must have an opening_fence (pushed onto the pest match stack)");

    let body_pair = inner
        .next()
        .expect("heredoc_literal must have heredoc_body");
    let body_start_line = body_pair.line_col().0;
    let body_start_offset = body_pair.as_span().start();
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

    dedent_heredoc_body(body_text, indent, body_start_line, body_start_offset)
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
/// `body_start_offset` is that line's byte offset in the whole source, used to
/// map dedented offsets back onto it.
fn dedent_heredoc_body(
    body: &str,
    indent: &str,
    body_start_line: usize,
    body_start_offset: usize,
) -> Result<DedentedHeredoc, ParseError> {
    let mut text = String::with_capacity(body.len());
    let mut lines = Vec::new();
    let mut source_offset = body_start_offset;
    for (i, (content, ending)) in split_lines_keep_ending(body).into_iter().enumerate() {
        let is_blank = content.chars().all(|c| c == ' ' || c == '\t');
        // A blank line dedents to nothing, so it has no content offset worth
        // mapping; it still records an origin so the line ending it
        // contributes is attributed to the right source line.
        let stripped = if is_blank {
            ""
        } else {
            match content.strip_prefix(indent) {
                Some(stripped) => stripped,
                None => {
                    return Err(ParseError::ShallowHeredocIndent {
                        line: body_start_line + i,
                    });
                }
            }
        };
        let stripped_prefix_len = content.len() - stripped.len();
        lines.push(DedentedLine {
            dedented_start: text.len(),
            source_start: source_offset + stripped_prefix_len,
            line: body_start_line + i,
            // Only spaces and tabs are stripped, so byte length and column
            // count agree.
            stripped_columns: stripped_prefix_len,
        });
        text.push_str(stripped);
        text.push_str(ending);
        source_offset += content.len() + ending.len();
    }
    Ok(DedentedHeredoc { text, lines })
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
