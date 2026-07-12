//! The canonical run result document builder (artifact `result.json`).
//!
//! `build_run_result_document` turns an [`ExecutionReport`] into the canonical manifest written to `.reportage/runs/<run-id>/result.json` by [`crate::artifact::ArtifactWriter`].
//! The artifact bundle (`result.json` plus the evidence files it references) is the canonical record of a `reportage run`; the `--format=json` CLI stdout document is a stdout-safe projection derived from this document by the CLI renderer.
//! See `spec/artifacts/run-result/schema.json` for the external contract this builder implements, and docs/adr/20260708T130500Z_artifact-run-result-canonical-manifest.md for the decisions.
//!
//! ## Raw byte evidence is referenced, never inlined
//!
//! Captured stdout/stderr bytes are written as separate artifact files (`<test-id>/<action-id>/{stdout,stderr}.bin`, see [`crate::artifact::ArtifactWriter`]) and referenced from this document as `{ artifactRef, sizeBytes, sha256 }`.
//! `artifactRef` is relative to the directory containing `result.json`, and `sha256` makes it verifiable that the referenced evidence file is the one this manifest describes.

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::artifact::{action_id, test_id};
use crate::contents_diagnostic::mismatch_context;
use crate::diagnostic::{DiagnosticCode, DiagnosticLocation};
use crate::result::{
    ActionResult, CaseResult, CaseStatus, ContentsEqualsComparison, ContentsEqualsExpectedSource,
    ContentsEqualsObservation, ContentsEqualsOutcome, DirContainsObservation, DirExistsObservation,
    ExecutionReport, ExpectationKind, ExpectationResult, FileContentObservation, FileErrorKind,
    FileExistsObservation, TextEqualsExpectedSource,
};

/// Version of the canonical artifact result contract (`spec/artifacts/run-result/schema.json`).
/// Distinct from the `--format=json` stdout contract's own `schemaVersion` (`spec/output/json-report/schema.json`) and from the reportage CLI/crate version.
pub const RUN_RESULT_SCHEMA_VERSION: u32 = 1;

/// Accumulates `diagnostics[]` entries and assigns each one a document-local id
/// (`diagnostic-1`, `diagnostic-2`, ...) in the order they are pushed.
///
/// See docs/reference/semantic-diagnostics.md for the `category` / `severity` / `code` model this
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
    ///
    /// `location` is `Some` only for parse-domain diagnostics, whose `parser::ParseError`
    /// always carries a real line (and, for syntax errors, a column) — see
    /// `parser::ParseError::to_diagnostic`. Every other diagnostic (semantic, runtime,
    /// assertion, internal) passes `None`: source ranges are not yet tracked on the
    /// evaluator side (see issue #89's non-goals), so those diagnostics fall back to
    /// `location: null` plus `origin`.
    fn push(
        &mut self,
        category: &str,
        code: Option<DiagnosticCode>,
        severity: &str,
        message: &str,
        origin: Value,
        location: Option<DiagnosticLocation>,
    ) -> String {
        let id = format!("diagnostic-{}", self.entries.len() + 1);
        let location_json = match location {
            Some(loc) => json!({
                "line": loc.line,
                "column": loc.column,
            }),
            None => Value::Null,
        };
        let mut entry = json!({
            "id": id,
            "category": category,
            "severity": severity,
            "message": message,
            "origin": origin,
            "location": location_json,
        });
        if let Some(code) = code {
            entry["code"] = json!(code.as_str());
        }
        self.entries.push(entry);
        id
    }
}

