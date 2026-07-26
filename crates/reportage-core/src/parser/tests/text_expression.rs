//! Interpolated text literal parsing: segment structure, escape resolution,
//! marker diagnostics, and source spans.
//!
//! Behavior visible from the CLI is covered by `e2e/interpolated-text.repor`
//! and the checked-in syntax fixtures; these tests pin the internal segment
//! model and the spans a diagnostic points at, which self-testing cannot
//! observe.

use super::*;
use crate::model::{
    Expectation, FileMatcher, InterpolatedTextForm, InterpolatedTextSegment, Step,
    TextValueExpression,
};

/// Wraps `content` as a `write` step's content in a case that declares one
/// binding, so an interpolated literal referencing `tag` is in scope.
fn parse_write_content(content: &str) -> Result<TextValueExpression, ParseError> {
    let src = format!(
        "case \"x\" {{\n  $ true\n  let tag <- stdout_line\n  write <\"a.txt\"> {content}\n  assert {{ exit 0 }}\n}}\n"
    );
    let script = parse_script(&src)?;
    let Step::SideEffect(SideEffectingStep::WriteFile(step)) = &script.cases[0].steps[2] else {
        panic!("expected third step to be a write step");
    };
    Ok(step.content.clone())
}

fn interpolated(content: &str) -> (InterpolatedTextForm, Vec<InterpolatedTextSegment>) {
    match parse_write_content(content).unwrap() {
        TextValueExpression::Interpolated(text) => (text.form(), text.segments().to_vec()),
        other => panic!("expected an interpolated text expression, got {other:?}"),
    }
}

fn literal(value: &str) -> InterpolatedTextSegment {
    InterpolatedTextSegment::Literal(crate::model::TextValue::new(value.to_string()))
}

fn binding_names(segments: &[InterpolatedTextSegment]) -> Vec<&str> {
    segments
        .iter()
        .filter_map(|segment| match segment {
            InterpolatedTextSegment::Literal(_) => None,
            InterpolatedTextSegment::Binding(reference) => Some(reference.name.as_str()),
        })
        .collect()
}

// ─── Segment structure ──────────────────────────────────────────────────────

#[test]
fn interpolated_string_splits_into_literal_and_binding_segments() {
    let (form, segments) = interpolated("&\"prefix &{tag} suffix\"");
    assert_eq!(form, InterpolatedTextForm::String);
    assert_eq!(segments[0], literal("prefix "));
    assert_eq!(segments[2], literal(" suffix"));
    assert_eq!(binding_names(&segments), vec!["tag"]);
}

#[test]
fn interpolated_string_with_no_reference_is_one_literal_segment() {
    let (_, segments) = interpolated("&\"plain text\"");
    assert_eq!(segments, vec![literal("plain text")]);
}

#[test]
fn an_empty_interpolated_string_has_no_segments() {
    let (_, segments) = interpolated("&\"\"");
    assert!(segments.is_empty());
}

#[test]
fn the_same_binding_may_be_referenced_more_than_once() {
    let (_, segments) = interpolated("&\"&{tag}-&{tag}\"");
    assert_eq!(binding_names(&segments), vec!["tag", "tag"]);
}

#[test]
fn a_reference_at_each_end_produces_no_empty_literal_segments() {
    let (_, segments) = interpolated("&\"&{tag}&{tag}\"");
    assert_eq!(segments.len(), 2);
    assert_eq!(binding_names(&segments), vec!["tag", "tag"]);
}

#[test]
fn interpolated_heredoc_is_dedented_before_segments_are_built() {
    let (form, segments) = interpolated("&```\n    line &{tag}\n    ```");
    assert_eq!(form, InterpolatedTextForm::Heredoc);
    assert_eq!(segments[0], literal("line "));
    assert_eq!(segments[2], literal("\n"));
}

// ─── Escape resolution ──────────────────────────────────────────────────────

#[test]
fn interpolated_string_resolves_the_raw_escape_set_plus_ampersand() {
    let (_, segments) = interpolated("&\"\\\\ \\\" \\n \\t \\&\"");
    assert_eq!(segments, vec![literal("\\ \" \n \t &")]);
}

#[test]
fn an_escaped_marker_is_literal_text_rather_than_a_reference() {
    let (_, segments) = interpolated("&\"\\&{tag}\"");
    assert_eq!(segments, vec![literal("&{tag}")]);
}

#[test]
fn interpolated_heredoc_keeps_backslashes_that_introduce_no_escape() {
    let (_, segments) = interpolated("&```\n    \\d+ C:\\temp\n    ```");
    assert_eq!(segments, vec![literal("\\d+ C:\\temp\n")]);
}

