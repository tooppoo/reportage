use super::*;
use crate::model::{RequiredLiteralKind, ValueLiteralKind};

// ── Document block: `document file` ─────────────────────────────────────
//
// Field validation, placement rules, and the whitelist body contract.
// See #168 and the accompanying ADR; representative valid shapes live in
// examples/, e2e/, and tests/fixtures/syntax/valid/.

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
