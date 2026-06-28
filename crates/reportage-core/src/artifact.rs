use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

use crate::result::{CaseResult, CaseStatus, ExpectationKind, FileErrorKind, RunResult};

pub struct ArtifactWriter {
    run_dir: PathBuf,
}

impl ArtifactWriter {
    pub fn for_run(base_dir: &Path) -> Self {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        ArtifactWriter {
            run_dir: base_dir.join("runs").join(millis.to_string()),
        }
    }

    pub fn write(&self, result: &RunResult) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.run_dir)?;
        let value = build_json(result);
        let json = serde_json::to_string_pretty(&value)
            .expect("result JSON serialization should not fail");
        std::fs::write(self.run_dir.join("result.json"), json)?;
        Ok(())
    }
}

fn build_json(result: &RunResult) -> serde_json::Value {
    let overall = if !result.file_errors.is_empty() {
        "script_error"
    } else if result.exit_code() == 0 {
        "pass"
    } else {
        "fail"
    };

    let mut obj = json!({
        "result": overall,
        "cases": result.cases.iter().map(case_json).collect::<Vec<_>>()
    });

    if !result.file_errors.is_empty() {
        obj["file_errors"] = json!(
            result
                .file_errors
                .iter()
                .map(|e| {
                    let (kind_str, message) = match &e.kind {
                        FileErrorKind::ReadError(msg) => ("read_error", msg.as_str()),
                        FileErrorKind::ParseError(msg) => ("parse_error", msg.as_str()),
                    };
                    json!({
                        "source_path": e.source_path.display().to_string(),
                        "kind": kind_str,
                        "message": message
                    })
                })
                .collect::<Vec<_>>()
        );
    }

    obj
}

fn case_json(case: &CaseResult) -> serde_json::Value {
    let (status, message): (&str, Option<&str>) = match &case.status {
        CaseStatus::Pass => ("pass", None),
        CaseStatus::Fail => ("fail", None),
        CaseStatus::ScriptError(msg) => ("script_error", Some(msg)),
        CaseStatus::RuntimeError(msg) => ("runtime_error", Some(msg)),
    };

    let actions: Vec<serde_json::Value> = case
        .actions
        .iter()
        .enumerate()
        .map(|(i, a)| {
            json!({
                "index": i,
                "kind": "action",
                "command": a.command,
                "exit_code": a.exit_code,
                "stdout": a.stdout,
                "stderr": a.stderr
            })
        })
        .collect();

    let assertion_blocks: Vec<serde_json::Value> = case
        .assertion_blocks
        .iter()
        .map(|block| {
            let expectations: Vec<serde_json::Value> = block
                .expectations
                .iter()
                .map(|e| {
                    let result_str = if e.passed { "pass" } else { "fail" };
                    match &e.kind {
                        ExpectationKind::Exit { expected, actual } => json!({
                            "kind": "exit",
                            "expected": expected,
                            "actual": actual,
                            "result": result_str,
                        }),
                        ExpectationKind::StdoutContains { expected, actual } => json!({
                            "kind": "stdout_contains",
                            "expected": expected,
                            "actual": actual,
                            "result": result_str,
                        }),
                        ExpectationKind::StderrContains { expected, actual } => json!({
                            "kind": "stderr_contains",
                            "expected": expected,
                            "actual": actual,
                            "result": result_str,
                        }),
                        ExpectationKind::StdoutEmpty { actual } => json!({
                            "kind": "stdout_empty",
                            "actual": actual,
                            "result": result_str,
                        }),
                        ExpectationKind::StderrEmpty { actual } => json!({
                            "kind": "stderr_empty",
                            "actual": actual,
                            "result": result_str,
                        }),
                    }
                })
                .collect();
            json!({
                "step_index": block.step_index,
                "expectations": expectations,
                "result": if block.has_failures() { "fail" } else { "pass" }
            })
        })
        .collect();

    let mut obj = json!({
        "name": case.name,
        "status": status,
        "actions": actions,
        "assertion_blocks": assertion_blocks
    });

    if let Some(path) = &case.source_path {
        obj["source_path"] = json!(path.display().to_string());
    }

    if let Some(msg) = message {
        obj["message"] = json!(msg);
    }

    obj
}
