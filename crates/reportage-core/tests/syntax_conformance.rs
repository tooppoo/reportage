//! Syntax conformance fixtures for the production `parse()` entrypoint.
//!
//! These tests intentionally avoid raw pest parser access.
//! They lock down which checked-in scripts are accepted or rejected after grammar parsing plus parser construction validation.

use std::fs;
use std::path::{Path, PathBuf};

use reportage_core::model::{
    CountOp, DirMatcher, Expectation, FileContentsReference, FileMatcher, LogicalOperator,
    OutputMatcher, OutputSource, SideEffectingStep, Step, TextLiteral,
};
use reportage_core::parser::{ParseError, parse};
use reportage_core::source::{SourceCase, SourceFile};
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

fn format_snapshot(source_file: &SourceFile) -> String {
    let snapshot = SnapshotScript::from(source_file);
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
    /// Omitted (not `null`) when the source declares no `document file`
    /// block: absence in the snapshot mirrors absence in the source-level
    /// model, and keeps the snapshots of the pre-existing, undocumented
    /// fixtures unchanged.
    #[serde(skip_serializing_if = "Option::is_none")]
    file_documentation: Option<SnapshotFileDocumentation<'a>>,
    /// Omitted when the source declares no `before_each` block, mirroring
    /// `file_documentation` above.
    #[serde(skip_serializing_if = "Option::is_none")]
    before_each: Option<SnapshotBeforeEach<'a>>,
    cases: Vec<SnapshotCase<'a>>,
}

impl<'a> From<&'a SourceFile> for SnapshotScript<'a> {
    fn from(source_file: &'a SourceFile) -> Self {
        Self {
            file_documentation: source_file
                .file_documentation()
                .map(SnapshotFileDocumentation::from),
            before_each: source_file.before_each().map(SnapshotBeforeEach::from),
            cases: source_file.cases().iter().map(SnapshotCase::from).collect(),
        }
    }
}

/// Mirrors `model::BeforeEach`. Steps reuse `SnapshotStep`, so the snapshot
/// shows the same `write_file` step shape as a case body — but only that
/// shape can ever appear here, because the model holds `SideEffectingStep`s
/// only.
#[derive(Serialize)]
struct SnapshotBeforeEach<'a> {
    steps: Vec<SnapshotStep<'a>>,
}

impl<'a> From<&'a reportage_core::model::BeforeEach> for SnapshotBeforeEach<'a> {
    fn from(before_each: &'a reportage_core::model::BeforeEach) -> Self {
        Self {
            steps: before_each.steps().iter().map(SnapshotStep::from).collect(),
        }
    }
}

/// Mirrors `source::FileDocumentation`. Unset fields serialize as `null`
/// rather than being omitted, so the snapshot shows that the model holds
/// only what the source explicitly stated (no fallback materialization).
#[derive(Serialize)]
struct SnapshotFileDocumentation<'a> {
    title: Option<&'a str>,
    group: Option<&'a str>,
    order: Option<u64>,
    description: Option<&'a str>,
}

impl<'a> From<&'a reportage_core::source::FileDocumentation> for SnapshotFileDocumentation<'a> {
    fn from(documentation: &'a reportage_core::source::FileDocumentation) -> Self {
        Self {
            title: documentation.title.as_deref(),
            group: documentation.group.as_deref(),
            order: documentation.order,
            description: documentation.description.as_ref().map(|text| text.as_str()),
        }
    }
}

#[derive(Serialize)]
struct SnapshotCase<'a> {
    name: &'a str,
    /// Omitted (not `null`) when no `document case` block precedes the case,
    /// mirroring `file_documentation` above: absence in the snapshot mirrors
    /// absence in the source-level model, and keeps the snapshots of the
    /// pre-existing, undocumented fixtures unchanged.
    #[serde(skip_serializing_if = "Option::is_none")]
    documentation: Option<SnapshotCaseDocumentation<'a>>,
    /// The case block's byte range in the fixture source, so span drift is
    /// visible in the snapshot diff without duplicating the source text here.
    span: SnapshotSpan,
    steps: Vec<SnapshotStep<'a>>,
}

impl<'a> From<&'a SourceCase> for SnapshotCase<'a> {
    fn from(source_case: &'a SourceCase) -> Self {
        let case = source_case.case();
        Self {
            name: &case.name,
            documentation: source_case
                .documentation()
                .map(SnapshotCaseDocumentation::from),
            span: SnapshotSpan {
                start: source_case.span().start(),
                end: source_case.span().end(),
            },
            steps: case.steps.iter().map(SnapshotStep::from).collect(),
        }
    }
}

/// Mirrors `source::CaseDocumentation`. Unset fields serialize as `null`
/// rather than being omitted, so the snapshot shows that the model holds
/// only what the source explicitly stated (no case-name fallback
/// materialization).
#[derive(Serialize)]
struct SnapshotCaseDocumentation<'a> {
    title: Option<&'a str>,
    description: Option<&'a str>,
}

impl<'a> From<&'a reportage_core::source::CaseDocumentation> for SnapshotCaseDocumentation<'a> {
    fn from(documentation: &'a reportage_core::source::CaseDocumentation) -> Self {
        Self {
            title: documentation.title.as_deref(),
            description: documentation.description.as_ref().map(|text| text.as_str()),
        }
    }
}

