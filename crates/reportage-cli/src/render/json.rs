//! The structured JSON execution report renderer (`--format=json`).
//!
//! `JsonRenderer` turns an `ExecutionReport` into the external, camelCase JSON document
//! described in issue #75: `schemaVersion` / `tool` / `status` / `processExitCode` /
//! `artifactRoot` / `summary` / `diagnostics[]` / `tests[]`. Like [`super::human::HumanRenderer`],
//! it only *reads* the report; it never re-derives pass/fail from message text, and the runner
//! (`evaluator`, `executor`) has no knowledge this renderer exists.
//!
//! ## CLI stdout vs. captured stdout / captured stderr
//!
//! `CLI stdout` here means this process's own standard output: the single JSON document this
//! renderer prints, and nothing else. It is a distinct concept from `captured stdout` /
//! `captured stderr` — the output of an action's `$ ...` command, recorded on `ActionResult`.
//! Captured output is never inlined into this JSON document; `tests[].actions[].stdout` /
//! `.stderr`, and any `stdoutContains` / `stderrContains` / `stdoutEmpty` / `stderrEmpty`
//! assertion's `actualRef`, only reference it by relative path (`artifactRef`) under
//! `artifactRoot`. The referenced files are written by
//! `reportage_core::artifact::ArtifactWriter::write`. Confusing the two would either leak
//! captured action output onto this process's stdout (breaking the "single JSON document"
//! contract) or bloat the JSON document with raw action output.

use std::path::Path;

use reportage_core::artifact::{action_id, test_id};
use reportage_core::diagnostic::DiagnosticCode;
use reportage_core::result::{
    ActionResult, CaseResult, CaseStatus, DirContainsObservation, DirExistsObservation,
    ExecutionReport, ExpectationKind, ExpectationResult, FileContentObservation, FileErrorKind,
    FileExistsObservation,
};
use serde_json::{Value, json};

use super::OutputRenderer;

pub struct JsonRenderer {
    artifact_root: std::path::PathBuf,
}

impl JsonRenderer {
    pub fn new(artifact_root: std::path::PathBuf) -> Self {
        Self { artifact_root }
    }
}

impl OutputRenderer for JsonRenderer {
    fn render(&self, report: &ExecutionReport) {
        let document = build_document(report, &self.artifact_root);
        println!(
            "{}",
            serde_json::to_string_pretty(&document)
                .expect("JSON execution report serialization should not fail")
        );
    }
}

/// Accumulates `diagnostics[]` entries and assigns each one a document-local id
/// (`diagnostic-1`, `diagnostic-2`, ...) in the order they are pushed.
///
/// See docs/semantic-diagnostics.md for the `category` / `severity` / `code` model this
/// mirrors, and issue #75's "Document-local ids" section for the id stability policy:
/// stable within one document, not a long-term stable identifier.
struct DiagnosticsBuilder {
    entries: Vec<Value>,
}

impl DiagnosticsBuilder {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Pushes one diagnostic and returns its document-local id for a caller
    /// (e.g. an assertion) to reference via `diagnosticRef`.
    fn push(
        &mut self,
        category: &str,
        code: Option<DiagnosticCode>,
        severity: &str,
        message: &str,
        origin: Value,
    ) -> String {
        let id = format!("diagnostic-{}", self.entries.len() + 1);
        let mut entry = json!({
            "id": id,
            "category": category,
            "severity": severity,
            "message": message,
            "origin": origin,
            // Source ranges (line/column) are not yet tracked for case-level and
            // assertion-level diagnostics; see follow-up issue #89.
            "location": Value::Null,
        });
        if let Some(code) = code {
            entry["code"] = json!(code.as_str());
        }
        self.entries.push(entry);
        id
    }
}

