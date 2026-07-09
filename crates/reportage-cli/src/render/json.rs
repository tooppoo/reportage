//! The structured JSON execution report renderer (`--format=json`).
//!
//! `JsonRenderer` prints the external, camelCase JSON document described in issue #75:
//! `schemaVersion` / `tool` / `status` / `processExitCode` / `artifactRoot` / `summary` /
//! `diagnostics[]` / `tests[]`. Like [`super::human::HumanRenderer`], it only *reads* the
//! report; it never re-derives pass/fail from message text, and the runner (`evaluator`,
//! `executor`) has no knowledge this renderer exists.
//!
//! ## A stdout-safe projection of the canonical artifact result
//!
//! The document is not built here from scratch: it is derived from the canonical run result document (`reportage_core::run_result::build_run_result_document`, the same document `ArtifactWriter::write` persists as the artifact `result.json`) by [`project_run_result`].
//! The artifact bundle is the canonical record of a run; this renderer's output is the stdout-safe projection of it defined by `spec/output/json-report/schema.json` (issue #89, issue #102).
//! The projection keeps the two contracts aligned by construction; their remaining differences are intentional and enumerated on [`project_run_result`].
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

use reportage_core::result::ExecutionReport;
use reportage_core::run_result::build_run_result_document;
use serde_json::{Value, json};

use super::OutputRenderer;

/// Version of the `--format=json` stdout contract (`spec/output/json-report/schema.json`).
/// Distinct from the artifact result contract's own `schemaVersion` (`reportage_core::run_result::RUN_RESULT_SCHEMA_VERSION`); the two contracts version independently even while their current values coincide.
const JSON_REPORT_SCHEMA_VERSION: u32 = 1;

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

fn build_document(report: &ExecutionReport, artifact_root: &Path) -> Value {
    project_run_result(build_run_result_document(report), artifact_root)
}

/// Projects the canonical run result document into the `--format=json` stdout document.
///
/// The differences between the two contracts, applied here:
///
/// - `schemaVersion` is replaced with this stdout contract's own version.
/// - `noop` is dropped: the stdout contract does not carry it (a no-op run is recognizable from its empty `tests` and zeroed `summary`).
/// - `artifactRoot` is added: the artifact `result.json` resolves `artifactRef` values against its own directory, while a stdout consumer needs to be told where that directory is.
/// - Each action's `stdout` / `stderr` reference loses its `sha256`: evidence digests are part of the artifact bundle's integrity contract, not the stdout summary (`artifactRef` / `sizeBytes` are kept — see issue #102's projection parity items).
///
/// Everything else passes through unchanged, so the stdout document stays a faithful projection of the canonical record.
fn project_run_result(mut doc: Value, artifact_root: &Path) -> Value {
    doc["schemaVersion"] = json!(JSON_REPORT_SCHEMA_VERSION);
    doc.as_object_mut()
        .expect("run result document must be a JSON object")
        .remove("noop");
    doc["artifactRoot"] = json!(artifact_root.display().to_string());

    if let Some(tests) = doc["tests"].as_array_mut() {
        for test in tests {
            if let Some(actions) = test["actions"].as_array_mut() {
                for action in actions {
                    for stream in ["stdout", "stderr"] {
                        if let Some(reference) = action[stream].as_object_mut() {
                            reference.remove("sha256");
                        }
                    }
                }
            }
        }
    }

    doc
}

#[cfg(test)]
mod tests {
    use super::*;
    use reportage_core::result::{
        ActionResult, AssertionBlockResult, CaseResult, CaseStatus, ExpectationKind,
        ExpectationResult,
    };
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

    fn passing_case() -> CaseResult {
        CaseResult {
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
        }
    }

    fn report() -> ExecutionReport {
        ExecutionReport {
            cases: vec![passing_case()],
            file_errors: vec![],
        }
    }

    #[test]
    fn artifact_root_is_reflected_verbatim() {
        let doc = build_document(&report(), Path::new(".reportage/runs/42"));
        assert_eq!(doc["artifactRoot"], ".reportage/runs/42");
    }

    #[test]
    fn projection_keeps_document_semantics() {
        let doc = build_document(&report(), Path::new(".reportage/runs/1"));

        assert_eq!(doc["schemaVersion"], 1);
        assert_eq!(doc["status"], "passed");
        assert_eq!(doc["processExitCode"], 0);
        assert_eq!(doc["tests"][0]["id"], "test-1");
        assert_eq!(
            doc["tests"][0]["actions"][0]["stdout"]["artifactRef"],
            "test-1/action-1/stdout.bin"
        );
        assert_eq!(doc["tests"][0]["actions"][0]["stdout"]["sizeBytes"], 6);
        assert_eq!(
            doc["tests"][0]["assertions"][0]["expectation"]["actualRef"],
            "test-1/action-1/stdout.bin"
        );
    }

    #[test]
    fn projection_drops_noop_and_evidence_digests() {
        let doc = build_document(&report(), Path::new(".reportage/runs/1"));

        assert!(
            doc.get("noop").is_none(),
            "the stdout document must not carry the artifact-only `noop` field"
        );
        for stream in ["stdout", "stderr"] {
            assert!(
                doc["tests"][0]["actions"][0][stream]
                    .get("sha256")
                    .is_none(),
                "the stdout document must not carry evidence digests"
            );
            assert!(
                doc["tests"][0]["actions"][0][stream].get("data").is_none(),
                "captured bytes are never inlined"
            );
        }
    }

    #[test]
    fn projection_matches_canonical_document_except_for_defined_differences() {
        // The reverse direction of `project_run_result`'s difference list: adding back `sha256` digests and `noop`, and removing `artifactRoot`, must reproduce the canonical document exactly.
        // This pins that the projection never silently drops or rewrites anything else.
        let report = report();
        let canonical = build_run_result_document(&report);
        let mut projected = build_document(&report, Path::new(".reportage/runs/1"));

        projected
            .as_object_mut()
            .unwrap()
            .remove("artifactRoot")
            .expect("projection must add artifactRoot");
        projected["noop"] = canonical["noop"].clone();
        for (test_index, test) in canonical["tests"].as_array().unwrap().iter().enumerate() {
            for (action_index, action) in test["actions"].as_array().unwrap().iter().enumerate() {
                for stream in ["stdout", "stderr"] {
                    projected["tests"][test_index]["actions"][action_index][stream]["sha256"] =
                        action[stream]["sha256"].clone();
                }
            }
        }

        assert_eq!(projected, canonical);
    }
}