#[derive(Serialize)]
struct SnapshotSpan {
    start: usize,
    end: usize,
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
            Step::SideEffect(side_effect) => Self::from(side_effect),
        }
    }
}

impl<'a> From<&'a SideEffectingStep> for SnapshotStep<'a> {
    fn from(step: &'a SideEffectingStep) -> Self {
        let SideEffectingStep::WriteFile(write_step) = step;
        Self::WriteFile {
            path: write_step.path.as_str(),
            content: SnapshotTextLiteral::from(&write_step.content),
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
    Contains {
        value: &'a str,
    },
    NotContains {
        value: &'a str,
    },
    Matches {
        value: &'a str,
    },
    ContentsEquals {
        value: SnapshotFileContentsReference<'a>,
    },
    TextEquals {
        value: SnapshotTextLiteral<'a>,
    },
}

impl<'a> From<&'a OutputMatcher> for SnapshotOutputMatcher<'a> {
    fn from(matcher: &'a OutputMatcher) -> Self {
        match matcher {
            OutputMatcher::Empty => Self::Empty,
            OutputMatcher::Contains(value) => Self::Contains { value },
            OutputMatcher::NotContains(value) => Self::NotContains { value },
            OutputMatcher::Matches(value) => Self::Matches { value },
            OutputMatcher::ContentsEquals(value) => Self::ContentsEquals {
                value: SnapshotFileContentsReference::from(value),
            },
            OutputMatcher::TextEquals(value) => Self::TextEquals {
                value: SnapshotTextLiteral::from(value),
            },
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SnapshotFileMatcher<'a> {
    Exists,
    NotExists,
    Contains {
        value: SnapshotTextLiteral<'a>,
    },
    Matches {
        value: &'a str,
    },
    ContentsEquals {
        value: SnapshotFileContentsReference<'a>,
    },
    TextEquals {
        value: SnapshotTextLiteral<'a>,
    },
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
            FileMatcher::ContentsEquals(value) => Self::ContentsEquals {
                value: SnapshotFileContentsReference::from(value),
            },
            FileMatcher::TextEquals(value) => Self::TextEquals {
                value: SnapshotTextLiteral::from(value),
            },
        }
    }
}