/// Builds the canonical run result document for `report`.
///
/// The returned document is what `ArtifactWriter::write` persists as `result.json`, and what the CLI's `--format=json` renderer projects its stdout document from.
pub fn build_run_result_document(report: &ExecutionReport) -> Value {
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
                diagnostics.push("internal", None, "error", message, origin, None);
            }
            FileErrorKind::ParseError {
                message,
                diagnostic_code,
                location,
            } => {
                diagnostics.push(
                    "parse",
                    Some(*diagnostic_code),
                    "error",
                    message,
                    origin,
                    *location,
                );
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
        "schemaVersion": RUN_RESULT_SCHEMA_VERSION,
        "tool": {
            "name": "reportage",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "status": top_level_status(report),
        "processExitCode": report.exit_code(),
        "noop": report.summary().noop,
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
                None,
            );
        }
        CaseStatus::RuntimeError(err) => {
            diagnostics.push(
                "runtime",
                err.diagnostic_code,
                "error",
                &err.message,
                origin.clone(),
                None,
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
        "stdout": stream_artifact_json(test_id, &id, "stdout", &action.stdout),
        "stderr": stream_artifact_json(test_id, &id, "stderr", &action.stderr),
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

/// The evidence reference for one action's captured stdout or stderr: a path relative to the directory containing `result.json` (`<test_id>/<action_id>/<stream>.bin`), the byte size, and the SHA-256 digest of the referenced bytes — never the raw bytes themselves.
/// The digest is computed from the same in-memory bytes `ArtifactWriter` writes to the referenced file, so a consumer can verify the bundle's evidence file matches this manifest.
fn stream_artifact_json(test_id: &str, action_id: &str, stream: &str, bytes: &[u8]) -> Value {
    json!({
        "artifactRef": format!("{test_id}/{action_id}/{stream}.bin"),
        "sizeBytes": bytes.len(),
        "sha256": sha256_hex(bytes),
    })
}

/// Lowercase hex SHA-256 digest of `bytes`, the format `result.json` evidence references use.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
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
        "expectation": expectation_json(test_id, checkpoint_action_index, expectation, diagnostics, true),
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
/// `ExpectationResult`; see `ExpectationKind::Logical`) so a nested failure's own detail is
/// never lost — every child's `status` reflects its own raw held/did-not-hold outcome,
/// regardless of the composition's outcome, mirroring the human renderer.
///
/// `attribute_diagnostics` controls whether *this* node may register a `diagnostics[]` entry
/// and attach `diagnosticRef` to itself. It is `true` only for the node passed in directly
/// from [`assertion_json`] (the top of one assertion); every recursive call for a `Logical`
/// composition's children passes `false`. This matters because a child's own `passed` is its
/// raw, never-negated result (see `ExpectationKind::Logical`'s doc comment) — for `not`
/// specifically, a child that "did not hold" is exactly what makes the *composition* hold, so
/// treating that child's own failure code as a document-level diagnostic would report a
/// failure on an otherwise-passing assertion (and, conversely, a composition that fails
/// because a child *held* would report no diagnostic at all). Per-child diagnostic
/// attribution across `not` / `all` / `any` is explicitly not required in v0 — see
/// docs/reference/semantic-diagnostics.md, "Logical Composition and Nested Diagnostics" — so instead a
/// failing composition gets exactly one composition-level diagnostic (below), and children
/// contribute descriptive detail only, never their own `diagnostics[]` entry.
fn expectation_json(
    test_id: &str,
    checkpoint_action_index: Option<usize>,
    expectation: &ExpectationResult,
    diagnostics: &mut DiagnosticsBuilder,
    attribute_diagnostics: bool,
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
        ExpectationKind::FileContentsEquals {
            path,
            expected_source,
            observation,
        } => {
            let mut value = json!({
                "kind": "fileContentsEquals",
                "path": path,
                "expectedSource": expected_source_json(expected_source),
            });
            contents_equals_observation_json(&mut value, observation);
            value
        }
        ExpectationKind::FileTextEquals {
            path,
            expected_source,
            observation,
        } => {
            let mut value = json!({
                "kind": "fileTextEquals",
                "path": path,
                "expectedSource": text_equals_expected_source_json(expected_source),
            });
            contents_equals_observation_json(&mut value, observation);
            value
        }
        ExpectationKind::StdoutContentsEquals {
            expected_source,
            comparison,
        } => stream_contents_equals_json(
            "stdoutContentsEquals",
            "stdout",
            test_id,
            action_ref.as_deref(),
            expected_source,
            comparison,
        ),
        ExpectationKind::StderrContentsEquals {
            expected_source,
            comparison,
        } => stream_contents_equals_json(
            "stderrContentsEquals",
            "stderr",
            test_id,
            action_ref.as_deref(),
            expected_source,
            comparison,
        ),
        ExpectationKind::StdoutTextEquals {
            expected_source,
            comparison,
        } => stream_text_equals_json(
            "stdoutTextEquals",
            "stdout",
            test_id,
            action_ref.as_deref(),
            expected_source,
            comparison,
        ),
        ExpectationKind::StderrTextEquals {
            expected_source,
            comparison,
        } => stream_text_equals_json(
            "stderrTextEquals",
            "stderr",
            test_id,
            action_ref.as_deref(),
            expected_source,
            comparison,
        ),
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
                .map(|child| {
                    expectation_json(test_id, checkpoint_action_index, child, diagnostics, false)
                })
                .collect();
            json!({
                "kind": "logical",
                "operator": operator.keyword(),
                "children": children_json,
            })
        }
    };

    value["status"] = json!(status);

    if attribute_diagnostics {
        match &expectation.kind {
            ExpectationKind::Logical { operator, .. } => {
                // See this function's doc comment: children never register their own
                // diagnostic, so a failing composition needs its own summary entry here,
                // or a genuinely failing case would surface no diagnostic at all.
                if !expectation.passed {
                    let origin = json!({ "kind": "test", "test": test_id });
                    let message = format!("'{}' composition did not hold", operator.keyword());
                    let diagnostic_id =
                        diagnostics.push("assertion", None, "failure", &message, origin, None);
                    value["diagnosticRef"] = json!(diagnostic_id);
                }
            }
            _ => {
                if let Some(code) = expectation.failure_diagnostic_code() {
                    let origin = json!({ "kind": "test", "test": test_id });
                    let message = assertion_failure_message(&expectation.kind, code);
                    let diagnostic_id = diagnostics.push(
                        "assertion",
                        Some(code),
                        "failure",
                        &message,
                        origin,
                        None,
                    );
                    value["diagnosticRef"] = json!(diagnostic_id);
                }
            }
        }
    }

    value
}