fn build_document(report: &ExecutionReport, artifact_root: &Path) -> Value {
    let mut diagnostics = DiagnosticsBuilder::new();

    for file_error in &report.file_errors {
        let origin = json!({
            "kind": "source",
            "source": file_error.source_path.display().to_string(),
        });
        match &file_error.kind {
            FileErrorKind::ReadError(message) => {
                // A read failure predates parsing, so no `parse.*` / `semantic.*` code
                // applies; it does not fit `parse` / `semantic` / `runtime` / `assertion`
                // either, since it is neither a script-domain failure nor an action
                // execution infrastructure failure. See issue #75's allowance for an
                // `internal` category alongside the four required ones.
                diagnostics.push("internal", None, "error", message, origin);
            }
            FileErrorKind::ParseError {
                message,
                diagnostic_code,
            } => {
                diagnostics.push("parse", Some(*diagnostic_code), "error", message, origin);
            }
        }
    }

    let tests: Vec<Value> = report
        .cases
        .iter()
        .enumerate()
        .map(|(case_index, case)| case_json(case_index, case, &mut diagnostics))
        .collect();

    json!({
        "schemaVersion": 1,
        "tool": {
            "name": "reportage",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "status": top_level_status(report),
        "processExitCode": report.exit_code(),
        "artifactRoot": artifact_root.display().to_string(),
        "summary": summary_json(report),
        "diagnostics": diagnostics.entries,
        "tests": tests,
    })
}

/// Top-level `status`, precedence `error > failed > passed` (see issue #75).
fn top_level_status(report: &ExecutionReport) -> &'static str {
    if !report.file_errors.is_empty() {
        return "error";
    }
    let mut has_failed = false;
    for case in &report.cases {
        match &case.status {
            CaseStatus::ScriptError(_) | CaseStatus::RuntimeError(_) => return "error",
            CaseStatus::Fail => has_failed = true,
            CaseStatus::Pass => {}
        }
    }
    if has_failed { "failed" } else { "passed" }
}

fn summary_json(report: &ExecutionReport) -> Value {
    // Distinct source files, not concrete cases: a single script file can produce more than
    // one concrete case, and "scripts" is meant to count the former.
    let mut scripts = std::collections::BTreeSet::new();
    let mut actions = 0usize;
    let mut assertions = 0usize;
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut errors = report.file_errors.len();

    for case in &report.cases {
        if let Some(path) = &case.source_path {
            scripts.insert(path.clone());
        }
        actions += case.actions.len();
        assertions += case
            .assertion_blocks
            .iter()
            .map(|block| block.expectations.len())
            .sum::<usize>();
        match &case.status {
            CaseStatus::Pass => passed += 1,
            CaseStatus::Fail => failed += 1,
            CaseStatus::ScriptError(_) | CaseStatus::RuntimeError(_) => errors += 1,
        }
    }

    // Fall back to the case count when no case carries a source path (defensive; every
    // case evaluated through the CLI's `run_scripts` currently has one).
    let scripts_count = if scripts.is_empty() {
        report.cases.len()
    } else {
        scripts.len()
    };

    json!({
        "scripts": scripts_count,
        "actions": actions,
        "assertions": assertions,
        "passed": passed,
        "failed": failed,
        "errors": errors,
    })
}