/// Escapes resolve before interpolation markers, left to right, so a backslash
/// run's effect follows from that order rather than from a parity rule.
#[test]
fn a_doubled_backslash_before_a_marker_leaves_the_marker_active() {
    let (_, segments) = interpolated("&```\n    \\\\&{tag}\n    ```");
    assert_eq!(segments[0], literal("\\"));
    assert_eq!(binding_names(&segments), vec!["tag"]);
}

#[test]
fn a_doubled_backslash_before_an_escaped_marker_leaves_the_marker_literal() {
    let (_, segments) = interpolated("&```\n    \\\\\\&{tag}\n    ```");
    assert_eq!(segments, vec![literal("\\&{tag}\n")]);
}

// ─── Marker diagnostics ─────────────────────────────────────────────────────

#[test]
fn an_unescaped_ampersand_that_opens_no_reference_is_rejected() {
    let err = parse_write_content("&\"a & b\"").unwrap_err();
    assert!(matches!(
        err,
        ParseError::MalformedInterpolationMarker { .. }
    ));
    assert_eq!(
        err.code().as_str(),
        "parse.interpolated_text.malformed_marker"
    );
}

#[test]
fn a_trailing_ampersand_is_rejected() {
    let err = parse_write_content("&\"trailing &\"").unwrap_err();
    assert!(matches!(
        err,
        ParseError::MalformedInterpolationMarker { .. }
    ));
}

#[test]
fn an_unterminated_reference_is_rejected() {
    let err = parse_write_content("&\"&{tag\"").unwrap_err();
    assert_eq!(
        err.code().as_str(),
        "parse.interpolated_text.unterminated_reference"
    );
}

/// A reference never runs past its own line, so an unterminated marker in a
/// heredoc reports where it was written instead of swallowing the body.
#[test]
fn an_unterminated_heredoc_reference_stops_at_the_line_ending() {
    let err = parse_write_content("&```\n    &{tag\n    more body\n    ```").unwrap_err();
    assert_eq!(
        err.code().as_str(),
        "parse.interpolated_text.unterminated_reference"
    );
}

#[test]
fn an_empty_binding_name_is_rejected() {
    let err = parse_write_content("&\"&{}\"").unwrap_err();
    assert_eq!(
        err.code().as_str(),
        "parse.interpolated_text.empty_binding_name"
    );
}

#[test]
fn a_name_that_is_not_a_binding_identifier_is_rejected() {
    let err = parse_write_content("&\"&{1tag}\"").unwrap_err();
    assert_eq!(err.code().as_str(), "semantic.binding.invalid_identifier");
}

#[test]
fn an_undefined_binding_reference_is_rejected() {
    let err = parse_write_content("&\"&{missing}\"").unwrap_err();
    assert_eq!(err.code().as_str(), "semantic.binding.undefined");
}

// ─── Source spans ───────────────────────────────────────────────────────────

/// The reference span addresses the source the user wrote, so a diagnostic can
/// quote it directly.
#[test]
fn an_inline_reference_span_covers_the_marker_in_the_source() {
    let src = "case \"x\" {\n  $ true\n  let tag <- stdout_line\n  write <\"a.txt\"> &\"ab &{tag} cd\"\n  assert { exit 0 }\n}\n";
    let source_file = parse(src).unwrap();
    let script = source_file.into_script();
    let Step::SideEffect(SideEffectingStep::WriteFile(step)) = &script.cases[0].steps[2] else {
        panic!("expected third step to be a write step");
    };
    let TextValueExpression::Interpolated(text) = &step.content else {
        panic!("expected an interpolated text expression");
    };
    let reference = text.binding_references().next().expect("one reference");
    assert_eq!(
        &src[reference.reference_span.start..reference.reference_span.end],
        "&{tag}"
    );
    assert_eq!(reference.reference_span.line, 4);
}

/// Dedenting builds an intermediate text; a reference span must still address
/// the original body, offset by the indentation the dedent stripped.
#[test]
fn a_heredoc_reference_span_addresses_the_original_indented_body() {
    let src = "case \"x\" {\n  $ true\n  let tag <- stdout_line\n  write <\"a.txt\"> &```\n    prefix &{tag}\n    ```\n  assert { exit 0 }\n}\n";
    let source_file = parse(src).unwrap();
    let script = source_file.into_script();
    let Step::SideEffect(SideEffectingStep::WriteFile(step)) = &script.cases[0].steps[2] else {
        panic!("expected third step to be a write step");
    };
    let TextValueExpression::Interpolated(text) = &step.content else {
        panic!("expected an interpolated text expression");
    };
    let reference = text.binding_references().next().expect("one reference");
    assert_eq!(
        &src[reference.reference_span.start..reference.reference_span.end],
        "&{tag}"
    );
    assert_eq!(reference.reference_span.line, 5);
    // 4 columns of stripped indentation, then "prefix ".
    assert_eq!(reference.reference_span.column, 12);
}

