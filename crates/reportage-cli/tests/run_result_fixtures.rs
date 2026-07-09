//! Representative-fixture conformance for the artifact `result.json` manifest (issue #102).
//!
//! Mirrors `json_report_fixtures.rs`'s approach to "schema validation": each fixture run's `result.json` is deserialised into typed Rust structs marked `#[serde(deny_unknown_fields)]`, rather than run through an external JSON Schema validator.
//! See `spec/artifacts/run-result/schema.json` for the authoritative contract.
//!
//! Unlike `json_report_fixtures.rs` (whose structs deliberately model only the expectation kinds its fixtures exercise), the structs here model the *full* stable contract the schema defines — every expectation kind, observation enum, and diagnostic shape — because `result.json` is the canonical manifest of a run (see issue #102's requirement that typed validation covers the whole stable contract, not just fixture-exercised shapes).
//!
//! Fixtures live in `tests/fixtures/run_result/*.repor`.
//! Each has a companion `<name>.snapshot.json` with the volatile field (`tool.version`) normalised out, refreshed via `UPDATE_RUN_RESULT_SNAPSHOTS`, mirroring `json_report_fixtures.rs`'s convention.
//!
//! This suite also verifies:
//!
//! - evidence integrity: every `artifactRef` in `result.json` names an existing file inside the bundle whose byte size and SHA-256 digest match the manifest;
//! - projection parity: for the same run, the `--format=json` stdout document agrees with `result.json` on the parity items required by issue #102, and is exactly the canonical document minus the defined projection differences.

// Serde-populated struct fields are not "used" in the conventional sense; their value comes from deserialisation rather than direct assignment.
// Mirrors json_report_fixtures.rs.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use assert_fs::TempDir;
use assert_fs::prelude::*;
use reportage_core::run_result::sha256_hex;
use serde::Deserialize;
use serde_json::Value;

