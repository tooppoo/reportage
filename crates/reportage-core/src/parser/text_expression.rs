//! The single entry point every `TextValue` argument position uses to parse
//! its expected text.
//!
//! Consumers never enumerate the raw / direct-binding / interpolated forms.
//! They name one [`TextSurface`] — the forms their grammar position wires up —
//! and call [`parse_inline_text_value_expression`] or
//! [`parse_heredoc_text_value_expression`], which lower both grammar
//! categories into the same [`TextValueExpression`]. Adding a `TextValue`
//! consumer therefore takes a grammar category reference and a call here, not
//! a new branch per form.
//!
//! See docs/adr/20260726T060000Z_interpolated-text-literal.md.

use super::heredoc::{DedentedHeredoc, parse_heredoc_literal};
use super::literal::{RequiredKind, parse_value_literal};
use super::step::valid_binding_identifier;
use super::{ParseError, Rule};
use crate::model::{
    BindingReference, InterpolatedText, InterpolatedTextForm, InterpolatedTextSegment, LocatedSpan,
    TextLiteral, TextValue, TextValueExpression,
};

/// Which text forms an argument position's grammar accepts.
///
/// This is the only per-consumer difference the text expression parser knows
/// about. It is declared by the caller, never inferred from a consumer name or
/// a diagnostic position string.
#[derive(Clone, Copy)]
pub(super) enum TextSurface {
    /// The position wires up the inline forms only (`stdout` / `stderr
    /// contains`).
    InlineOnly,
    /// The position wires up the inline forms and the heredoc forms through a
    /// second grammar rule (`write` content, `file contains`, every
    /// `text_equals`).
    InlineAndHeredoc,
}

impl TextSurface {
    /// The literal-kind requirement a wrong-kind literal is reported against,
    /// so the suggested replacement only names forms this position accepts.
    const fn required_kind(self) -> RequiredKind {
        match self {
            TextSurface::InlineOnly => RequiredKind::TextValueStringOnly,
            TextSurface::InlineAndHeredoc => RequiredKind::TextValueStringOrHeredoc,
        }
    }
}

/// An argument position that takes a `TextValue`: its name in diagnostics, and
/// the text forms it accepts.
///
/// The two are carried together so a position can never declare a diagnostic
/// name without also declaring what it accepts.
#[derive(Clone, Copy)]
pub(super) struct TextValuePosition {
    /// Human-readable name of the position, e.g. "`write` step content".
    name: &'static str,
    surface: TextSurface,
}

impl TextValuePosition {
    pub(super) const fn new(name: &'static str, surface: TextSurface) -> Self {
        Self { name, surface }
    }
}

/// Parses an `inline_text_value_expression` pair: a raw string literal, a
/// direct `&name` binding reference, or an `&"..."` interpolated string
/// literal.
pub(super) fn parse_inline_text_value_expression(
    pair: pest::iterators::Pair<Rule>,
    position: TextValuePosition,
) -> Result<TextValueExpression, ParseError> {
    debug_assert_eq!(pair.as_rule(), Rule::inline_text_value_expression);
    let inner = pair
        .into_inner()
        .next()
        .expect("inline_text_value_expression must have a variant");

    match inner.as_rule() {
        Rule::interpolated_string => parse_interpolated_string(inner),
        Rule::value_literal => parse_inline_value_literal(inner, position),
        rule => unreachable!("unexpected rule in inline_text_value_expression: {rule:?}"),
    }
}

/// Parses a `heredoc_text_value_expression` pair: a raw heredoc literal or an
/// `&` + heredoc interpolated heredoc literal.
pub(super) fn parse_heredoc_text_value_expression(
    pair: pest::iterators::Pair<Rule>,
) -> Result<TextValueExpression, ParseError> {
    debug_assert_eq!(pair.as_rule(), Rule::heredoc_text_value_expression);
    let inner = pair
        .into_inner()
        .next()
        .expect("heredoc_text_value_expression must have a variant");

    match inner.as_rule() {
        Rule::heredoc_literal => Ok(TextValueExpression::Raw(TextLiteral::Heredoc(
            parse_heredoc_literal(inner)?.into_text(),
        ))),
        Rule::interpolated_heredoc => parse_interpolated_heredoc(inner),
        rule => unreachable!("unexpected rule in heredoc_text_value_expression: {rule:?}"),
    }
}

