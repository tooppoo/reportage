//! Semantic spec schema validation.
//!
//! Loads every `spec/language/semantics/*.json` file, deserialises it into typed Rust structs (with `deny_unknown_fields`), checks fixture consistency invariants, and runs every conformance case against the production semantic evaluator or parser.
//! This is the CI-integrated validation gate that ensures semantic spec files conform to the expected schema and remain executable.
//!
//! Two conformance case shapes exist, because not every semantic rule is a checkpoint-field
//! comparison (see spec/language/semantics/README.md):
//!
//! - An "eval case" (`assertion` + `checkpoint` present) declares a normalized assertion and
//!   static checkpoint data, builds the corresponding production `Expectation`, and runs it
//!   through `evaluate_expectation_at_checkpoint`. `assertion.*` and `logical-composition.*`
//!   rules use this shape.
//! - A "parser case" (`assertion`/`checkpoint` absent) declares a full `assertionSource` and an
//!   expected parse outcome (`valid` or `parseError` with a diagnostic code), run through the
//!   production parser. `value-reference.*` rules (and empty-block cases for
//!   `logical-composition.*`) use this shape, since they concern acceptance/rejection of syntax
//!   or a literal, not a pass/fail assertion outcome.

// Serde-populated struct fields are not "used" in the conventional sense; their value comes from deserialisation rather than direct assignment.
#![allow(dead_code)]

