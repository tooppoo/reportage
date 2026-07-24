use super::*;
use crate::model::{AssertionBlock, Expectation, OutputMatcher, Step};

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
    let script = parse_script(src).unwrap();
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
    let script = parse_script(src).unwrap();
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
    let script = parse_script(src).unwrap();
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
    let script = parse_script(src).unwrap();
    assert_eq!(script.cases[0].steps.len(), 2);
    let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
        panic!("expected AssertionBlock");
    };
    assert_eq!(block.expectations().len(), 2);
}

#[test]
fn top_level_action_is_error() {
    let src = "$ true\n";
    let err = parse_script(src).unwrap_err();
    assert!(matches!(err, ParseError::Syntax { .. }));
}

#[test]
fn top_level_assert_is_error() {
    let src = "assert { exit 0 }\n";
    let err = parse_script(src).unwrap_err();
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
    let err = parse_script(src).unwrap_err();
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
    let err = parse_script(src).unwrap_err();
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
    assert!(parse_script(src).is_ok());
}

#[test]
fn unclosed_case_is_error() {
    let src = "case \"x\" {\n  $ true\n";
    let err = parse_script(src).unwrap_err();
    assert!(matches!(err, ParseError::Syntax { .. }));
}

#[test]
fn unclosed_assert_block_is_error() {
    let src = "case \"x\" {\n  $ true\n  assert {\n    exit 0\n";
    let err = parse_script(src).unwrap_err();
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
    let err = parse_script(src).unwrap_err();
    assert!(matches!(err, ParseError::Syntax { .. }));
}

#[test]
fn empty_assert_block_single_line_is_error() {
    let src = "case \"x\" {\n  $ true\n  assert { }\n}\n";
    let err = parse_script(src).unwrap_err();
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
    let err = parse_script(src).unwrap_err();
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
    let err = parse_script(src).unwrap_err();
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
    let script = parse_script(src).unwrap();
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
    let err = parse_script(src).unwrap_err();
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
    let script = parse_script(src).unwrap();
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
    let script = parse_script(src).unwrap();
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
    let script = parse_script(src).unwrap();
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
    let script = parse_script(src).unwrap();
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
    let script = parse_script(src).unwrap();
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
    let script = parse_script(src).unwrap();
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
    // `\\n` is an escaped backslash followed by a literal `n`, not an escaped newline.
    // See docs/adr/20260701T214658Z_string-literal-escape-sequences.md.
    let src = r#"
case "x" {
  $ true
  assert {
stdout contains "a\\nb"
  }
}
"#;
    let script = parse_script(src).unwrap();
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
    let script = parse_script(src).unwrap();
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
    let script = parse_script(src).unwrap();
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
    let script = parse_script(src).unwrap();
    assert_eq!(script.cases[0].name, "a \"b\" c");

    let err = parse_script("case \"a\\xb\" {\n  $ true\n  assert {\n    exit 0\n  }\n}\n")
        .unwrap_err();
    assert!(matches!(err, ParseError::Syntax { .. }));

    let err = parse_script("case \"a\nb\" {\n  $ true\n  assert {\n    exit 0\n  }\n}\n")
        .unwrap_err();
    assert!(matches!(err, ParseError::Syntax { .. }));
}

#[test]
fn raw_newline_in_string_is_rejected() {
    let src = "case \"x\" {\n  $ true\n  assert {\n    stdout contains \"a\nb\"\n  }\n}\n";
    let err = parse_script(src).unwrap_err();
    assert!(matches!(err, ParseError::Syntax { .. }));
}

#[test]
fn crlf_raw_newline_in_string_is_rejected() {
    let src = "case \"x\" {\n  $ true\n  assert {\n    stdout contains \"a\r\nb\"\n  }\n}\n";
    let err = parse_script(src).unwrap_err();
    assert!(matches!(err, ParseError::Syntax { .. }));
}

#[test]
fn bare_cr_in_string_is_rejected() {
    let src = "case \"x\" {\n  $ true\n  assert {\n    stdout contains \"a\rb\"\n  }\n}\n";
    let err = parse_script(src).unwrap_err();
    assert!(matches!(err, ParseError::Syntax { .. }));
}

#[test]
fn unclosed_string_literal_is_rejected() {
    let src =
        "case \"x\" {\n  $ true\n  assert {\n    stdout contains \"never closed\n  }\n}\n";
    let err = parse_script(src).unwrap_err();
    assert!(matches!(err, ParseError::Syntax { .. }));
}