fn case_json(case_index: usize, case: &CaseResult, diagnostics: &mut DiagnosticsBuilder) -> Value {
    let id = test_id(case_index);

    let status = match &case.status {
        CaseStatus::Pass => "passed",
        CaseStatus::Fail => "failed",
        CaseStatus::ScriptError(_) | CaseStatus::RuntimeError(_) => "error",
    };

    let origin = match &case.source_path {
        Some(path) => json!({ "kind": "source", "source": path.display().to_string() }),
        None => json!({ "kind": "test", "test": id }),
    };

    match &case.status {
        CaseStatus::ScriptError(err) => {
            // `ParseMissingAssertionBlock` is the one parse-domain code detected at
            // evaluation time rather than by the parser itself; every other `ScriptError`
            // site carries a `semantic.*` code. See `evaluator::evaluate_case`.
            let category = match err.diagnostic_code {
                Some(DiagnosticCode::ParseMissingAssertionBlock) => "parse",
                _ => "semantic",
            };
            diagnostics.push(
                category,
                err.diagnostic_code,
                "error",
                &err.message,
                origin.clone(),
            );
        }
        CaseStatus::RuntimeError(err) => {
            diagnostics.push(
                "runtime",
                err.diagnostic_code,
                "error",
                &err.message,
                origin.clone(),
            );
        }
        CaseStatus::Pass | CaseStatus::Fail => {}
    }

    let actions: Vec<Value> = case
        .actions
        .iter()
        .enumerate()
        .map(|(action_index, action)| action_json(&id, action_index, action))
        .collect();

    let mut assertions = Vec::new();
    let mut assertion_counter = 0usize;
    for block in &case.assertion_blocks {
        for expectation in &block.expectations {
            assertion_counter += 1;
            let assertion_id = format!("assertion-{assertion_counter}");
            assertions.push(assertion_json(
                &id,
                &assertion_id,
                block.checkpoint_action_index,
                expectation,
                diagnostics,
            ));
        }
    }

    json!({
        "id": id,
        "name": case.name,
        "path": case.source_path.as_ref().map(|p| p.display().to_string()),
        "status": status,
        "actions": actions,
        "assertions": assertions,
    })
}

fn action_json(test_id: &str, action_index: usize, action: &ActionResult) -> Value {
    let id = action_id(action_index);
    let mut value = json!({
        "id": id,
        "command": action.command,
        "exitCode": action.exit_code,
        "stdout": stream_artifact_json(test_id, &id, "stdout", action.stdout.len()),
        "stderr": stream_artifact_json(test_id, &id, "stderr", action.stderr.len()),
    });

    if !action.shim_invocations.is_empty() {
        value["shimInvocations"] = json!(
            action
                .shim_invocations
                .iter()
                .map(|ev| json!({
                    "schemaVersion": ev.schema_version,
                    "commandName": ev.command_name,
                    "shimPath": ev.shim_path.display().to_string(),
                    "target": {
                        "program": ev.target.program.display().to_string(),
                        "args": ev.target.args,
                    },
                    "forwardsCallerArgs": ev.forwards_caller_args,
                }))
                .collect::<Vec<_>>()
        );
    }
    if !action.shim_event_parse_warnings.is_empty() {
        value["shimEventParseWarnings"] = json!(action.shim_event_parse_warnings);
    }

    value
}

/// The artifact reference for one action's captured stdout or stderr: a relative path under
/// `artifactRoot` (`<test_id>/<action_id>/<stream>.bin`) plus the byte size, never the raw
/// bytes themselves. See the module-level "CLI stdout vs. captured stdout/stderr" note.
fn stream_artifact_json(test_id: &str, action_id: &str, stream: &str, size_bytes: usize) -> Value {
    json!({
        "artifactRef": format!("{test_id}/{action_id}/{stream}.bin"),
        "sizeBytes": size_bytes,
    })
}

fn assertion_json(
    test_id: &str,
    assertion_id: &str,
    checkpoint_action_index: Option<usize>,
    expectation: &ExpectationResult,
    diagnostics: &mut DiagnosticsBuilder,
) -> Value {
    let checkpoint = match checkpoint_action_index {
        Some(idx) => action_id(idx),
        None => "initial".to_string(),
    };

    let mut value = json!({
        "id": assertion_id,
        "status": if expectation.passed { "passed" } else { "failed" },
        "checkpoint": checkpoint,
        "expectation": expectation_json(test_id, checkpoint_action_index, expectation, diagnostics),
    });

    // The top-level assertion's `diagnosticRef` mirrors its own expectation node's, for
    // callers that only look at `tests[].assertions[]` without descending into
    // `expectation` (e.g. a non-`logical` assertion, the common case).
    if let Some(diagnostic_ref) = value["expectation"].get("diagnosticRef").cloned() {
        value["diagnosticRef"] = diagnostic_ref;
    }

    value
}

