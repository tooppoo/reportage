//! Representative-fixture conformance for `reportage run --format=json` (issue #89).
//!
//! Mirrors `crates/reportage-core/tests/semantic_specs.rs`'s approach to "schema validation":
//! each fixture's JSON output is deserialised into typed Rust structs marked
//! `#[serde(deny_unknown_fields)]`, rather than run through an external JSON Schema validator.
//! See `spec/output/json-report/schema.json` for the authoritative external contract these
//! structs are a CI-enforced subset of (only the `exit` / `stdoutContains` expectation kinds
//! these six fixtures exercise are modelled here; the schema itself covers all thirteen kinds).
//!
//! Fixtures live in `tests/fixtures/json_report/*.repor`, one per representative scenario
//! required by issue #89. Each has a companion `<name>.snapshot.json` with volatile fields
//! (`artifactRoot`, `tool.version`) normalised out, refreshed via `UPDATE_JSON_REPORT_SNAPSHOTS`,
//! mirroring `syntax_conformance.rs`'s `UPDATE_AST_SNAPSHOTS` convention.

// Serde-populated struct fields are not "used" in the conventional sense; their value comes
// from deserialisation rather than direct assignment. Mirrors semantic_specs.rs.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use assert_fs::TempDir;
use assert_fs::prelude::*;
use serde::Deserialize;
use serde_json::Value;

// ---------------------------------------------------------------------------
// Typed representation of the `--format=json` document (schema validation)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct JsonReportDocument {
    schema_version: u32,
    tool: Tool,
    status: TopStatus,
    process_exit_code: i32,
    artifact_root: String,
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
    /// Deliberately not `Option<Location>`: with no `#[serde(default)]`, a required
    /// non-`Option` field makes a *missing* `location` key a deserialization error, distinct
    /// from a *present* `location: null`. An `Option<Location>` would silently accept both,
    /// defeating the point of testing that this field is always present (see
    /// `docs/adr/20260707T050100Z_json-output-schema-and-validation-policy.md`). Its shape
    /// (`null` or a `Location`) is checked separately by `assert_location_shape_is_valid`.
    location: Value,
    code: Option<String>,
}

