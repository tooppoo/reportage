use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

use crate::result::{AssertionKind, CaseResult, CaseStatus, RunResult};

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
    let overall = if result.exit_code() == 0 {
        "pass"
    } else {
        "fail"
    };
    json!({
        "result": overall,
        "cases": result.cases.iter().map(case_json).collect::<Vec<_>>()
    })
}

fn case_json(case: &CaseResult) -> serde_json::Value {
    let (status, message): (&str, Option<&str>) = match &case.status {
        CaseStatus::Pass => ("pass", None),
        CaseStatus::Fail => ("fail", None),
        CaseStatus::ValidationError(msg) => ("validation_error", Some(msg)),
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

    let assertions: Vec<serde_json::Value> = case
        .assertions
        .iter()
        .map(|a| {
            let (expected, actual) = match a.kind {
                AssertionKind::Exit { expected, actual } => (expected as i64, actual as i64),
            };
            json!({
                "index": a.step_index,
                "kind": "assertion",
                "type": "exit",
                "target_action_index": a.target_action_index,
                "expected": expected,
                "actual": actual,
                "result": if a.passed { "pass" } else { "fail" }
            })
        })
        .collect();

    let mut obj = json!({
        "name": case.name,
        "status": status,
        "actions": actions,
        "assertions": assertions
    });

    if let Some(msg) = message {
        obj["message"] = json!(msg);
    }

    obj
}