#[test]
fn undefined_escape_sequence_is_rejected() {
    let src = "case \"x\" {\n  $ true\n  assert {\n    stdout contains \"a\\xb\"\n  }\n}\n";
    let err = parse_script(src).unwrap_err();
    assert!(matches!(err, ParseError::Syntax { .. }));
}

#[test]
fn undefined_escape_r_is_rejected() {
    let src = "case \"x\" {\n  $ true\n  assert {\n    stdout contains \"a\\rb\"\n  }\n}\n";
    let err = parse_script(src).unwrap_err();
    assert!(matches!(err, ParseError::Syntax { .. }));
}

#[test]
fn undefined_unicode_escape_is_rejected() {
    let src =
        "case \"x\" {\n  $ true\n  assert {\n    stdout contains \"a\\u{1245}b\"\n  }\n}\n";
    let err = parse_script(src).unwrap_err();
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
    let err = parse_script(src).unwrap_err();
    assert!(matches!(err, ParseError::Syntax { .. }));
}

// Trailing whitespace on any line must be accepted (parity with the hand-written parser, which called trim() on every line).
#[test]
fn trailing_whitespace_is_accepted() {
    // Trailing spaces on case opener, steps, assertion body, and closers.
    let src = "case \"x\" {   \n  $ true   \n  assert {   \n    exit 0   \n  }   \n}   \n";
    assert!(parse_script(src).is_ok());
}

// See #77 / docs/adr/20260705T184047Z_use-hash-comment-marker.md.
#[test]
fn line_comment_before_case_block_is_ignored() {
    let src = "# leading comment\ncase \"x\" {\n  $ true\n  assert { exit 0 }\n}\n";
    let script = parse_script(src).unwrap();
    assert_eq!(script.cases.len(), 1);
}

#[test]
fn comment_only_line_inside_case_block_is_ignored() {
    let src = "case \"x\" {\n  # comment\n  $ true\n  assert { exit 0 }\n}\n";
    let script = parse_script(src).unwrap();
    assert_eq!(script.cases[0].steps.len(), 2);
}

#[test]
fn comment_only_line_inside_assertion_block_is_ignored() {
    let src = "case \"x\" {\n  $ true\n  assert {\n    # comment\n    exit 0\n  }\n}\n";
    let script = parse_script(src).unwrap();
    let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
        panic!("expected AssertionBlock");
    };
    assert_eq!(block.expectations().len(), 1);
}

#[test]
fn inline_comment_after_case_and_assertion_block_boundaries_is_ignored() {
    let src = r#"
case "x" { # case open
  assert { # assert open
exit 0 # expectation
  } # assert close
} # case close
"#;
    let script = parse_script(src).unwrap();
    assert_eq!(script.cases[0].steps.len(), 1);
}

#[test]
fn inline_comment_after_single_line_assertion_block_is_ignored() {
    let src = "case \"x\" {\n  $ true\n  assert { exit 0 } # trailing\n}\n";
    assert!(parse_script(src).is_ok());
}

#[test]
fn hash_in_string_literal_is_not_treated_as_comment() {
    let src = "case \"x\" {\n  $ true\n  assert {\n    stdout contains \"hello # world\" # trailing comment\n  }\n}\n";
    let script = parse_script(src).unwrap();
    let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
        panic!("expected AssertionBlock");
    };
    assert!(matches!(
        &block.expectations()[0],
        Expectation::Stdout(e) if matches!(&e.matcher, OutputMatcher::Contains(s) if s == "hello # world")
    ));
}

#[test]
fn hash_in_action_command_is_preserved_as_command_text() {
    let src = "case \"x\" {\n  $ echo hello # passed to shell\n  assert { exit 0 }\n}\n";
    let script = parse_script(src).unwrap();
    let Step::Action(action) = &script.cases[0].steps[0] else {
        panic!("expected Action step");
    };
    assert_eq!(action.command, "echo hello # passed to shell");
}

// ─── Action line continuation (#80) ────────────────────────────────────

#[test]
fn action_continuation_joins_two_physical_lines() {
    let src = "case \"x\" {\n  $ echo one \\\n  two\n  assert { exit 0 }\n}\n";
    let script = parse_script(src).unwrap();
    assert_eq!(script.cases[0].steps.len(), 2);
    let Step::Action(action) = &script.cases[0].steps[0] else {
        panic!("expected Action step");
    };
    // The marker and the line break are preserved verbatim, as is the
    // continued line's own indentation.
    assert_eq!(action.command, "echo one \\\n  two");
}