/// JSON representation of a `contents_equals` expected value's source.
fn expected_source_json(source: &ContentsEqualsExpectedSource) -> Value {
    match source {
        ContentsEqualsExpectedSource::Workspace(path) => json!({
            "kind": "workspace",
            "path": path,
        }),
        ContentsEqualsExpectedSource::Fixture(path) => json!({
            "kind": "fixture",
            "path": path,
        }),
    }
}

/// JSON representation of a `text_equals` expected value's source. Unlike
/// `expected_source_json` (a reference to another file), the full inline text is included
/// verbatim — it is already present in the script source, mirroring how `fileContains`'s
/// `expected` field includes its full inline expected text.
fn text_equals_expected_source_json(source: &TextEqualsExpectedSource) -> Value {
    match source {
        TextEqualsExpectedSource::Quoted(value) => json!({
            "kind": "quoted",
            "value": value,
        }),
        TextEqualsExpectedSource::Heredoc(value) => json!({
            "kind": "heredoc",
            "value": value,
        }),
    }
}

/// Adds `outcome` / `actualSizeBytes` / `expectedSizeBytes`, and on mismatch a bounded `mismatch`
/// object, to `value`. Never adds the full actual/expected bytes — see the module-level raw byte
/// evidence note and docs/reference/semantic-diagnostics.md.
fn contents_equals_comparison_json(value: &mut Value, comparison: &ContentsEqualsComparison) {
    value["actualSizeBytes"] = json!(comparison.actual.len());
    value["expectedSizeBytes"] = json!(comparison.expected.len());
    match &comparison.outcome {
        ContentsEqualsOutcome::Match => {
            value["outcome"] = json!("match");
        }
        ContentsEqualsOutcome::Mismatch(mismatch) => {
            value["outcome"] = json!("mismatch");
            let ctx = mismatch_context(&comparison.actual, &comparison.expected, mismatch);
            value["mismatch"] = json!({
                "firstDiffOffset": mismatch.first_diff_offset,
                "firstDiffLine": ctx.first_diff_line,
                "actualContext": ctx.actual_context,
                "expectedContext": ctx.expected_context,
            });
        }
    }
}

/// Adds `observed`, and (only when the actual side was successfully read) the comparison
/// outcome, to `value`.
fn contents_equals_observation_json(value: &mut Value, observation: &ContentsEqualsObservation) {
    match observation {
        ContentsEqualsObservation::Compared(comparison) => {
            value["observed"] = json!("compared");
            contents_equals_comparison_json(value, comparison);
        }
        ContentsEqualsObservation::ActualMissing => value["observed"] = json!("actualMissing"),
        ContentsEqualsObservation::ActualNotRegularFile => {
            value["observed"] = json!("actualNotARegularFile")
        }
        ContentsEqualsObservation::ActualUnreadable => {
            value["observed"] = json!("actualUnreadable")
        }
    }
}

/// Builds the `expectation` object for `stdout` / `stderr contents_equals`. `actualRef` reuses
/// the same per-action artifact reference `stdoutContains` / `stderrContains` already use (the
/// captured stream is written to `<test_id>/<action_id>/<stream>.bin` regardless of which
/// assertion reads it); no artifact reference is written for the expected side — persisting
/// mismatch bytes as evidence is explicitly not required in v0.
fn stream_contents_equals_json(
    kind: &str,
    stream: &str,
    test_id: &str,
    action_ref: Option<&str>,
    expected_source: &ContentsEqualsExpectedSource,
    comparison: &ContentsEqualsComparison,
) -> Value {
    let mut value = json!({
        "kind": kind,
        "expectedSource": expected_source_json(expected_source),
    });
    if let Some(action_ref) = action_ref {
        value["actualRef"] = json!(format!("{test_id}/{action_ref}/{stream}.bin"));
    }
    contents_equals_comparison_json(&mut value, comparison);
    value
}

