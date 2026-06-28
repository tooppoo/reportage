use crate::executor::execute_action;
use crate::model::{Case, Expectation, OutputMatcher, Script, Step};
use crate::result::{
    ActionResult, AssertionBlockResult, CaseResult, CaseStatus, ExpectationKind, ExpectationResult,
    RunResult,
};

/// Observable evidence available at a point in case execution.
///
/// A checkpoint is an evidence context, not a full filesystem snapshot. The
/// initial checkpoint has workspace state but no last action result.
///
/// See docs/semantics.md — Checkpoint.
pub struct Checkpoint {
    pub workspace: WorkspaceState,
    pub last_action: Option<ActionResult>,
}

impl Checkpoint {
    /// The initial checkpoint: workspace state present, no last action result.
    pub fn initial() -> Self {
        Self {
            workspace: WorkspaceState,
            last_action: None,
        }
    }

    /// An action-updated checkpoint after `$ ...` completes.
    pub fn after_action(action: ActionResult) -> Self {
        Self {
            workspace: WorkspaceState,
            last_action: Some(action),
        }
    }
}

/// Placeholder for observable workspace state.
///
/// In v0, filesystem access is performed directly when workspace expectations
/// are evaluated. A richer snapshot type may be introduced in future versions.
pub struct WorkspaceState;

pub fn evaluate(script: &Script) -> RunResult {
    RunResult {
        cases: script.cases.iter().map(evaluate_case).collect(),
        file_errors: vec![],
    }
}

fn evaluate_case(case: &Case) -> CaseResult {
    // Every case must contain at least one assertion block.
    let has_assertion_block = case
        .steps
        .iter()
        .any(|s| matches!(s, Step::AssertionBlock(_)));
    if !has_assertion_block {
        return CaseResult {
            name: case.name.clone(),
            source_path: None,
            status: CaseStatus::ScriptError(format!(
                "case '{}' has no assertion block; every case requires at least one assert {{ ... }} block",
                case.name
            )),
            actions: vec![],
            assertion_blocks: vec![],
        };
    }

    let mut action_results: Vec<ActionResult> = Vec::new();
    let mut assertion_block_results: Vec<AssertionBlockResult> = Vec::new();
    // Steps are processed in source order.
    // Assertion block failure stops execution before the next action.
    // See docs/semantics.md — Assertion block and the checkpoint-based assertion ADR.
    let mut checkpoint = Checkpoint::initial();
    let mut case_failed = false;

    for (step_idx, step) in case.steps.iter().enumerate() {
        match step {
            Step::Action(action) => {
                if case_failed {
                    // Do not proceed to next action after a block failure.
                    break;
                }
                match execute_action(&action.command) {
                    Ok(result) => {
                        checkpoint = Checkpoint::after_action(result.clone());
                        action_results.push(result);
                    }
                    Err(e) => {
                        return CaseResult {
                            name: case.name.clone(),
                            source_path: None,
                            status: CaseStatus::RuntimeError(e.message),
                            actions: action_results,
                            assertion_blocks: assertion_block_results,
                        };
                    }
                }
            }

            Step::AssertionBlock(block) => {
                if case_failed {
                    break;
                }

                // Check that all expectations have the evidence they require.
                for expectation in block.expectations() {
                    if expectation.required_evidence().needs_action_result()
                        && checkpoint.last_action.is_none()
                    {
                        return CaseResult {
                            name: case.name.clone(),
                            source_path: None,
                            status: CaseStatus::ScriptError(format!(
                                "case '{}': assertion block at step {} uses a process expectation \
                                 (exit, stdout, stderr) but no '$' action has run yet; \
                                 the initial checkpoint has no last action result",
                                case.name,
                                step_idx + 1,
                            )),
                            actions: action_results,
                            assertion_blocks: assertion_block_results,
                        };
                    }
                }

                // Evaluate all expectations in the block independently.
                let expectation_results: Vec<ExpectationResult> = block
                    .expectations()
                    .iter()
                    .map(|exp| evaluate_expectation(exp, &checkpoint))
                    .collect();

                let block_result = AssertionBlockResult {
                    step_index: step_idx,
                    expectations: expectation_results,
                };

                if block_result.has_failures() {
                    case_failed = true;
                }

                assertion_block_results.push(block_result);
            }
        }
    }

    CaseResult {
        name: case.name.clone(),
        source_path: None,
        status: if case_failed {
            CaseStatus::Fail
        } else {
            CaseStatus::Pass
        },
        actions: action_results,
        assertion_blocks: assertion_block_results,
    }
}

