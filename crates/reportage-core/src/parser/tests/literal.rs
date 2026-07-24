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