/// Builds the `expectation` object for one evaluated expectation, recursing into a `not` /
/// `all` / `any` composition's own children (each already an independently evaluated
/// `ExpectationResult`; see `ExpectationKind::Logical`) so a nested failure's own code and
/// message are never lost. Registers a `diagnostics[]` entry — and attaches its id as
/// `diagnosticRef` — for every failing node (leaf or, in principle, composition) that has a
/// stable diagnostic code; today only leaves do, since `Logical` itself carries none.
fn expectation_json(
    test_id: &str,
    checkpoint_action_index: Option<usize>,
    expectation: &ExpectationResult,
    diagnostics: &mut DiagnosticsBuilder,
) -> Value {
    let status = if expectation.passed {
        "passed"
    } else {
        "failed"
    };
    let action_ref = checkpoint_action_index.map(action_id);

    let mut value = match &expectation.kind {
        ExpectationKind::Exit { expected, actual } => json!({
            "kind": "exit",
            "expected": expected,
            "actual": actual,
        }),
        ExpectationKind::StdoutContains { expected, actual } => stream_expectation_json(
            "stdoutContains",
            "stdout",
            test_id,
            action_ref.as_deref(),
            Some(expected),
            actual,
        ),
        ExpectationKind::StderrContains { expected, actual } => stream_expectation_json(
            "stderrContains",
            "stderr",
            test_id,
            action_ref.as_deref(),
            Some(expected),
            actual,
        ),
        ExpectationKind::StdoutEmpty { actual } => stream_expectation_json(
            "stdoutEmpty",
            "stdout",
            test_id,
            action_ref.as_deref(),
            None,
            actual,
        ),
        ExpectationKind::StderrEmpty { actual } => stream_expectation_json(
            "stderrEmpty",
            "stderr",
            test_id,
            action_ref.as_deref(),
            None,
            actual,
        ),
        ExpectationKind::FileExists { path, observation } => json!({
            "kind": "fileExists",
            "path": path,
            "observed": file_exists_observation_str(*observation),
        }),
        ExpectationKind::FileContains {
            path,
            expected,
            observation,
        } => json!({
            "kind": "fileContains",
            "path": path,
            "expected": expected,
            "observed": file_content_observation_str(*observation),
        }),
        ExpectationKind::DirExists { path, observation } => json!({
            "kind": "dirExists",
            "path": path,
            "observed": dir_exists_observation_str(*observation),
        }),
        ExpectationKind::DirContains {
            path,
            expected_entry,
            observation,
        } => json!({
            "kind": "dirContains",
            "path": path,
            "expectedEntry": expected_entry,
            "observed": dir_contains_observation_str(*observation),
        }),
        ExpectationKind::Logical { operator, children } => {
            let children_json: Vec<Value> = children
                .iter()
                .map(|child| expectation_json(test_id, checkpoint_action_index, child, diagnostics))
                .collect();
            json!({
                "kind": "logical",
                "operator": operator.keyword(),
                "children": children_json,
            })
        }
    };

    value["status"] = json!(status);

    if let Some(code) = expectation.failure_diagnostic_code() {
        let origin = json!({ "kind": "test", "test": test_id });
        let message = assertion_failure_message(&expectation.kind, code);
        let diagnostic_id = diagnostics.push("assertion", Some(code), "failure", &message, origin);
        value["diagnosticRef"] = json!(diagnostic_id);
    }

    value
}

fn stream_expectation_json(
    kind: &str,
    stream: &str,
    test_id: &str,
    action_ref: Option<&str>,
    expected: Option<&String>,
    actual: &[u8],
) -> Value {
    let mut value = json!({ "kind": kind });
    if let Some(expected) = expected {
        value["expected"] = json!(expected);
    }
    if let Some(action_ref) = action_ref {
        value["actualRef"] = json!(format!("{test_id}/{action_ref}/{stream}.bin"));
    }
    value["actualSizeBytes"] = json!(actual.len());
    value
}