fn evaluate_expectation(expectation: &Expectation, checkpoint: &Checkpoint) -> ExpectationResult {
    match expectation {
        Expectation::Exit(exp) => {
            let actual = checkpoint
                .last_action
                .as_ref()
                .map(|a| a.exit_code)
                .unwrap_or(-1);
            let passed = actual == exp.expected as i32;
            ExpectationResult {
                kind: ExpectationKind::Exit {
                    expected: exp.expected,
                    actual,
                },
                passed,
            }
        }
        Expectation::Stdout(exp) => {
            let actual = checkpoint
                .last_action
                .as_ref()
                .map(|a| a.stdout.clone())
                .unwrap_or_default();
            match &exp.matcher {
                OutputMatcher::Contains(expected) => {
                    let passed = actual.contains(expected.as_str());
                    ExpectationResult {
                        kind: ExpectationKind::StdoutContains {
                            expected: expected.clone(),
                            actual,
                        },
                        passed,
                    }
                }
                OutputMatcher::Empty => {
                    let passed = actual.trim().is_empty();
                    ExpectationResult {
                        kind: ExpectationKind::StdoutEmpty { actual },
                        passed,
                    }
                }
                _ => unreachable!("output matcher variant not implemented in v0 evaluator"),
            }
        }
        Expectation::Stderr(exp) => {
            let actual = checkpoint
                .last_action
                .as_ref()
                .map(|a| a.stderr.clone())
                .unwrap_or_default();
            match &exp.matcher {
                OutputMatcher::Contains(expected) => {
                    let passed = actual.contains(expected.as_str());
                    ExpectationResult {
                        kind: ExpectationKind::StderrContains {
                            expected: expected.clone(),
                            actual,
                        },
                        passed,
                    }
                }
                OutputMatcher::Empty => {
                    let passed = actual.trim().is_empty();
                    ExpectationResult {
                        kind: ExpectationKind::StderrEmpty { actual },
                        passed,
                    }
                }
                _ => unreachable!("output matcher variant not implemented in v0 evaluator"),
            }
        }
        _ => unreachable!("expectation variant not implemented in v0 evaluator"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ActionStep, AssertionBlock, Case, ExitExpectation, Expectation, Script};

    fn make_script(cases: Vec<Case>) -> Script {
        Script { cases }
    }

    fn action(cmd: &str) -> Step {
        Step::Action(ActionStep {
            command: cmd.to_string(),
        })
    }

    fn assert_exit(code: u8) -> Step {
        let expectations = vec![Expectation::Exit(ExitExpectation { expected: code })];
        Step::AssertionBlock(AssertionBlock::new(expectations).unwrap())
    }

    fn assert_exits(codes: &[u8]) -> Step {
        let expectations = codes
            .iter()
            .map(|&c| Expectation::Exit(ExitExpectation { expected: c }))
            .collect();
        Step::AssertionBlock(AssertionBlock::new(expectations).unwrap())
    }

    #[test]
    fn passing_case_exits_zero() {
        let script = make_script(vec![Case {
            name: "pass".to_string(),
            steps: vec![action("true"), assert_exit(0)],
        }]);
        let result = evaluate(&script);
        assert_eq!(result.exit_code(), 0);
        assert!(matches!(result.cases[0].status, CaseStatus::Pass));
    }

    #[test]
    fn failing_expectation_sets_fail_status() {
        let script = make_script(vec![Case {
            name: "fail".to_string(),
            steps: vec![action("false"), assert_exit(0)],
        }]);
        let result = evaluate(&script);
        assert_eq!(result.exit_code(), 1);
        assert!(matches!(result.cases[0].status, CaseStatus::Fail));
    }

    #[test]
    fn false_with_assert_exit_one_passes() {
        let script = make_script(vec![Case {
            name: "nonzero pass".to_string(),
            steps: vec![action("false"), assert_exit(1)],
        }]);
        let result = evaluate(&script);
        assert_eq!(result.exit_code(), 0);
        assert!(matches!(result.cases[0].status, CaseStatus::Pass));
    }

    #[test]
    fn missing_assertion_block_is_script_error() {
        let script = make_script(vec![Case {
            name: "no assert".to_string(),
            steps: vec![action("true")],
        }]);
        let result = evaluate(&script);
        assert_eq!(result.exit_code(), 2);
        assert!(matches!(result.cases[0].status, CaseStatus::ScriptError(_)));
    }

    #[test]
    fn process_expectation_at_initial_checkpoint_is_script_error() {
        let script = make_script(vec![Case {
            name: "assert first".to_string(),
            steps: vec![assert_exit(0)],
        }]);
        let result = evaluate(&script);
        assert_eq!(result.exit_code(), 2);
        assert!(matches!(result.cases[0].status, CaseStatus::ScriptError(_)));
    }

    #[test]
    fn multiple_expectations_in_one_block_all_evaluated() {
        let script = make_script(vec![Case {
            name: "multi expect".to_string(),
            steps: vec![action("true"), assert_exits(&[0, 0])],
        }]);
        let result = evaluate(&script);
        assert_eq!(result.exit_code(), 0);
        assert_eq!(result.cases[0].assertion_blocks.len(), 1);
        assert_eq!(result.cases[0].assertion_blocks[0].expectations.len(), 2);
    }

    #[test]
    fn both_expectations_in_block_reported_when_both_fail() {
        let script = make_script(vec![Case {
            name: "two fails".to_string(),
            steps: vec![action("true"), assert_exits(&[1, 1])],
        }]);
        let result = evaluate(&script);
        assert!(matches!(result.cases[0].status, CaseStatus::Fail));
        let block = &result.cases[0].assertion_blocks[0];
        assert_eq!(block.expectations.len(), 2);
        assert!(!block.expectations[0].passed);
        assert!(!block.expectations[1].passed);
    }

    #[test]
    fn exit_code_is_max_across_cases() {
        let script = make_script(vec![
            Case {
                name: "fail".to_string(),
                steps: vec![action("false"), assert_exit(0)],
            },
            Case {
                name: "no assert".to_string(),
                steps: vec![action("true")], // no assertion block -> script error
            },
        ]);
        let result = evaluate(&script);
        assert_eq!(result.exit_code(), 2); // script error beats assertion failure
    }

    #[test]
    fn assertion_block_failure_stops_subsequent_action() {
        // assert_exit(1) fails because true exits 0.
        // Block failure must not run the second action.
        let script = make_script(vec![Case {
            name: "source order stop".to_string(),
            steps: vec![
                action("true"),
                assert_exit(1),  // fails: true exits 0
                action("false"), // must not run
            ],
        }]);
        let result = evaluate(&script);
        assert!(matches!(result.cases[0].status, CaseStatus::Fail));
        // Only the first action should have executed.
        assert_eq!(result.cases[0].actions.len(), 1);
        assert_eq!(result.cases[0].assertion_blocks.len(), 1);
    }
}
