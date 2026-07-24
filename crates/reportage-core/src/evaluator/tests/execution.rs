use super::*;

#[test]
fn passing_case_exits_zero() {
    let script = make_script(vec![Case {
        name: "pass".to_string(),
        steps: vec![action("true"), assert_exit(0)],
    }]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert_eq!(result.exit_code(), 0);
    assert!(matches!(result.cases[0].status, CaseStatus::Pass));
}

#[test]
fn failing_expectation_sets_fail_status() {
    let script = make_script(vec![Case {
        name: "fail".to_string(),
        steps: vec![action("false"), assert_exit(0)],
    }]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert_eq!(result.exit_code(), 1);
    assert!(matches!(result.cases[0].status, CaseStatus::Fail));
}

#[test]
fn false_with_assert_exit_one_passes() {
    let script = make_script(vec![Case {
        name: "nonzero pass".to_string(),
        steps: vec![action("false"), assert_exit(1)],
    }]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert_eq!(result.exit_code(), 0);
    assert!(matches!(result.cases[0].status, CaseStatus::Pass));
}

#[test]
fn missing_assertion_block_is_script_error() {
    let script = make_script(vec![Case {
        name: "no assert".to_string(),
        steps: vec![action("true")],
    }]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert_eq!(result.exit_code(), 2);
    assert!(matches!(result.cases[0].status, CaseStatus::ScriptError(_)));
}

#[test]
fn process_expectation_at_initial_checkpoint_is_script_error() {
    let script = make_script(vec![Case {
        name: "assert first".to_string(),
        steps: vec![assert_exit(0)],
    }]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert_eq!(result.exit_code(), 2);
    assert!(matches!(result.cases[0].status, CaseStatus::ScriptError(_)));
}

#[test]
fn multiple_expectations_in_one_block_all_evaluated() {
    let script = make_script(vec![Case {
        name: "multi expect".to_string(),
        steps: vec![action("true"), assert_exits(&[0, 0])],
    }]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
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
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
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
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
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
            write_step("must-not-exist.txt", "skipped"),
            assert_exit(0),
        ],
    }]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert!(matches!(result.cases[0].status, CaseStatus::Fail));
    // Only the first action should have executed.
    assert_eq!(result.cases[0].actions.len(), 1);
    assert_eq!(result.cases[0].assertion_blocks.len(), 1);
    assert_eq!(result.cases[0].side_effects_executed, 0);
}

#[test]
fn before_each_file_is_visible_at_initial_checkpoint_and_counted() {
    let script = Script {
        before_each: Some(before_each_writing("seed.txt", "seed\n")),
        cases: vec![Case {
            name: "sees setup".to_string(),
            steps: vec![assert_file_exists_step("seed.txt")],
        }],
    };
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert!(matches!(result.cases[0].status, CaseStatus::Pass));
    assert_eq!(result.cases[0].side_effects_executed, 1);
}

#[test]
fn before_each_replays_into_every_concrete_case_workspace() {
    // The first case removes the seeded file; if `before_each` were shared
    // state rather than replayed per concrete case, the second case's
    // existence assertion would fail.
    let script = Script {
        before_each: Some(before_each_writing("seed.txt", "seed\n")),
        cases: vec![
            Case {
                name: "removes the seed".to_string(),
                steps: vec![action("rm seed.txt"), assert_exit(0)],
            },
            Case {
                name: "still sees the seed".to_string(),
                steps: vec![assert_file_exists_step("seed.txt")],
            },
        ],
    };
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert!(matches!(result.cases[0].status, CaseStatus::Pass));
    assert!(matches!(result.cases[1].status, CaseStatus::Pass));
}

#[test]
fn before_each_write_failure_is_runtime_error_without_case_step_index() {
    // Two `before_each` writes to the same path: the second violates the
    // create-only overwrite policy. The failure belongs to the
    // module-level block, not to any case body step, so `step_index` is
    // absent and the message carries the position inside `before_each`.
    let before_each = BeforeEach::new(vec![
        SideEffectingStep::WriteFile(WriteFileStep {
            path: WorkspacePath::parse("a.txt").unwrap(),
            content: TextLiteral::Quoted("first".to_string()),
        }),
        SideEffectingStep::WriteFile(WriteFileStep {
            path: WorkspacePath::parse("a.txt").unwrap(),
            content: TextLiteral::Quoted("second".to_string()),
        }),
    ])
    .unwrap();
    let script = Script {
        before_each: Some(before_each),
        cases: vec![Case {
            name: "never runs its body".to_string(),
            steps: vec![action("true"), assert_exit(0)],
        }],
    };
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    let CaseStatus::RuntimeError(runtime_error) = &result.cases[0].status else {
        panic!(
            "expected CaseStatus::RuntimeError, got {:?}",
            result.cases[0].status
        );
    };
    assert!(
        runtime_error.message.contains("before_each write step 2"),
        "message must name the failing before_each step: {}",
        runtime_error.message
    );
    assert_eq!(runtime_error.step_index, None);
    assert_eq!(
        runtime_error.diagnostic_code,
        Some(DiagnosticCode::StepWriteTargetExists)
    );
    // The first write completed before the failure and is still counted.
    assert_eq!(result.cases[0].side_effects_executed, 1);
    assert!(result.cases[0].actions.is_empty());

    let case_write_failure = make_script(vec![Case {
        name: "write fails in case body".to_string(),
        steps: vec![
            write_step("a.txt", "first"),
            write_step("a.txt", "second"),
            assert_file_exists_step("a.txt"),
        ],
    }]);
    let result = evaluate(
        &case_write_failure,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    let CaseStatus::RuntimeError(runtime_error) = &result.cases[0].status else {
        panic!("expected CaseStatus::RuntimeError");
    };
    assert_eq!(runtime_error.step_index, Some(1));
    assert_eq!(
        runtime_error.diagnostic_code,
        Some(DiagnosticCode::StepWriteTargetExists)
    );
    assert_eq!(result.cases[0].side_effects_executed, 1);
}