use base64::Engine as _;
use reportage_core::evaluator::{
    Checkpoint as EvaluatorCheckpoint, WorkspaceState, evaluate_expectation_at_checkpoint,
};
use reportage_core::model::{
    DirExpectation, DirMatcher, ExitExpectation, Expectation, FileContentsReference,
    FileExpectation, FileMatcher, FixtureReference, LogicalExpectation, LogicalOperator,
    OutputExpectation, OutputMatcher, Step, TextLiteral, WorkspacePath,
};
use reportage_core::parser::parse;
use reportage_core::result::ActionResult;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Typed representation of a semantic spec file
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticSpec {
    #[serde(rename = "$schema")]
    schema: String,
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
    id: String,
    category: Category,
    syntax: String,
    // Category-specific shape; validated separately once `category` is known (see
    // `category_specific_normative_fields_are_well_formed`), rather than by a single struct that
    // would have to union three unrelated shapes.
    normative: serde_json::Value,
    #[serde(rename = "conformanceCases")]
    conformance_cases: Vec<ConformanceCase>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
enum Category {
    Assertion,
    LogicalComposition,
    ValueReference,
}

// ---------------------------------------------------------------------------
// `assertion` category normative fields
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
enum CheckpointField {
    ExitCode,
    Stdout,
    Stderr,
    File,
    Dir,
}

#[derive(Debug, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
enum ExpectedValueType {
    Uint8,
    Utf8String,
    FileContentsReference,
    #[serde(rename = "none")]
    NoValue,
}

#[derive(Debug, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
enum Comparison {
    /// Exact equality over a scalar value (e.g. `exit`'s `uint8`).
    Exact,
    /// Substring search over raw bytes (`stdout`/`stderr contains`).
    ByteSubstring,
    /// Substring search over decoded UTF-8 text (`file contains`).
    TextSubstring,
    /// Exact equality over a full byte buffer, no normalization (`contents_equals`/`text_equals`).
    ByteExact,
    /// Filesystem presence-and-type check (`exists`).
    Existence,
    /// Byte-length-zero check (`empty`).
    Emptiness,
    /// Exact match against one member of a directory entry name set (`dir contains`).
    EntryNameEquality,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct MatchSemantics {
    comparison: Comparison,
    case_sensitive: Option<bool>,
    line_ending_normalization: Option<bool>,
    empty_expected_always_matches: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AssertionNormative {
    checkpoint_field: CheckpointField,
    operator: AssertionOperator,
    expected_value_type: ExpectedValueType,
    match_semantics: MatchSemantics,
    /// Cross-reference to the `value-reference.*` rule governing this rule's expected-value
    /// resolution, if any (e.g. `assertion.file.contents_equals` references
    /// `value-reference.file-contents-reference.resolve`).
    #[serde(default)]
    referenced_value_reference_rule: Option<String>,
    /// Expected-value categories this rule's `expectedValueType` never implicitly converts from.
    #[serde(default)]
    no_implicit_conversion_from: Vec<String>,
}

// ---------------------------------------------------------------------------
// `logical-composition` category normative fields
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
enum LogicalOperatorId {
    Not,
    All,
    Any,
}

#[derive(Debug, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
enum PassCondition {
    NotAllChildrenPassed,
    AllChildrenPassed,
    AnyChildPassed,
}

#[derive(Debug, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
enum EmptyBlockPolicy {
    SemanticError,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct LogicalCompositionNormative {
    operator: LogicalOperatorId,
    evaluates_all_children: bool,
    pass_condition: PassCondition,
    empty_block_policy: EmptyBlockPolicy,
    empty_block_diagnostic_code: String,
}

// ---------------------------------------------------------------------------
// Conformance cases: normalized assertion / checkpoint (shared by `assertion` and
// `logical-composition`), and their JSON building blocks
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
enum AssertionSubject {
    Exit,
    Stdout,
    Stderr,
    File,
    Dir,
    Logical,
}

#[derive(Debug, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
enum AssertionOperator {
    Equals,
    Contains,
    Empty,
    Exists,
    ContentsEquals,
    TextEquals,
    Not,
    All,
    Any,
}

/// A normalized assertion. `path` is required (by `expectation_from_assertion`, not by serde) when
/// `subject` is `file`/`dir`; `children` is required and recurses when `subject` is `logical`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Assertion {
    subject: AssertionSubject,
    path: Option<String>,
    operator: AssertionOperator,
    #[serde(default)]
    expected: serde_json::Value,
    #[serde(default)]
    children: Vec<Assertion>,
}

#[derive(Debug, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
enum FileContentsReferenceKind {
    WorkspacePath,
    FixtureReference,
}

/// The JSON shape of a `contents_equals`-family expected value: `{"kind": ..., "value": ...}`,
/// mirroring `FileContentsReference = WorkspacePath | FixtureReference`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct FileContentsReferenceValue {
    kind: FileContentsReferenceKind,
    value: String,
}

#[derive(Debug, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
enum Encoding {
    Base64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StreamData {
    data: String,
    encoding: Encoding,
    text: Option<String>,
}

/// A file materialized on disk before a conformance case runs: under the case workspace root
/// (`checkpoint.workspace.files`) or under `repor_dir` (`checkpoint.workspace.reporDirFiles`, for
/// resolving a `contents_equals` expected `@"..."` fixture reference).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceFile {
    path: String,
    contents: StreamData,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WorkspaceFixtureSet {
    #[serde(default)]
    files: Vec<WorkspaceFile>,
    /// Empty directories to create under the case workspace root, for `dir`/`file` cases that
    /// need to observe a path that exists but is not a regular file.
    #[serde(default)]
    dirs: Vec<String>,
    #[serde(default)]
    repor_dir_files: Vec<WorkspaceFile>,
    #[serde(default)]
    repor_dir_dirs: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Checkpoint {
    exit_code: i32,
    stdout: StreamData,
    stderr: StreamData,
    workspace: Option<WorkspaceFixtureSet>,
}

#[derive(Debug, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
enum EvalExpectedResult {
    Pass,
    Fail,
    /// The expected value failed to resolve (e.g. a missing `contents_equals` expected file): a
    /// test-definition problem, surfaced as `evaluate_expectation_at_checkpoint`'s `Err`, never as
    /// an assertion pass/fail.
    ScriptError,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct EvalCase {
    description: String,
    assertion_source: String,
    assertion: Assertion,
    checkpoint: Checkpoint,
    expected_result: EvalExpectedResult,
    expected_diagnostic_code: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
enum ParserExpectedResult {
    Valid,
    ParseError,
}

/// A conformance case verified against the production parser directly, for rules that concern
/// acceptance/rejection of syntax or a literal rather than a checkpoint comparison.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ParserCase {
    description: String,
    assertion_source: String,
    expected_result: ParserExpectedResult,
    expected_diagnostic_code: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ConformanceCase {
    Eval(EvalCase),
    Parser(ParserCase),
}

impl ConformanceCase {
    fn description(&self) -> &str {
        match self {
            ConformanceCase::Eval(c) => &c.description,
            ConformanceCase::Parser(c) => &c.description,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers: loading spec files
// ---------------------------------------------------------------------------

fn spec_dir() -> PathBuf {
    // Resolve relative to the workspace root, not the crate root.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent() // crates/
        .unwrap()
        .parent() // workspace root
        .unwrap()
        .join("spec/language/semantics")
}

fn load_spec_files() -> Vec<(PathBuf, SemanticSpec)> {
    let dir = spec_dir();
    let mut entries: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", dir.display(), e))
        .filter_map(|entry| {
            let path = entry.expect("dir entry error").path();
            if path.extension().is_some_and(|e| e == "json")
                && path.file_name().is_some_and(|n| n != "schema.json")
            {
                Some(path)
            } else {
                None
            }
        })
        .collect();
    entries.sort();

    entries
        .into_iter()
        .map(|path| {
            let src = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("cannot read {}: {}", path.display(), e));
            let spec: SemanticSpec = serde_json::from_str(&src)
                .unwrap_or_else(|e| panic!("failed to deserialise {}: {}", path.display(), e));
            (path, spec)
        })
        .collect()
}

fn decode_base64_stream(stream: &StreamData) -> Vec<u8> {
    if stream.data.is_empty() {
        return vec![];
    }
    base64::engine::general_purpose::STANDARD
        .decode(&stream.data)
        .unwrap_or_else(|e| panic!("invalid base64 in stream data '{}': {}", stream.data, e))
}

/// Every `StreamData` reachable from an eval case, labelled for failure messages.
fn stream_data_entries(case: &EvalCase) -> Vec<(String, &StreamData)> {
    let mut entries = vec![
        ("checkpoint.stdout".to_string(), &case.checkpoint.stdout),
        ("checkpoint.stderr".to_string(), &case.checkpoint.stderr),
    ];
    if let Some(ws) = &case.checkpoint.workspace {
        for (i, f) in ws.files.iter().enumerate() {
            entries.push((
                format!("checkpoint.workspace.files[{i}] ({})", f.path),
                &f.contents,
            ));
        }
        for (i, f) in ws.repor_dir_files.iter().enumerate() {
            entries.push((
                format!("checkpoint.workspace.reporDirFiles[{i}] ({})", f.path),
                &f.contents,
            ));
        }
    }
    entries
}

// ---------------------------------------------------------------------------
// Helpers: materializing a real workspace/repor_dir for an eval case
// ---------------------------------------------------------------------------

/// Owns the temporary directories (if any) backing a materialized checkpoint, so they stay alive
/// for the duration of evaluation. Dropped (and deleted) at the end of the owning scope.
struct CheckpointMaterialization {
    checkpoint: EvaluatorCheckpoint,
    _workspace_dir: Option<tempfile::TempDir>,
    _repor_dir: Option<tempfile::TempDir>,
}

fn write_workspace_file(root: &std::path::Path, file: &WorkspaceFile) {
    let target = root.join(&file.path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|e| panic!("cannot create dir {}: {}", parent.display(), e));
    }
    fs::write(&target, decode_base64_stream(&file.contents))
        .unwrap_or_else(|e| panic!("cannot write {}: {}", target.display(), e));
}

fn checkpoint_for_case(case: &EvalCase) -> CheckpointMaterialization {
    let workspace = case.checkpoint.workspace.as_ref();

    // These conformance cases only exercise process expectations (exit/stdout/stderr) when
    // `workspace` is absent, so `.` is inert there, exactly as before file/dir rules existed.
    let (workspace_root, workspace_dir) = match workspace {
        Some(ws) if !ws.files.is_empty() || !ws.dirs.is_empty() => {
            let dir = tempfile::TempDir::new().expect("failed to create conformance workspace");
            for file in &ws.files {
                write_workspace_file(dir.path(), file);
            }
            for d in &ws.dirs {
                fs::create_dir_all(dir.path().join(d))
                    .unwrap_or_else(|e| panic!("cannot create dir {d}: {e}"));
            }
            (dir.path().to_path_buf(), Some(dir))
        }
        _ => (PathBuf::from("."), None),
    };

    let (repor_dir_path, repor_dir) = match workspace {
        Some(ws) if !ws.repor_dir_files.is_empty() || !ws.repor_dir_dirs.is_empty() => {
            let dir = tempfile::TempDir::new().expect("failed to create conformance repor_dir");
            for file in &ws.repor_dir_files {
                write_workspace_file(dir.path(), file);
            }
            for d in &ws.repor_dir_dirs {
                fs::create_dir_all(dir.path().join(d))
                    .unwrap_or_else(|e| panic!("cannot create dir {d}: {e}"));
            }
            (dir.path().to_path_buf(), Some(dir))
        }
        _ => (PathBuf::from("."), None),
    };

    let checkpoint = EvaluatorCheckpoint {
        workspace: WorkspaceState {
            root: workspace_root,
        },
        last_action: Some(ActionResult {
            command: "<semantic conformance checkpoint>".to_string(),
            // Raw bytes, not lossy-decoded text: the evaluator's stdout/stderr semantics are
            // defined over raw process output bytes, so the fixture harness must feed it the same
            // raw bytes production capture would.
            exit_code: case.checkpoint.exit_code,
            stdout: decode_base64_stream(&case.checkpoint.stdout),
            stderr: decode_base64_stream(&case.checkpoint.stderr),
            shim_invocations: vec![],
            shim_event_parse_warnings: vec![],
        }),
        repor_dir: repor_dir_path,
    };

    CheckpointMaterialization {
        checkpoint,
        _workspace_dir: workspace_dir,
        _repor_dir: repor_dir,
    }
}

// ---------------------------------------------------------------------------
// Helpers: building a production `Expectation` from a normalized `Assertion`
// ---------------------------------------------------------------------------

fn json_expected_u8(expected: &serde_json::Value, label: &str) -> u8 {
    let value = expected
        .as_u64()
        .or_else(|| expected.as_i64().filter(|&v| v >= 0).map(|v| v as u64))
        .unwrap_or_else(|| {
            panic!("{label}: expected must be a non-negative integer, got {expected:?}")
        });
    u8::try_from(value).unwrap_or_else(|_| panic!("{label}: expected {value} does not fit in u8"))
}

fn json_expected_str<'a>(expected: &'a serde_json::Value, label: &str) -> &'a str {
    expected
        .as_str()
        .unwrap_or_else(|| panic!("{label}: expected must be a string, got {expected:?}"))
}

fn file_contents_reference_from_json(
    expected: &serde_json::Value,
    label: &str,
) -> FileContentsReference {
    let value: FileContentsReferenceValue = serde_json::from_value(expected.clone())
        .unwrap_or_else(|e| {
            panic!("{label}: expected must be a FileContentsReference object {{kind,value}}: {e}")
        });
    match value.kind {
        FileContentsReferenceKind::WorkspacePath => FileContentsReference::Workspace(
            WorkspacePath::parse(&value.value).unwrap_or_else(|e| {
                panic!(
                    "{label}: expected.value {:?} is not a valid workspace path: {e:?}",
                    value.value
                )
            }),
        ),
        FileContentsReferenceKind::FixtureReference => FileContentsReference::Fixture(
            FixtureReference::parse(&value.value).unwrap_or_else(|e| {
                panic!(
                    "{label}: expected.value {:?} is not a valid fixture reference: {e:?}",
                    value.value
                )
            }),
        ),
    }
}

fn expectation_from_assertion(a: &Assertion, description: &str) -> Expectation {
    match a.subject {
        AssertionSubject::Exit => {
            assert_eq!(
                a.operator,
                AssertionOperator::Equals,
                "case '{description}': exit assertions must use equals"
            );
            Expectation::Exit(ExitExpectation {
                expected: json_expected_u8(&a.expected, description),
            })
        }
        AssertionSubject::Stdout | AssertionSubject::Stderr => {
            let matcher = match a.operator {
                AssertionOperator::Contains => {
                    OutputMatcher::Contains(json_expected_str(&a.expected, description).to_string())
                }
                AssertionOperator::Empty => OutputMatcher::Empty,
                other => {
                    panic!("case '{description}': unsupported stdout/stderr operator {other:?}")
                }
            };
            match a.subject {
                AssertionSubject::Stdout => Expectation::Stdout(OutputExpectation { matcher }),
                AssertionSubject::Stderr => Expectation::Stderr(OutputExpectation { matcher }),
                _ => unreachable!(),
            }
        }
        AssertionSubject::File => {
            let path = a
                .path
                .clone()
                .unwrap_or_else(|| panic!("case '{description}': file assertion requires 'path'"));
            let matcher = match a.operator {
                AssertionOperator::Exists => FileMatcher::Exists,
                AssertionOperator::Contains => FileMatcher::Contains(TextLiteral::Quoted(
                    json_expected_str(&a.expected, description).to_string(),
                )),
                AssertionOperator::ContentsEquals => FileMatcher::ContentsEquals(
                    file_contents_reference_from_json(&a.expected, description),
                ),
                AssertionOperator::TextEquals => FileMatcher::TextEquals(TextLiteral::Quoted(
                    json_expected_str(&a.expected, description).to_string(),
                )),
                other => panic!("case '{description}': unsupported file operator {other:?}"),
            };
            Expectation::File(FileExpectation { path, matcher })
        }
        AssertionSubject::Dir => {
            let path = a
                .path
                .clone()
                .unwrap_or_else(|| panic!("case '{description}': dir assertion requires 'path'"));
            let matcher = match a.operator {
                AssertionOperator::Exists => DirMatcher::Exists,
                AssertionOperator::Contains => {
                    DirMatcher::Contains(json_expected_str(&a.expected, description).to_string())
                }
                other => panic!("case '{description}': unsupported dir operator {other:?}"),
            };
            Expectation::Dir(DirExpectation { path, matcher })
        }
        AssertionSubject::Logical => {
            let operator = match a.operator {
                AssertionOperator::Not => LogicalOperator::Not,
                AssertionOperator::All => LogicalOperator::All,
                AssertionOperator::Any => LogicalOperator::Any,
                other => panic!("case '{description}': unsupported logical operator {other:?}"),
            };
            assert!(
                !a.children.is_empty(),
                "case '{description}': logical assertion requires non-empty 'children'"
            );
            let children: Vec<Expectation> = a
                .children
                .iter()
                .map(|c| expectation_from_assertion(c, description))
                .collect();
            Expectation::Logical(
                LogicalExpectation::new(operator, children)
                    .unwrap_or_else(|e| panic!("case '{description}': {e:?}")),
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers: cross-checking `assertionSource` against the normalized `assertion`
// ---------------------------------------------------------------------------

fn assert_file_contents_reference_matches(
    parsed: &FileContentsReference,
    declared_expected: &serde_json::Value,
    description: &str,
) {
    let declared: FileContentsReferenceValue = serde_json::from_value(declared_expected.clone())
        .unwrap_or_else(|e| {
            panic!(
                "case '{description}': declared expected is not a FileContentsReference object: {e}"
            )
        });
    match (parsed, declared.kind) {
        (FileContentsReference::Workspace(p), FileContentsReferenceKind::WorkspacePath) => {
            assert_eq!(
                p.as_str(),
                declared.value,
                "case '{description}': workspace path mismatch"
            );
        }
        (FileContentsReference::Fixture(f), FileContentsReferenceKind::FixtureReference) => {
            assert_eq!(
                f.as_str(),
                declared.value,
                "case '{description}': fixture reference mismatch"
            );
        }
        (parsed, kind) => panic!(
            "case '{description}': parsed file contents reference {parsed:?} does not match declared kind {kind:?}"
        ),
    }
}

fn assert_expectation_matches_assertion(
    parsed: &Expectation,
    declared: &Assertion,
    description: &str,
) {
    match (parsed, declared.subject) {
        (Expectation::Exit(e), AssertionSubject::Exit) => {
            assert_eq!(
                declared.operator,
                AssertionOperator::Equals,
                "case '{description}': exit assertion must declare equals"
            );
            assert_eq!(
                e.expected,
                json_expected_u8(&declared.expected, description),
                "case '{description}': exit expected mismatch"
            );
        }
        (Expectation::Stdout(e), AssertionSubject::Stdout)
        | (Expectation::Stderr(e), AssertionSubject::Stderr) => {
            match (&e.matcher, declared.operator) {
                (OutputMatcher::Contains(s), AssertionOperator::Contains) => assert_eq!(
                    s.as_str(),
                    json_expected_str(&declared.expected, description),
                    "case '{description}': contains expected mismatch"
                ),
                (OutputMatcher::Empty, AssertionOperator::Empty) => {}
                (matcher, operator) => panic!(
                    "case '{description}': parsed matcher {matcher:?} does not match declared operator {operator:?}"
                ),
            }
        }
        (Expectation::File(f), AssertionSubject::File) => {
            assert_eq!(
                Some(f.path.as_str()),
                declared.path.as_deref(),
                "case '{description}': file path mismatch"
            );
            match (&f.matcher, declared.operator) {
                (FileMatcher::Exists, AssertionOperator::Exists) => {}
                (FileMatcher::Contains(t), AssertionOperator::Contains) => assert_eq!(
                    t.to_text_value().as_str(),
                    json_expected_str(&declared.expected, description),
                    "case '{description}': file contains expected mismatch"
                ),
                (FileMatcher::ContentsEquals(r), AssertionOperator::ContentsEquals) => {
                    assert_file_contents_reference_matches(r, &declared.expected, description);
                }
                (FileMatcher::TextEquals(t), AssertionOperator::TextEquals) => assert_eq!(
                    t.to_text_value().as_str(),
                    json_expected_str(&declared.expected, description),
                    "case '{description}': file text_equals expected mismatch"
                ),
                (matcher, operator) => panic!(
                    "case '{description}': parsed file matcher {matcher:?} does not match declared operator {operator:?}"
                ),
            }
        }
        (Expectation::Dir(d), AssertionSubject::Dir) => {
            assert_eq!(
                Some(d.path.as_str()),
                declared.path.as_deref(),
                "case '{description}': dir path mismatch"
            );
            match (&d.matcher, declared.operator) {
                (DirMatcher::Exists, AssertionOperator::Exists) => {}
                (DirMatcher::Contains(name), AssertionOperator::Contains) => assert_eq!(
                    name.as_str(),
                    json_expected_str(&declared.expected, description),
                    "case '{description}': dir contains expected mismatch"
                ),
                (matcher, operator) => panic!(
                    "case '{description}': parsed dir matcher {matcher:?} does not match declared operator {operator:?}"
                ),
            }
        }
        (Expectation::Logical(l), AssertionSubject::Logical) => {
            let operator_matches = matches!(
                (l.operator(), declared.operator),
                (LogicalOperator::Not, AssertionOperator::Not)
                    | (LogicalOperator::All, AssertionOperator::All)
                    | (LogicalOperator::Any, AssertionOperator::Any)
            );
            assert!(
                operator_matches,
                "case '{description}': logical operator mismatch"
            );
            assert_eq!(
                l.children().len(),
                declared.children.len(),
                "case '{description}': logical children count mismatch"
            );
            for (parsed_child, declared_child) in l.children().iter().zip(declared.children.iter())
            {
                assert_expectation_matches_assertion(parsed_child, declared_child, description);
            }
        }
        (parsed, subject) => panic!(
            "case '{description}': parsed expectation {parsed:?} does not match declared subject {subject:?}"
        ),
    }
}

fn assert_source_matches_assertion(source: &str, declared: &Assertion, description: &str) {
    let script_src = format!("case \"c\" {{\n  assert {{\n    {source}\n  }}\n}}\n");
    let script = parse(&script_src).unwrap_or_else(|e| {
        panic!("case '{description}': failed to parse assertionSource '{source}': {e}")
    });
    let block = match &script.cases[0].steps[0] {
        Step::AssertionBlock(b) => b,
        other => panic!(
            "case '{description}': assertionSource '{source}' did not parse to an assertion block: {other:?}"
        ),
    };
    assert_eq!(
        block.expectations().len(),
        1,
        "case '{description}': assertionSource '{source}' must parse to exactly one expectation"
    );
    assert_expectation_matches_assertion(&block.expectations()[0], declared, description);
}

// ---------------------------------------------------------------------------
// Tests: structural
// ---------------------------------------------------------------------------

#[test]
fn all_semantic_spec_files_load_successfully() {
    let specs = load_spec_files();
    assert!(
        !specs.is_empty(),
        "no semantic spec files found in {}",
        spec_dir().display()
    );
    // Loading without panic is the assertion; names are printed for context.
    for (path, spec) in &specs {
        println!("loaded: {} (id={})", path.display(), spec.id);
    }
}

#[test]
fn all_specs_reference_local_schema() {
    for (path, spec) in load_spec_files() {
        assert_eq!(
            spec.schema,
            "./schema.json",
            "{}: expected $schema ./schema.json, got {}",
            path.display(),
            spec.schema
        );
    }
}

#[test]
fn all_specs_have_schema_version_1() {
    for (path, spec) in load_spec_files() {
        assert_eq!(
            spec.schema_version,
            1,
            "{}: expected schemaVersion 1, got {}",
            path.display(),
            spec.schema_version
        );
    }
}

#[test]
fn every_rule_category_has_at_least_one_spec() {
    let categories: std::collections::BTreeSet<Category> = load_spec_files()
        .into_iter()
        .map(|(_, spec)| spec.category)
        .collect();
    for expected in [
        Category::Assertion,
        Category::LogicalComposition,
        Category::ValueReference,
    ] {
        assert!(
            categories.iter().any(|c| *c == expected),
            "no semantic spec file uses category {expected:?}"
        );
    }
}

#[test]
fn all_specs_have_at_least_one_conformance_case() {
    for (path, spec) in load_spec_files() {
        assert!(
            !spec.conformance_cases.is_empty(),
            "{}: conformanceCases must not be empty",
            path.display()
        );
    }
}

#[test]
fn all_spec_ids_are_non_empty_and_dot_separated() {
    for (path, spec) in load_spec_files() {
        assert!(
            !spec.id.is_empty(),
            "{}: id must not be empty",
            path.display()
        );
        let parts: Vec<&str> = spec.id.split('.').collect();
        assert_eq!(
            parts.len(),
            3,
            "{}: id '{}' must have exactly three dot-separated parts",
            path.display(),
            spec.id
        );
        for part in &parts {
            assert!(
                !part.is_empty(),
                "{}: id '{}' must not have empty parts",
                path.display(),
                spec.id
            );
        }
    }
}

#[test]
fn all_spec_ids_match_filenames() {
    for (path, spec) in load_spec_files() {
        let expected_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_else(|| panic!("cannot read file stem from {}", path.display()));
        assert_eq!(
            spec.id,
            expected_id,
            "{}: spec id '{}' must match the filename stem '{}'",
            path.display(),
            spec.id,
            expected_id
        );
    }
}

#[test]
fn v0_required_spec_ids_are_all_present() {
    // These are the v0 initial semantic rules defined in Issue #29 and listed in spec/language/semantics/README.md.
    // Removing any of these files must fail CI.
    const REQUIRED_IDS: &[&str] = &[
        "assertion.exit.equals",
        "assertion.stdout.contains",
        "assertion.stderr.contains",
    ];

    let loaded_ids: std::collections::BTreeSet<String> = load_spec_files()
        .into_iter()
        .map(|(_, spec)| spec.id)
        .collect();

    for required in REQUIRED_IDS {
        assert!(
            loaded_ids.contains(*required),
            "required v0 semantic spec '{}' is missing from spec/language/semantics/",
            required
        );
    }
}

const BANNED_NORMATIVE_KEYS: &[&str] = &["notes", "explanation", "aiNote", "rationale", "status"];

fn assert_no_banned_keys(value: &serde_json::Value, path: &std::path::Path) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, v) in map {
                assert!(
                    !BANNED_NORMATIVE_KEYS.contains(&key.as_str()),
                    "{}: normative field '{}' is a banned free-form/rationale key; \
                     rationale belongs in an ADR, TBD items in docs/TBD.md",
                    path.display(),
                    key
                );
                assert_no_banned_keys(v, path);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                assert_no_banned_keys(item, path);
            }
        }
        _ => {}
    }
}

#[test]
fn no_semantic_spec_normative_field_contains_banned_keys() {
    for (path, spec) in load_spec_files() {
        assert_no_banned_keys(&spec.normative, &path);
    }
}

#[test]
fn category_specific_normative_fields_are_well_formed() {
    for (path, spec) in load_spec_files() {
        match spec.category {
            Category::Assertion => {
                let _: AssertionNormative = serde_json::from_value(spec.normative.clone())
                    .unwrap_or_else(|e| {
                        panic!(
                            "{}: invalid assertion normative fields: {}",
                            path.display(),
                            e
                        )
                    });
            }
            Category::LogicalComposition => {
                let _: LogicalCompositionNormative = serde_json::from_value(spec.normative.clone())
                    .unwrap_or_else(|e| {
                        panic!(
                            "{}: invalid logical-composition normative fields: {}",
                            path.display(),
                            e
                        )
                    });
            }
            Category::ValueReference => {
                // value-reference rules are heterogeneous point-facts about a single literal kind
                // or resolution policy, not a uniform operator/comparison model, so normative
                // fields are a free-form (but non-empty, banned-key-free) object rather than a
                // single shared struct.
                let obj = spec.normative.as_object().unwrap_or_else(|| {
                    panic!("{}: normative must be a JSON object", path.display())
                });
                assert!(
                    !obj.is_empty(),
                    "{}: normative must not be empty",
                    path.display()
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests: eval-case (`assertion` + `checkpoint`) structural invariants
// ---------------------------------------------------------------------------

#[test]
fn stream_data_text_matches_base64_decoded_data() {
    // If `text` is present, it must equal the UTF-8 decoding of base64-decoded `data`.
    // This is a fixture consistency check, not a semantic rule.
    for (path, spec) in load_spec_files() {
        for case in &spec.conformance_cases {
            let ConformanceCase::Eval(case) = case else {
                continue;
            };
            for (label, stream) in stream_data_entries(case) {
                if let Some(text) = &stream.text {
                    let decoded = decode_base64_stream(stream);
                    let decoded_str = std::str::from_utf8(&decoded).unwrap_or_else(|e| {
                        panic!(
                            "{} case '{}' {}: base64 data is not valid UTF-8: {}",
                            path.display(),
                            case.description,
                            label,
                            e
                        )
                    });
                    assert_eq!(
                        decoded_str,
                        text.as_str(),
                        "{} case '{}' {}: text does not match base64-decoded data",
                        path.display(),
                        case.description,
                        label
                    );
                }
            }
        }
    }
}

#[test]
fn all_stream_data_bytes_are_valid_base64() {
    for (path, spec) in load_spec_files() {
        for case in &spec.conformance_cases {
            let ConformanceCase::Eval(case) = case else {
                continue;
            };
            for (label, stream) in stream_data_entries(case) {
                if !stream.data.is_empty() {
                    base64::engine::general_purpose::STANDARD
                        .decode(&stream.data)
                        .unwrap_or_else(|e| {
                            panic!(
                                "{} case '{}' {}: data is not valid base64: {}",
                                path.display(),
                                case.description,
                                label,
                                e
                            )
                        });
                }
            }
        }
    }
}

#[test]
fn exit_assertion_expected_is_integer() {
    for (path, spec) in load_spec_files() {
        for case in &spec.conformance_cases {
            let ConformanceCase::Eval(case) = case else {
                continue;
            };
            if case.assertion.subject == AssertionSubject::Exit {
                assert!(
                    case.assertion.expected.is_i64() || case.assertion.expected.is_u64(),
                    "{} case '{}': exit assertion expected must be an integer, got {:?}",
                    path.display(),
                    case.description,
                    case.assertion.expected
                );
            }
        }
    }
}

#[test]
fn exit_assertion_expected_is_in_uint8_range() {
    for (path, spec) in load_spec_files() {
        for case in &spec.conformance_cases {
            let ConformanceCase::Eval(case) = case else {
                continue;
            };
            if case.assertion.subject == AssertionSubject::Exit {
                let label = format!("{} case '{}'", path.display(), case.description);
                let _ = json_expected_u8(&case.assertion.expected, &label);
            }
        }
    }
}

#[test]
fn stdout_stderr_contains_expected_is_string() {
    for (path, spec) in load_spec_files() {
        for case in &spec.conformance_cases {
            let ConformanceCase::Eval(case) = case else {
                continue;
            };
            let is_stdout_or_stderr = matches!(
                case.assertion.subject,
                AssertionSubject::Stdout | AssertionSubject::Stderr
            );
            if is_stdout_or_stderr && case.assertion.operator == AssertionOperator::Contains {
                assert!(
                    case.assertion.expected.is_string(),
                    "{} case '{}': stdout/stderr contains expected must be a string, got {:?}",
                    path.display(),
                    case.description,
                    case.assertion.expected
                );
            }
        }
    }
}

#[test]
fn assertion_conformance_case_subjects_match_normative_checkpoint_field() {
    for (path, spec) in load_spec_files() {
        if spec.category != Category::Assertion {
            continue;
        }
        let normative: AssertionNormative = serde_json::from_value(spec.normative.clone())
            .unwrap_or_else(|e| {
                panic!(
                    "{}: invalid assertion normative fields: {}",
                    path.display(),
                    e
                )
            });
        for case in &spec.conformance_cases {
            let ConformanceCase::Eval(case) = case else {
                continue;
            };
            let subject_matches = matches!(
                (case.assertion.subject, normative.checkpoint_field),
                (AssertionSubject::Exit, CheckpointField::ExitCode)
                    | (AssertionSubject::Stdout, CheckpointField::Stdout)
                    | (AssertionSubject::Stderr, CheckpointField::Stderr)
                    | (AssertionSubject::File, CheckpointField::File)
                    | (AssertionSubject::Dir, CheckpointField::Dir)
            );
            assert!(
                subject_matches,
                "{} case '{}': assertion subject {:?} does not match normative checkpointField {:?}",
                path.display(),
                case.description,
                case.assertion.subject,
                normative.checkpoint_field
            );
        }
    }
}

#[test]
fn assertion_conformance_case_operators_match_normative_operator() {
    for (path, spec) in load_spec_files() {
        if spec.category != Category::Assertion {
            continue;
        }
        let normative: AssertionNormative = serde_json::from_value(spec.normative.clone())
            .unwrap_or_else(|e| {
                panic!(
                    "{}: invalid assertion normative fields: {}",
                    path.display(),
                    e
                )
            });
        for case in &spec.conformance_cases {
            let ConformanceCase::Eval(case) = case else {
                continue;
            };
            assert_eq!(
                case.assertion.operator,
                normative.operator,
                "{} case '{}': assertion operator does not match normative operator",
                path.display(),
                case.description
            );
        }
    }
}

#[test]
fn initial_v0_rules_have_expected_normative_fields() {
    // Pin the normative fields of the three v0 rules so that a machine-readable source of truth cannot silently change its comparison semantics.
    for (path, spec) in load_spec_files() {
        if !matches!(
            spec.id.as_str(),
            "assertion.exit.equals" | "assertion.stdout.contains" | "assertion.stderr.contains"
        ) {
            continue;
        }
        let n: AssertionNormative =
            serde_json::from_value(spec.normative.clone()).unwrap_or_else(|e| {
                panic!(
                    "{}: invalid assertion normative fields: {}",
                    path.display(),
                    e
                )
            });
        let ms = &n.match_semantics;
        match spec.id.as_str() {
            "assertion.exit.equals" => {
                assert_eq!(
                    n.checkpoint_field,
                    CheckpointField::ExitCode,
                    "{}",
                    path.display()
                );
                assert_eq!(n.operator, AssertionOperator::Equals, "{}", path.display());
                assert_eq!(
                    n.expected_value_type,
                    ExpectedValueType::Uint8,
                    "{}",
                    path.display()
                );
                assert_eq!(ms.comparison, Comparison::Exact, "{}", path.display());
            }
            "assertion.stdout.contains" | "assertion.stderr.contains" => {
                let want_field = if spec.id == "assertion.stdout.contains" {
                    CheckpointField::Stdout
                } else {
                    CheckpointField::Stderr
                };
                assert_eq!(n.checkpoint_field, want_field, "{}", path.display());
                assert_eq!(
                    n.operator,
                    AssertionOperator::Contains,
                    "{}",
                    path.display()
                );
                assert_eq!(
                    n.expected_value_type,
                    ExpectedValueType::Utf8String,
                    "{}",
                    path.display()
                );
                assert_eq!(
                    ms.comparison,
                    Comparison::ByteSubstring,
                    "{}",
                    path.display()
                );
                assert_eq!(ms.case_sensitive, Some(true), "{}", path.display());
                assert_eq!(
                    ms.line_ending_normalization,
                    Some(false),
                    "{}",
                    path.display()
                );
                assert_eq!(
                    ms.empty_expected_always_matches,
                    Some(true),
                    "{}",
                    path.display()
                );
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn conformance_case_assertion_source_matches_normalized_assertion() {
    // Parsing the human-facing `assertionSource` through the real parser must yield the same
    // normalized assertion. This prevents the two representations from drifting apart. Parser
    // cases are validated by `parser_conformance_cases_match_production_parser` instead.
    for (path, spec) in load_spec_files() {
        for case in &spec.conformance_cases {
            let ConformanceCase::Eval(case) = case else {
                continue;
            };
            let label = format!("{}: {}", path.display(), case.description);
            assert_source_matches_assertion(&case.assertion_source, &case.assertion, &label);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests: conformance — the main production-code gates
// ---------------------------------------------------------------------------

#[test]
fn conformance_case_expected_result_matches_v0_semantics() {
    // The normalized assertion representation and checkpoint data from the JSON semantic specs
    // (materializing a real workspace/repor_dir on disk when the case declares one) are fed
    // directly to the production semantic evaluator.
    for (path, spec) in load_spec_files() {
        for case in &spec.conformance_cases {
            let ConformanceCase::Eval(case) = case else {
                continue;
            };
            let expectation = expectation_from_assertion(&case.assertion, &case.description);
            let materialized = checkpoint_for_case(case);
            let result = evaluate_expectation_at_checkpoint(&expectation, &materialized.checkpoint);
            match case.expected_result {
                EvalExpectedResult::Pass | EvalExpectedResult::Fail => {
                    let result = result.unwrap_or_else(|e| {
                        panic!(
                            "{} case '{}': expected {:?} but evaluation returned a script error: {} ({})",
                            path.display(),
                            case.description,
                            case.expected_result,
                            e.message,
                            e.diagnostic_code.as_str()
                        )
                    });
                    let want_pass = matches!(case.expected_result, EvalExpectedResult::Pass);
                    assert_eq!(
                        result.passed,
                        want_pass,
                        "{} case '{}': evaluator passed={} but expectedResult={:?}",
                        path.display(),
                        case.description,
                        result.passed,
                        case.expected_result
                    );
                }
                EvalExpectedResult::ScriptError => {
                    let err = result.err().unwrap_or_else(|| {
                        panic!(
                            "{} case '{}': expected a script error but evaluation succeeded",
                            path.display(),
                            case.description
                        )
                    });
                    if let Some(expected_code) = &case.expected_diagnostic_code {
                        assert_eq!(
                            err.diagnostic_code.as_str(),
                            expected_code,
                            "{} case '{}': script error diagnostic code mismatch",
                            path.display(),
                            case.description
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn parser_conformance_cases_match_production_parser() {
    for (path, spec) in load_spec_files() {
        for case in &spec.conformance_cases {
            let ConformanceCase::Parser(case) = case else {
                continue;
            };
            let script_src = format!(
                "case \"c\" {{\n  assert {{\n    {}\n  }}\n}}\n",
                case.assertion_source
            );
            let result = parse(&script_src);
            match case.expected_result {
                ParserExpectedResult::Valid => {
                    result.unwrap_or_else(|e| {
                        panic!(
                            "{} case '{}': expected valid syntax but parsing failed: {}",
                            path.display(),
                            case.description,
                            e
                        )
                    });
                }
                ParserExpectedResult::ParseError => {
                    let err = result.err().unwrap_or_else(|| {
                        panic!(
                            "{} case '{}': expected a parse error but parsing succeeded",
                            path.display(),
                            case.description
                        )
                    });
                    let expected_code =
                        case.expected_diagnostic_code.as_deref().unwrap_or_else(|| {
                            panic!(
                                "{} case '{}': parseError cases must set expectedDiagnosticCode",
                                path.display(),
                                case.description
                            )
                        });
                    assert_eq!(
                        err.code().as_str(),
                        expected_code,
                        "{} case '{}': parse error diagnostic code mismatch",
                        path.display(),
                        case.description
                    );
                }
            }
        }
    }
}

#[test]
fn every_conformance_case_has_a_non_empty_description_and_source() {
    for (path, spec) in load_spec_files() {
        for case in &spec.conformance_cases {
            assert!(
                !case.description().is_empty(),
                "{}: conformance case has an empty description",
                path.display()
            );
        }
    }
}
