// ─── Write step: string literal / heredoc literal (#67, #86) ──────────

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