/// Mirrors `model::FileContentsReference`, keeping the reference kind
/// (`workspace` vs. `fixture`) visible in the AST snapshot.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SnapshotFileContentsReference<'a> {
    Workspace { path: &'a str },
    Fixture { path: &'a str },
}

impl<'a> From<&'a FileContentsReference> for SnapshotFileContentsReference<'a> {
    fn from(value: &'a FileContentsReference) -> Self {
        match value {
            FileContentsReference::Workspace(path) => Self::Workspace {
                path: path.as_str(),
            },
            FileContentsReference::Fixture(path) => Self::Fixture {
                path: path.as_str(),
            },
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
        let source_file = parse(&source).unwrap_or_else(|e| {
            panic!(
                "valid syntax fixture {} must parse successfully before snapshotting: {e}",
                path.display()
            )
        });

        let snapshot_path = snapshot_path_for_fixture(&path);
        let actual = format_snapshot(&source_file);

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
            // Covers the case body and the `before_each` body: both write
            // step positions share the same `WorkspacePath::parse` policy.
            "write_step_absolute_path" | "before_each_write_step_absolute_path" => {
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
                assert_eq!(err.code().as_str(), "parse.heredoc_literal.shallow_indent");
            }
            // Document block body rules the grammar deliberately leaves open
            // (see the "Document block" section of reportage.pest) are
            // rejected during parser construction with fine-grained codes.
            // Covers both the field-less body and the comment-only body:
            // comment lines are not documentation fields.
            "document_file_empty_block"
            | "document_file_comment_only_block"
            | "document_case_empty_block"
            | "document_case_comment_only_block" => {
                assert!(matches!(err, ParseError::EmptyDocumentBlock { .. }));
                assert_eq!(err.code().as_str(), "parse.document_block.empty");
            }
            "document_file_duplicate_field" | "document_case_duplicate_field" => {
                assert!(matches!(err, ParseError::DuplicateDocumentationField { .. }));
                assert_eq!(err.code().as_str(), "parse.document_block.duplicate_field");
            }
            "document_file_order_out_of_range" => {
                assert!(matches!(err, ParseError::InvalidDocumentationOrder { .. }));
                assert_eq!(err.code().as_str(), "parse.document_block.invalid_order");
            }
            "document_file_duplicate_block" => {
                assert!(matches!(err, ParseError::DuplicateDocumentFile { .. }));
                assert_eq!(err.code().as_str(), "parse.document_file.duplicate");
            }
            // `document file` placement covers the whole canonical top-level
            // form `document file? before_each? (document case? case)*`: the
            // block is rejected after the first case, after a pending
            // `document case`, and after a `before_each` block.
            // See #168 / #169 / #70.
            "document_file_after_case"
            | "document_case_then_document_file"
            | "before_each_then_document_file" => {
                assert!(matches!(err, ParseError::DocumentFileAfterCase { .. }));
                assert_eq!(err.code().as_str(), "parse.document_file.after_case");
            }
            // `document case` association rules: a second block before the
            // target case, and a block with no case to attach to. See #169.
            "document_case_duplicate_block" => {
                assert!(matches!(err, ParseError::DuplicateDocumentCase { .. }));
                assert_eq!(err.code().as_str(), "parse.document_case.duplicate");
            }
            "document_case_orphan" => {
                assert!(matches!(err, ParseError::OrphanDocumentCase { .. }));
                assert_eq!(err.code().as_str(), "parse.document_case.orphan");
            }
            // `before_each` placement and body rules: at most one block, before
            // the first case, `write` steps only, at least one step. See #70
            // and the before_each ADR.
            "before_each_duplicate" => {
                assert!(matches!(err, ParseError::DuplicateBeforeEach { .. }));
                assert_eq!(err.code().as_str(), "parse.before_each.duplicate");
            }
            "before_each_after_case" => {
                assert!(matches!(err, ParseError::BeforeEachAfterCase { .. }));
                assert_eq!(err.code().as_str(), "parse.before_each.after_case");
            }
            "before_each_action_step" => {
                assert!(matches!(err, ParseError::BeforeEachActionStep { .. }));
                assert_eq!(err.code().as_str(), "parse.before_each.action_step");
            }
            "before_each_assertion_block" => {
                assert!(matches!(err, ParseError::BeforeEachAssertionBlock { .. }));
                assert_eq!(err.code().as_str(), "parse.before_each.assertion_block");
            }
            "before_each_empty" => {
                assert!(matches!(err, ParseError::EmptyBeforeEach { .. }));
                assert_eq!(err.code().as_str(), "parse.before_each.empty");
            }
            // Literal kind mismatches parse at the grammar level and are
            // rejected as semantic invalid cases with an actionable
            // diagnostic. See docs/reference/semantic-diagnostics.md and
            // docs/adr/20260706T160000Z_workspace-path-literal-syntax.md.
            "file_subject_string_literal"
            | "file_subject_fixture_reference"
            | "dir_subject_string_literal"
            | "write_step_path_string_literal"
            | "write_step_content_workspace_path_literal"
            | "file_contains_expected_workspace_path_literal"
            | "file_text_equals_workspace_path_literal"
            | "stdout_text_equals_workspace_path_literal"
            | "stdout_contains_workspace_path_literal"
            | "dir_contains_entry_workspace_path_literal"
            // A FixtureReference is only valid in a FileContentsReference
            // expected position: never as a `file` checkpoint subject, never
            // as `text_equals` expected text, and never outside an assertion
            // block (`write`'s path / content positions). See #92 and
            // docs/adr/20260706T170000Z_fixture-reference-value-syntax.md.
            | "file_contents_equals_subject_fixture_reference"
            | "file_text_equals_fixture_reference"
            | "stderr_text_equals_fixture_reference"
            | "write_step_path_fixture_reference"
            | "write_step_content_fixture_reference"
            // Documentation field positions parse the kind-agnostic
            // value_literal too: `title` / `group` require a string literal,
            // `description` requires a text literal. See #168 / #169.
            | "document_file_title_workspace_path_literal"
            | "document_file_description_fixture_reference"
            | "document_case_title_workspace_path_literal"
            | "document_case_description_fixture_reference" => {
                assert!(matches!(err, ParseError::LiteralKindMismatch { .. }));
                assert_eq!(err.code().as_str(), "semantic.literal.kind_mismatch");
            }
            // An `@"<path>"` fixture reference literal's lexical validation
            // (empty / absolute / `.` / `..` segment) mirrors WorkspacePath's
            // policy. See #92 and
            // docs/adr/20260706T170000Z_fixture-reference-value-syntax.md.
            "fixture_reference_empty_path" => {
                assert!(matches!(err, ParseError::InvalidFixtureReference { .. }));
                assert_eq!(err.code().as_str(), "semantic.fixture_reference.empty");
            }
            "fixture_reference_absolute_path" => {
                assert!(matches!(err, ParseError::InvalidFixtureReference { .. }));
                assert_eq!(err.code().as_str(), "semantic.fixture_reference.absolute");
            }
            // Covers every dot-segment shape: a leading `.` / `..` segment,
            // and a `.` / `..` segment in the middle of the path.
            "fixture_reference_dot_segment_leading_parent_path"
            | "fixture_reference_dot_segment_leading_current_path"
            | "fixture_reference_dot_segment_middle_current_path"
            | "fixture_reference_dot_segment_middle_parent_path" => {
                assert!(matches!(err, ParseError::InvalidFixtureReference { .. }));
                assert_eq!(
                    err.code().as_str(),
                    "semantic.fixture_reference.dot_segment"
                );
            }
            // Remaining fixtures are rejected as plain pest syntax errors; they share the coarse-grained "parse.syntax" code and are not asserted individually here.
            // See docs/reference/diagnostics.md.
            _ => {
                assert_eq!(err.code().as_str(), "parse.syntax");
            }
        }
    }
}