/// Lowers the `value_literal` alternative of an inline text position: either a
/// direct binding reference, or a raw string literal whose kind must match the
/// position's signature.
fn parse_inline_value_literal(
    pair: pest::iterators::Pair<Rule>,
    position: TextValuePosition,
) -> Result<TextValueExpression, ParseError> {
    let literal = parse_value_literal(pair);
    if let Some(reference) = literal.binding_reference() {
        return Ok(TextValueExpression::Binding(reference?));
    }
    let value = literal.expect_kind(position.surface.required_kind(), position.name)?;
    Ok(TextValueExpression::Raw(TextLiteral::Quoted(value)))
}

fn parse_interpolated_string(
    pair: pest::iterators::Pair<Rule>,
) -> Result<TextValueExpression, ParseError> {
    // interpolated_string = { "&" ~ "\"" ~ interpolated_string_inner ~ "\"" }
    let literal_span = pair.as_span();
    let (literal_line, literal_column) = pair.line_col();
    let inner = pair
        .into_inner()
        .next()
        .expect("interpolated_string must have interpolated_string_inner");
    let inner_span = inner.as_span();
    let (inner_line, inner_column) = inner.line_col();

    // A string literal never contains a raw newline, so every offset inside it
    // maps onto one source line by simple character counting.
    let text = inner.as_str();
    let map = |start: usize, end: usize| LocatedSpan {
        start: inner_span.start() + start,
        end: inner_span.start() + end,
        line: inner_line,
        column: inner_column + text[..start].chars().count(),
    };

    let segments = scan_interpolated_text(text, EscapePolicy::String, &map)?;
    Ok(TextValueExpression::Interpolated(InterpolatedText::new(
        InterpolatedTextForm::String,
        segments,
        LocatedSpan {
            start: literal_span.start(),
            end: literal_span.end(),
            line: literal_line,
            column: literal_column,
        },
    )))
}

fn parse_interpolated_heredoc(
    pair: pest::iterators::Pair<Rule>,
) -> Result<TextValueExpression, ParseError> {
    // interpolated_heredoc = { "&" ~ heredoc_literal }
    let literal_span = pair.as_span();
    let (literal_line, literal_column) = pair.line_col();
    let heredoc_pair = pair
        .into_inner()
        .next()
        .expect("interpolated_heredoc must have a heredoc_literal");

    // Dedent first, then scan: the heredoc's own source processing decides the
    // literal segments, and binding values are inserted into the result
    // untouched. `DedentedHeredoc` maps every marker offset back onto the
    // original body, so no diagnostic ever points into the dedented text.
    let dedented: DedentedHeredoc = parse_heredoc_literal(heredoc_pair)?;
    let map = |start: usize, end: usize| dedented.source_span(start, end);
    let segments = scan_interpolated_text(dedented.text(), EscapePolicy::Heredoc, &map)?;

    Ok(TextValueExpression::Interpolated(InterpolatedText::new(
        InterpolatedTextForm::Heredoc,
        segments,
        LocatedSpan {
            start: literal_span.start(),
            end: literal_span.end(),
            line: literal_line,
            column: literal_column,
        },
    )))
}

/// Which backslash sequences an interpolated literal treats as escapes.
///
/// The two policies differ only in what a backslash means, never in how
/// interpolation markers are recognized — the marker scan below is shared, so
/// the two forms can never drift apart on `&{name}` handling.
#[derive(Clone, Copy)]
enum EscapePolicy {
    /// The raw string literal escape set plus `\&`. Anything else is already
    /// rejected by the grammar.
    String,
    /// `\\` and `\&` only. Every other backslash stays literal text, exactly as
    /// in a raw heredoc literal, so regex and Windows paths need no escaping.
    Heredoc,
}