// ---------------------------------------------------------------------------
// Typed representation of the artifact result document (schema validation)
//
// One struct/enum per shape in spec/artifacts/run-result/schema.json, in the same order.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RunResultDocument {
    schema_version: u32,
    tool: Tool,
    status: TopStatus,
    process_exit_code: i32,
    noop: bool,
    summary: Summary,
    diagnostics: Vec<Diagnostic>,
    tests: Vec<TestEntry>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum TopStatus {
    Passed,
    Failed,
    Error,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tool {
    name: String,
    version: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Summary {
    scripts: u32,
    actions: u32,
    assertions: u32,
    passed: u32,
    failed: u32,
    errors: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Diagnostic {
    id: String,
    category: Category,
    severity: Severity,
    message: String,
    origin: Origin,
    /// Deliberately not `Option<Location>`: with no `#[serde(default)]`, a required non-`Option` field makes a *missing* `location` key a deserialization error, distinct from a *present* `location: null`.
    /// An `Option<Location>` would silently accept both, defeating the point of testing that this field is always present.
    /// Its shape (`null` or a `Location`) is checked separately by `assert_location_shape_is_valid`.
    /// Mirrors `json_report_fixtures.rs`.
    location: Value,
    code: Option<String>,
}

/// Asserts `location` is JSON `null` or deserializes as a valid `Location`, without collapsing "missing key" and "present but null" the way an `Option<Location>` struct field would.
fn assert_location_shape_is_valid(location: &Value) {
    if location.is_null() {
        return;
    }
    serde_json::from_value::<Location>(location.clone()).unwrap_or_else(|e| {
        panic!("diagnostic location is neither null nor a valid Location: {e}\n{location}")
    });
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum Category {
    Parse,
    Semantic,
    Runtime,
    Assertion,
    Internal,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum Severity {
    Error,
    Failure,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum Origin {
    Source { source: String },
    Test { test: String },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Location {
    line: u32,
    column: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct TestEntry {
    id: String,
    name: String,
    path: Option<String>,
    status: TestStatus,
    actions: Vec<Action>,
    assertions: Vec<Assertion>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum TestStatus {
    Passed,
    Failed,
    Error,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Action {
    id: String,
    command: String,
    exit_code: i32,
    stdout: EvidenceReference,
    stderr: EvidenceReference,
    #[serde(default)]
    shim_invocations: Vec<Value>,
    #[serde(default)]
    shim_event_parse_warnings: Vec<String>,
}

/// The `{ artifactRef, sizeBytes, sha256 }` evidence reference triple.
/// `sha256` is required here, unlike the `--format=json` stdout contract's two-field reference.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct EvidenceReference {
    artifact_ref: String,
    size_bytes: u64,
    sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Assertion {
    id: String,
    status: Status,
    checkpoint: String,
    expectation: Expectation,
    #[serde(default)]
    diagnostic_ref: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum Status {
    Passed,
    Failed,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum Outcome {
    Match,
    Mismatch,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum ExpectedSource {
    Workspace { path: String },
    Fixture { path: String },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum TextExpectedSource {
    Quoted { value: String },
    Heredoc { value: String },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ContentsMismatch {
    first_diff_offset: u64,
    first_diff_line: u64,
    actual_context: String,
    expected_context: String,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum FileExistsObserved {
    RegularFile,
    NotRegularFile,
    Missing,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum FileContentObserved {
    Found,
    NotFound,
    Missing,
    NotRegularFile,
    Unreadable,
    NotUtf8,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum ContentsEqualsObserved {
    Compared,
    ActualMissing,
    #[serde(rename = "actualNotARegularFile")]
    ActualNotARegularFile,
    ActualUnreadable,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum DirExistsObserved {
    Directory,
    NotADirectory,
    Missing,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum DirContainsObserved {
    Found,
    EntryMissing,
    SubjectMissing,
    SubjectNotADirectory,
    SubjectUnreadable,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum LogicalOperator {
    Not,
    All,
    Any,
}

/// The full 13-kind expectation contract of `spec/artifacts/run-result/schema.json`.
/// Every variant the schema defines is modelled, whether or not a fixture currently exercises it.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum Expectation {
    // `rename_all` on the enum itself only renames variant names, not the fields inside a struct-like variant, so each variant needs its own `rename_all` for its fields.
    #[serde(rename_all = "camelCase")]
    Exit {
        status: Status,
        expected: i64,
        actual: i64,
        #[serde(default)]
        diagnostic_ref: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    StdoutContains {
        status: Status,
        expected: String,
        #[serde(default)]
        actual_ref: Option<String>,
        actual_size_bytes: u64,
        #[serde(default)]
        diagnostic_ref: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    StderrContains {
        status: Status,
        expected: String,
        #[serde(default)]
        actual_ref: Option<String>,
        actual_size_bytes: u64,
        #[serde(default)]
        diagnostic_ref: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    StdoutEmpty {
        status: Status,
        #[serde(default)]
        actual_ref: Option<String>,
        actual_size_bytes: u64,
        #[serde(default)]
        diagnostic_ref: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    StderrEmpty {
        status: Status,
        #[serde(default)]
        actual_ref: Option<String>,
        actual_size_bytes: u64,
        #[serde(default)]
        diagnostic_ref: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    FileExists {
        status: Status,
        path: String,
        observed: FileExistsObserved,
        #[serde(default)]
        diagnostic_ref: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    FileContains {
        status: Status,
        path: String,
        expected: String,
        observed: FileContentObserved,
        #[serde(default)]
        diagnostic_ref: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    FileContentsEquals {
        status: Status,
        path: String,
        expected_source: ExpectedSource,
        observed: ContentsEqualsObserved,
        #[serde(default)]
        outcome: Option<Outcome>,
        #[serde(default)]
        actual_size_bytes: Option<u64>,
        #[serde(default)]
        expected_size_bytes: Option<u64>,
        #[serde(default)]
        mismatch: Option<ContentsMismatch>,
        #[serde(default)]
        diagnostic_ref: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    FileTextEquals {
        status: Status,
        path: String,
        expected_source: TextExpectedSource,
        observed: ContentsEqualsObserved,
        #[serde(default)]
        outcome: Option<Outcome>,
        #[serde(default)]
        actual_size_bytes: Option<u64>,
        #[serde(default)]
        expected_size_bytes: Option<u64>,
        #[serde(default)]
        mismatch: Option<ContentsMismatch>,
        #[serde(default)]
        diagnostic_ref: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    StdoutContentsEquals {
        status: Status,
        expected_source: ExpectedSource,
        #[serde(default)]
        actual_ref: Option<String>,
        outcome: Outcome,
        actual_size_bytes: u64,
        expected_size_bytes: u64,
        #[serde(default)]
        mismatch: Option<ContentsMismatch>,
        #[serde(default)]
        diagnostic_ref: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    StderrContentsEquals {
        status: Status,
        expected_source: ExpectedSource,
        #[serde(default)]
        actual_ref: Option<String>,
        outcome: Outcome,
        actual_size_bytes: u64,
        expected_size_bytes: u64,
        #[serde(default)]
        mismatch: Option<ContentsMismatch>,
        #[serde(default)]
        diagnostic_ref: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    DirExists {
        status: Status,
        path: String,
        observed: DirExistsObserved,
        #[serde(default)]
        diagnostic_ref: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    DirContains {
        status: Status,
        path: String,
        expected_entry: String,
        observed: DirContainsObserved,
        #[serde(default)]
        diagnostic_ref: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Logical {
        status: Status,
        operator: LogicalOperator,
        children: Vec<Expectation>,
        #[serde(default)]
        diagnostic_ref: Option<String>,
    },
}

impl Expectation {
    /// Enforces the schema's conditional requirements that `deny_unknown_fields` alone cannot express: `observed: compared` requires the comparison fields, and `outcome: mismatch` requires `mismatch`.
    /// Recurses into logical children, which must also never carry their own `diagnosticRef` (diagnostic attribution is composition-level only).
    fn assert_conditional_invariants(&self, is_logical_child: bool) {
        if is_logical_child {
            let diagnostic_ref = match self {
                Expectation::Exit { diagnostic_ref, .. }
                | Expectation::StdoutContains { diagnostic_ref, .. }
                | Expectation::StderrContains { diagnostic_ref, .. }
                | Expectation::StdoutEmpty { diagnostic_ref, .. }
                | Expectation::StderrEmpty { diagnostic_ref, .. }
                | Expectation::FileExists { diagnostic_ref, .. }
                | Expectation::FileContains { diagnostic_ref, .. }
                | Expectation::FileContentsEquals { diagnostic_ref, .. }
                | Expectation::FileTextEquals { diagnostic_ref, .. }
                | Expectation::StdoutContentsEquals { diagnostic_ref, .. }
                | Expectation::StderrContentsEquals { diagnostic_ref, .. }
                | Expectation::DirExists { diagnostic_ref, .. }
                | Expectation::DirContains { diagnostic_ref, .. }
                | Expectation::Logical { diagnostic_ref, .. } => diagnostic_ref,
            };
            assert!(
                diagnostic_ref.is_none(),
                "a logical composition's child must never carry its own diagnosticRef: {self:?}"
            );
        }

        match self {
            Expectation::FileContentsEquals {
                observed,
                outcome,
                actual_size_bytes,
                expected_size_bytes,
                mismatch,
                ..
            }
            | Expectation::FileTextEquals {
                observed,
                outcome,
                actual_size_bytes,
                expected_size_bytes,
                mismatch,
                ..
            } => {
                if *observed == ContentsEqualsObserved::Compared {
                    assert!(
                        outcome.is_some()
                            && actual_size_bytes.is_some()
                            && expected_size_bytes.is_some(),
                        "observed: compared requires outcome/actualSizeBytes/expectedSizeBytes: {self:?}"
                    );
                }
                if *outcome == Some(Outcome::Mismatch) {
                    assert!(
                        mismatch.is_some(),
                        "outcome: mismatch requires a mismatch object: {self:?}"
                    );
                }
            }
            Expectation::StdoutContentsEquals {
                outcome, mismatch, ..
            }
            | Expectation::StderrContentsEquals {
                outcome, mismatch, ..
            } => {
                if *outcome == Outcome::Mismatch {
                    assert!(
                        mismatch.is_some(),
                        "outcome: mismatch requires a mismatch object: {self:?}"
                    );
                }
            }
            Expectation::Logical { children, .. } => {
                for child in children {
                    child.assert_conditional_invariants(true);
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Fixture / CLI helpers
// ---------------------------------------------------------------------------

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture_dir() -> PathBuf {
    repo_root().join("tests/fixtures/run_result")
}

fn fixture_paths() -> Vec<PathBuf> {
    let pattern = fixture_dir()
        .join("*.repor")
        .to_str()
        .expect("fixture glob path must be valid UTF-8")
        .to_string();

    let mut paths = glob::glob(&pattern)
        .expect("run_result fixture glob pattern must be valid")
        .map(|entry| entry.expect("run_result fixture path must be readable"))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn fixture_stem(path: &Path) -> &str {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .expect("run_result fixture file name must be valid UTF-8")
}

fn snapshot_path_for_fixture(path: &Path) -> PathBuf {
    path.with_extension("snapshot.json")
}

fn update_snapshots_enabled() -> bool {
    std::env::var_os("UPDATE_RUN_RESULT_SNAPSHOTS").is_some()
}

/// The fixed run id every fixture run in this suite uses; each run gets its own temp dir, so reuse across runs never collides.
const RUN_ID: &str = "run-result-fixture";

/// Copies `fixture` into a fresh temp dir (under its own file name) and runs `reportage --debug-run-id <RUN_ID> [--format json] <file>` there.
/// Returns the run directory containing `result.json`, the parsed stdout (JSON document when `json_stdout`, otherwise `None`), the process exit code, and the temp dir keeping everything alive.
fn run_fixture(fixture: &Path, json_stdout: bool) -> (PathBuf, Option<Value>, i32, TempDir) {
    let dir = TempDir::new().unwrap();
    let name = fixture.file_name().unwrap().to_str().unwrap();
    let content = std::fs::read_to_string(fixture).unwrap();
    dir.child(name).write_str(&content).unwrap();

    let mut cmd = Command::cargo_bin("reportage").unwrap();
    cmd.current_dir(&dir).arg("--debug-run-id").arg(RUN_ID);
    if json_stdout {
        cmd.arg("--format").arg("json");
    }
    let output = cmd.arg(name).output().unwrap();

    let stdout_doc = if json_stdout {
        let stdout = String::from_utf8(output.stdout).unwrap();
        Some(serde_json::from_str(&stdout).unwrap_or_else(|e| {
            panic!("stdout was not a single valid JSON document: {e}\n{stdout}")
        }))
    } else {
        None
    };

    let run_dir = dir.path().join(".reportage/runs").join(RUN_ID);
    (run_dir, stdout_doc, output.status.code().unwrap(), dir)
}

fn read_result_json(run_dir: &Path) -> Value {
    let path = run_dir.join("result.json");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", path.display()))
}

/// Replaces the volatile field (`tool.version`) with a fixed placeholder before snapshotting.
/// Unlike the `--format=json` snapshots there is no `artifactRoot` to normalise: the artifact document resolves references against its own directory, and evidence digests/sizes are deterministic for these fixtures.
fn normalize_for_snapshot(mut doc: Value) -> Value {
    doc["tool"]["version"] = Value::String("<VERSION>".to_string());
    doc
}

fn format_snapshot(doc: &Value) -> String {
    let mut json = serde_json::to_string_pretty(doc).expect("snapshot serialization must succeed");
    json.push('\n');
    json
}

// ---------------------------------------------------------------------------
// Completeness
// ---------------------------------------------------------------------------

#[test]
fn all_required_representative_scenarios_are_present() {
    const REQUIRED: &[&str] = &[
        "passed",
        "assertion_failure",
        "parse_error",
        "semantic_error",
        "runtime_error",
        "partial_execution_after_runtime_error",
        "expectation_kinds",
        "contents_equals",
        "noop",
    ];

    let stems: std::collections::BTreeSet<String> = fixture_paths()
        .iter()
        .map(|p| fixture_stem(p).to_string())
        .collect();

    for required in REQUIRED {
        assert!(
            stems.contains(*required),
            "required representative fixture '{required}' is missing from tests/fixtures/run_result/"
        );
    }
    assert_eq!(
        stems.len(),
        REQUIRED.len(),
        "unexpected extra fixture(s) in tests/fixtures/run_result/: {stems:?}"
    );
}

// ---------------------------------------------------------------------------
// Schema validation (typed deserialization over the full stable contract)
// ---------------------------------------------------------------------------

#[test]
fn every_fixture_run_writes_a_schema_valid_result_json() {
    let paths = fixture_paths();
    assert!(
        !paths.is_empty(),
        "expected at least one run_result fixture"
    );

    for path in paths {
        let (run_dir, _stdout, _exit_code, _dir) = run_fixture(&path, false);
        let json = read_result_json(&run_dir);
        let text = serde_json::to_string(&json).unwrap();
        let doc: RunResultDocument = serde_json::from_str(&text).unwrap_or_else(|e| {
            panic!(
                "fixture {} produced a result.json that does not conform to the typed schema: {e}\n{text}",
                path.display()
            )
        });
        for diagnostic in &doc.diagnostics {
            assert_location_shape_is_valid(&diagnostic.location);
        }
        for test in &doc.tests {
            for assertion in &test.assertions {
                assertion.expectation.assert_conditional_invariants(false);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Evidence integrity
// ---------------------------------------------------------------------------

#[test]
fn evidence_files_match_their_manifest_references() {
    for path in fixture_paths() {
        let (run_dir, _stdout, _exit_code, _dir) = run_fixture(&path, false);
        let json = read_result_json(&run_dir);

        let mut references = 0usize;
        for test in json["tests"].as_array().unwrap() {
            for action in test["actions"].as_array().unwrap() {
                for stream in ["stdout", "stderr"] {
                    let reference = &action[stream];
                    let artifact_ref = reference["artifactRef"].as_str().unwrap();
                    let evidence_path = run_dir.join(artifact_ref);
                    let bytes = std::fs::read(&evidence_path).unwrap_or_else(|e| {
                        panic!(
                            "fixture {}: evidence file {} referenced by result.json is missing: {e}",
                            path.display(),
                            evidence_path.display()
                        )
                    });
                    assert_eq!(
                        bytes.len() as u64,
                        reference["sizeBytes"].as_u64().unwrap(),
                        "fixture {}: sizeBytes must match the evidence file {}",
                        path.display(),
                        artifact_ref
                    );
                    assert_eq!(
                        sha256_hex(&bytes),
                        reference["sha256"].as_str().unwrap(),
                        "fixture {}: sha256 must match the evidence file {}",
                        path.display(),
                        artifact_ref
                    );
                    references += 1;
                }
            }
        }
        // Only fixtures whose run executed at least one action produce references; for those, an accidentally empty loop must not vacuously pass.
        if !json["tests"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["actions"].as_array().unwrap().is_empty())
        {
            assert!(references > 0);
        }
    }
}

// ---------------------------------------------------------------------------
// Snapshot validation
// ---------------------------------------------------------------------------

#[test]
fn snapshots_for_run_result_fixtures_are_current() {
    let paths = fixture_paths();
    assert!(
        !paths.is_empty(),
        "expected at least one run_result fixture"
    );

    let update_snapshots = update_snapshots_enabled();
    for path in paths {
        let (run_dir, _stdout, _exit_code, _dir) = run_fixture(&path, false);
        let normalized = normalize_for_snapshot(read_result_json(&run_dir));
        let actual = format_snapshot(&normalized);

        let snapshot_path = snapshot_path_for_fixture(&path);
        if update_snapshots {
            std::fs::write(&snapshot_path, actual).unwrap_or_else(|e| {
                panic!("failed to update snapshot {}: {e}", snapshot_path.display())
            });
            continue;
        }

        let expected = std::fs::read_to_string(&snapshot_path).unwrap_or_else(|e| {
            panic!(
                "failed to read snapshot {}: {e}\n\
                 run `UPDATE_RUN_RESULT_SNAPSHOTS=1 cargo test -p reportage-cli --test run_result_fixtures snapshots_for_run_result_fixtures_are_current` to create or refresh snapshots",
                snapshot_path.display()
            )
        });

        assert_eq!(
            expected,
            actual,
            "snapshot for {} is stale; run \
             `UPDATE_RUN_RESULT_SNAPSHOTS=1 cargo test -p reportage-cli --test run_result_fixtures snapshots_for_run_result_fixtures_are_current` \
             and review the JSON diff",
            path.display()
        );
    }
}

// ---------------------------------------------------------------------------
// Projection parity with --format=json
//
// Issue #102's minimum parity items, checked field-by-field, plus a strict structural check that the stdout document is exactly the canonical document minus the defined projection differences (artifactRoot added; noop and evidence sha256 dropped).
// ---------------------------------------------------------------------------

#[test]
fn stdout_projection_agrees_with_the_artifact_result_from_the_same_run() {
    for path in fixture_paths() {
        let (run_dir, stdout_doc, exit_code, dir) = run_fixture(&path, true);
        let stdout_doc = stdout_doc.unwrap();
        let artifact_doc = read_result_json(&run_dir);
        let context = path.display();

        // The stdout document's artifactRoot must name the run directory this result.json and its evidence files were written to.
        assert_eq!(
            dir.path()
                .join(stdout_doc["artifactRoot"].as_str().unwrap()),
            run_dir,
            "{context}: artifactRoot must point at the run directory"
        );

        // Top-level status / processExitCode.
        assert_eq!(stdout_doc["status"], artifact_doc["status"], "{context}");
        assert_eq!(
            stdout_doc["processExitCode"], artifact_doc["processExitCode"],
            "{context}"
        );
        assert_eq!(
            stdout_doc["processExitCode"],
            serde_json::json!(exit_code),
            "{context}: the observed reportage process exit code must match both documents"
        );

        // Summary.
        assert_eq!(stdout_doc["summary"], artifact_doc["summary"], "{context}");

        // Diagnostics code / category / severity (and ids, which both documents share).
        let stdout_diagnostics = stdout_doc["diagnostics"].as_array().unwrap();
        let artifact_diagnostics = artifact_doc["diagnostics"].as_array().unwrap();
        assert_eq!(
            stdout_diagnostics.len(),
            artifact_diagnostics.len(),
            "{context}"
        );
        for (s, a) in stdout_diagnostics.iter().zip(artifact_diagnostics) {
            for field in ["id", "code", "category", "severity"] {
                assert_eq!(
                    s.get(field),
                    a.get(field),
                    "{context}: diagnostics[].{field}"
                );
            }
        }

        // Test / action / assertion ids, action exitCode, expectation kind/status, and
        // captured stdout/stderr artifactRef / sizeBytes.
        let stdout_tests = stdout_doc["tests"].as_array().unwrap();
        let artifact_tests = artifact_doc["tests"].as_array().unwrap();
        assert_eq!(stdout_tests.len(), artifact_tests.len(), "{context}");
        for (s_test, a_test) in stdout_tests.iter().zip(artifact_tests) {
            assert_eq!(s_test["id"], a_test["id"], "{context}");
            assert_eq!(s_test["status"], a_test["status"], "{context}");

            let s_actions = s_test["actions"].as_array().unwrap();
            let a_actions = a_test["actions"].as_array().unwrap();
            assert_eq!(s_actions.len(), a_actions.len(), "{context}");
            for (s_action, a_action) in s_actions.iter().zip(a_actions) {
                assert_eq!(s_action["id"], a_action["id"], "{context}");
                assert_eq!(s_action["exitCode"], a_action["exitCode"], "{context}");
                for stream in ["stdout", "stderr"] {
                    for field in ["artifactRef", "sizeBytes"] {
                        assert_eq!(
                            s_action[stream][field], a_action[stream][field],
                            "{context}: actions[].{stream}.{field}"
                        );
                    }
                }
            }

            let s_assertions = s_test["assertions"].as_array().unwrap();
            let a_assertions = a_test["assertions"].as_array().unwrap();
            assert_eq!(s_assertions.len(), a_assertions.len(), "{context}");
            for (s_assertion, a_assertion) in s_assertions.iter().zip(a_assertions) {
                assert_eq!(s_assertion["id"], a_assertion["id"], "{context}");
                assert_eq!(s_assertion["status"], a_assertion["status"], "{context}");
                assert_eq!(
                    s_assertion["expectation"]["kind"], a_assertion["expectation"]["kind"],
                    "{context}"
                );
                assert_eq!(
                    s_assertion["expectation"]["status"], a_assertion["expectation"]["status"],
                    "{context}"
                );
            }
        }
    }
}

#[test]
fn stdout_projection_is_the_artifact_result_minus_the_defined_differences() {
    for path in fixture_paths() {
        let (run_dir, stdout_doc, _exit_code, _dir) = run_fixture(&path, true);
        let stdout_doc = stdout_doc.unwrap();
        let mut artifact_doc = read_result_json(&run_dir);

        // Apply the projection differences documented in spec/artifacts/run-result/README.md to the canonical document; the outcome must be the stdout document exactly.
        let object = artifact_doc.as_object_mut().unwrap();
        object.remove("noop").expect("result.json must carry noop");
        object.insert(
            "artifactRoot".to_string(),
            stdout_doc["artifactRoot"].clone(),
        );
        for test in artifact_doc["tests"].as_array_mut().unwrap() {
            for action in test["actions"].as_array_mut().unwrap() {
                for stream in ["stdout", "stderr"] {
                    action[stream]
                        .as_object_mut()
                        .unwrap()
                        .remove("sha256")
                        .expect("result.json evidence references must carry sha256");
                }
            }
        }

        assert_eq!(
            artifact_doc,
            stdout_doc,
            "fixture {}: the stdout document must be derivable from result.json by the defined projection",
            path.display()
        );
    }
}

// ---------------------------------------------------------------------------
// Docs drift check
//
// docs/artifacts.md marks each fixture-derived example with a `<!-- checked-against: `<repo-relative snapshot path>` -->` comment directly above a ```json fence.
// Those examples are the "checked" sections of the generated / checked / handwritten boundary defined in docs/artifacts.md: this test fails when an example drifts from the snapshot it claims to mirror.
// ---------------------------------------------------------------------------

#[test]
fn docs_artifacts_examples_match_their_fixture_snapshots() {
    let docs_path = repo_root().join("docs/artifacts.md");
    let docs = std::fs::read_to_string(&docs_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", docs_path.display()));

    let mut checked = 0usize;
    let mut lines = docs.lines().peekable();
    while let Some(line) = lines.next() {
        let Some(marker) = line.trim().strip_prefix("<!-- checked-against:") else {
            continue;
        };
        let snapshot_rel = marker
            .trim()
            .trim_end_matches("-->")
            .trim()
            .trim_matches('`');

        assert_eq!(
            lines.next().map(str::trim),
            Some("```json"),
            "docs/artifacts.md: a checked-against marker must be immediately followed by a ```json fence ({snapshot_rel})"
        );
        let mut example = String::new();
        for fence_line in lines.by_ref() {
            if fence_line.trim() == "```" {
                break;
            }
            example.push_str(fence_line);
            example.push('\n');
        }

        let snapshot_path = repo_root().join(snapshot_rel);
        let snapshot = std::fs::read_to_string(&snapshot_path).unwrap_or_else(|e| {
            panic!(
                "docs/artifacts.md references snapshot {} which cannot be read: {e}",
                snapshot_path.display()
            )
        });
        assert_eq!(
            example, snapshot,
            "docs/artifacts.md: the example marked checked-against {snapshot_rel} has drifted from the snapshot; update the docs example to match"
        );
        checked += 1;
    }

    assert!(
        checked > 0,
        "docs/artifacts.md must contain at least one checked-against example"
    );
}