#[test]
fn action_continuation_marker_only_line_continues_further() {
    let src = "case \"x\" {\n  $ echo one \\\n\\\ntwo\n  assert { exit 0 }\n}\n";
    let script = parse_script(src).unwrap();
    assert_eq!(script.cases[0].steps.len(), 2);
    let Step::Action(action) = &script.cases[0].steps[0] else {
        panic!("expected Action step");
    };
    assert_eq!(action.command, "echo one \\\n\\\ntwo");
}

#[test]
fn action_continuation_includes_blank_line_then_resumes_normal_syntax() {
    let src = "case \"x\" {\n  $ echo one \\\n\n  assert { exit 0 }\n}\n";
    let script = parse_script(src).unwrap();
    assert_eq!(script.cases[0].steps.len(), 2);
    let Step::Action(action) = &script.cases[0].steps[0] else {
        panic!("expected Action step");
    };
    // The blank line itself is consumed as part of this action step (its
    // own newline ends the step, not a further continuation), and the
    // next line resumes as an ordinary case_step.
    assert_eq!(action.command, "echo one \\\n");
}

// Per the review note on #80, an action line immediately followed by an
// `assert {` line is a caution/invalid example, not a valid one: only the
// `assert {` line is swallowed into the action body (it does not itself
// end in a marker), so the block's real contents are left dangling as
// bare `Reportage` syntax and fail to parse.
#[test]
fn action_continuation_swallows_only_the_next_line_before_ending() {
    let src = "case \"x\" {\n  $ true \\\n  assert {\n    exit 0\n  }\n}\n";
    let err = parse_script(src).unwrap_err();
    assert!(matches!(err, ParseError::Syntax { .. }));
}

#[test]
fn action_continuation_marker_followed_by_space_is_not_continuation() {
    let src = "case \"x\" {\n  $ echo hi \\ \n  assert { exit 0 }\n}\n";
    let script = parse_script(src).unwrap();
    assert_eq!(script.cases[0].steps.len(), 2);
    let Step::Action(action) = &script.cases[0].steps[0] else {
        panic!("expected Action step");
    };
    // The trailing space after `\` is trimmed like any other trailing
    // whitespace, but the `\` itself is ordinary command text, not a
    // continuation marker, since it wasn't the line's last character.
    assert_eq!(action.command, "echo hi \\");
}

#[test]
fn action_continuation_marker_followed_by_hash_is_not_continuation() {
    let src = "case \"x\" {\n  $ echo hi \\# comment\n  assert { exit 0 }\n}\n";
    let script = parse_script(src).unwrap();
    assert_eq!(script.cases[0].steps.len(), 2);
    let Step::Action(action) = &script.cases[0].steps[0] else {
        panic!("expected Action step");
    };
    assert_eq!(action.command, "echo hi \\# comment");
}

#[test]
fn action_continuation_marker_immediately_before_eof_is_plain_syntax_error() {
    // No dedicated "unterminated continuation" error: EOF right after a
    // marker just leaves the enclosing case block unclosed, the same
    // generic `parse.syntax` failure as any other unclosed block.
    let src = "case \"x\" {\n  $ echo hi \\";
    let err = parse_script(src).unwrap_err();
    assert!(matches!(err, ParseError::Syntax { .. }));
    assert_eq!(err.code().as_str(), "parse.syntax");
}

#[test]
fn action_continuation_marker_is_a_literal_last_char_check_not_shell_unescaping() {
    // Reportage does not reinterpret shell escaping: only the physical
    // character immediately before the line break decides continuation,
    // so two consecutive backslashes still continue (the first is
    // ordinary command text; the second is the marker).
    let src = "case \"x\" {\n  $ echo hi \\\\\ntwo\n  assert { exit 0 }\n}\n";
    let script = parse_script(src).unwrap();
    let Step::Action(action) = &script.cases[0].steps[0] else {
        panic!("expected Action step");
    };
    assert_eq!(action.command, "echo hi \\\\\ntwo");
}

#[test]
fn inline_comment_glued_to_token_is_error() {
    let src = "case \"x\" {\n  $ true\n  assert {\n    exit 0#comment\n  }\n}\n";
    let err = parse_script(src).unwrap_err();
    assert!(matches!(err, ParseError::Syntax { .. }));
}

#[test]
fn double_slash_is_not_a_comment_marker() {
    let src = "// not a comment\ncase \"x\" {\n  $ true\n  assert { exit 0 }\n}\n";
    let err = parse_script(src).unwrap_err();
    assert!(matches!(err, ParseError::Syntax { .. }));
}