fn assertion_failure_message(kind: &ExpectationKind, code: DiagnosticCode) -> String {
    match kind {
        ExpectationKind::Exit { expected, actual } => {
            format!("expected exit code {expected}, but got {actual}")
        }
        ExpectationKind::StdoutContains { expected, .. } => {
            format!("stdout did not contain expected substring {expected:?}")
        }
        ExpectationKind::StderrContains { expected, .. } => {
            format!("stderr did not contain expected substring {expected:?}")
        }
        ExpectationKind::StdoutEmpty { .. } => "stdout was expected to be empty".to_string(),
        ExpectationKind::StderrEmpty { .. } => "stderr was expected to be empty".to_string(),
        ExpectationKind::FileExists { path, .. } => {
            format!("file {path:?} did not satisfy the exists check")
        }
        ExpectationKind::FileContains { path, expected, .. } => {
            format!("file {path:?} did not contain expected substring {expected:?}")
        }
        ExpectationKind::DirExists { path, .. } => {
            format!("dir {path:?} did not satisfy the exists check")
        }
        ExpectationKind::DirContains {
            path,
            expected_entry,
            ..
        } => {
            format!("dir {path:?} did not contain expected entry {expected_entry:?}")
        }
        // `Logical`'s own `failure_diagnostic_code()` is always `None` (see
        // `ExpectationKind::failure_diagnostic_code`), so this function is never called for it.
        ExpectationKind::Logical { .. } => {
            unreachable!("a `Logical` expectation never has its own diagnostic code: {code}")
        }
    }
}

fn file_exists_observation_str(observation: FileExistsObservation) -> &'static str {
    match observation {
        FileExistsObservation::RegularFile => "regularFile",
        FileExistsObservation::NotRegularFile => "notRegularFile",
        FileExistsObservation::Missing => "missing",
    }
}

fn file_content_observation_str(observation: FileContentObservation) -> &'static str {
    match observation {
        FileContentObservation::Found => "found",
        FileContentObservation::NotFound => "notFound",
        FileContentObservation::Missing => "missing",
        FileContentObservation::NotRegularFile => "notRegularFile",
        FileContentObservation::Unreadable => "unreadable",
        FileContentObservation::NotUtf8 => "notUtf8",
    }
}

fn dir_exists_observation_str(observation: DirExistsObservation) -> &'static str {
    match observation {
        DirExistsObservation::Directory => "directory",
        DirExistsObservation::NotADirectory => "notADirectory",
        DirExistsObservation::Missing => "missing",
    }
}

