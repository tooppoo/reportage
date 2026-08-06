use super::*;
use crate::diagnostic::DiagnosticCode;
use crate::model::{
    RuntimeEvidenceSource, SideEffectingStep, Step, TextLiteral, TextValueExpression,
    WorkspacePathError,
};

// ─── Write step: string literal / heredoc literal (#67, #86) ──────────

#[test]
fn parse_basic_write_step() {
    let src = "case \"x\" {\n  write <\"a.txt\"> ```\n    hello\n    ```\n  $ true\n  assert {\n    exit 0\n  }\n}\n";
    let script = parse_script(src).unwrap();
    let step = write_file_step(&script);
    assert_eq!(step.path.as_str(), "a.txt");
    assert_eq!(
        step.content.binding_free_text_value().unwrap().as_str(),
        "hello\n"
    );
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
    assert_eq!(
        step.content.binding_free_text_value().unwrap().as_str(),
        "hello\n"
    );
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
    let Step::SideEffect(SideEffectingStep::WriteFile(first)) = &script.cases[0].steps[0] else {
        panic!("expected write step");
    };
    let Step::SideEffect(SideEffectingStep::WriteFile(second)) = &script.cases[0].steps[1] else {
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
    let Step::SideEffect(SideEffectingStep::WriteFile(first)) = &before_each.steps()[0] else {
        panic!("before_each write step must parse as a write step");
    };
    assert_eq!(first.path.as_str(), "a.txt");
    assert_eq!(
        first.content,
        TextValueExpression::Raw(TextLiteral::Quoted("a\n".to_string()))
    );
    let Step::SideEffect(SideEffectingStep::WriteFile(second)) = &before_each.steps()[1] else {
        panic!("before_each write step must parse as a write step");
    };
    assert_eq!(second.path.as_str(), "b/c.txt");
    assert_eq!(
        second.content,
        TextValueExpression::Raw(TextLiteral::Heredoc("content\n".to_string()))
    );
    assert_eq!(script.cases.len(), 1);
}

#[test]
fn binding_capture_parses_with_source_and_span() {
    let src = "case \"x\" {\n  $ printf value\n  let output <- stdout_line\n  assert {\n    stdout text_equals &output\n  }\n}\n";
    let file = parse(src).unwrap();
    let Step::Binding(binding) = &file.cases()[0].case().steps[1] else {
        panic!("expected binding step");
    };
    assert_eq!(binding.name, "output");
    assert_eq!(binding.source, RuntimeEvidenceSource::StdoutLine);
    assert_eq!(
        &src[binding.declaration_span.start..binding.declaration_span.end],
        "let output <- stdout_line"
    );
}

#[test]
fn duplicate_binding_is_rejected_before_execution() {
    let src = "case \"x\" {\n  $ true\n  let value <- stdout\n  let value <- stderr\n  assert { exit 0 }\n}\n";
    let error = parse(src).unwrap_err();
    assert_eq!(error.code(), DiagnosticCode::SemanticBindingDuplicate);
    let location = error.to_diagnostic().location.unwrap();
    assert_eq!(location.line, 4);
    assert_eq!(location.column, Some(3));
}

#[test]
fn undefined_and_use_before_declaration_have_distinct_diagnostics() {
    let undefined = "case \"x\" {\n  $ true\n  assert { stdout text_equals &missing }\n}\n";
    let error = parse(undefined).unwrap_err();
    assert_eq!(error.code(), DiagnosticCode::SemanticBindingUndefined);
    let location = error.to_diagnostic().location.unwrap();
    assert_eq!(location.line, 3);
    assert!(location.column.is_some());

    let early = "case \"x\" {\n  $ true\n  assert { stdout text_equals &later }\n  let later <- stdout\n}\n";
    let error = parse(early).unwrap_err();
    assert_eq!(
        error.code(),
        DiagnosticCode::SemanticBindingUseBeforeDeclaration
    );
    let location = error.to_diagnostic().location.unwrap();
    assert_eq!(location.line, 3);
    assert!(location.column.is_some());
}

#[test]
fn binding_capture_requires_a_preceding_action() {
    let src = "case \"x\" {\n  let value <- stdout\n  assert { file <\"x\"> exists }\n}\n";
    assert_eq!(
        parse(src).unwrap_err().code(),
        DiagnosticCode::SemanticBindingRequiresAction
    );
}

#[test]
fn invalid_binding_identifier_and_before_each_binding_are_rejected() {
    let invalid = "case \"x\" {\n  $ true\n  let 1value <- stdout\n  assert { exit 0 }\n}\n";
    let error = parse(invalid).unwrap_err();
    assert_eq!(
        error.code(),
        DiagnosticCode::SemanticBindingInvalidIdentifier
    );
    let location = error.to_diagnostic().location.unwrap();
    assert_eq!(location.line, 3);
    assert!(location.column.is_some());

    for reference in [
        "case \"x\" {\n  $ true\n  assert { stdout contains &1value }\n}\n",
        "case \"x\" {\n  $ true\n  assert { exit &1value }\n}\n",
        "case \"x\" {\n  $ true\n  assert { file &1value exists }\n}\n",
    ] {
        let error = parse(reference).unwrap_err();
        assert_eq!(
            error.code(),
            DiagnosticCode::SemanticBindingInvalidIdentifier
        );
        let location = error.to_diagnostic().location.unwrap();
        assert_eq!(location.line, 3);
        assert!(location.column.is_some());
    }
}

#[test]
fn a_before_each_binding_is_in_scope_for_the_whole_case_body() {
    let src = "before_each {\n  $ pwd\n  let workspace <- stdout_line\n}\ncase \"x\" {\n  write <\"a.txt\"> &workspace\n  assert { file <\"a.txt\"> exists }\n}\n";
    let script = parse_script(src).unwrap();
    let before_each = script.before_each.expect("before_each must be parsed");
    assert!(matches!(before_each.steps()[1], Step::Binding(_)));
    assert_eq!(script.cases.len(), 1);
}

#[test]
fn binding_scope_does_not_flow_backwards_from_a_case_body_into_before_each() {
    // `before_each` runs first, so a case body declaration is undefined there —
    // not used-before-declaration, which would imply it becomes available later.
    let src = "before_each {\n  write <\"a.txt\"> &workspace\n}\ncase \"x\" {\n  $ pwd\n  let workspace <- stdout_line\n  assert { exit 0 }\n}\n";
    assert_eq!(
        parse(src).unwrap_err().code(),
        DiagnosticCode::SemanticBindingUndefined
    );
}

#[test]
fn a_case_body_binding_requires_a_case_body_action() {
    // The body-entry checkpoint drops the last setup action's process evidence,
    // so `action seen` restarts with the case body.
    let src = "before_each {\n  $ pwd\n  assert { exit 0 }\n}\ncase \"x\" {\n  let workspace <- stdout_line\n  assert { file <\"a.txt\"> exists }\n}\n";
    assert_eq!(
        parse(src).unwrap_err().code(),
        DiagnosticCode::SemanticBindingRequiresAction
    );
}

#[test]
fn a_case_body_must_not_redeclare_a_before_each_binding() {
    let src = "before_each {\n  $ pwd\n  let workspace <- stdout_line\n}\ncase \"x\" {\n  $ pwd\n  let workspace <- stdout_line\n  assert { exit 0 }\n}\n";
    assert_eq!(
        parse(src).unwrap_err().code(),
        DiagnosticCode::SemanticBindingDuplicate
    );
}

#[test]
fn binding_reference_in_non_text_positions_is_a_type_mismatch() {
    let path =
        "case \"x\" {\n  $ true\n  let value <- stdout\n  assert { file &value exists }\n}\n";
    let error = parse(path).unwrap_err();
    assert_eq!(error.code(), DiagnosticCode::SemanticBindingTypeMismatch);
    let location = error.to_diagnostic().location.unwrap();
    assert_eq!(location.line, 4);
    assert!(location.column.is_some());

    let exit = "case \"x\" {\n  $ true\n  let value <- stdout\n  assert { exit &value }\n}\n";
    let error = parse(exit).unwrap_err();
    assert_eq!(error.code(), DiagnosticCode::SemanticBindingTypeMismatch);
    let location = error.to_diagnostic().location.unwrap();
    assert_eq!(location.line, 4);
    assert!(location.column.is_some());
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
fn before_each_keeps_action_assert_and_write_steps_in_source_order() {
    // The same three step kinds a case body accepts, parsed into the same
    // `Step` values and left in the order they were written.
    let src = format!(
        "before_each {{\n  $ mkdir -p fixtures\n  assert {{ dir <\"fixtures\"> exists }}\n  write <\"seed.txt\"> \"seed\\n\"\n}}\n\n{PASSING_CASE}"
    );
    let script = parse_script(&src).unwrap();
    let before_each = script.before_each.expect("before_each must be parsed");
    let steps = before_each.steps();
    assert_eq!(steps.len(), 3);
    let Step::Action(action) = &steps[0] else {
        panic!("first before_each step must be an action");
    };
    assert_eq!(action.command, "mkdir -p fixtures");
    assert!(matches!(steps[1], Step::AssertionBlock(_)));
    let Step::SideEffect(SideEffectingStep::WriteFile(write)) = &steps[2] else {
        panic!("third before_each step must be a write step");
    };
    assert_eq!(write.path.as_str(), "seed.txt");
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
