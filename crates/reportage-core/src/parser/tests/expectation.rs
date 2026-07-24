use super::*;
use crate::model::{DirMatcher, Expectation, FileMatcher, LogicalOperator, Step};

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
