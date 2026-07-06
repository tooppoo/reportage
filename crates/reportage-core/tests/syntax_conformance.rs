//! Syntax conformance fixtures for the production `parse()` entrypoint.
//!
//! These tests intentionally avoid raw pest parser access.
//! They lock down which checked-in scripts are accepted or rejected after grammar parsing plus parser construction validation.

use std::fs;
use std::path::{Path, PathBuf};

use reportage_core::model::{
    Case, CountOp, DirMatcher, Expectation, FileMatcher, LogicalOperator, OutputMatcher,
    OutputSource, Script, SideEffectingStep, Step, TextLiteral,
};
use reportage_core::parser::{ParseError, parse};
use serde::Serialize;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture_paths(kind: &str) -> Vec<PathBuf> {
    let pattern = repo_root()
        .join(format!("tests/fixtures/syntax/{kind}/*.repor"))
        .to_str()
        .expect("fixture glob path must be valid UTF-8")
        .to_string();

    let mut paths = glob::glob(&pattern)
        .expect("syntax fixture glob pattern must be valid")
        .map(|entry| entry.expect("syntax fixture path must be readable"))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn snapshot_paths() -> Vec<PathBuf> {
    let pattern = repo_root()
        .join("tests/fixtures/syntax/valid/*.ast.json")
        .to_str()
        .expect("snapshot glob path must be valid UTF-8")
        .to_string();

    let mut paths = glob::glob(&pattern)
        .expect("AST snapshot glob pattern must be valid")
        .map(|entry| entry.expect("AST snapshot path must be readable"))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn read_fixture(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read syntax fixture {}: {e}", path.display()))
}

fn fixture_stem(path: &Path) -> &str {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .expect("syntax fixture file name must be valid UTF-8")
}

fn snapshot_stem(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".ast.json"))
        .expect("AST snapshot file name must end with .ast.json")
}

fn snapshot_path_for_fixture(path: &Path) -> PathBuf {
    path.with_extension("ast.json")
}

fn format_snapshot(script: &Script) -> String {
    let snapshot = SnapshotScript::from(script);
    let mut json =
        serde_json::to_string_pretty(&snapshot).expect("AST snapshot serialization must succeed");
    json.push('\n');
    json
}

fn update_snapshots_enabled() -> bool {
    std::env::var_os("UPDATE_AST_SNAPSHOTS").is_some()
}

#[derive(Serialize)]
struct SnapshotScript<'a> {
    cases: Vec<SnapshotCase<'a>>,
}

impl<'a> From<&'a Script> for SnapshotScript<'a> {
    fn from(script: &'a Script) -> Self {
        Self {
            cases: script.cases.iter().map(SnapshotCase::from).collect(),
        }
    }
}

#[derive(Serialize)]
struct SnapshotCase<'a> {
    name: &'a str,
    steps: Vec<SnapshotStep<'a>>,
}

impl<'a> From<&'a Case> for SnapshotCase<'a> {
    fn from(case: &'a Case) -> Self {
        Self {
            name: &case.name,
            steps: case.steps.iter().map(SnapshotStep::from).collect(),
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SnapshotStep<'a> {
    Action {
        command: &'a str,
    },
    AssertionBlock {
        expectations: Vec<SnapshotExpectation<'a>>,
    },
    WriteFile {
        path: &'a str,
        content: SnapshotTextLiteral<'a>,
    },
}

impl<'a> From<&'a Step> for SnapshotStep<'a> {
    fn from(step: &'a Step) -> Self {
        match step {
            Step::Action(action) => Self::Action {
                command: &action.command,
            },
            Step::AssertionBlock(block) => Self::AssertionBlock {
                expectations: block
                    .expectations()
                    .iter()
                    .map(SnapshotExpectation::from)
                    .collect(),
            },
            Step::SideEffect(SideEffectingStep::WriteFile(write_step)) => Self::WriteFile {
                path: write_step.path.as_str(),
                content: SnapshotTextLiteral::from(&write_step.content),
            },
        }
    }
}

