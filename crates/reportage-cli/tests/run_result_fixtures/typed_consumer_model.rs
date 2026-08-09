//! The Rust shape this repository's consumers read the artifact `result.json` manifest through.
//!
//! This is a consumer compatibility check, not schema validation: deserializing proves that a
//! repository-internal consumer can still read what the producer writes, and it enforces none of
//! the schema's `const`, `pattern`, `minimum`, or conditional requirements. Instance conformance
//! is `support/json_schema.rs`; the invariants no schema can state are `support/invariants.rs`.
//! See docs/adr/20260728T092956Z_json-contract-validation-policy.md.
//!
//! Unlike `json_report_fixtures.rs`, whose structs deliberately model only the expectation kinds
//! its fixtures exercise, the structs here model the *full* stable contract the schema defines,
//! because `result.json` is the canonical manifest of a run: a shape no fixture produces must
//! still be visible in review (issue #102).
//!
//! Only [`assert_deserializes`] is visible to the test target. The types below are how this check
//! is implemented, not a second statement of the contract for other suites to assert against:
//! `spec/artifacts/run-result/schema.internal.json` is where the contract is stated.

// Serde-populated struct fields are not "used" in the conventional sense; their value comes from
// deserialisation rather than direct assignment.
// Mirrors json_report_fixtures.rs.
#![allow(dead_code)]

use serde::Deserialize;
use serde_json::Value;

/// Asserts `document` deserializes into the typed model of the full stable contract.
///
/// `context` names the fixture the document came from, and prefixes a deserialization failure.
pub(super) fn assert_deserializes(document: &Value, context: &str) {
    // Round-tripped through text rather than deserialised from the value directly, so a failure can
    // print the document that caused it next to serde's path into it.
    let text = serde_json::to_string(document).unwrap();
    let doc: RunResultDocument = serde_json::from_str(&text).unwrap_or_else(|e| {
        panic!(
            "fixture {context} produced a result.json the typed Rust consumer model cannot deserialize: {e}\n{text}"
        )
    });
    for diagnostic in &doc.diagnostics {
        assert_location_shape_is_valid(&diagnostic.location);
    }
}

// ---------------------------------------------------------------------------
// Typed representation of the artifact result document
//
// One struct/enum per shape in spec/artifacts/run-result/schema.internal.json, the hand-edited
// source of the generated public spec/artifacts/run-result/schema.json, in the same order.
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
    /// Present only when the diagnostic is attributed to a step.
    #[serde(default)]
    step: Option<StepOrigin>,
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

/// Mirrors the schema's `StepOrigin`: which block a step belongs to, and its
/// 0-based position within that block. Deserialized rather than ignored so a
/// consumer model proves the phase enum stays closed to these two values.
/// Mirrors `json_report_fixtures.rs`.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct StepOrigin {
    phase: StepPhase,
    index: usize,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum StepPhase {
    BeforeEach,
    Case,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Action {
    id: String,
    command: String,
    step: StepOrigin,
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
    step: StepOrigin,
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
    Quoted {
        value: String,
    },
    Heredoc {
        value: String,
    },
    Binding {
        name: String,
        action_index: u64,
        stream: String,
        capture_mode: String,
    },
    #[serde(rename_all = "camelCase")]
    Interpolated {
        form: String,
        line: u64,
        column: u64,
        references: Vec<InterpolatedBindingReference>,
    },
}

/// One binding an interpolated expected value substituted. This object records
/// only where each part came from, never the resolved value that combines
/// script text with captured process output; a failing comparison's bounded
/// mismatch context remains the only field showing any part of it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct InterpolatedBindingReference {
    name: String,
    line: u64,
    column: u64,
    action_index: u64,
    stream: String,
    capture_mode: String,
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

/// The full expectation contract of `spec/artifacts/run-result/schema.json`.
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
        expected_source: TextExpectedSource,
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
        expected_source: TextExpectedSource,
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
        expected_source: TextExpectedSource,
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
    StdoutTextEquals {
        status: Status,
        expected_source: TextExpectedSource,
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
    StderrTextEquals {
        status: Status,
        expected_source: TextExpectedSource,
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

// The schema's conditional requirements — `observed: compared` requiring the comparison fields,
// `outcome: mismatch` requiring `mismatch` — used to be re-asserted here because
// `deny_unknown_fields` cannot express them. They are now enforced where they are declared, by
// validating the manifest against the schema's `if`/`then` keywords, and exercised directly by
// `json_contract_schemas.rs`. Restating them in Rust would give two places to update and one of
// them no way to notice it had fallen behind.
//
// The one requirement that survives here is not a schema constraint at all: a logical
// composition's child must never carry its own `diagnosticRef`. That is a domain invariant, and it
// lives with the other invariants in `support/invariants.rs`.