fn dir_contains_observation_str(observation: DirContainsObservation) -> &'static str {
    match observation {
        DirContainsObservation::Found => "found",
        DirContainsObservation::EntryMissing => "entryMissing",
        DirContainsObservation::SubjectMissing => "subjectMissing",
        DirContainsObservation::SubjectNotADirectory => "subjectNotADirectory",
        DirContainsObservation::SubjectUnreadable => "subjectUnreadable",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reportage_core::result::{AssertionBlockResult, FileError, RuntimeError, ScriptError};
    use std::path::PathBuf;

    fn passing_action() -> ActionResult {
        ActionResult {
            command: "echo hello".to_string(),
            exit_code: 0,
            stdout: b"hello\n".to_vec(),
            stderr: vec![],
            shim_invocations: vec![],
            shim_event_parse_warnings: vec![],
        }
    }

    fn report_with_cases(cases: Vec<CaseResult>) -> ExecutionReport {
        ExecutionReport {
            cases,
            file_errors: vec![],
        }
    }

    #[test]
    fn passed_case_produces_passed_status_and_artifact_references() {
        let case = CaseResult {
            name: "greets".to_string(),
            source_path: Some(PathBuf::from("hello.repor")),
            status: CaseStatus::Pass,
            actions: vec![passing_action()],
            assertion_blocks: vec![AssertionBlockResult {
                step_index: 1,
                checkpoint_action_index: Some(0),
                expectations: vec![ExpectationResult {
                    kind: ExpectationKind::StdoutContains {
                        expected: "hello".to_string(),
                        actual: b"hello\n".to_vec(),
                    },
                    passed: true,
                }],
            }],
            side_effects_executed: 0,
        };
        let report = report_with_cases(vec![case]);

        let doc = build_document(&report, Path::new(".reportage/runs/1"));

        assert_eq!(doc["status"], "passed");
        assert_eq!(doc["processExitCode"], 0);
        assert_eq!(doc["schemaVersion"], 1);
        assert!(doc["diagnostics"].as_array().unwrap().is_empty());
        assert_eq!(doc["tests"][0]["id"], "test-1");
        assert_eq!(doc["tests"][0]["status"], "passed");
        assert_eq!(
            doc["tests"][0]["actions"][0]["stdout"]["artifactRef"],
            "test-1/action-1/stdout.bin"
        );
        assert_eq!(doc["tests"][0]["actions"][0]["stdout"]["sizeBytes"], 6);
        // Captured bytes are never inlined: only artifactRef + sizeBytes are present.
        assert!(
            doc["tests"][0]["actions"][0]["stdout"]
                .get("data")
                .is_none()
        );
        assert_eq!(
            doc["tests"][0]["assertions"][0]["expectation"]["actualRef"],
            "test-1/action-1/stdout.bin"
        );
        assert!(
            doc["tests"][0]["assertions"][0]["expectation"]
                .get("actual")
                .is_none()
        );
    }

    #[test]
    fn assertion_failure_produces_failed_status_and_assertion_diagnostic() {
        let case = CaseResult {
            name: "wrong".to_string(),
            source_path: Some(PathBuf::from("fail.repor")),
            status: CaseStatus::Fail,
            actions: vec![passing_action()],
            assertion_blocks: vec![AssertionBlockResult {
                step_index: 1,
                checkpoint_action_index: Some(0),
                expectations: vec![ExpectationResult {
                    kind: ExpectationKind::StdoutContains {
                        expected: "world".to_string(),
                        actual: b"hello\n".to_vec(),
                    },
                    passed: false,
                }],
            }],
            side_effects_executed: 0,
        };
        let report = report_with_cases(vec![case]);

        let doc = build_document(&report, Path::new(".reportage/runs/1"));

        assert_eq!(doc["status"], "failed");
        assert_eq!(doc["processExitCode"], 1);
        let diagnostics = doc["diagnostics"].as_array().unwrap();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0]["category"], "assertion");
        assert_eq!(diagnostics[0]["severity"], "failure");
        assert_eq!(diagnostics[0]["code"], "assertion.stdout.contains_mismatch");
        assert_eq!(
            doc["tests"][0]["assertions"][0]["diagnosticRef"],
            "diagnostic-1"
        );
    }

    #[test]
    fn parse_error_produces_error_status_with_parse_category() {
        let mut report = report_with_cases(vec![]);
        report.file_errors.push(FileError {
            source_path: PathBuf::from("broken.repor"),
            kind: FileErrorKind::ParseError {
                message: "parse error at line 1".to_string(),
                diagnostic_code: DiagnosticCode::ParseSyntax,
            },
        });

        let doc = build_document(&report, Path::new(".reportage/runs/1"));

        assert_eq!(doc["status"], "error");
        assert_eq!(doc["processExitCode"], 2);
        assert_eq!(doc["diagnostics"][0]["category"], "parse");
        assert_eq!(doc["diagnostics"][0]["code"], "parse.syntax");
        assert_eq!(doc["diagnostics"][0]["severity"], "error");
    }

    #[test]
    fn read_error_produces_error_status_with_internal_category_and_no_code() {
        let mut report = report_with_cases(vec![]);
        report.file_errors.push(FileError {
            source_path: PathBuf::from("missing.repor"),
            kind: FileErrorKind::ReadError("No such file or directory".to_string()),
        });

        let doc = build_document(&report, Path::new(".reportage/runs/1"));

        assert_eq!(doc["status"], "error");
        assert_eq!(doc["diagnostics"][0]["category"], "internal");
        assert!(doc["diagnostics"][0].get("code").is_none());
    }

    #[test]
    fn semantic_script_error_produces_error_status_with_semantic_category() {
        let case = CaseResult {
            name: "no action yet".to_string(),
            source_path: Some(PathBuf::from("noaction.repor")),
            status: CaseStatus::ScriptError(ScriptError {
                message: "uses a process expectation but no action has run yet".to_string(),
                diagnostic_code: Some(DiagnosticCode::SemanticExpectationRequiresAction),
                step_index: Some(0),
            }),
            actions: vec![],
            assertion_blocks: vec![],
            side_effects_executed: 0,
        };
        let report = report_with_cases(vec![case]);

        let doc = build_document(&report, Path::new(".reportage/runs/1"));

        assert_eq!(doc["status"], "error");
        assert_eq!(doc["processExitCode"], 2);
        assert_eq!(doc["tests"][0]["status"], "error");
        assert_eq!(doc["diagnostics"][0]["category"], "semantic");
        assert_eq!(
            doc["diagnostics"][0]["code"],
            "semantic.expectation.requires_action"
        );
    }

    #[test]
    fn runtime_error_produces_error_status_with_runtime_category() {
        let case = CaseResult {
            name: "write conflict".to_string(),
            source_path: Some(PathBuf::from("runtimeerr.repor")),
            status: CaseStatus::RuntimeError(RuntimeError {
                message: "write step at step 2 failed: target path already exists".to_string(),
                diagnostic_code: Some(DiagnosticCode::StepWriteTargetExists),
                step_index: Some(1),
            }),
            actions: vec![],
            assertion_blocks: vec![],
            side_effects_executed: 1,
        };
        let report = report_with_cases(vec![case]);

        let doc = build_document(&report, Path::new(".reportage/runs/1"));

        assert_eq!(doc["status"], "error");
        assert_eq!(doc["processExitCode"], 3);
        assert_eq!(doc["diagnostics"][0]["category"], "runtime");
        assert_eq!(doc["diagnostics"][0]["code"], "step.write.target_exists");
    }

    #[test]
    fn status_precedence_prefers_error_over_failed() {
        let failed_case = CaseResult {
            name: "fails".to_string(),
            source_path: Some(PathBuf::from("a.repor")),
            status: CaseStatus::Fail,
            actions: vec![],
            assertion_blocks: vec![],
            side_effects_executed: 0,
        };
        let error_case = CaseResult {
            name: "errors".to_string(),
            source_path: Some(PathBuf::from("b.repor")),
            status: CaseStatus::RuntimeError(RuntimeError {
                message: "boom".to_string(),
                diagnostic_code: None,
                step_index: None,
            }),
            actions: vec![],
            assertion_blocks: vec![],
            side_effects_executed: 0,
        };
        let report = report_with_cases(vec![failed_case, error_case]);

        let doc = build_document(&report, Path::new(".reportage/runs/1"));

        assert_eq!(doc["status"], "error");
        assert_eq!(doc["summary"]["failed"], 1);
        assert_eq!(doc["summary"]["errors"], 1);
    }

    #[test]
    fn artifact_root_is_reflected_verbatim() {
        let report = report_with_cases(vec![]);
        let doc = build_document(&report, Path::new(".reportage/runs/42"));
        assert_eq!(doc["artifactRoot"], ".reportage/runs/42");
    }
}