#[test]
fn comment_presence_does_not_change_ast_shape() {
    let with_comments = r#"
# leading comment
case "x" { # case open
  # standalone comment
  $ true
  assert { # assert open
# standalone comment
exit 0 # expectation
  } # assert close
} # case close
"#;
    let without_comments = r#"
case "x" {
  $ true
  assert {
exit 0
  }
}
"#;
    let with = parse_script(with_comments).unwrap();
    let without = parse_script(without_comments).unwrap();
    assert_eq!(format!("{with:?}"), format!("{without:?}"));
}

#[test]
fn comment_splitting_case_header_before_open_brace_is_error() {
    let src = "case \"x\" # comment\n{\n  $ true\n  assert { exit 0 }\n}\n";
    let err = parse_script(src).unwrap_err();
    assert!(matches!(err, ParseError::Syntax { .. }));
}

#[test]
fn comment_swallowing_single_line_assertion_close_brace_is_error() {
    let src = "case \"x\" {\n  $ true\n  assert { exit 0 # comment\n}\n";
    let err = parse_script(src).unwrap_err();
    assert!(matches!(err, ParseError::Syntax { .. }));
}

#[test]
fn comment_splitting_expectation_tokens_is_error() {
    let src = "case \"x\" {\n  $ true\n  assert {\n    exit # comment\n    0\n  }\n}\n";
    let err = parse_script(src).unwrap_err();
    assert!(matches!(err, ParseError::Syntax { .. }));
}

// A comment-only assertion block has no real expectation and must be rejected the same way an empty assertion block is, not accepted with an empty expectations list (which would panic in parse_assertion_block).
#[test]
fn comment_only_assertion_block_is_error() {
    let src = "case \"x\" {\n  $ true\n  assert {\n    # comment only\n  }\n}\n";
    let err = parse_script(src).unwrap_err();
    assert!(matches!(err, ParseError::Syntax { .. }));
}

// Diagnostic codes are the stable, external identifier of a ParseError.
// These tests pin the string form directly, independent of the enum variant name and of Display message text.
// See docs/reference/diagnostics.md.
#[test]
fn syntax_error_has_stable_code() {
    let err = parse_script("$ true\n").unwrap_err();
    assert_eq!(err.code().as_str(), "parse.syntax");

    let diagnostic = err.to_diagnostic();
    assert_eq!(diagnostic.code.as_str(), "parse.syntax");
    assert!(diagnostic.details.pest_message.is_some());
}

// A bare `expected X` message doesn't show what's actually on the offending
// line, which is often the only way to tell a syntax error apart from a
// deeper structural issue (e.g. a stray fence line closing the wrong block).
// The Display impl now echoes the source line with a caret under the column.
#[test]
fn syntax_error_display_includes_source_line_and_caret() {
    let src = "case \"x\" {\n  $ true\n  assert {\n    exit 0\n  }\n} extra\n";
    let err = parse_script(src).unwrap_err();
    let rendered = err.to_string();

    assert!(rendered.contains("} extra"));
    assert!(rendered.contains("  |   ^"));
}

#[test]
fn empty_case_has_stable_code() {
    let src = "case \"x\" {\n}\n";
    let err = parse_script(src).unwrap_err();
    assert_eq!(err.code().as_str(), "parse.empty_case");

    let diagnostic = err.to_diagnostic();
    assert_eq!(diagnostic.code.as_str(), "parse.empty_case");
    assert_eq!(diagnostic.details.raw_value.as_deref(), Some("x"));
}

#[test]
fn missing_assertion_block_has_stable_code() {
    let src = "case \"x\" {\n  $ true\n}\n";
    let err = parse_script(src).unwrap_err();
    assert_eq!(err.code().as_str(), "parse.missing_assertion_block");

    let diagnostic = err.to_diagnostic();
    assert_eq!(diagnostic.code.as_str(), "parse.missing_assertion_block");
    assert_eq!(diagnostic.details.raw_value.as_deref(), Some("x"));
}

#[test]
fn empty_action_has_stable_code() {
    let src = "case \"x\" {\n  $\n  assert {\n    exit 0\n  }\n}\n";
    let err = parse_script(src).unwrap_err();
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
    let err = parse_script(src).unwrap_err();
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
    let err = parse_script(src).unwrap_err();
    let diagnostic = err.to_diagnostic();

    assert_eq!(diagnostic.code.as_str(), "parse.invalid_exit_code");
    assert_eq!(diagnostic.message, err.to_string());
    assert_eq!(
        diagnostic.location.expect("location must be present").line,
        5
    );
    assert_eq!(diagnostic.details.raw_value.as_deref(), Some("999"));
}