/// Asserts `location` is JSON `null` or deserializes as a valid `Location`, without collapsing
/// "missing key" and "present but null" the way an `Option<Location>` struct field would.
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
    stdout: StreamArtifact,
    stderr: StreamArtifact,
    #[serde(default)]
    shim_invocations: Vec<Value>,
    #[serde(default)]
    shim_event_parse_warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StreamArtifact {
    artifact_ref: String,
    size_bytes: u64,
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

/// Only the `exit` and `stdoutContains` kinds these six fixtures exercise. The full 12-kind
/// contract is `spec/output/json-report/schema.json`'s responsibility, not this test's.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum Expectation {
    // `rename_all` on the enum itself only renames variant names, not the fields inside a
    // struct-like variant, so each variant needs its own `rename_all` for its fields.
    #[serde(rename_all = "camelCase")]
    Exit {
        status: Status,
        expected: u8,
        actual: i32,
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
}

// ---------------------------------------------------------------------------
// Fixture / CLI helpers
// ---------------------------------------------------------------------------

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture_dir() -> PathBuf {
    repo_root().join("tests/fixtures/json_report")
}

fn fixture_paths() -> Vec<PathBuf> {
    let pattern = fixture_dir()
        .join("*.repor")
        .to_str()
        .expect("fixture glob path must be valid UTF-8")
        .to_string();

    let mut paths = glob::glob(&pattern)
        .expect("json_report fixture glob pattern must be valid")
        .map(|entry| entry.expect("json_report fixture path must be readable"))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn fixture_stem(path: &Path) -> &str {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .expect("json_report fixture file name must be valid UTF-8")
}

fn snapshot_path_for_fixture(path: &Path) -> PathBuf {
    path.with_extension("snapshot.json")
}

fn update_snapshots_enabled() -> bool {
    std::env::var_os("UPDATE_JSON_REPORT_SNAPSHOTS").is_some()
}

/// Copies `fixture` into a fresh temp dir (under its own file name) and runs
/// `reportage --format=json <file>` there, returning parsed stdout and the process exit code.
fn run_json(fixture: &Path) -> (Value, i32, TempDir) {
    let dir = TempDir::new().unwrap();
    let name = fixture.file_name().unwrap().to_str().unwrap();
    let content = std::fs::read_to_string(fixture).unwrap();
    dir.child(name).write_str(&content).unwrap();

    let output = Command::cargo_bin("reportage")
        .unwrap()
        .current_dir(&dir)
        .arg("--format")
        .arg("json")
        .arg(name)
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout was not a single valid JSON document: {e}\n{stdout}"));
    (json, output.status.code().unwrap(), dir)
}

/// Runs the same fixture through the default (human-readable) renderer, returning combined
/// stdout+stderr text. Used only for the semantic-parity checks below.
///
/// Uses its own fresh temp dir rather than reusing `run_json`'s: some fixtures use create-only
/// `write` steps, so replaying the fixture a second time in a directory that already has the
/// first run's output on disk would change which step fails and could mask a real regression in
/// the parity check.
fn run_human(fixture: &Path) -> String {
    let dir = TempDir::new().unwrap();
    let name = fixture.file_name().unwrap().to_str().unwrap();
    let content = std::fs::read_to_string(fixture).unwrap();
    dir.child(name).write_str(&content).unwrap();

    let output = Command::cargo_bin("reportage")
        .unwrap()
        .current_dir(&dir)
        .arg(name)
        .output()
        .unwrap();
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Replaces volatile fields (`artifactRoot`, `tool.version`) with fixed placeholders before
/// snapshotting, per the fixture/snapshot validation policy in
/// docs/adr/20260707T050100Z_json-output-schema-and-validation-policy.md.
fn normalize_for_snapshot(mut doc: Value) -> Value {
    doc["artifactRoot"] = Value::String("<ARTIFACT_ROOT>".to_string());
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
    ];

    let stems: std::collections::BTreeSet<String> = fixture_paths()
        .iter()
        .map(|p| fixture_stem(p).to_string())
        .collect();

    for required in REQUIRED {
        assert!(
            stems.contains(*required),
            "required representative fixture '{required}' is missing from tests/fixtures/json_report/"
        );
    }
    assert_eq!(
        stems.len(),
        REQUIRED.len(),
        "unexpected extra fixture(s) in tests/fixtures/json_report/: {stems:?}"
    );
}

// ---------------------------------------------------------------------------
// Schema validation
// ---------------------------------------------------------------------------

#[test]
fn every_fixture_produces_schema_valid_json() {
    let paths = fixture_paths();
    assert!(
        !paths.is_empty(),
        "expected at least one json_report fixture"
    );

    for path in paths {
        let (json, _exit_code, _dir) = run_json(&path);
        let text = serde_json::to_string(&json).unwrap();
        let doc: JsonReportDocument = serde_json::from_str(&text).unwrap_or_else(|e| {
            panic!(
                "fixture {} produced JSON that does not conform to the typed schema: {e}\n{text}",
                path.display()
            )
        });
        for diagnostic in &doc.diagnostics {
            assert_location_shape_is_valid(&diagnostic.location);
        }
    }
}

// ---------------------------------------------------------------------------
// Snapshot validation
// ---------------------------------------------------------------------------

#[test]
fn snapshots_for_json_report_fixtures_are_current() {
    let paths = fixture_paths();
    assert!(
        !paths.is_empty(),
        "expected at least one json_report fixture"
    );

    let update_snapshots = update_snapshots_enabled();
    for path in paths {
        let (json, _exit_code, _dir) = run_json(&path);
        let normalized = normalize_for_snapshot(json);
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
                 run `UPDATE_JSON_REPORT_SNAPSHOTS=1 cargo test -p reportage-cli --test json_report_fixtures snapshots_for_json_report_fixtures_are_current` to create or refresh snapshots",
                snapshot_path.display()
            )
        });

        assert_eq!(
            expected,
            actual,
            "snapshot for {} is stale; run \
             `UPDATE_JSON_REPORT_SNAPSHOTS=1 cargo test -p reportage-cli --test json_report_fixtures snapshots_for_json_report_fixtures_are_current` \
             and review the JSON diff",
            path.display()
        );
    }
}

// ---------------------------------------------------------------------------
// Human/JSON semantic parity
//
// Per docs/adr/20260707T050100Z_json-output-schema-and-validation-policy.md, this compares
// against the semantic information inventory (status, diagnostic code), not display wording.
// ---------------------------------------------------------------------------

#[test]
fn passed_fixture_is_passed_in_both_renderers() {
    let path = fixture_dir().join("passed.repor");
    let (json, exit_code, _dir) = run_json(&path);
    assert_eq!(json["status"], "passed");
    assert_eq!(exit_code, 0);

    let human = run_human(&path);
    assert!(
        human.contains("PASS"),
        "human-readable output must also report a pass: {human}"
    );
}