// ─── Consumer surface ───────────────────────────────────────────────────────

/// Interpolation is available from every `TextValue` position because each one
/// references the shared grammar category, not because each was enumerated.
#[test]
fn every_text_value_position_accepts_an_interpolated_literal() {
    let src = concat!(
        "case \"x\" {\n",
        "  $ true\n",
        "  let tag <- stdout_line\n",
        "  write <\"a.txt\"> &\"&{tag}\"\n",
        "  assert {\n",
        "    stdout contains &\"&{tag}\"\n",
        "    stdout text_equals &\"&{tag}\"\n",
        "    stderr contains &\"&{tag}\"\n",
        "    stderr text_equals &\"&{tag}\"\n",
        "    file <\"a.txt\"> contains &\"&{tag}\"\n",
        "    file <\"a.txt\"> text_equals &\"&{tag}\"\n",
        "  }\n",
        "}\n",
    );
    parse_script(src).expect("every TextValue position accepts an interpolated string literal");
}

/// The heredoc form is available wherever the heredoc grammar category is
/// wired up, which is every `TextValue` position except the `contains` form of
/// a stream subject.
#[test]
fn every_heredoc_text_value_position_accepts_an_interpolated_heredoc() {
    let src = concat!(
        "case \"x\" {\n",
        "  $ true\n",
        "  let tag <- stdout_line\n",
        "  write <\"a.txt\"> &```\n    &{tag}\n    ```\n",
        "  assert {\n",
        "    stdout text_equals &```\n      &{tag}\n      ```\n",
        "    stderr text_equals &```\n      &{tag}\n      ```\n",
        "    file <\"a.txt\"> contains &```\n      &{tag}\n      ```\n",
        "    file <\"a.txt\"> text_equals &```\n      &{tag}\n      ```\n",
        "  }\n",
        "}\n",
    );
    parse_script(src).expect("every heredoc TextValue position accepts an interpolated heredoc");
}

/// A raw literal is unaffected: `&{name}` inside one stays literal text, which
/// is what keeps shell scripts and other template syntax predictable.
#[test]
fn a_raw_literal_keeps_a_marker_as_literal_text() {
    let raw_string = parse_write_content("\"&{tag}\"").unwrap();
    assert_eq!(
        raw_string.binding_free_text_value().unwrap().as_str(),
        "&{tag}"
    );
    let raw_heredoc = parse_write_content("```\n    echo \"&{tag}\"\n    ```").unwrap();
    assert_eq!(
        raw_heredoc.binding_free_text_value().unwrap().as_str(),
        "echo \"&{tag}\"\n"
    );
}

/// `contains` on a stream subject is the one inline-only position; its
/// wrong-kind diagnostic must not suggest a heredoc literal it would reject.
#[test]
fn an_inline_only_position_suggests_only_the_inline_form() {
    let src = "case \"x\" {\n  $ true\n  assert { stdout contains <\"a.txt\"> }\n}\n";
    let err = parse_script(src).unwrap_err();
    let ParseError::LiteralKindMismatch { suggestion, .. } = &err else {
        panic!("expected a literal kind mismatch, got {err:?}");
    };
    assert_eq!(suggestion, "\"a.txt\"");
}

/// `file contains` accepts a heredoc literal through `file_exp_heredoc`, so its
/// suggestion names both forms. The accepted forms come from the position's
/// declared surface, never from its diagnostic label.
#[test]
fn an_inline_and_heredoc_position_suggests_both_forms() {
    let src = "case \"x\" {\n  $ true\n  assert { file <\"a.txt\"> contains <\"b.txt\"> }\n}\n";
    let err = parse_script(src).unwrap_err();
    let ParseError::LiteralKindMismatch { suggestion, .. } = &err else {
        panic!("expected a literal kind mismatch, got {err:?}");
    };
    assert!(
        suggestion.contains("heredoc literal"),
        "expected the suggestion to name the heredoc form, got {suggestion:?}"
    );
}

/// `file <"path"> contains <heredoc>` must still reach the heredoc rule, so the
/// suggestion above is not steering authors into a syntax error.
#[test]
fn file_contains_accepts_a_raw_heredoc_literal() {
    let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"a.txt\"> contains ```\n      hello\n      ```\n  }\n}\n";
    let script = parse_script(src).unwrap();
    let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
        panic!("expected an assertion block");
    };
    let Expectation::File(file) = &block.expectations()[0] else {
        panic!("expected a file expectation");
    };
    assert!(matches!(
        file.matcher,
        FileMatcher::Contains(TextValueExpression::Raw(_))
    ));
}