// ── Source-level model: case spans ──────────────────────────────────────
//
// The case span contract: a span equals the pest `case_block` pair's byte
// range — leading indentation through the closing brace line's trailing
// whitespace / inline comment and line ending (when present) — and never
// includes surrounding blank lines or comment lines.
// See docs/adr/20260712T090000Z_parser-returns-source-level-model.md.

/// The one expected case's span slice, for single-case span tests.
fn single_case_source(src: &str) -> String {
    let source_file = parse(src).unwrap();
    assert_eq!(source_file.cases().len(), 1);
    source_file.case_source(&source_file.cases()[0]).to_string()
}

#[test]
fn source_file_owns_input_text() {
    let src = "case \"x\" {\n  $ true\n  assert { exit 0 }\n}\n";
    let source_file = parse(src).unwrap();
    assert_eq!(source_file.source().as_str(), src);
}

#[test]
fn case_span_covers_whole_block_and_final_newline() {
    let src = "case \"x\" {\n  $ true\n  assert { exit 0 }\n}\n";
    assert_eq!(single_case_source(src), src);
}

#[test]
fn case_span_excludes_surrounding_blank_and_comment_lines() {
    let block = "case \"x\" {\n  $ true\n  assert { exit 0 }\n}\n";
    let src = format!("\n# leading comment\n\n{block}\n# trailing comment\n\n");
    let source_file = parse(&src).unwrap();
    assert_eq!(source_file.cases().len(), 1);
    let span = source_file.cases()[0].span();
    assert_eq!(source_file.case_source(&source_file.cases()[0]), block);
    assert_eq!(span.start(), src.find("case").unwrap());
}

#[test]
fn case_span_includes_leading_indentation() {
    let block = "  case \"x\" {\n    $ true\n    assert { exit 0 }\n  }\n";
    let src = format!("\n{block}");
    assert_eq!(single_case_source(&src), block);
}

#[test]
fn case_span_includes_closing_brace_trailing_inline_comment() {
    let src = "case \"x\" {\n  $ true\n  assert { exit 0 }\n} # done\n";
    assert_eq!(single_case_source(src), src);
}

#[test]
fn case_span_without_final_newline_ends_at_eoi() {
    let src = "case \"x\" {\n  $ true\n  assert { exit 0 }\n}";
    assert_eq!(single_case_source(src), src);
}

#[test]
fn case_span_with_crlf_line_endings_includes_final_crlf() {
    let src = "case \"x\" {\r\n  $ true\r\n  assert { exit 0 }\r\n}\r\n";
    assert_eq!(single_case_source(src), src);
}

#[test]
fn case_span_with_heredoc_body_covers_whole_block() {
    let src = "case \"x\" {\n  write <\"o.txt\"> ```\n  line\n  ```\n  $ true\n  assert { exit 0 }\n}\n";
    assert_eq!(single_case_source(src), src);
}

#[test]
fn case_span_with_multibyte_text_stays_on_char_boundaries() {
    let block = "case \"日本語のケース\" {\n  $ echo \"あいうえお\"\n  assert { exit 0 }\n}\n";
    let src = format!("# 説明コメント\n{block}");
    let source_file = parse(&src).unwrap();
    let span = source_file.cases()[0].span();
    assert!(src.is_char_boundary(span.start()) && src.is_char_boundary(span.end()));
    assert_eq!(source_file.case_source(&source_file.cases()[0]), block);
}

#[test]
fn multiple_case_spans_are_ordered_and_exclude_gaps() {
    let first = "case \"first\" {\n  $ true\n  assert { exit 0 }\n}\n";
    let second = "case \"second\" {\n  $ false\n  assert { exit 1 }\n}\n";
    let src = format!("{first}\n# between\n\n{second}");
    let source_file = parse(&src).unwrap();
    assert_eq!(source_file.cases().len(), 2);
    let (a, b) = (&source_file.cases()[0], &source_file.cases()[1]);
    assert_eq!(source_file.case_source(a), first);
    assert_eq!(source_file.case_source(b), second);
    assert!(a.span().end() <= b.span().start());
}

#[test]
fn into_script_preserves_case_order_and_drops_source() {
    let src = "case \"a\" {\n  $ true\n  assert { exit 0 }\n}\ncase \"b\" {\n  $ true\n  assert { exit 0 }\n}\n";
    let script = parse(src).unwrap().into_script();
    assert_eq!(script.cases.len(), 2);
    assert_eq!(script.cases[0].name, "a");
    assert_eq!(script.cases[1].name, "b");
}