/// Builds the `expectation` object for `stdout` / `stderr text_equals`. Mirrors
/// `stream_contents_equals_json` — same `actualRef` per-action artifact reference, same
/// comparison fields — differing only in the expected side's shape: an inline
/// `TextExpectedSource`, not a reference to another file.
fn stream_text_equals_json(
    kind: &str,
    stream: &str,
    test_id: &str,
    action_ref: Option<&str>,
    expected_source: &TextEqualsExpectedSource,
    comparison: &ContentsEqualsComparison,
) -> Value {
    let mut value = json!({
        "kind": kind,
        "expectedSource": text_equals_expected_source_json(expected_source),
    });
    if let Some(action_ref) = action_ref {
        value["actualRef"] = json!(format!("{test_id}/{action_ref}/{stream}.bin"));
    }
    contents_equals_comparison_json(&mut value, comparison);
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
        ExpectationKind::FileContentsEquals {
            path, observation, ..
        } => match observation {
            ContentsEqualsObservation::Compared(comparison) => {
                contents_equals_mismatch_message(&format!("file {path:?}"), comparison)
            }
            ContentsEqualsObservation::ActualMissing => format!("file {path:?} does not exist"),
            ContentsEqualsObservation::ActualNotRegularFile => {
                format!("file {path:?} is not a regular file (e.g. a directory)")
            }
            ContentsEqualsObservation::ActualUnreadable => {
                format!("file {path:?} could not be read")
            }
        },
        ExpectationKind::FileTextEquals {
            path, observation, ..
        } => match observation {
            ContentsEqualsObservation::Compared(comparison) => {
                contents_equals_mismatch_message(&format!("file {path:?}"), comparison)
            }
            ContentsEqualsObservation::ActualMissing => format!("file {path:?} does not exist"),
            ContentsEqualsObservation::ActualNotRegularFile => {
                format!("file {path:?} is not a regular file (e.g. a directory)")
            }
            ContentsEqualsObservation::ActualUnreadable => {
                format!("file {path:?} could not be read")
            }
        },
        ExpectationKind::StdoutContentsEquals { comparison, .. } => {
            contents_equals_mismatch_message("stdout", comparison)
        }
        ExpectationKind::StderrContentsEquals { comparison, .. } => {
            contents_equals_mismatch_message("stderr", comparison)
        }
        ExpectationKind::StdoutTextEquals { comparison, .. } => {
            contents_equals_mismatch_message("stdout", comparison)
        }
        ExpectationKind::StderrTextEquals { comparison, .. } => {
            contents_equals_mismatch_message("stderr", comparison)
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

/// Human-facing summary of a `contents_equals` mismatch, for the `diagnostics[]` entry's
/// `message` field. Bounded and escaped, like every other diagnostic derived from `comparison`
/// — see `contents_equals_comparison_json`.
fn contents_equals_mismatch_message(
    subject: &str,
    comparison: &ContentsEqualsComparison,
) -> String {
    match &comparison.outcome {
        ContentsEqualsOutcome::Mismatch(mismatch) => {
            let ctx = mismatch_context(&comparison.actual, &comparison.expected, mismatch);
            format!(
                "{subject} contents did not match expected bytes (first differing byte at offset {}, line {})",
                mismatch.first_diff_offset, ctx.first_diff_line,
            )
        }
        ContentsEqualsOutcome::Match => {
            unreachable!("a matching comparison never has a failure diagnostic code")
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
    use crate::result::{AssertionBlockResult, FileError, RuntimeError, ScriptError};
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
    fn passed_case_produces_passed_status_and_evidence_references() {
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

        let doc = build_run_result_document(&report);

        assert_eq!(doc["status"], "passed");
        assert_eq!(doc["processExitCode"], 0);
        assert_eq!(doc["schemaVersion"], 1);
        assert_eq!(doc["noop"], false);
        assert!(doc["diagnostics"].as_array().unwrap().is_empty());
        assert_eq!(doc["tests"][0]["id"], "test-1");
        assert_eq!(doc["tests"][0]["status"], "passed");
        assert_eq!(
            doc["tests"][0]["actions"][0]["stdout"]["artifactRef"],
            "test-1/action-1/stdout.bin"
        );
        assert_eq!(doc["tests"][0]["actions"][0]["stdout"]["sizeBytes"], 6);
        assert_eq!(
            doc["tests"][0]["actions"][0]["stdout"]["sha256"],
            sha256_hex(b"hello\n")
        );
        // Captured bytes are never inlined: only the reference triple is present.
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
    fn sha256_hex_matches_known_digest() {
        // Digest of the empty input is a well-known SHA-256 test vector.
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn noop_report_produces_noop_true_and_empty_tests() {
        let report = report_with_cases(vec![]);

        let doc = build_run_result_document(&report);

        assert_eq!(doc["status"], "passed");
        assert_eq!(doc["processExitCode"], 0);
        assert_eq!(doc["noop"], true);
        assert!(doc["tests"].as_array().unwrap().is_empty());
        assert_eq!(doc["summary"]["scripts"], 0);
        assert_eq!(doc["summary"]["actions"], 0);
        assert_eq!(doc["summary"]["assertions"], 0);
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

        let doc = build_run_result_document(&report);

        assert_eq!(doc["status"], "failed");
        assert_eq!(doc["processExitCode"], 1);
        let diagnostics = doc["diagnostics"].as_array().unwrap();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0]["category"], "assertion");
        assert_eq!(diagnostics[0]["severity"], "failure");
        assert_eq!(diagnostics[0]["code"], "assertion.stdout.contains.mismatch");
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
                location: Some(DiagnosticLocation {
                    line: 1,
                    column: Some(3),
                }),
            },
        });

        let doc = build_run_result_document(&report);

        assert_eq!(doc["status"], "error");
        assert_eq!(doc["processExitCode"], 2);
        assert_eq!(doc["diagnostics"][0]["category"], "parse");
        assert_eq!(doc["diagnostics"][0]["code"], "parse.syntax");
        assert_eq!(doc["diagnostics"][0]["severity"], "error");
        assert_eq!(doc["diagnostics"][0]["location"]["line"], 1);
        assert_eq!(doc["diagnostics"][0]["location"]["column"], 3);
    }

    #[test]
    fn parse_error_without_column_has_null_column_but_a_line() {
        // Not every `ParseError` variant carries a column (e.g. `EmptyCase` only knows the
        // line a construct started on) — see `parser::ParseError::to_diagnostic`.
        let mut report = report_with_cases(vec![]);
        report.file_errors.push(FileError {
            source_path: PathBuf::from("broken.repor"),
            kind: FileErrorKind::ParseError {
                message: "case 'x' has no steps".to_string(),
                diagnostic_code: DiagnosticCode::ParseEmptyCase,
                location: Some(DiagnosticLocation {
                    line: 4,
                    column: None,
                }),
            },
        });

        let doc = build_run_result_document(&report);

        assert_eq!(doc["diagnostics"][0]["location"]["line"], 4);
        assert!(doc["diagnostics"][0]["location"]["column"].is_null());
    }

    #[test]
    fn non_parse_diagnostics_have_null_location() {
        // Semantic / runtime / assertion diagnostics never carry a location in v0: source
        // ranges are not yet tracked on the evaluator side. See issue #89's non-goals.
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
        let doc = build_run_result_document(&report_with_cases(vec![case]));

        assert!(doc["diagnostics"][0]["location"].is_null());
    }

    #[test]
    fn read_error_produces_error_status_with_internal_category_and_no_code() {
        let mut report = report_with_cases(vec![]);
        report.file_errors.push(FileError {
            source_path: PathBuf::from("missing.repor"),
            kind: FileErrorKind::ReadError("No such file or directory".to_string()),
        });

        let doc = build_run_result_document(&report);

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

        let doc = build_run_result_document(&report);

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

        let doc = build_run_result_document(&report);

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

        let doc = build_run_result_document(&report);

        assert_eq!(doc["status"], "error");
        assert_eq!(doc["summary"]["failed"], 1);
        assert_eq!(doc["summary"]["errors"], 1);
    }

    // --- Logical composition diagnostic attribution ---
    //
    // A composition's children are independently evaluated and never flipped by `not` (see
    // `ExpectationKind::Logical`'s own doc comment): a child's raw `passed` does not indicate
    // whether it is "responsible" for the composition's outcome. These tests pin down that a
    // *passing* composition never contributes a `diagnostics[]` entry (even when one of its
    // children individually "did not hold"), and a *failing* composition always contributes
    // exactly one composition-level entry (even when every child individually "held").

    #[test]
    fn passing_not_composition_with_a_failing_child_produces_no_diagnostics() {
        // not { file <"x"> exists } passes because the file is genuinely missing, i.e. the
        // child itself did not hold.
        let case = CaseResult {
            name: "not passes".to_string(),
            source_path: Some(PathBuf::from("notpass.repor")),
            status: CaseStatus::Pass,
            actions: vec![passing_action()],
            assertion_blocks: vec![AssertionBlockResult {
                step_index: 1,
                checkpoint_action_index: Some(0),
                expectations: vec![ExpectationResult {
                    kind: ExpectationKind::Logical {
                        operator: crate::model::LogicalOperator::Not,
                        children: vec![ExpectationResult {
                            kind: ExpectationKind::FileExists {
                                path: "does-not-exist.txt".to_string(),
                                observation: FileExistsObservation::Missing,
                            },
                            passed: false,
                        }],
                    },
                    passed: true,
                }],
            }],
            side_effects_executed: 0,
        };
        let report = report_with_cases(vec![case]);

        let doc = build_run_result_document(&report);

        assert_eq!(doc["status"], "passed");
        assert!(
            doc["diagnostics"].as_array().unwrap().is_empty(),
            "a passing composition must not contribute any diagnostic, \
             even when a child's own raw result is `failed`: {doc:#}"
        );
        assert!(
            doc["tests"][0]["assertions"][0]
                .get("diagnosticRef")
                .is_none()
        );
        assert!(
            doc["tests"][0]["assertions"][0]["expectation"]["children"][0]
                .get("diagnosticRef")
                .is_none(),
            "children must never carry their own diagnosticRef"
        );
    }

    #[test]
    fn failing_not_composition_with_a_holding_child_produces_one_diagnostic() {
        // not { file <"x"> exists } fails because the file genuinely exists, i.e. the child
        // itself held (passed: true) — the opposite raw state from the case above, yet this
        // is the one that must produce a diagnostic.
        let case = CaseResult {
            name: "not fails".to_string(),
            source_path: Some(PathBuf::from("notfail.repor")),
            status: CaseStatus::Fail,
            actions: vec![passing_action()],
            assertion_blocks: vec![AssertionBlockResult {
                step_index: 1,
                checkpoint_action_index: Some(0),
                expectations: vec![ExpectationResult {
                    kind: ExpectationKind::Logical {
                        operator: crate::model::LogicalOperator::Not,
                        children: vec![ExpectationResult {
                            kind: ExpectationKind::FileExists {
                                path: "present.txt".to_string(),
                                observation: FileExistsObservation::RegularFile,
                            },
                            passed: true,
                        }],
                    },
                    passed: false,
                }],
            }],
            side_effects_executed: 0,
        };
        let report = report_with_cases(vec![case]);

        let doc = build_run_result_document(&report);

        assert_eq!(doc["status"], "failed");
        let diagnostics = doc["diagnostics"].as_array().unwrap();
        assert_eq!(
            diagnostics.len(),
            1,
            "a failing composition must contribute exactly one diagnostic, \
             even when every child's own raw result is `passed`: {doc:#}"
        );
        assert_eq!(diagnostics[0]["category"], "assertion");
        assert!(diagnostics[0].get("code").is_none());
        assert_eq!(
            doc["tests"][0]["assertions"][0]["diagnosticRef"],
            "diagnostic-1"
        );
        assert!(
            doc["tests"][0]["assertions"][0]["expectation"]["children"][0]
                .get("diagnosticRef")
                .is_none(),
            "children must never carry their own diagnosticRef"
        );
    }

    // --- expectation kind coverage ---

    #[test]
    fn exit_expectation_kind_json_shape() {
        let case = CaseResult {
            name: "exit".to_string(),
            source_path: Some(PathBuf::from("exit.repor")),
            status: CaseStatus::Fail,
            actions: vec![passing_action()],
            assertion_blocks: vec![AssertionBlockResult {
                step_index: 1,
                checkpoint_action_index: Some(0),
                expectations: vec![ExpectationResult {
                    kind: ExpectationKind::Exit {
                        expected: 1,
                        actual: 0,
                    },
                    passed: false,
                }],
            }],
            side_effects_executed: 0,
        };
        let doc = build_run_result_document(&report_with_cases(vec![case]));

        let expectation = &doc["tests"][0]["assertions"][0]["expectation"];
        assert_eq!(expectation["kind"], "exit");
        assert_eq!(expectation["expected"], 1);
        assert_eq!(expectation["actual"], 0);
        assert_eq!(
            doc["diagnostics"][0]["code"],
            "assertion.exit.equals.mismatch"
        );
    }

    #[test]
    fn file_and_dir_expectation_kinds_json_shape() {
        let case = CaseResult {
            name: "file and dir".to_string(),
            source_path: Some(PathBuf::from("filedir.repor")),
            status: CaseStatus::Fail,
            actions: vec![],
            assertion_blocks: vec![AssertionBlockResult {
                step_index: 0,
                checkpoint_action_index: None,
                expectations: vec![
                    ExpectationResult {
                        kind: ExpectationKind::FileContains {
                            path: "out.txt".to_string(),
                            expected: "ok".to_string(),
                            observation: FileContentObservation::NotFound,
                        },
                        passed: false,
                    },
                    ExpectationResult {
                        kind: ExpectationKind::DirExists {
                            path: "out".to_string(),
                            observation: DirExistsObservation::Missing,
                        },
                        passed: false,
                    },
                    ExpectationResult {
                        kind: ExpectationKind::DirContains {
                            path: "out".to_string(),
                            expected_entry: "a.txt".to_string(),
                            observation: DirContainsObservation::EntryMissing,
                        },
                        passed: false,
                    },
                ],
            }],
            side_effects_executed: 0,
        };
        let doc = build_run_result_document(&report_with_cases(vec![case]));

        let assertions = doc["tests"][0]["assertions"].as_array().unwrap();
        assert_eq!(assertions[0]["expectation"]["kind"], "fileContains");
        assert_eq!(assertions[0]["expectation"]["observed"], "notFound");
        assert_eq!(assertions[1]["expectation"]["kind"], "dirExists");
        assert_eq!(assertions[1]["expectation"]["observed"], "missing");
        assert_eq!(assertions[2]["expectation"]["kind"], "dirContains");
        assert_eq!(assertions[2]["expectation"]["observed"], "entryMissing");

        let diagnostics = doc["diagnostics"].as_array().unwrap();
        assert_eq!(diagnostics.len(), 3);
        assert_eq!(diagnostics[0]["code"], "assertion.file.contains.mismatch");
        assert_eq!(diagnostics[1]["code"], "assertion.dir.exists.missing");
        assert_eq!(
            diagnostics[2]["code"],
            "assertion.dir.contains.entry_missing"
        );
    }

    #[test]
    fn file_text_equals_expectation_kind_json_shape() {
        let comparison = ContentsEqualsComparison::compare(b"hellp".to_vec(), b"hello".to_vec());
        let case = CaseResult {
            name: "file text_equals".to_string(),
            source_path: Some(PathBuf::from("textequals.repor")),
            status: CaseStatus::Fail,
            actions: vec![],
            assertion_blocks: vec![AssertionBlockResult {
                step_index: 0,
                checkpoint_action_index: None,
                expectations: vec![ExpectationResult {
                    kind: ExpectationKind::FileTextEquals {
                        path: "out.txt".to_string(),
                        expected_source: TextEqualsExpectedSource::Quoted("hello".to_string()),
                        observation: ContentsEqualsObservation::Compared(comparison),
                    },
                    passed: false,
                }],
            }],
            side_effects_executed: 0,
        };
        let doc = build_run_result_document(&report_with_cases(vec![case]));

        let expectation = &doc["tests"][0]["assertions"][0]["expectation"];
        assert_eq!(expectation["kind"], "fileTextEquals");
        assert_eq!(expectation["path"], "out.txt");
        assert_eq!(expectation["expectedSource"]["kind"], "quoted");
        assert_eq!(expectation["expectedSource"]["value"], "hello");
        assert_eq!(expectation["observed"], "compared");
        assert_eq!(expectation["outcome"], "mismatch");
        assert_eq!(
            doc["diagnostics"][0]["code"],
            "assertion.file.text_equals.mismatch"
        );
    }

    #[test]
    fn contents_equals_fixture_source_and_stream_shapes_use_contract_field_names() {
        // The representative fixtures only exercise `contents_equals` with a workspace expected source (see tests/fixtures/run_result/contents_equals.repor).
        // Without this test, the `fixture` expected-source shape and `stderrContentsEquals` would have no coverage pinning their manifest field names against the schema.
        let mismatch = ContentsEqualsComparison::compare(b"hellp\n".to_vec(), b"hello\n".to_vec());
        let matching = ContentsEqualsComparison::compare(b"".to_vec(), b"".to_vec());
        let case = CaseResult {
            name: "contents_equals shapes".to_string(),
            source_path: Some(PathBuf::from("contentsequals.repor")),
            status: CaseStatus::Fail,
            actions: vec![passing_action()],
            assertion_blocks: vec![AssertionBlockResult {
                step_index: 1,
                checkpoint_action_index: Some(0),
                expectations: vec![
                    ExpectationResult {
                        kind: ExpectationKind::FileContentsEquals {
                            path: "actual.txt".to_string(),
                            expected_source: ContentsEqualsExpectedSource::Fixture(
                                "expected.txt".to_string(),
                            ),
                            observation: ContentsEqualsObservation::Compared(mismatch),
                        },
                        passed: false,
                    },
                    ExpectationResult {
                        kind: ExpectationKind::StderrContentsEquals {
                            expected_source: ContentsEqualsExpectedSource::Workspace(
                                "empty.txt".to_string(),
                            ),
                            comparison: matching,
                        },
                        passed: true,
                    },
                ],
            }],
            side_effects_executed: 0,
        };
        let doc = build_run_result_document(&report_with_cases(vec![case]));

        let file_expectation = &doc["tests"][0]["assertions"][0]["expectation"];
        assert_eq!(file_expectation["kind"], "fileContentsEquals");
        assert_eq!(file_expectation["expectedSource"]["kind"], "fixture");
        assert_eq!(file_expectation["expectedSource"]["path"], "expected.txt");
        assert_eq!(file_expectation["observed"], "compared");
        assert_eq!(file_expectation["outcome"], "mismatch");
        assert_eq!(file_expectation["actualSizeBytes"], 6);
        assert_eq!(file_expectation["expectedSizeBytes"], 6);
        let mismatch_json = &file_expectation["mismatch"];
        assert_eq!(mismatch_json["firstDiffOffset"], 4);
        assert_eq!(mismatch_json["firstDiffLine"], 1);
        assert!(mismatch_json["actualContext"].is_string());
        assert!(mismatch_json["expectedContext"].is_string());
        assert_eq!(
            doc["diagnostics"][0]["code"],
            "assertion.file.contents_equals.mismatch"
        );

        let stderr_expectation = &doc["tests"][0]["assertions"][1]["expectation"];
        assert_eq!(stderr_expectation["kind"], "stderrContentsEquals");
        assert_eq!(stderr_expectation["expectedSource"]["kind"], "workspace");
        assert_eq!(
            stderr_expectation["actualRef"],
            "test-1/action-1/stderr.bin"
        );
        assert_eq!(stderr_expectation["outcome"], "match");
        assert!(stderr_expectation.get("mismatch").is_none());
    }

    #[test]
    fn file_text_equals_heredoc_expected_source_json_shape() {
        // Mirrors `file_text_equals_expectation_kind_json_shape`, but for the `Heredoc` variant
        // of `TextEqualsExpectedSource`: the `Quoted` case above must not be the only one
        // exercised, since `text_equals_expected_source_json`'s two match arms are otherwise
        // untested on the `Heredoc` side.
        let comparison = ContentsEqualsComparison::compare(
            b"hello\nWORLD\n".to_vec(),
            b"hello\nworld\n".to_vec(),
        );
        let case = CaseResult {
            name: "file text_equals heredoc".to_string(),
            source_path: Some(PathBuf::from("textequals.repor")),
            status: CaseStatus::Fail,
            actions: vec![],
            assertion_blocks: vec![AssertionBlockResult {
                step_index: 0,
                checkpoint_action_index: None,
                expectations: vec![ExpectationResult {
                    kind: ExpectationKind::FileTextEquals {
                        path: "out.txt".to_string(),
                        expected_source: TextEqualsExpectedSource::Heredoc(
                            "hello\nworld\n".to_string(),
                        ),
                        observation: ContentsEqualsObservation::Compared(comparison),
                    },
                    passed: false,
                }],
            }],
            side_effects_executed: 0,
        };
        let doc = build_run_result_document(&report_with_cases(vec![case]));

        let expectation = &doc["tests"][0]["assertions"][0]["expectation"];
        assert_eq!(expectation["expectedSource"]["kind"], "heredoc");
        assert_eq!(expectation["expectedSource"]["value"], "hello\nworld\n");
    }

    #[test]
    fn checkpoint_reflects_the_action_in_effect_at_each_assertion_block() {
        // Two actions, two assertion blocks: the second block's checkpoint must reference the
        // second action, not the first.
        let case = CaseResult {
            name: "two actions".to_string(),
            source_path: Some(PathBuf::from("two.repor")),
            status: CaseStatus::Pass,
            actions: vec![passing_action(), passing_action()],
            assertion_blocks: vec![
                AssertionBlockResult {
                    step_index: 1,
                    checkpoint_action_index: Some(0),
                    expectations: vec![ExpectationResult {
                        kind: ExpectationKind::Exit {
                            expected: 0,
                            actual: 0,
                        },
                        passed: true,
                    }],
                },
                AssertionBlockResult {
                    step_index: 3,
                    checkpoint_action_index: Some(1),
                    expectations: vec![ExpectationResult {
                        kind: ExpectationKind::Exit {
                            expected: 0,
                            actual: 0,
                        },
                        passed: true,
                    }],
                },
            ],
            side_effects_executed: 0,
        };
        let doc = build_run_result_document(&report_with_cases(vec![case]));

        let assertions = doc["tests"][0]["assertions"].as_array().unwrap();
        assert_eq!(assertions[0]["checkpoint"], "action-1");
        assert_eq!(assertions[1]["checkpoint"], "action-2");
    }
}