#[test]
fn assertion_failure_fixture_reports_the_same_diagnostic_code_in_both_renderers() {
    let path = fixture_dir().join("assertion_failure.repor");
    let (json, exit_code, _dir) = run_json(&path);
    assert_eq!(json["status"], "failed");
    assert_eq!(exit_code, 1);
    let code = json["diagnostics"][0]["code"].as_str().unwrap();
    assert_eq!(code, "assertion.stdout.contains_mismatch");

    let human = run_human(&path);
    assert!(
        human.contains(code),
        "human-readable output must surface the same diagnostic code '{code}': {human}"
    );
}

#[test]
fn parse_error_fixture_reports_the_same_diagnostic_code_and_has_a_location_in_json_only() {
    let path = fixture_dir().join("parse_error.repor");
    let (json, exit_code, _dir) = run_json(&path);
    assert_eq!(json["status"], "error");
    assert_eq!(exit_code, 2);
    let diagnostic = &json["diagnostics"][0];
    assert_eq!(diagnostic["category"], "parse");
    let code = diagnostic["code"].as_str().unwrap();
    assert_eq!(code, "parse.syntax");
    // The one case where `location` is populated: see issue #89 and
    // docs/adr/20260707T050100Z_json-output-schema-and-validation-policy.md.
    assert!(diagnostic["location"].is_object());
    assert!(diagnostic["location"]["line"].as_u64().unwrap() >= 1);

    let human = run_human(&path);
    assert!(
        human.contains(code),
        "human-readable output must surface the same diagnostic code '{code}': {human}"
    );
}

#[test]
fn semantic_error_fixture_reports_the_same_diagnostic_code_with_null_location_in_json() {
    let path = fixture_dir().join("semantic_error.repor");
    let (json, exit_code, _dir) = run_json(&path);
    assert_eq!(json["status"], "error");
    assert_eq!(exit_code, 2);
    let diagnostic = &json["diagnostics"][0];
    assert_eq!(diagnostic["category"], "semantic");
    assert!(
        diagnostic["location"].is_null(),
        "semantic diagnostics fall back to origin, not location, in v0"
    );
    let code = diagnostic["code"].as_str().unwrap();
    assert_eq!(code, "semantic.expectation.requires_action");

    let human = run_human(&path);
    assert!(
        human.contains(code),
        "human-readable output must surface the same diagnostic code '{code}': {human}"
    );
}

#[test]
fn runtime_error_fixture_reports_the_same_diagnostic_code_with_empty_actions_and_assertions() {
    let path = fixture_dir().join("runtime_error.repor");
    let (json, exit_code, _dir) = run_json(&path);
    assert_eq!(json["status"], "error");
    assert_eq!(exit_code, 3);
    let diagnostic = &json["diagnostics"][0];
    assert_eq!(diagnostic["category"], "runtime");
    let code = diagnostic["code"].as_str().unwrap();
    assert_eq!(code, "step.write.target_exists");
    assert!(json["tests"][0]["actions"].as_array().unwrap().is_empty());
    assert!(
        json["tests"][0]["assertions"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let human = run_human(&path);
    assert!(
        human.contains(code),
        "human-readable output must surface the same diagnostic code '{code}': {human}"
    );
}

#[test]
fn partial_execution_fixture_has_error_status_but_retains_prior_action_and_assertion_evidence() {
    let path = fixture_dir().join("partial_execution_after_runtime_error.repor");
    let (json, exit_code, _dir) = run_json(&path);
    assert_eq!(json["status"], "error");
    assert_eq!(exit_code, 3);
    assert_eq!(json["diagnostics"][0]["category"], "runtime");

    // The core requirement of this fixture: at least one action and assertion result were
    // recorded before the later runtime error, and both survive in the JSON document even
    // though the case's overall status is "error".
    let actions = json["tests"][0]["actions"].as_array().unwrap();
    let assertions = json["tests"][0]["assertions"].as_array().unwrap();
    assert!(
        !actions.is_empty(),
        "expected at least one prior action to be recorded"
    );
    assert!(
        !assertions.is_empty(),
        "expected at least one prior assertion result to be recorded"
    );
    assert!(assertions.iter().all(|a| a["status"] == "passed"));
    assert_eq!(json["tests"][0]["status"], "error");

    let human = run_human(&path);
    assert!(
        human.contains("step.write.target_exists"),
        "human-readable output must surface the same diagnostic code: {human}"
    );
    assert!(
        human.contains("ERROR"),
        "human-readable output must still report the case's final ERROR tag: {human}"
    );
}