/// Scans an interpolated literal's text left to right into literal and binding
/// reference segments.
///
/// Escape sequences are resolved before interpolation markers, in one pass, so
/// the result of a backslash run followed by `&` follows from this order rather
/// than from any rule about even or odd backslash counts.
///
/// `map` turns a `start..end` byte range of `text` into the corresponding
/// source span; the two interpolated forms differ only in that function.
fn scan_interpolated_text(
    text: &str,
    policy: EscapePolicy,
    map: &dyn Fn(usize, usize) -> LocatedSpan,
) -> Result<Vec<InterpolatedTextSegment>, ParseError> {
    let mut segments = Vec::new();
    let mut literal = String::new();
    let mut rest = text;
    let mut offset = 0;

    while let Some(character) = rest.chars().next() {
        match character {
            '\\' => {
                let escaped = rest[1..].chars().next();
                let consumed = match (policy, escaped) {
                    (_, Some('\\')) => {
                        literal.push('\\');
                        2
                    }
                    (_, Some('&')) => {
                        literal.push('&');
                        2
                    }
                    (EscapePolicy::String, Some('"')) => {
                        literal.push('"');
                        2
                    }
                    (EscapePolicy::String, Some('n')) => {
                        literal.push('\n');
                        2
                    }
                    (EscapePolicy::String, Some('t')) => {
                        literal.push('\t');
                        2
                    }
                    (EscapePolicy::String, other) => unreachable!(
                        "grammar guarantees only \\\\, \\\", \\n, \\t, \\& escapes in an interpolated string, got {other:?}"
                    ),
                    // A heredoc keeps every other backslash as literal text and
                    // resumes at the next character, so that character is still
                    // eligible to start an escape or an interpolation marker.
                    (EscapePolicy::Heredoc, _) => {
                        literal.push('\\');
                        1
                    }
                };
                rest = &rest[consumed..];
                offset += consumed;
            }
            '&' => {
                let scanned = scan_binding_reference(rest, offset, map)?;
                if !literal.is_empty() {
                    segments.push(InterpolatedTextSegment::Literal(TextValue::new(
                        std::mem::take(&mut literal),
                    )));
                }
                segments.push(InterpolatedTextSegment::Binding(scanned.reference));
                rest = &rest[scanned.consumed..];
                offset += scanned.consumed;
            }
            _ => {
                let consumed = character.len_utf8();
                literal.push(character);
                rest = &rest[consumed..];
                offset += consumed;
            }
        }
    }

    if !literal.is_empty() {
        segments.push(InterpolatedTextSegment::Literal(TextValue::new(literal)));
    }
    Ok(segments)
}

struct ScannedBindingReference {
    reference: BindingReference,
    /// Byte length of the whole `&{name}` marker.
    consumed: usize,
}

/// Reads one `&{name}` marker starting at `rest`'s leading `&`.
///
/// An unescaped `&` is reserved, so every shape that is not a well-formed
/// marker is rejected with its own diagnostic rather than silently kept as
/// literal text — writing a literal `&` is what `\&` is for.
fn scan_binding_reference(
    rest: &str,
    offset: usize,
    map: &dyn Fn(usize, usize) -> LocatedSpan,
) -> Result<ScannedBindingReference, ParseError> {
    if !rest[1..].starts_with('{') {
        return Err(ParseError::MalformedInterpolationMarker {
            span: map(offset, offset + '&'.len_utf8()),
        });
    }
    // A binding name is a single-line token, so the search for `}` stops at a
    // line ending: an unterminated marker is reported where it was written
    // instead of swallowing the rest of a heredoc body.
    let name = rest[2..]
        .split(['}', '\n', '\r'])
        .next()
        .expect("split always yields a first element");
    let name_start = offset + 2;
    let name_end = name_start + name.len();
    if !rest[2 + name.len()..].starts_with('}') {
        return Err(ParseError::UnterminatedInterpolationReference {
            span: map(offset, name_end),
        });
    }
    let marker_end = name_end + 1;
    if name.is_empty() {
        return Err(ParseError::EmptyInterpolationBindingName {
            span: map(offset, marker_end),
        });
    }
    if !valid_binding_identifier(name) {
        return Err(ParseError::InvalidBindingIdentifier {
            name: name.to_string(),
            span: map(name_start, name_end),
        });
    }
    Ok(ScannedBindingReference {
        reference: BindingReference {
            name: name.to_string(),
            reference_span: map(offset, marker_end),
        },
        consumed: marker_end - offset,
    })
}
