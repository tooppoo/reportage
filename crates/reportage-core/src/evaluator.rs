use crate::executor::execute_action;
use crate::model::{AssertStep, Case, Script, Step};
use crate::result::{AssertionKind, AssertionResult, CaseResult, CaseStatus, RunResult};

pub fn evaluate(script: &Script) -> RunResult {
    RunResult {
        cases: script.cases.iter().map(evaluate_case).collect(),
    }
}

fn evaluate_case(case: &Case) -> CaseResult {
    if let Err(msg) = validate_case(case) {
        return CaseResult {
            name: case.name.clone(),
            status: CaseStatus::ValidationError(msg),
            actions: vec![],
            assertions: vec![],
        };
    }

    let mut action_outputs = Vec::new();
    for step in &case.steps {
        if let Step::Action(action) = step {
            match execute_action(&action.command) {
                Ok(output) => action_outputs.push(output),
                Err(e) => {
                    return CaseResult {
                        name: case.name.clone(),
                        status: CaseStatus::RuntimeError(e.message),
                        actions: action_outputs,
                        assertions: vec![],
                    };
                }
            }
        }
    }

    let mut assertion_results = Vec::new();
    let mut all_pass = true;

    for (step_idx, step) in case.steps.iter().enumerate() {
        if let Step::Assert(assert_step) = step {
            let target_idx = preceding_action_output_idx(&case.steps, step_idx).unwrap();
            let action_output = &action_outputs[target_idx];

            match assert_step {
                AssertStep::Exit { expected } => {
                    let actual = action_output.exit_code;
                    let passed = actual == *expected as i32;
                    if !passed {
                        all_pass = false;
                    }
                    assertion_results.push(AssertionResult {
                        step_index: step_idx,
                        target_action_index: target_idx,
                        kind: AssertionKind::Exit {
                            expected: *expected,
                            actual,
                        },
                        passed,
                    });
                }
            }
        }
    }

    CaseResult {
        name: case.name.clone(),
        status: if all_pass {
            CaseStatus::Pass
        } else {
            CaseStatus::Fail
        },
        actions: action_outputs,
        assertions: assertion_results,
    }
}

fn validate_case(case: &Case) -> Result<(), String> {
    let has_assertion = case.steps.iter().any(|s| matches!(s, Step::Assert(_)));
    if !has_assertion {
        return Err(format!(
            "case '{}' has no assertion; every case requires at least one explicit assertion",
            case.name
        ));
    }

    for (idx, step) in case.steps.iter().enumerate() {
        if matches!(step, Step::Assert(_))
            && preceding_action_output_idx(&case.steps, idx).is_none()
        {
            return Err(format!(
                "case '{}': assertion at step {} has no preceding action; 'assert exit' requires a preceding action in the same case",
                case.name,
                idx + 1,
            ));
        }
    }

    Ok(())
}

/// Returns the index into `action_outputs` for the action immediately preceding
/// `assertion_step_idx`, ignoring any assertion steps in between.
fn preceding_action_output_idx(steps: &[Step], assertion_step_idx: usize) -> Option<usize> {
    let mut count = 0usize;
    let mut last = None;
    for (i, step) in steps.iter().enumerate() {
        if i >= assertion_step_idx {
            break;
        }
        if matches!(step, Step::Action(_)) {
            last = Some(count);
            count += 1;
        }
    }
    last
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Action, AssertStep, Case, Script, Step};

    fn make_script(cases: Vec<Case>) -> Script {
        Script { cases }
    }

    fn action(cmd: &str) -> Step {
        Step::Action(Action {
            command: cmd.to_string(),
        })
    }

    fn assert_exit(code: u8) -> Step {
        Step::Assert(AssertStep::Exit { expected: code })
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
    fn failing_assertion_sets_fail_status() {
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
    fn missing_assertion_is_validation_error() {
        let script = make_script(vec![Case {
            name: "no assert".to_string(),
            steps: vec![action("true")],
        }]);
        let result = evaluate(&script);
        assert_eq!(result.exit_code(), 2);
        assert!(matches!(
            result.cases[0].status,
            CaseStatus::ValidationError(_)
        ));
    }

    #[test]
    fn assertion_before_action_is_validation_error() {
        let script = make_script(vec![Case {
            name: "assert first".to_string(),
            steps: vec![assert_exit(0)],
        }]);
        let result = evaluate(&script);
        assert_eq!(result.exit_code(), 2);
        assert!(matches!(
            result.cases[0].status,
            CaseStatus::ValidationError(_)
        ));
    }

    #[test]
    fn multiple_assertions_target_same_action() {
        let script = make_script(vec![Case {
            name: "multi assert".to_string(),
            steps: vec![action("true"), assert_exit(0), assert_exit(0)],
        }]);
        let result = evaluate(&script);
        assert_eq!(result.exit_code(), 0);
        assert_eq!(result.cases[0].assertions.len(), 2);
        assert_eq!(result.cases[0].assertions[0].target_action_index, 0);
        assert_eq!(result.cases[0].assertions[1].target_action_index, 0);
    }

    #[test]
    fn exit_code_is_max_across_cases() {
        let script = make_script(vec![
            Case {
                name: "fail".to_string(),
                steps: vec![action("false"), assert_exit(0)],
            },
            Case {
                name: "valid error".to_string(),
                steps: vec![action("true")], // no assertion -> validation error
            },
        ]);
        let result = evaluate(&script);
        assert_eq!(result.exit_code(), 2); // validation error beats assertion failure
    }
}
