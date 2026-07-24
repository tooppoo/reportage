use super::*;

#[test]
fn all_passes_when_every_child_passes() {
    let expectation = logical(LogicalOperator::All, vec![exit_exp(0), exit_exp(0)]);
    let result =
        evaluate_expectation_at_checkpoint(&expectation, &checkpoint_after_exit(0)).unwrap();
    assert!(result.passed);
}

#[test]
fn all_fails_when_one_child_fails() {
    let expectation = logical(LogicalOperator::All, vec![exit_exp(0), exit_exp(1)]);
    let result =
        evaluate_expectation_at_checkpoint(&expectation, &checkpoint_after_exit(0)).unwrap();
    assert!(!result.passed);
}

#[test]
fn any_passes_when_one_child_passes() {
    let expectation = logical(LogicalOperator::Any, vec![exit_exp(1), exit_exp(0)]);
    let result =
        evaluate_expectation_at_checkpoint(&expectation, &checkpoint_after_exit(0)).unwrap();
    assert!(result.passed);
}

#[test]
fn any_fails_when_every_child_fails() {
    let expectation = logical(LogicalOperator::Any, vec![exit_exp(1), exit_exp(2)]);
    let result =
        evaluate_expectation_at_checkpoint(&expectation, &checkpoint_after_exit(0)).unwrap();
    assert!(!result.passed);
}

#[test]
fn any_passes_when_all_children_pass() {
    let expectation = logical(LogicalOperator::Any, vec![exit_exp(0), exit_exp(0)]);
    let result =
        evaluate_expectation_at_checkpoint(&expectation, &checkpoint_after_exit(0)).unwrap();
    assert!(result.passed);
}

#[test]
fn not_passes_when_single_child_fails() {
    let expectation = logical(LogicalOperator::Not, vec![exit_exp(1)]);
    let result =
        evaluate_expectation_at_checkpoint(&expectation, &checkpoint_after_exit(0)).unwrap();
    assert!(result.passed);
}

#[test]
fn not_fails_when_single_child_passes() {
    let expectation = logical(LogicalOperator::Not, vec![exit_exp(0)]);
    let result =
        evaluate_expectation_at_checkpoint(&expectation, &checkpoint_after_exit(0)).unwrap();
    assert!(!result.passed);
}

#[test]
fn not_with_multiple_children_negates_implicit_all_not_each_child() {
    // not { A B } is not(all(A, B)), not not(A) and not(B).
    // Here A passes (exit 0) and B fails (exit 1): all(A, B) is false, so not(all(A, B)) is true — the block passes as a whole, even though per-child negation (not(A) and not(B)) would fail on A.
    let expectation = logical(LogicalOperator::Not, vec![exit_exp(0), exit_exp(1)]);
    let result =
        evaluate_expectation_at_checkpoint(&expectation, &checkpoint_after_exit(0)).unwrap();
    assert!(result.passed);
}

#[test]
fn not_with_all_children_passing_fails() {
    // all(A, B) is true here, so not(all(A, B)) must fail.
    let expectation = logical(LogicalOperator::Not, vec![exit_exp(0), exit_exp(0)]);
    let result =
        evaluate_expectation_at_checkpoint(&expectation, &checkpoint_after_exit(0)).unwrap();
    assert!(!result.passed);
}

#[test]
fn not_with_all_children_failing_passes() {
    // all(A, B) is false here, so not(all(A, B)) must pass.
    let expectation = logical(LogicalOperator::Not, vec![exit_exp(1), exit_exp(1)]);
    let result =
        evaluate_expectation_at_checkpoint(&expectation, &checkpoint_after_exit(0)).unwrap();
    assert!(result.passed);
}

#[test]
fn nested_logical_composition_evaluates_recursively() {
    // all { not { exit 1 } any { exit 0 exit 2 } }
    let expectation = logical(
        LogicalOperator::All,
        vec![
            logical(LogicalOperator::Not, vec![exit_exp(1)]),
            logical(LogicalOperator::Any, vec![exit_exp(0), exit_exp(2)]),
        ],
    );
    let result =
        evaluate_expectation_at_checkpoint(&expectation, &checkpoint_after_exit(0)).unwrap();
    assert!(result.passed);
}

#[test]
fn logical_result_retains_each_child_outcome() {
    // Nothing is lost: an `any` whose candidates all fail must retain each candidate's own failure reason.
    let expectation = logical(LogicalOperator::Any, vec![exit_exp(1), exit_exp(2)]);
    let result =
        evaluate_expectation_at_checkpoint(&expectation, &checkpoint_after_exit(0)).unwrap();
    let ExpectationKind::Logical { operator, children } = &result.kind else {
        panic!("expected ExpectationKind::Logical");
    };
    assert!(matches!(operator, LogicalOperator::Any));
    assert_eq!(children.len(), 2);
    assert!(!children[0].passed);
    assert!(!children[1].passed);
    assert!(matches!(
        children[0].kind,
        ExpectationKind::Exit {
            expected: 1,
            actual: 0
        }
    ));
    assert!(matches!(
        children[1].kind,
        ExpectationKind::Exit {
            expected: 2,
            actual: 0
        }
    ));
}

#[test]
fn logical_composition_at_top_level_case_evaluates_via_evaluate_case() {
    let script = make_script(vec![Case {
        name: "any exit".to_string(),
        steps: vec![
            action("false"), // exit 1
            Step::AssertionBlock(
                AssertionBlock::new(vec![logical(
                    LogicalOperator::Any,
                    vec![exit_exp(0), exit_exp(1)],
                )])
                .unwrap(),
            ),
        ],
    }]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert!(matches!(result.cases[0].status, CaseStatus::Pass));
}

#[test]
fn logical_composition_wrapping_process_expectation_at_initial_checkpoint_is_script_error() {
    // A composition wrapping exit/stdout/stderr still requires a preceding action, exactly like a bare process expectation.
    let script = make_script(vec![Case {
        name: "no action yet".to_string(),
        steps: vec![Step::AssertionBlock(
            AssertionBlock::new(vec![logical(LogicalOperator::All, vec![exit_exp(0)])]).unwrap(),
        )],
    }]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert!(matches!(result.cases[0].status, CaseStatus::ScriptError(_)));
}

// ─── `contents_equals` comparison evaluation (#87) ─────────────────────