/// Mirrors `model::TextLiteral`, keeping the literal kind (`quoted` vs.
/// `heredoc`) visible in the AST snapshot rather than flattening straight to
/// the resolved value.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SnapshotTextLiteral<'a> {
    Quoted { value: &'a str },
    Heredoc { value: &'a str },
}

impl<'a> From<&'a TextLiteral> for SnapshotTextLiteral<'a> {
    fn from(literal: &'a TextLiteral) -> Self {
        match literal {
            TextLiteral::Quoted(value) => Self::Quoted { value },
            TextLiteral::Heredoc(value) => Self::Heredoc { value },
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SnapshotExpectation<'a> {
    Exit {
        expected: u8,
    },
    Stdout {
        matcher: SnapshotOutputMatcher<'a>,
    },
    Stderr {
        matcher: SnapshotOutputMatcher<'a>,
    },
    File {
        path: &'a str,
        matcher: SnapshotFileMatcher<'a>,
    },
    Dir {
        path: &'a str,
        matcher: SnapshotDirMatcher<'a>,
    },
    FileCount {
        glob: &'a str,
        op: SnapshotCountOp,
        count: usize,
    },
    Jq {
        source: SnapshotOutputSource,
        expression: &'a str,
    },
    Logical {
        operator: SnapshotLogicalOperator,
        children: Vec<SnapshotExpectation<'a>>,
    },
}

impl<'a> From<&'a Expectation> for SnapshotExpectation<'a> {
    fn from(expectation: &'a Expectation) -> Self {
        match expectation {
            Expectation::Exit(exit) => Self::Exit {
                expected: exit.expected,
            },
            Expectation::Stdout(output) => Self::Stdout {
                matcher: SnapshotOutputMatcher::from(&output.matcher),
            },
            Expectation::Stderr(output) => Self::Stderr {
                matcher: SnapshotOutputMatcher::from(&output.matcher),
            },
            Expectation::File(file) => Self::File {
                path: &file.path,
                matcher: SnapshotFileMatcher::from(&file.matcher),
            },
            Expectation::Dir(dir) => Self::Dir {
                path: &dir.path,
                matcher: SnapshotDirMatcher::from(&dir.matcher),
            },
            Expectation::FileCount(file_count) => Self::FileCount {
                glob: &file_count.glob,
                op: SnapshotCountOp::from(&file_count.op),
                count: file_count.count,
            },
            Expectation::Jq(jq) => Self::Jq {
                source: SnapshotOutputSource::from(&jq.source),
                expression: &jq.expression,
            },
            Expectation::Logical(logical) => Self::Logical {
                operator: SnapshotLogicalOperator::from(logical.operator()),
                children: logical.children().iter().map(Self::from).collect(),
            },
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum SnapshotLogicalOperator {
    Not,
    All,
    Any,
}

impl From<LogicalOperator> for SnapshotLogicalOperator {
    fn from(operator: LogicalOperator) -> Self {
        match operator {
            LogicalOperator::Not => Self::Not,
            LogicalOperator::All => Self::All,
            LogicalOperator::Any => Self::Any,
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SnapshotOutputMatcher<'a> {
    Empty,
    Contains { value: &'a str },
    NotContains { value: &'a str },
    Matches { value: &'a str },
}

impl<'a> From<&'a OutputMatcher> for SnapshotOutputMatcher<'a> {
    fn from(matcher: &'a OutputMatcher) -> Self {
        match matcher {
            OutputMatcher::Empty => Self::Empty,
            OutputMatcher::Contains(value) => Self::Contains { value },
            OutputMatcher::NotContains(value) => Self::NotContains { value },
            OutputMatcher::Matches(value) => Self::Matches { value },
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SnapshotFileMatcher<'a> {
    Exists,
    NotExists,
    Contains { value: SnapshotTextLiteral<'a> },
    Matches { value: &'a str },
}

impl<'a> From<&'a FileMatcher> for SnapshotFileMatcher<'a> {
    fn from(matcher: &'a FileMatcher) -> Self {
        match matcher {
            FileMatcher::Exists => Self::Exists,
            FileMatcher::NotExists => Self::NotExists,
            FileMatcher::Contains(value) => Self::Contains {
                value: SnapshotTextLiteral::from(value),
            },
            FileMatcher::Matches(value) => Self::Matches { value },
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SnapshotDirMatcher<'a> {
    Exists,
    NotExists,
    Contains { value: &'a str },
}

impl<'a> From<&'a DirMatcher> for SnapshotDirMatcher<'a> {
    fn from(matcher: &'a DirMatcher) -> Self {
        match matcher {
            DirMatcher::Exists => Self::Exists,
            DirMatcher::NotExists => Self::NotExists,
            DirMatcher::Contains(value) => Self::Contains { value },
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum SnapshotCountOp {
    Eq,
    Gte,
}

impl From<&CountOp> for SnapshotCountOp {
    fn from(op: &CountOp) -> Self {
        match op {
            CountOp::Eq => Self::Eq,
            CountOp::Gte => Self::Gte,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum SnapshotOutputSource {
    Stdout,
    Stderr,
}

impl From<&OutputSource> for SnapshotOutputSource {
    fn from(source: &OutputSource) -> Self {
        match source {
            OutputSource::Stdout => Self::Stdout,
            OutputSource::Stderr => Self::Stderr,
        }
    }
}

#[test]
fn valid_syntax_fixtures_parse_successfully() {
    let paths = fixture_paths("valid");
    assert!(
        !paths.is_empty(),
        "expected at least one valid syntax fixture"
    );

    for path in paths {
        let source = read_fixture(&path);
        parse(&source).unwrap_or_else(|e| {
            panic!(
                "valid syntax fixture {} must parse successfully: {e}",
                path.display()
            )
        });
    }
}

#[test]
fn ast_snapshots_for_valid_syntax_fixtures_are_current() {
    let paths = fixture_paths("valid");
    assert!(
        !paths.is_empty(),
        "expected at least one valid syntax fixture"
    );

    let update_snapshots = update_snapshots_enabled();
    for path in paths {
        let source = read_fixture(&path);
        let script = parse(&source).unwrap_or_else(|e| {
            panic!(
                "valid syntax fixture {} must parse successfully before snapshotting: {e}",
                path.display()
            )
        });

        let snapshot_path = snapshot_path_for_fixture(&path);
        let actual = format_snapshot(&script);

        if update_snapshots {
            fs::write(&snapshot_path, actual).unwrap_or_else(|e| {
                panic!(
                    "failed to update AST snapshot {}: {e}",
                    snapshot_path.display()
                )
            });
            continue;
        }

        let expected = fs::read_to_string(&snapshot_path).unwrap_or_else(|e| {
            panic!(
                "failed to read AST snapshot {}: {e}\n\
                 run `UPDATE_AST_SNAPSHOTS=1 cargo test -p reportage-core --test syntax_conformance ast_snapshots_for_valid_syntax_fixtures_are_current` to create or refresh snapshots",
                snapshot_path.display()
            )
        });

        assert_eq!(
            expected,
            actual,
            "AST snapshot for {} is stale; run \
             `UPDATE_AST_SNAPSHOTS=1 cargo test -p reportage-core --test syntax_conformance ast_snapshots_for_valid_syntax_fixtures_are_current` \
             and review the JSON diff",
            path.display()
        );
    }
}

#[test]
fn valid_syntax_fixture_snapshots_have_matching_fixtures() {
    let valid_stems = fixture_paths("valid")
        .into_iter()
        .map(|path| fixture_stem(&path).to_string())
        .collect::<std::collections::BTreeSet<_>>();
    let snapshot_stems = snapshot_paths()
        .into_iter()
        .map(|path| snapshot_stem(&path).to_string())
        .collect::<std::collections::BTreeSet<_>>();

    assert_eq!(
        valid_stems, snapshot_stems,
        "AST snapshot files must exactly match valid syntax fixtures; run \
         `UPDATE_AST_SNAPSHOTS=1 cargo test -p reportage-core --test syntax_conformance ast_snapshots_for_valid_syntax_fixtures_are_current` \
         after adding, removing, or renaming valid fixtures"
    );
}

#[test]
fn invalid_syntax_fixtures_are_rejected() {
    let paths = fixture_paths("invalid");
    assert!(
        !paths.is_empty(),
        "expected at least one invalid syntax fixture"
    );

    for path in paths {
        let source = read_fixture(&path);
        let err = match parse(&source) {
            Ok(_) => panic!("invalid syntax fixture {} must be rejected", path.display()),
            Err(err) => err,
        };

        match fixture_stem(&path) {
            "empty_action" | "whitespace_only_action" => {
                assert!(matches!(err, ParseError::EmptyAction { .. }));
                assert_eq!(err.code().as_str(), "parse.empty_action");
            }
            "empty_case_block" => {
                assert!(matches!(err, ParseError::EmptyCase { .. }));
                assert_eq!(err.code().as_str(), "parse.empty_case");
            }
            "case_without_assertion_block" => {
                assert!(matches!(err, ParseError::MissingAssertionBlock { .. }));
                assert_eq!(err.code().as_str(), "parse.missing_assertion_block");
            }
            "exit_code_out_of_range" => {
                assert!(matches!(err, ParseError::InvalidExitCode { .. }));
                assert_eq!(err.code().as_str(), "parse.invalid_exit_code");
            }
            "empty_not_block" => {
                assert!(matches!(
                    err,
                    ParseError::EmptyLogicalCompositionBlock { .. }
                ));
                assert_eq!(err.code().as_str(), "semantic.expectation.empty_block");
            }
            "write_step_absolute_path" => {
                assert!(matches!(err, ParseError::InvalidWorkspacePath { .. }));
                assert_eq!(err.code().as_str(), "semantic.workspace_path.absolute");
            }
            "write_step_dot_segment_path" => {
                assert!(matches!(err, ParseError::InvalidWorkspacePath { .. }));
                assert_eq!(err.code().as_str(), "semantic.workspace_path.dot_segment");
            }
            "write_step_empty_path" => {
                assert!(matches!(err, ParseError::InvalidWorkspacePath { .. }));
                assert_eq!(err.code().as_str(), "semantic.workspace_path.empty");
            }
            "write_step_shallow_indent" => {
                assert!(matches!(err, ParseError::ShallowHeredocIndent { .. }));
                assert_eq!(err.code().as_str(), "parse.raw_block.shallow_indent");
            }
            // Literal kind mismatches parse at the grammar level and are
            // rejected as semantic invalid cases with an actionable
            // diagnostic. See docs/semantic-diagnostics.md and
            // docs/adr/20260706T160000Z_workspace-path-literal-syntax.md.
            "file_subject_string_literal"
            | "file_subject_fixture_reference"
            | "dir_subject_string_literal"
            | "write_step_path_string_literal"
            | "write_step_content_workspace_path_literal"
            | "file_contains_expected_workspace_path_literal"
            | "stdout_contains_workspace_path_literal"
            | "dir_contains_entry_workspace_path_literal" => {
                assert!(matches!(err, ParseError::LiteralKindMismatch { .. }));
                assert_eq!(err.code().as_str(), "semantic.literal.kind_mismatch");
            }
            // Remaining fixtures are rejected as plain pest syntax errors; they share the coarse-grained "parse.syntax" code and are not asserted individually here.
            // See docs/diagnostics.md.
            _ => {
                assert_eq!(err.code().as_str(), "parse.syntax");
            }
        }
    }
}
