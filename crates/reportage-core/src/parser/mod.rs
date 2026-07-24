use pest::Parser;
use pest_derive::Parser;

use crate::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticDetails, DiagnosticLocation};
use crate::model::{
    ActionStep, AssertionBlock, BeforeEach, BeforeEachError, Case, DirExpectation, DirMatcher,
    ExitExpectation, Expectation, FileContentsReference, FileExpectation, FileMatcher,
    FixtureReference, FixtureReferenceError, LogicalExpectation, LogicalOperator,
    OutputExpectation, OutputMatcher, RequiredLiteralKind, SideEffectingStep, Step, TextLiteral,
    ValueLiteralKind, WorkspacePath, WorkspacePathError, WriteFileStep,
};
use crate::source::{
    CaseDocumentation, DocumentationText, FileDocumentation, SourceCase, SourceFile, SourceSpan,
    SourceText,
};

#[derive(Parser)]
#[grammar = "reportage.pest"]
struct ReportageParser;

#[derive(Debug, PartialEq)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Script;

    /// Most tests here assert against the execution model, so they project
    /// the parse result immediately; span-focused tests call `parse` directly.
    fn parse_script(src: &str) -> Result<Script, ParseError> {
        parse(src).map(SourceFile::into_script)
    }

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

    #[test]
    fn parse_file_exists() {
        let src = r#"
case "x" {
  $ true
  assert {
    file <"out/result.json"> exists
  }
}
"#;
        let script = parse_script(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert!(matches!(
            &block.expectations()[0],
            Expectation::File(f) if f.path == "out/result.json" && matches!(f.matcher, FileMatcher::Exists)
        ));
    }

    #[test]
    fn parse_file_contains() {
        let src = r#"
case "x" {
  $ true
  assert {
    file <"out/result.json"> contains "\"status\":\"passed\""
  }
}
"#;
        let script = parse_script(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert!(matches!(
            &block.expectations()[0],
            Expectation::File(f) if f.path == "out/result.json"
                && matches!(&f.matcher, FileMatcher::Contains(s)
                    if s.to_text_value().as_str() == "\"status\":\"passed\"")
        ));
    }

    #[test]
    fn file_exists_and_contains_combine_with_process_expectations() {
        let src = r#"
case "x" {
  $ true
  assert {
    exit 0
    file <"a.txt"> exists
    file <"a.txt"> contains "hi"
  }
}
"#;
        let script = parse_script(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert_eq!(block.expectations().len(), 3);
    }

    // `file <expectation> <path> <...args>` (expectation-first) is not the v0 syntax; only the subject-first `file <"path"> <predicate>` form parses.
    // See docs/adr/20260704T112155Z_subject-first-file-assertion-syntax.md.
    #[test]
    fn expectation_first_file_form_is_rejected() {
        let src = r#"
case "x" {
  $ true
  assert {
    file exists "a.txt"
  }
}
"#;
        let err = parse_script(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn file_predicate_without_path_is_rejected() {
        let src = r#"
case "x" {
  $ true
  assert {
    file exists
  }
}
"#;
        let err = parse_script(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn file_contains_without_text_is_rejected() {
        let src = r#"
case "x" {
  $ true
  assert {
    file <"a.txt"> contains
  }
}
"#;
        let err = parse_script(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    // ─── dir assertions (#66) ───────────────────────────────────────────────

    #[test]
    fn parse_dir_exists() {
        let src = r#"
case "x" {
  $ true
  assert {
    dir <"out"> exists
  }
}
"#;
        let script = parse_script(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert!(matches!(
            &block.expectations()[0],
            Expectation::Dir(d) if d.path == "out" && matches!(d.matcher, DirMatcher::Exists)
        ));
    }

    #[test]
    fn parse_dir_contains() {
        let src = r#"
case "x" {
  $ true
  assert {
    dir <"artifacts"> contains "result.json"
  }
}
"#;
        let script = parse_script(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert!(matches!(
            &block.expectations()[0],
            Expectation::Dir(d) if d.path == "artifacts"
                && matches!(&d.matcher, DirMatcher::Contains(s) if s == "result.json")
        ));
    }

    #[test]
    fn dir_exists_and_contains_combine_with_process_expectations() {
        let src = r#"
case "x" {
  $ true
  assert {
    exit 0
    dir <"a"> exists
    dir <"a"> contains "b"
  }
}
"#;
        let script = parse_script(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert_eq!(block.expectations().len(), 3);
    }

    // `dir <expectation> <path> <...args>` (expectation-first) is not the v0 syntax; only the subject-first `dir <"path"> <predicate>` form parses.
    // See docs/adr/20260706T000000Z_subject-first-directory-assertion-syntax.md.
    #[test]
    fn expectation_first_dir_form_is_rejected() {
        let src = r#"
case "x" {
  $ true
  assert {
    dir exists "a"
  }
}
"#;
        let err = parse_script(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn dir_predicate_without_path_is_rejected() {
        let src = r#"
case "x" {
  $ true
  assert {
    dir exists
  }
}
"#;
        let err = parse_script(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn dir_contains_without_name_is_rejected() {
        let src = r#"
case "x" {
  $ true
  assert {
    dir <"a"> contains
  }
}
"#;
        let err = parse_script(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    // ─── Logical composition (#25) ────────────────────────────────────────

    fn logical_children(expectation: &Expectation) -> &[Expectation] {
        match expectation {
            Expectation::Logical(l) => l.children(),
            other => panic!("expected Expectation::Logical, got {other:?}"),
        }
    }

    #[test]
    fn parse_not_block_single_line() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    not { exit 1 }\n  }\n}\n";
        let script = parse_script(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        let Expectation::Logical(l) = &block.expectations()[0] else {
            panic!("expected Logical expectation");
        };
        assert!(matches!(l.operator(), LogicalOperator::Not));
        assert_eq!(l.children().len(), 1);
        assert!(matches!(l.children()[0], Expectation::Exit(_)));
    }

    #[test]
    fn parse_all_block_multi_line() {
        let src = r#"
case "x" {
  $ true
  assert {
    all {
      exit 0
      stdout empty
    }
  }
}
"#;
        let script = parse_script(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        let Expectation::Logical(l) = &block.expectations()[0] else {
            panic!("expected Logical expectation");
        };
        assert!(matches!(l.operator(), LogicalOperator::All));
        assert_eq!(l.children().len(), 2);
    }

    #[test]
    fn parse_any_block_multi_line() {
        let src = r#"
case "x" {
  $ true
  assert {
    any {
      exit 0
      exit 1
    }
  }
}
"#;
        let script = parse_script(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert!(matches!(
            &block.expectations()[0],
            Expectation::Logical(l) if matches!(l.operator(), LogicalOperator::Any)
        ));
    }

    #[test]
    fn parse_nested_logical_composition() {
        let src = r#"
case "x" {
  $ true
  assert {
    all {
      not {
        exit 1
      }
      any {
        exit 0
        exit 2
      }
    }
  }
}
"#;
        let script = parse_script(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        let outer_children = logical_children(&block.expectations()[0]);
        assert_eq!(outer_children.len(), 2);
        assert!(matches!(
            &outer_children[0],
            Expectation::Logical(l) if matches!(l.operator(), LogicalOperator::Not)
        ));
        assert!(matches!(
            &outer_children[1],
            Expectation::Logical(l) if matches!(l.operator(), LogicalOperator::Any)
        ));
    }

    #[test]
    fn empty_not_block_is_semantic_empty_block_error() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    not { }\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::EmptyLogicalCompositionBlock {
                operator: LogicalOperator::Not,
                ..
            }
        ));
        assert_eq!(err.code().as_str(), "semantic.expectation.empty_block");
    }

    #[test]
    fn empty_all_block_multi_line_is_semantic_empty_block_error() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    all {\n    }\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::EmptyLogicalCompositionBlock {
                operator: LogicalOperator::All,
                ..
            }
        ));
        assert_eq!(err.code().as_str(), "semantic.expectation.empty_block");
    }

    #[test]
    fn empty_any_block_with_comment_only_is_semantic_empty_block_error() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    any {\n      # no expectations here\n    }\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::EmptyLogicalCompositionBlock {
                operator: LogicalOperator::Any,
                ..
            }
        ));
    }

    #[test]
    fn empty_logical_composition_block_diagnostic_details_record_operator() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    all { }\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        let diagnostic = err.to_diagnostic();
        assert_eq!(diagnostic.code.as_str(), "semantic.expectation.empty_block");
        assert_eq!(diagnostic.details.raw_value.as_deref(), Some("all"));
    }

    #[test]
    fn and_block_is_not_accepted_as_logical_composition() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    and { exit 0 }\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn or_block_is_not_accepted_as_logical_composition() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    or { exit 0 }\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn infix_and_between_expectations_is_rejected() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    exit 0 and exit 0\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn infix_or_between_expectations_is_rejected() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    exit 0 or exit 1\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn single_line_composition_block_multiple_expectations_is_error() {
        // Mirrors single_line_assert_multiple_expectations_is_error: a composition block's single-line form accepts exactly one expectation, same as assert { ... }'s.
        let src = "case \"x\" {\n  $ true\n  assert {\n    all { exit 0 exit 1 }\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    // ─── Write step: string literal / heredoc literal (#67, #86) ──────────

    fn write_file_step(script: &Script) -> &WriteFileStep {
        let Step::SideEffect(SideEffectingStep::WriteFile(step)) = &script.cases[0].steps[0] else {
            panic!("expected first step to be a write step");
        };
        step
    }

    #[test]
    fn parse_basic_write_step() {
        let src = "case \"x\" {\n  write <\"a.txt\"> ```\n    hello\n    ```\n  $ true\n  assert {\n    exit 0\n  }\n}\n";
        let script = parse_script(src).unwrap();
        let step = write_file_step(&script);
        assert_eq!(step.path.as_str(), "a.txt");
        assert_eq!(step.content.to_text_value().as_str(), "hello\n");
        assert_eq!(script.cases[0].steps.len(), 3);
    }

    #[test]
    fn write_step_can_follow_an_action_in_source_order() {
        let src = "case \"x\" {\n  $ true\n  write <\"a.txt\"> ```\n    hello\n    ```\n  assert { exit 0 }\n}\n";
        let script = parse_script(src).unwrap();
        let Step::SideEffect(SideEffectingStep::WriteFile(step)) = &script.cases[0].steps[1] else {
            panic!("expected second step to be a write step");
        };
        assert_eq!(step.path.as_str(), "a.txt");
        assert_eq!(step.content.to_text_value().as_str(), "hello\n");
    }

    #[test]
    fn write_step_empty_block_content_is_empty_string() {
        let src = "case \"x\" {\n  write <\"empty.txt\"> ```\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
        let script = parse_script(src).unwrap();
        let step = write_file_step(&script);
        assert_eq!(step.content.to_text_value().as_str(), "");
    }

    #[test]
    fn write_step_blank_line_is_preserved_as_empty_line_after_dedent() {
        let src = "case \"x\" {\n  write <\"a.txt\"> ```\n    first\n\n    third\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
        let script = parse_script(src).unwrap();
        let step = write_file_step(&script);
        assert_eq!(step.content.to_text_value().as_str(), "first\n\nthird\n");
    }

    #[test]
    fn write_step_whitespace_only_line_is_dedented_to_empty_line() {
        // The blank line has trailing spaces shallower than the closing fence's indent;
        // it must still be exempt from the shallow-indent check and dedent to empty.
        let src = "case \"x\" {\n  write <\"a.txt\"> ```\n    first\n  \n    third\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
        let script = parse_script(src).unwrap();
        let step = write_file_step(&script);
        assert_eq!(step.content.to_text_value().as_str(), "first\n\nthird\n");
    }

    #[test]
    fn write_step_tab_indent_is_treated_as_literal_prefix_not_width() {
        // Closing fence indented with a tab; body lines must match that exact
        // tab character as a string prefix, not a width-equivalent number of spaces.
        let src = "case \"x\" {\n  write <\"a.txt\"> ```\n\thello\n\t```\n  $ true\n  assert { exit 0 }\n}\n";
        let script = parse_script(src).unwrap();
        let step = write_file_step(&script);
        assert_eq!(step.content.to_text_value().as_str(), "hello\n");
    }

    #[test]
    fn write_step_crlf_line_endings_are_preserved() {
        let src = "case \"x\" {\r\n  write <\"a.txt\"> ```\r\n    hello\r\n    ```\r\n  $ true\r\n  assert { exit 0 }\r\n}\r\n";
        let script = parse_script(src).unwrap();
        let step = write_file_step(&script);
        assert_eq!(step.content.to_text_value().as_str(), "hello\r\n");
    }

    #[test]
    fn write_step_content_preserves_variable_looking_text_literally() {
        let src = "case \"x\" {\n  write <\"a.txt\"> ```\n    ${ENTRY_KIND}\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
        let script = parse_script(src).unwrap();
        let step = write_file_step(&script);
        assert_eq!(step.content.to_text_value().as_str(), "${ENTRY_KIND}\n");
    }

    #[test]
    fn write_step_closing_fence_longer_than_opening_is_accepted() {
        let src = "case \"x\" {\n  write <\"a.txt\"> ```\n    hello\n    ````\n  $ true\n  assert { exit 0 }\n}\n";
        let script = parse_script(src).unwrap();
        let step = write_file_step(&script);
        assert_eq!(step.content.to_text_value().as_str(), "hello\n");
    }

    #[test]
    fn write_step_longer_opening_fence_allows_embedded_triple_backticks() {
        let src = "case \"x\" {\n  write <\"a.md\"> ````\n    ```ts\n    console.log(1)\n    ```\n    ````\n  $ true\n  assert { exit 0 }\n}\n";
        let script = parse_script(src).unwrap();
        let step = write_file_step(&script);
        assert_eq!(
            step.content.to_text_value().as_str(),
            "```ts\nconsole.log(1)\n```\n"
        );
    }

    #[test]
    fn write_step_shallow_indent_is_rejected() {
        // "mid" is indented less than the closing fence's 4 spaces.
        let src = "case \"x\" {\n  write <\"a.txt\"> ```\n    first\n  mid\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(err, ParseError::ShallowHeredocIndent { .. }));
        assert_eq!(err.code().as_str(), "parse.heredoc_literal.shallow_indent");
    }

    #[test]
    fn write_step_unterminated_fence_is_a_syntax_error() {
        let src =
            "case \"x\" {\n  write <\"a.txt\"> ```\n    hello\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn write_step_opening_fence_inline_comment_is_rejected() {
        let src = "case \"x\" {\n  write <\"a.txt\"> ``` # comment\n    hello\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn write_step_absolute_path_is_rejected() {
        let src = "case \"x\" {\n  write <\"/etc/passwd\"> ```\n    x\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidWorkspacePath {
                reason: WorkspacePathError::Absolute,
                ..
            }
        ));
        assert_eq!(err.code().as_str(), "semantic.workspace_path.absolute");
    }

    #[test]
    fn write_step_dot_segment_path_is_rejected() {
        let src = "case \"x\" {\n  write <\"../a.txt\"> ```\n    x\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidWorkspacePath {
                reason: WorkspacePathError::DotSegment,
                ..
            }
        ));
        assert_eq!(err.code().as_str(), "semantic.workspace_path.dot_segment");
    }

    #[test]
    fn invalid_workspace_path_message_names_its_own_position_not_write_step() {
        // `WorkspacePath::parse` backs both a `write` step's target path and a `contents_equals`
        // expected `<"...">` value. The Display message must name whichever position the raw
        // path actually came from, not hardcode "write step path" regardless of origin.
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"actual.txt\"> contents_equals <\"../expected.txt\">\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidWorkspacePath {
                reason: WorkspacePathError::DotSegment,
                position: "`file contents_equals` expected value",
                ..
            }
        ));
        let message = err.to_string();
        assert!(message.contains("`file contents_equals` expected value"));
        assert!(!message.contains("write step"));
    }

    #[test]
    fn write_step_empty_path_is_rejected() {
        let src =
            "case \"x\" {\n  write <\"\"> ```\n    x\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidWorkspacePath {
                reason: WorkspacePathError::Empty,
                ..
            }
        ));
        assert_eq!(err.code().as_str(), "semantic.workspace_path.empty");
    }

    #[test]
    fn multiple_write_steps_are_kept_in_source_order() {
        let src = "case \"x\" {\n  write <\"a.txt\"> ```\n    a\n    ```\n  write <\"b.txt\"> ```\n    b\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
        let script = parse_script(src).unwrap();
        assert_eq!(script.cases[0].steps.len(), 4);
        let Step::SideEffect(SideEffectingStep::WriteFile(first)) = &script.cases[0].steps[0]
        else {
            panic!("expected write step");
        };
        let Step::SideEffect(SideEffectingStep::WriteFile(second)) = &script.cases[0].steps[1]
        else {
            panic!("expected write step");
        };
        assert_eq!(first.path.as_str(), "a.txt");
        assert_eq!(second.path.as_str(), "b.txt");
    }

    // Known limitation (documented in docs/reference/semantics.md and the ADR): a
    // `write` step missing its own closing fence does not always produce a
    // syntax error. The grammar scans forward for the next line shaped like
    // a valid closing fence, which here belongs to what the author intended
    // as a *separate* `write <"b.txt">` step. That step's opening line is
    // silently absorbed as literal content of `a.txt`, and `b.txt`'s write
    // step disappears from the AST entirely — this test pins that exact
    // behavior so a future grammar change cannot silently alter it further
    // without a test failure calling it out.
    #[test]
    fn missing_closing_fence_silently_absorbs_a_later_write_step_as_content() {
        let src = concat!(
            "case \"x\" {\n",
            "  write <\"a.txt\"> ```\n",
            "    first\n",
            "    write <\"b.txt\"> ```\n",
            "    second\n",
            "    ```\n",
            "  $ true\n",
            "  assert { exit 0 }\n",
            "}\n",
        );
        let script = parse_script(src).unwrap();

        // Only 3 steps: the intended `write <"b.txt">` step never materializes.
        assert_eq!(script.cases[0].steps.len(), 3);

        let step = write_file_step(&script);
        assert_eq!(step.path.as_str(), "a.txt");
        assert_eq!(
            step.content.to_text_value().as_str(),
            "first\nwrite <\"b.txt\"> ```\nsecond\n"
        );

        assert!(matches!(script.cases[0].steps[1], Step::Action(_)));
        assert!(matches!(script.cases[0].steps[2], Step::AssertionBlock(_)));
    }

    // ─── Workspace path literal / literal kind mismatch (#93) ──────────────

    #[test]
    fn workspace_path_literal_reuses_string_literal_escape_rules() {
        // The inner quoted content of <"..."> shares quoted_string's escape
        // rules; the unescaped value is what reaches the AST.
        let src =
            "case \"x\" {\n  write <\"a\\tb.txt\"> \"content\"\n  $ true\n  assert { exit 0 }\n}\n";
        let script = parse_script(src).unwrap();
        let step = write_file_step(&script);
        assert_eq!(step.path.as_str(), "a\tb.txt");
    }

    #[test]
    fn file_subject_string_literal_is_kind_mismatch() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file \"out.txt\" exists\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::WorkspacePath,
                actual: ValueLiteralKind::StringLiteral,
                ..
            }
        ));
        assert_eq!(err.code().as_str(), "semantic.literal.kind_mismatch");

        // The message must be actionable: expected kind, actual kind, and
        // the suggested replacement.
        let message = err.to_string();
        assert!(message.contains("`file` checkpoint subject"));
        assert!(message.contains("WorkspacePath"));
        assert!(message.contains("StringLiteral"));
        assert!(message.contains("<\"out.txt\">"));
    }

    #[test]
    fn file_subject_fixture_reference_is_kind_mismatch() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file @\"out.txt\" exists\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::WorkspacePath,
                actual: ValueLiteralKind::FixtureReference,
                ..
            }
        ));

        let message = err.to_string();
        assert!(message.contains("FixtureReference"));
        assert!(message.contains("@\"out.txt\""));
        assert!(message.contains("<\"out.txt\">"));
    }

    #[test]
    fn heredoc_file_contains_subject_string_literal_is_kind_mismatch() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file \"out.txt\" contains ```\n    hi\n    ```\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::WorkspacePath,
                ..
            }
        ));
    }

    #[test]
    fn dir_subject_string_literal_is_kind_mismatch() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    dir \"out\" exists\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::WorkspacePath,
                actual: ValueLiteralKind::StringLiteral,
                ..
            }
        ));
        let message = err.to_string();
        assert!(message.contains("`dir` checkpoint subject"));
        assert!(message.contains("<\"out\">"));
    }

    #[test]
    fn write_path_string_literal_is_kind_mismatch() {
        let src = "case \"x\" {\n  write \"a.txt\" \"content\"\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::WorkspacePath,
                actual: ValueLiteralKind::StringLiteral,
                ..
            }
        ));
        let message = err.to_string();
        assert!(message.contains("`write` step path"));
    }

    #[test]
    fn write_heredoc_path_string_literal_is_kind_mismatch() {
        let src = "case \"x\" {\n  write \"a.txt\" ```\n    hello\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::WorkspacePath,
                ..
            }
        ));
    }

    #[test]
    fn write_content_workspace_path_literal_is_kind_mismatch() {
        let src =
            "case \"x\" {\n  write <\"a.txt\"> <\"b.txt\">\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::TextValue,
                actual: ValueLiteralKind::WorkspacePath,
                ..
            }
        ));
        let message = err.to_string();
        assert!(message.contains("`write` step content"));
        assert!(message.contains("string literal or heredoc literal"));
    }

    #[test]
    fn stdout_contains_workspace_path_literal_is_kind_mismatch() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    stdout contains <\"expected.stdout\">\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::TextValue,
                actual: ValueLiteralKind::WorkspacePath,
                ..
            }
        ));
        let message = err.to_string();
        assert!(message.contains("`stdout contains` expected text"));
        assert!(message.contains("TextValue"));
        // v0's grammar only wires the heredoc TextValue form into `write`
        // content and `file contains`; the suggestion here must not steer
        // the author toward a heredoc literal the grammar would reject.
        assert!(message.contains("use \"expected.stdout\" instead"));
        assert!(!message.contains("heredoc"));
    }

    #[test]
    fn stderr_contains_fixture_reference_is_kind_mismatch() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    stderr contains @\"expected.stderr\"\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::TextValue,
                actual: ValueLiteralKind::FixtureReference,
                ..
            }
        ));
        let message = err.to_string();
        assert!(message.contains("`stderr contains` expected text"));
        assert!(message.contains("use \"expected.stderr\" instead"));
        assert!(!message.contains("heredoc"));
    }

    #[test]
    fn file_contains_expected_workspace_path_literal_is_kind_mismatch() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> contains <\"expected.txt\">\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::TextValue,
                actual: ValueLiteralKind::WorkspacePath,
                ..
            }
        ));
        let message = err.to_string();
        assert!(message.contains("`file contains` expected text"));
    }

    #[test]
    fn dir_contains_entry_workspace_path_literal_is_kind_mismatch() {
        let src =
            "case \"x\" {\n  $ true\n  assert {\n    dir <\"out\"> contains <\"entry\">\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::StringLiteral,
                actual: ValueLiteralKind::WorkspacePath,
                ..
            }
        ));
        // The suggestion for a StringLiteral requirement is the same quoted
        // content without the workspace path wrapper.
        let message = err.to_string();
        assert!(message.contains("`dir contains` entry name"));
        assert!(message.contains("use \"entry\" instead"));
    }

    #[test]
    fn literal_kind_mismatch_diagnostic_carries_expected_actual_and_suggestion() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file \"out.txt\" exists\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        let diagnostic = err.to_diagnostic();

        assert_eq!(diagnostic.code.as_str(), "semantic.literal.kind_mismatch");
        assert_eq!(
            diagnostic.location.expect("location must be present").line,
            4
        );
        assert_eq!(diagnostic.details.raw_value.as_deref(), Some("\"out.txt\""));
        assert_eq!(
            diagnostic.details.expected_kind.as_deref(),
            Some("WorkspacePath")
        );
        assert_eq!(
            diagnostic.details.actual_kind.as_deref(),
            Some("StringLiteral")
        );
        assert_eq!(
            diagnostic.details.suggestion.as_deref(),
            Some("<\"out.txt\">")
        );
    }

    #[test]
    fn whitespace_between_path_marker_and_quote_is_syntax_error() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file < \"out.txt\"> exists\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));

        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\" > exists\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    // ─── Fixture reference literal / contents_equals / text_equals (#92) ───

    #[test]
    fn file_contents_equals_accepts_workspace_path_literal() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> contents_equals <\"expected.txt\">\n  }\n}\n";
        let script = parse_script(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected assertion block");
        };
        let Expectation::File(file_exp) = &block.expectations()[0] else {
            panic!("expected file expectation");
        };
        assert!(matches!(
            file_exp.matcher,
            FileMatcher::ContentsEquals(FileContentsReference::Workspace(_))
        ));
    }

    #[test]
    fn file_contents_equals_accepts_fixture_reference_literal() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> contents_equals @\"expected.txt\"\n  }\n}\n";
        let script = parse_script(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected assertion block");
        };
        let Expectation::File(file_exp) = &block.expectations()[0] else {
            panic!("expected file expectation");
        };
        match &file_exp.matcher {
            FileMatcher::ContentsEquals(FileContentsReference::Fixture(fixture)) => {
                assert_eq!(fixture.as_str(), "expected.txt");
            }
            other => panic!("expected fixture contents_equals, got {other:?}"),
        }
    }

    #[test]
    fn file_contents_equals_rejects_string_literal() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> contents_equals \"expected.txt\"\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::FileContentsReference,
                actual: ValueLiteralKind::StringLiteral,
                ..
            }
        ));
        let message = err.to_string();
        assert!(message.contains("`file contents_equals` expected value"));
        assert!(message.contains("workspace path literal or fixture reference literal"));
    }

    #[test]
    fn file_contents_equals_subject_fixture_reference_is_kind_mismatch() {
        // The `file` checkpoint subject requires a WorkspacePath, never a
        // FixtureReference, regardless of which predicate follows it.
        let src = "case \"x\" {\n  $ true\n  assert {\n    file @\"actual.txt\" contents_equals @\"expected.txt\"\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::WorkspacePath,
                actual: ValueLiteralKind::FixtureReference,
                ..
            }
        ));
    }

    #[test]
    fn file_text_equals_accepts_string_literal() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> text_equals \"expected\"\n  }\n}\n";
        let script = parse_script(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected assertion block");
        };
        let Expectation::File(file_exp) = &block.expectations()[0] else {
            panic!("expected file expectation");
        };
        match &file_exp.matcher {
            FileMatcher::TextEquals(TextLiteral::Quoted(value)) => {
                assert_eq!(value, "expected");
            }
            other => panic!("expected quoted text_equals, got {other:?}"),
        }
    }

    #[test]
    fn file_text_equals_rejects_fixture_reference() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> text_equals @\"expected.txt\"\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::TextValue,
                actual: ValueLiteralKind::FixtureReference,
                ..
            }
        ));
        let message = err.to_string();
        assert!(message.contains("`file text_equals` expected text"));
    }

    #[test]
    fn file_text_equals_rejects_workspace_path_literal() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> text_equals <\"expected.txt\">\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::TextValue,
                actual: ValueLiteralKind::WorkspacePath,
                ..
            }
        ));
        let message = err.to_string();
        assert!(message.contains("`file text_equals` expected text"));
        assert!(message.contains("string literal or heredoc literal"));
    }

    #[test]
    fn file_text_equals_accepts_heredoc_literal() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> text_equals ```\n    hello\n    world\n    ```\n  }\n}\n";
        let script = parse_script(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected assertion block");
        };
        let Expectation::File(file_exp) = &block.expectations()[0] else {
            panic!("expected file expectation");
        };
        match &file_exp.matcher {
            FileMatcher::TextEquals(TextLiteral::Heredoc(value)) => {
                assert_eq!(value, "hello\nworld\n");
            }
            other => panic!("expected heredoc text_equals, got {other:?}"),
        }
    }

    #[test]
    fn heredoc_file_text_equals_subject_string_literal_is_kind_mismatch() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file \"out.txt\" text_equals ```\n    hi\n    ```\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::WorkspacePath,
                ..
            }
        ));
    }

    #[test]
    fn stdout_text_equals_accepts_string_literal() {
        let src =
            "case \"x\" {\n  $ true\n  assert {\n    stdout text_equals \"hello\\n\"\n  }\n}\n";
        let script = parse_script(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected assertion block");
        };
        match &block.expectations()[0] {
            Expectation::Stdout(OutputExpectation {
                matcher: OutputMatcher::TextEquals(TextLiteral::Quoted(value)),
            }) => assert_eq!(value, "hello\n"),
            other => panic!("expected quoted stdout text_equals, got {other:?}"),
        }
    }

    #[test]
    fn stderr_text_equals_accepts_heredoc_literal() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    stderr text_equals ```\n    warn\n    line\n    ```\n  }\n}\n";
        let script = parse_script(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected assertion block");
        };
        match &block.expectations()[0] {
            Expectation::Stderr(OutputExpectation {
                matcher: OutputMatcher::TextEquals(TextLiteral::Heredoc(value)),
            }) => assert_eq!(value, "warn\nline\n"),
            other => panic!("expected heredoc stderr text_equals, got {other:?}"),
        }
    }

    #[test]
    fn stdout_text_equals_heredoc_parses_alongside_other_expectations() {
        // The heredoc form is a separate heredoc_assertion_line alternative (see the grammar);
        // an ordinary expectation line following the heredoc must still parse.
        let src = "case \"x\" {\n  $ true\n  assert {\n    stdout text_equals ```\n    hello\n    ```\n    exit 0\n  }\n}\n";
        let script = parse_script(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected assertion block");
        };
        assert_eq!(block.expectations().len(), 2);
        assert!(matches!(
            &block.expectations()[0],
            Expectation::Stdout(OutputExpectation {
                matcher: OutputMatcher::TextEquals(TextLiteral::Heredoc(_)),
            })
        ));
        assert!(matches!(&block.expectations()[1], Expectation::Exit(_)));
    }

    #[test]
    fn stdout_text_equals_workspace_path_literal_is_kind_mismatch() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    stdout text_equals <\"expected.txt\">\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::TextValue,
                actual: ValueLiteralKind::WorkspacePath,
                ..
            }
        ));
        let message = err.to_string();
        assert!(message.contains("`stdout text_equals` expected text"));
        assert!(message.contains("string literal or heredoc literal"));
    }

    #[test]
    fn stderr_text_equals_fixture_reference_is_kind_mismatch() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    stderr text_equals @\"expected.txt\"\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::TextValue,
                actual: ValueLiteralKind::FixtureReference,
                ..
            }
        ));
        let message = err.to_string();
        assert!(message.contains("`stderr text_equals` expected text"));
    }

    #[test]
    fn stdout_contents_equals_accepts_fixture_reference_literal() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    stdout contents_equals @\"stdout.snapshot\"\n  }\n}\n";
        let script = parse_script(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected assertion block");
        };
        assert!(matches!(
            &block.expectations()[0],
            Expectation::Stdout(OutputExpectation {
                matcher: OutputMatcher::ContentsEquals(FileContentsReference::Fixture(_)),
            })
        ));
    }

    #[test]
    fn stderr_contents_equals_accepts_workspace_path_literal() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    stderr contents_equals <\"expected.txt\">\n  }\n}\n";
        let script = parse_script(src).unwrap();
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected assertion block");
        };
        assert!(matches!(
            &block.expectations()[0],
            Expectation::Stderr(OutputExpectation {
                matcher: OutputMatcher::ContentsEquals(FileContentsReference::Workspace(_)),
            })
        ));
    }

    #[test]
    fn fixture_reference_empty_path_is_rejected() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> contents_equals @\"\"\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidFixtureReference {
                reason: FixtureReferenceError::Empty,
                ..
            }
        ));
        assert_eq!(err.code().as_str(), "semantic.fixture_reference.empty");
    }

    #[test]
    fn fixture_reference_absolute_path_is_rejected() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> contents_equals @\"/etc/passwd\"\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidFixtureReference {
                reason: FixtureReferenceError::Absolute,
                ..
            }
        ));
        assert_eq!(err.code().as_str(), "semantic.fixture_reference.absolute");
    }

    #[test]
    fn fixture_reference_dot_segment_leading_parent_path_is_rejected() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> contents_equals @\"../escape.txt\"\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidFixtureReference {
                reason: FixtureReferenceError::DotSegment,
                ..
            }
        ));
        assert_eq!(
            err.code().as_str(),
            "semantic.fixture_reference.dot_segment"
        );
    }

    #[test]
    fn fixture_reference_dot_segment_leading_current_path_is_rejected() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> contents_equals @\"./escape.txt\"\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidFixtureReference {
                reason: FixtureReferenceError::DotSegment,
                ..
            }
        ));
        assert_eq!(
            err.code().as_str(),
            "semantic.fixture_reference.dot_segment"
        );
    }

    #[test]
    fn fixture_reference_dot_segment_middle_current_path_is_rejected() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> contents_equals @\"snapshots/./stdout.json\"\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidFixtureReference {
                reason: FixtureReferenceError::DotSegment,
                ..
            }
        ));
        assert_eq!(
            err.code().as_str(),
            "semantic.fixture_reference.dot_segment"
        );
    }

    #[test]
    fn fixture_reference_dot_segment_middle_parent_path_is_rejected() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    file <\"out.txt\"> contents_equals @\"snapshots/../stdout.json\"\n  }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidFixtureReference {
                reason: FixtureReferenceError::DotSegment,
                ..
            }
        ));
        assert_eq!(
            err.code().as_str(),
            "semantic.fixture_reference.dot_segment"
        );
    }

    #[test]
    fn write_step_content_fixture_reference_is_kind_mismatch() {
        // Outside an assertion block, a fixture reference literal is still
        // just a value_literal whose kind never matches a write step's
        // TextValue content requirement: fixture references are only valid
        // in a FileContentsReference expected position (#92).
        let src = "case \"x\" {\n  write <\"out.txt\"> @\"expected.txt\"\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::TextValue,
                actual: ValueLiteralKind::FixtureReference,
                ..
            }
        ));
    }

    #[test]
    fn write_step_path_fixture_reference_is_kind_mismatch() {
        let src =
            "case \"x\" {\n  write @\"out.txt\" \"content\"\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::WorkspacePath,
                actual: ValueLiteralKind::FixtureReference,
                ..
            }
        ));
    }

    #[test]
    fn workspace_path_literal_value_validation_still_applies_to_write_path() {
        // Kind and value validation are separate layers: a correctly-kinded
        // workspace path literal whose unescaped value violates the
        // workspace path policy still fails with the existing
        // semantic.workspace_path.* diagnostics.
        let src =
            "case \"x\" {\n  write <\"/etc/passwd\"> \"x\"\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse_script(src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidWorkspacePath {
                reason: WorkspacePathError::Absolute,
                ..
            }
        ));
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

    // ── Document block: `document file` ─────────────────────────────────────
    //
    // Field validation, placement rules, and the whitelist body contract.
    // See #168 and the accompanying ADR; representative valid shapes live in
    // examples/, e2e/, and tests/fixtures/syntax/valid/.

    const PASSING_CASE: &str = "case \"x\" {\n  $ true\n  assert { exit 0 }\n}\n";

    #[test]
    fn document_file_all_fields_are_parsed() {
        let src = format!(
            "document file {{\n  title \"File assertions\"\n  group \"Filesystem\"\n  order 20\n  description \"Collected examples.\"\n}}\n\n{PASSING_CASE}"
        );
        let source_file = parse(&src).unwrap();
        let documentation = source_file.file_documentation().unwrap();
        assert_eq!(documentation.title.as_deref(), Some("File assertions"));
        assert_eq!(documentation.group.as_deref(), Some("Filesystem"));
        assert_eq!(documentation.order, Some(20));
        assert_eq!(
            documentation.description.as_ref().unwrap().as_str(),
            "Collected examples."
        );
    }

    #[test]
    fn document_file_holds_only_explicit_fields() {
        let src = format!("document file {{\n  title \"Only a title\"\n}}\n\n{PASSING_CASE}");
        let source_file = parse(&src).unwrap();
        let documentation = source_file.file_documentation().unwrap();
        assert_eq!(documentation.title.as_deref(), Some("Only a title"));
        assert_eq!(documentation.group, None);
        assert_eq!(documentation.order, None);
        assert!(documentation.description.is_none());
    }

    #[test]
    fn source_without_document_file_has_no_documentation() {
        let source_file = parse(PASSING_CASE).unwrap();
        assert!(source_file.file_documentation().is_none());
    }

    #[test]
    fn document_file_description_heredoc_is_dedented() {
        let src = format!(
            "document file {{\n  description ```\n    line one\n\n    line two\n    ```\n}}\n\n{PASSING_CASE}"
        );
        let source_file = parse(&src).unwrap();
        let documentation = source_file.file_documentation().unwrap();
        assert_eq!(
            documentation.description.as_ref().unwrap().as_str(),
            "line one\n\nline two\n"
        );
    }

    #[test]
    fn document_file_description_heredoc_shallow_indent_is_rejected() {
        let src = format!(
            "document file {{\n  description ```\n  shallow\n    ```\n}}\n\n{PASSING_CASE}"
        );
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::ShallowHeredocIndent { .. }));
        assert_eq!(err.code().as_str(), "parse.heredoc_literal.shallow_indent");
    }

    #[test]
    fn document_file_order_accepts_zero_and_u64_max() {
        let src = format!("document file {{\n  order 0\n}}\n\n{PASSING_CASE}");
        let source_file = parse(&src).unwrap();
        assert_eq!(source_file.file_documentation().unwrap().order, Some(0));

        let src = format!("document file {{\n  order 18446744073709551615\n}}\n\n{PASSING_CASE}");
        let source_file = parse(&src).unwrap();
        assert_eq!(
            source_file.file_documentation().unwrap().order,
            Some(u64::MAX)
        );
    }

    #[test]
    fn document_file_order_overflow_is_rejected() {
        let src = format!("document file {{\n  order 18446744073709551616\n}}\n\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::InvalidDocumentationOrder { line: 2, .. }
        ));
        assert_eq!(err.code().as_str(), "parse.document_block.invalid_order");
    }

    #[test]
    fn duplicate_documentation_field_is_rejected() {
        let src = format!(
            "document file {{\n  title \"first\"\n  title \"second\"\n}}\n\n{PASSING_CASE}"
        );
        let err = parse(&src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::DuplicateDocumentationField {
                line: 3,
                field: "title"
            }
        ));
        assert_eq!(err.code().as_str(), "parse.document_block.duplicate_field");
    }

    #[test]
    fn duplicate_description_across_string_and_heredoc_forms_is_rejected() {
        let src = format!(
            "document file {{\n  description \"first\"\n  description ```\n    second\n    ```\n}}\n\n{PASSING_CASE}"
        );
        let err = parse(&src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::DuplicateDocumentationField {
                field: "description",
                ..
            }
        ));
    }

    #[test]
    fn empty_document_block_is_rejected() {
        let src = format!("document file {{\n}}\n\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::EmptyDocumentBlock { line: 1 }));
        assert_eq!(err.code().as_str(), "parse.document_block.empty");
    }

    #[test]
    fn comment_only_document_block_is_rejected_as_empty() {
        let src = format!("document file {{\n  # not a field\n}}\n\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::EmptyDocumentBlock { .. }));
    }

    #[test]
    fn multiple_document_file_blocks_are_rejected() {
        let src = format!(
            "document file {{\n  title \"first\"\n}}\n\ndocument file {{\n  title \"second\"\n}}\n\n{PASSING_CASE}"
        );
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::DuplicateDocumentFile { line: 5 }));
        assert_eq!(err.code().as_str(), "parse.document_file.duplicate");
    }

    #[test]
    fn document_file_after_first_case_is_rejected() {
        let src = format!("{PASSING_CASE}\ndocument file {{\n  title \"too late\"\n}}\n");
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::DocumentFileAfterCase { .. }));
        assert_eq!(err.code().as_str(), "parse.document_file.after_case");
    }

    #[test]
    fn document_file_between_cases_is_rejected() {
        let src = format!(
            "{PASSING_CASE}\ndocument file {{\n  title \"between\"\n}}\n\ncase \"y\" {{\n  $ true\n  assert {{ exit 0 }}\n}}\n"
        );
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::DocumentFileAfterCase { .. }));
    }

    #[test]
    fn unknown_documentation_field_is_syntax_error() {
        let src = format!("document file {{\n  author \"someone\"\n}}\n\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn action_step_in_document_block_is_syntax_error() {
        let src = format!("document file {{\n  $ echo hello\n}}\n\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn assertion_block_in_document_block_is_syntax_error() {
        let src = format!("document file {{\n  assert {{\n    exit 0\n  }}\n}}\n\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn write_step_in_document_block_is_syntax_error() {
        let src =
            format!("document file {{\n  write <\"out.txt\"> \"value\"\n}}\n\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn case_block_in_document_block_is_syntax_error() {
        let src =
            "document file {\n  case \"nested\" {\n    $ true\n    assert { exit 0 }\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn nested_document_block_is_syntax_error() {
        let src = "document file {\n  document file {\n    title \"nested\"\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn document_block_inside_case_is_syntax_error() {
        let src = "case \"x\" {\n  document file {\n    title \"misplaced\"\n  }\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn unknown_document_scope_is_syntax_error() {
        // v0's only document scopes are `file` and `case`; any other scope
        // keyword is not part of the grammar.
        let src = format!("document step {{\n  title \"no such scope\"\n}}\n\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn document_title_workspace_path_literal_is_kind_mismatch() {
        let src = format!("document file {{\n  title <\"a.txt\">\n}}\n\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::StringLiteral,
                actual: ValueLiteralKind::WorkspacePath,
                ..
            }
        ));
        assert_eq!(err.code().as_str(), "semantic.literal.kind_mismatch");
    }

    #[test]
    fn document_description_fixture_reference_is_kind_mismatch() {
        let src = format!("document file {{\n  description @\"notes.txt\"\n}}\n\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::TextValue,
                actual: ValueLiteralKind::FixtureReference,
                ..
            }
        ));
    }

    #[test]
    fn documentation_only_source_without_final_newline_parses() {
        // The document block's `trail` matches EOI when the closing brace
        // ends the file, surfacing an explicit EOI pair inside the block —
        // the same shape case_block handles.
        let src = "document file {\n  title \"no final newline\"\n}";
        let source_file = parse(src).unwrap();
        assert!(source_file.file_documentation().is_some());
        assert!(source_file.cases().is_empty());
    }

    #[test]
    fn document_file_with_crlf_line_endings_parses() {
        let src = format!(
            "document file {{\r\n  title \"crlf\"\r\n}}\r\n\r\n{}",
            PASSING_CASE.replace('\n', "\r\n")
        );
        let source_file = parse(&src).unwrap();
        assert_eq!(
            source_file.file_documentation().unwrap().title.as_deref(),
            Some("crlf")
        );
    }

    #[test]
    fn into_script_drops_file_documentation() {
        let documented = format!("document file {{\n  title \"Documented\"\n}}\n\n{PASSING_CASE}");
        let documented_script = parse(&documented).unwrap().into_script();
        let undocumented_script = parse(PASSING_CASE).unwrap().into_script();
        assert_eq!(
            documented_script.cases.len(),
            undocumented_script.cases.len()
        );
        assert_eq!(
            documented_script.cases[0].name,
            undocumented_script.cases[0].name
        );
        assert_eq!(
            documented_script.cases[0].steps.len(),
            undocumented_script.cases[0].steps.len()
        );
    }

    #[test]
    fn first_case_span_excludes_document_block_and_gap_lines() {
        // Neither the document block nor the blank / comment lines between it
        // and the first case belong to the case span; the span still equals
        // the pest case_block pair's range (#167's contract, unchanged).
        let src = format!(
            "document file {{\n  title \"Documented\"\n}}\n\n# a comment between\n\n{PASSING_CASE}"
        );
        let source_file = parse(&src).unwrap();
        assert_eq!(source_file.cases().len(), 1);
        let source_case = &source_file.cases()[0];
        assert_eq!(source_file.case_source(source_case), PASSING_CASE);
        assert_eq!(source_case.span().start(), src.find("case \"x\"").unwrap());
    }

    // ── Document block: `document case` ─────────────────────────────────────
    //
    // Scope-specific field whitelist, association with the immediately
    // following case, and the orphan / duplicate placement rules. See #169
    // and the accompanying ADR; representative valid shapes live in
    // examples/, e2e/, and tests/fixtures/syntax/valid/.

    #[test]
    fn document_case_fields_are_parsed_and_associated_with_next_case() {
        let src = format!(
            "document case {{\n  title \"File creation\"\n  description \"Verifies the file is created.\"\n}}\n{PASSING_CASE}"
        );
        let source_file = parse(&src).unwrap();
        let documentation = source_file.cases()[0].documentation().unwrap();
        assert_eq!(documentation.title.as_deref(), Some("File creation"));
        assert_eq!(
            documentation.description.as_ref().unwrap().as_str(),
            "Verifies the file is created."
        );
    }

    #[test]
    fn document_case_holds_only_explicit_fields() {
        let src =
            format!("document case {{\n  description \"No title given.\"\n}}\n{PASSING_CASE}");
        let source_file = parse(&src).unwrap();
        let documentation = source_file.cases()[0].documentation().unwrap();
        // The omitted title stays `None`: the case-name fallback is applied
        // when the Documentation Catalog is built (#170), never here.
        assert_eq!(documentation.title, None);
        assert!(documentation.description.is_some());
    }

    #[test]
    fn case_without_document_case_has_no_documentation() {
        let source_file = parse(PASSING_CASE).unwrap();
        assert!(source_file.cases()[0].documentation().is_none());
    }

    #[test]
    fn document_case_description_heredoc_is_dedented() {
        let src = format!(
            "document case {{\n  description ```\n    line one\n\n    line two\n    ```\n}}\n{PASSING_CASE}"
        );
        let source_file = parse(&src).unwrap();
        let documentation = source_file.cases()[0].documentation().unwrap();
        assert_eq!(
            documentation.description.as_ref().unwrap().as_str(),
            "line one\n\nline two\n"
        );
    }

    #[test]
    fn blank_lines_and_comments_between_document_case_and_case_keep_association() {
        let src = format!(
            "document case {{\n  title \"Still associated\"\n}}\n\n# a comment between\n\n{PASSING_CASE}"
        );
        let source_file = parse(&src).unwrap();
        let documentation = source_file.cases()[0].documentation().unwrap();
        assert_eq!(documentation.title.as_deref(), Some("Still associated"));
    }

    #[test]
    fn document_case_applies_only_to_the_immediately_following_case() {
        let src = "document case {\n  title \"Only the first\"\n}\ncase \"first\" {\n  $ true\n  assert { exit 0 }\n}\ncase \"second\" {\n  $ true\n  assert { exit 0 }\n}\n";
        let source_file = parse(src).unwrap();
        assert!(source_file.cases()[0].documentation().is_some());
        assert!(source_file.cases()[1].documentation().is_none());
    }

    #[test]
    fn document_case_after_an_earlier_case_attaches_to_the_next_case() {
        // The canonical form repeats: a document case may follow an earlier
        // case (documented or not) and attaches to the case after it.
        let src = "case \"first\" {\n  $ true\n  assert { exit 0 }\n}\ndocument case {\n  title \"The second case\"\n}\ncase \"second\" {\n  $ true\n  assert { exit 0 }\n}\n";
        let source_file = parse(src).unwrap();
        assert!(source_file.cases()[0].documentation().is_none());
        assert_eq!(
            source_file.cases()[1]
                .documentation()
                .unwrap()
                .title
                .as_deref(),
            Some("The second case")
        );
    }

    #[test]
    fn each_case_may_carry_its_own_document_case() {
        // Association resets at every case: consecutive (document case, case)
        // pairs each bind their own block, never a predecessor's.
        let src = "document case {\n  title \"first doc\"\n}\ncase \"first\" {\n  $ true\n  assert { exit 0 }\n}\ndocument case {\n  title \"second doc\"\n}\ncase \"second\" {\n  $ true\n  assert { exit 0 }\n}\n";
        let source_file = parse(src).unwrap();
        assert_eq!(
            source_file.cases()[0]
                .documentation()
                .unwrap()
                .title
                .as_deref(),
            Some("first doc")
        );
        assert_eq!(
            source_file.cases()[1]
                .documentation()
                .unwrap()
                .title
                .as_deref(),
            Some("second doc")
        );
    }

    #[test]
    fn document_file_and_document_case_coexist_in_canonical_order() {
        let src = format!(
            "document file {{\n  title \"The file\"\n}}\n\ndocument case {{\n  title \"The case\"\n}}\n{PASSING_CASE}"
        );
        let source_file = parse(&src).unwrap();
        assert_eq!(
            source_file.file_documentation().unwrap().title.as_deref(),
            Some("The file")
        );
        assert_eq!(
            source_file.cases()[0]
                .documentation()
                .unwrap()
                .title
                .as_deref(),
            Some("The case")
        );
    }

    #[test]
    fn empty_document_case_block_is_rejected() {
        let src = format!("document case {{\n}}\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::EmptyDocumentBlock { line: 1 }));
        assert_eq!(err.code().as_str(), "parse.document_block.empty");
    }

    #[test]
    fn comment_only_document_case_block_is_rejected_as_empty() {
        let src = format!("document case {{\n  # not a field\n}}\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::EmptyDocumentBlock { .. }));
    }

    #[test]
    fn duplicate_document_case_field_is_rejected() {
        let src =
            format!("document case {{\n  title \"first\"\n  title \"second\"\n}}\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::DuplicateDocumentationField {
                line: 3,
                field: "title"
            }
        ));
        assert_eq!(err.code().as_str(), "parse.document_block.duplicate_field");
    }

    #[test]
    fn duplicate_document_case_description_across_literal_forms_is_rejected() {
        let src = format!(
            "document case {{\n  description \"first\"\n  description ```\n    second\n    ```\n}}\n{PASSING_CASE}"
        );
        let err = parse(&src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::DuplicateDocumentationField {
                field: "description",
                ..
            }
        ));
    }

    #[test]
    fn group_field_in_document_case_is_syntax_error() {
        // `group` belongs to the file scope's whitelist only; the case
        // scope's grammar never reaches it, same as an unknown field.
        let src = format!("document case {{\n  group \"Filesystem\"\n}}\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn order_field_in_document_case_is_syntax_error() {
        let src = format!("document case {{\n  order 10\n}}\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn unknown_field_in_document_case_is_syntax_error() {
        let src = format!("document case {{\n  author \"someone\"\n}}\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn action_step_in_document_case_is_syntax_error() {
        let src = format!("document case {{\n  $ echo hello\n}}\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn assertion_block_in_document_case_is_syntax_error() {
        let src = format!("document case {{\n  assert {{\n    exit 0\n  }}\n}}\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn write_step_in_document_case_is_syntax_error() {
        let src = format!("document case {{\n  write <\"out.txt\"> \"value\"\n}}\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn nested_document_block_in_document_case_is_syntax_error() {
        let src = "document case {\n  document case {\n    title \"nested\"\n  }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn document_case_inside_case_is_syntax_error() {
        let src = "case \"x\" {\n  document case {\n    title \"misplaced\"\n  }\n  $ true\n  assert { exit 0 }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn document_case_title_workspace_path_literal_is_kind_mismatch() {
        let src = format!("document case {{\n  title <\"a.txt\">\n}}\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::StringLiteral,
                actual: ValueLiteralKind::WorkspacePath,
                ..
            }
        ));
        assert_eq!(err.code().as_str(), "semantic.literal.kind_mismatch");
    }

    #[test]
    fn document_case_description_fixture_reference_is_kind_mismatch() {
        let src = format!("document case {{\n  description @\"notes.txt\"\n}}\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::LiteralKindMismatch {
                expected: RequiredLiteralKind::TextValue,
                actual: ValueLiteralKind::FixtureReference,
                ..
            }
        ));
    }

    #[test]
    fn orphan_document_case_at_end_of_source_is_rejected() {
        let src = format!("{PASSING_CASE}\ndocument case {{\n  title \"no case follows\"\n}}\n");
        let err = parse(&src).unwrap_err();
        // The location is the unassociated block's own start line.
        assert!(matches!(err, ParseError::OrphanDocumentCase { line: 6 }));
        assert_eq!(err.code().as_str(), "parse.document_case.orphan");
    }

    #[test]
    fn orphan_document_case_followed_only_by_comments_is_rejected() {
        let src = "document case {\n  title \"orphan\"\n}\n\n# only comments follow\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::OrphanDocumentCase { line: 1 }));
    }

    #[test]
    fn second_document_case_before_target_case_is_rejected_as_duplicate() {
        let src = format!(
            "document case {{\n  title \"first\"\n}}\n\ndocument case {{\n  title \"second\"\n}}\n\n{PASSING_CASE}"
        );
        let err = parse(&src).unwrap_err();
        // The location is the second block's start line, not the first's.
        assert!(matches!(err, ParseError::DuplicateDocumentCase { line: 5 }));
        assert_eq!(err.code().as_str(), "parse.document_case.duplicate");
    }

    #[test]
    fn duplicate_document_case_wins_over_orphan() {
        // Both blocks lack a target case; the second block is still reported
        // as a duplicate (of the pending first block), not as an orphan.
        let src =
            "document case {\n  title \"first\"\n}\n\ndocument case {\n  title \"second\"\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::DuplicateDocumentCase { line: 5 }));
    }

    #[test]
    fn document_file_after_pending_document_case_is_rejected() {
        // A `document file` between a pending `document case` and its target
        // case violates the canonical top-level form
        // `document file? before_each? (document case? case)*`, and is
        // classified as the existing `document file` placement violation.
        let src = format!(
            "document case {{\n  title \"pending\"\n}}\n\ndocument file {{\n  title \"too late\"\n}}\n\n{PASSING_CASE}"
        );
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::DocumentFileAfterCase { line: 5 }));
        assert_eq!(err.code().as_str(), "parse.document_file.after_case");
    }

    #[test]
    fn case_span_excludes_document_case_and_gap_lines() {
        // The associated document block and the blank / comment lines between
        // it and the case are not part of the case span; the span still
        // equals the pest case_block pair's range (#167's contract, unchanged).
        let src = format!(
            "document case {{\n  title \"Documented\"\n}}\n\n# a comment between\n\n{PASSING_CASE}"
        );
        let source_file = parse(&src).unwrap();
        let source_case = &source_file.cases()[0];
        assert_eq!(source_file.case_source(source_case), PASSING_CASE);
        assert_eq!(source_case.span().start(), src.find("case \"x\"").unwrap());
        assert!(source_case.documentation().is_some());
    }

    #[test]
    fn document_case_with_crlf_line_endings_parses() {
        let src = format!(
            "document case {{\r\n  title \"crlf\"\r\n}}\r\n{}",
            PASSING_CASE.replace('\n', "\r\n")
        );
        let source_file = parse(&src).unwrap();
        assert_eq!(
            source_file.cases()[0]
                .documentation()
                .unwrap()
                .title
                .as_deref(),
            Some("crlf")
        );
    }

    #[test]
    fn into_script_drops_case_documentation() {
        let documented = format!("document case {{\n  title \"Documented\"\n}}\n{PASSING_CASE}");
        let documented_script = parse(&documented).unwrap().into_script();
        let undocumented_script = parse(PASSING_CASE).unwrap().into_script();
        assert_eq!(
            documented_script.cases.len(),
            undocumented_script.cases.len()
        );
        assert_eq!(
            documented_script.cases[0].name,
            undocumented_script.cases[0].name
        );
        assert_eq!(
            documented_script.cases[0].steps.len(),
            undocumented_script.cases[0].steps.len()
        );
    }

    // ─── before_each block (#70) ────────────────────────────────────────────

    const BEFORE_EACH: &str = "before_each {\n  write <\"seed.txt\"> \"seed\\n\"\n}\n";

    #[test]
    fn parse_before_each_with_write_steps() {
        let src = format!(
            "before_each {{\n  write <\"a.txt\"> \"a\\n\"\n  write <\"b/c.txt\"> ```\n    content\n    ```\n}}\n\n{PASSING_CASE}"
        );
        let script = parse_script(&src).unwrap();
        let before_each = script.before_each.expect("before_each must be parsed");
        assert_eq!(before_each.steps().len(), 2);
        let SideEffectingStep::WriteFile(first) = &before_each.steps()[0];
        assert_eq!(first.path.as_str(), "a.txt");
        assert_eq!(first.content, TextLiteral::Quoted("a\n".to_string()));
        let SideEffectingStep::WriteFile(second) = &before_each.steps()[1];
        assert_eq!(second.path.as_str(), "b/c.txt");
        assert_eq!(
            second.content,
            TextLiteral::Heredoc("content\n".to_string())
        );
        assert_eq!(script.cases.len(), 1);
    }

    #[test]
    fn script_without_before_each_has_none() {
        let script = parse_script(PASSING_CASE).unwrap();
        assert!(script.before_each.is_none());
    }

    #[test]
    fn before_each_may_follow_document_file() {
        let src = format!("document file {{\n  title \"t\"\n}}\n\n{BEFORE_EACH}\n{PASSING_CASE}");
        let script = parse_script(&src).unwrap();
        assert!(script.before_each.is_some());
    }

    #[test]
    fn document_file_after_before_each_is_rejected() {
        // The canonical top-level form is strict: `document file` leads the
        // file, before `before_each`.
        let src = format!("{BEFORE_EACH}\ndocument file {{\n  title \"t\"\n}}\n\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::DocumentFileAfterCase { .. }));
        assert_eq!(err.code().as_str(), "parse.document_file.after_case");
    }

    #[test]
    fn duplicate_before_each_is_rejected() {
        let src = format!("{BEFORE_EACH}\n{BEFORE_EACH}\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::DuplicateBeforeEach { line: 5 }));
        assert_eq!(err.code().as_str(), "parse.before_each.duplicate");
    }

    #[test]
    fn before_each_after_case_is_rejected() {
        let src = format!("{PASSING_CASE}\n{BEFORE_EACH}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::BeforeEachAfterCase { .. }));
        assert_eq!(err.code().as_str(), "parse.before_each.after_case");
    }

    #[test]
    fn before_each_after_pending_document_case_is_rejected() {
        // `before_each` must not separate a `document case` block from its
        // target case, the same adjacency rule `document file` follows.
        let src = format!("document case {{\n  title \"t\"\n}}\n{BEFORE_EACH}{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::BeforeEachAfterCase { .. }));
        assert_eq!(err.code().as_str(), "parse.before_each.after_case");
    }

    #[test]
    fn action_step_in_before_each_is_rejected() {
        let src = format!("before_each {{\n  $ mkdir -p fixtures\n}}\n\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::BeforeEachActionStep { line: 2 }));
        assert_eq!(err.code().as_str(), "parse.before_each.action_step");
    }

    #[test]
    fn assertion_block_in_before_each_is_rejected() {
        let src = format!(
            "before_each {{\n  write <\"seed.txt\"> \"seed\\n\"\n  assert {{ file <\"seed.txt\"> exists }}\n}}\n\n{PASSING_CASE}"
        );
        let err = parse(&src).unwrap_err();
        assert!(matches!(
            err,
            ParseError::BeforeEachAssertionBlock { line: 3 }
        ));
        assert_eq!(err.code().as_str(), "parse.before_each.assertion_block");
    }

    #[test]
    fn empty_before_each_is_rejected() {
        let src = format!("before_each {{\n}}\n\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::EmptyBeforeEach { line: 1 }));
        assert_eq!(err.code().as_str(), "parse.before_each.empty");
    }

    #[test]
    fn comment_only_before_each_is_rejected() {
        // Comment lines are not steps, so a comment-only body is rejected the
        // same way an empty body is.
        let src = format!("before_each {{\n  # only a comment\n}}\n\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::EmptyBeforeEach { .. }));
    }

    #[test]
    fn before_each_inside_case_is_syntax_error() {
        let src = "case \"x\" {\n  before_each {\n    write <\"seed.txt\"> \"seed\\n\"\n  }\n  assert { exit 0 }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::Syntax { .. }));
    }

    #[test]
    fn before_each_write_step_absolute_path_is_rejected() {
        // A `before_each` write step's path goes through the same
        // `WorkspacePath::parse` validation as a case body write step.
        let src = format!("before_each {{\n  write <\"/abs.txt\"> \"x\"\n}}\n\n{PASSING_CASE}");
        let err = parse(&src).unwrap_err();
        assert!(matches!(err, ParseError::InvalidWorkspacePath { .. }));
        assert_eq!(err.code().as_str(), "semantic.workspace_path.absolute");
    }
}
