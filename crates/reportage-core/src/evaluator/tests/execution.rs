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
fn an_assertion_block_origin_counts_every_case_body_step_kind() {
    // The block is preceded by a write and an action, so a step index that
    // counted only assertion blocks — or only actions — would report 0 instead
    // of 2. Every step kind is counted, phase-local and 0-based.
    let script = make_script(vec![Case {
        name: "mixed step kinds".to_string(),
        steps: vec![
            write_step("a.txt", "x"),
            action("true"),
            assert_exit(0),
            assert_file_exists_step("a.txt"),
        ],
    }]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert!(matches!(result.cases[0].status, CaseStatus::Pass));
    let origins: Vec<StepOrigin> = result.cases[0]
        .assertion_blocks
        .iter()
        .map(|block| block.origin)
        .collect();
    assert_eq!(
        origins,
        vec![
            StepOrigin::new(StepPhase::Case, 2),
            StepOrigin::new(StepPhase::Case, 3)
        ],
        "case body blocks carry StepPhase::Case and their own step position"
    );
}

#[test]
fn a_script_error_is_attributed_to_the_step_that_raised_it() {
    // The process expectation sits at step 1, after a write. A `ScriptError`
    // must name that step, not the case as a whole and not the first step.
    let script = make_script(vec![Case {
        name: "process expectation before any action".to_string(),
        steps: vec![write_step("a.txt", "x"), assert_exit(0)],
    }]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    let CaseStatus::ScriptError(script_error) = &result.cases[0].status else {
        panic!(
            "expected CaseStatus::ScriptError, got {:?}",
            result.cases[0].status
        );
    };
    assert_eq!(
        script_error.diagnostic_code,
        Some(DiagnosticCode::SemanticExpectationRequiresAction)
    );
    assert_eq!(
        script_error.origin,
        Some(StepOrigin::new(StepPhase::Case, 1))
    );
}

#[test]
fn an_aborting_case_still_reports_the_evidence_gathered_before_the_abort() {
    // The second write violates the create-only policy, aborting the case after
    // an action and a passing assertion block have already produced evidence.
    // An aborted case reports that evidence exactly as a completed one does:
    // the artifact's per-case actions/assertions and the run summary counts
    // must not silently drop steps that did run.
    let script = make_script(vec![Case {
        name: "aborts after evidence".to_string(),
        steps: vec![
            action("true"),
            assert_exit(0),
            write_step("a.txt", "first"),
            write_step("a.txt", "second"),
            assert_file_exists_step("a.txt"),
        ],
    }]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert!(matches!(
        result.cases[0].status,
        CaseStatus::RuntimeError(_)
    ));
    assert_eq!(result.cases[0].actions.len(), 1);
    assert_eq!(result.cases[0].assertion_blocks.len(), 1);
    assert!(!result.cases[0].assertion_blocks[0].has_failures());
    assert_eq!(result.cases[0].side_effects_executed, 1);
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
fn before_each_runs_actions_and_assertions_in_source_order() {
    // A setup action, the assertion that verifies it, and a write that depends
    // on it. The case body then observes the workspace the setup produced.
    let before_each = BeforeEach::new(vec![
        action("mkdir -p fixtures"),
        assert_exit(0),
        write_step("fixtures/seed.txt", "seed\n"),
    ])
    .unwrap();
    let script = Script {
        before_each: Some(before_each),
        cases: vec![Case {
            name: "sees the setup result".to_string(),
            steps: vec![assert_file_exists_step("fixtures/seed.txt")],
        }],
    };
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert!(
        matches!(result.cases[0].status, CaseStatus::Pass),
        "{:?}",
        result.cases[0].status
    );
    assert_eq!(result.cases[0].actions.len(), 1);
    assert_eq!(result.cases[0].side_effects_executed, 1);
    // The setup assertion is reported with the rest of the case's evidence.
    assert_eq!(
        result.cases[0]
            .assertion_blocks
            .iter()
            .map(|block| block.origin)
            .collect::<Vec<_>>(),
        vec![
            StepOrigin::new(StepPhase::BeforeEach, 1),
            StepOrigin::new(StepPhase::Case, 0),
        ]
    );
}

#[test]
fn an_action_record_names_the_phase_and_step_it_ran_from() {
    // Each action is preceded by a different number of non-action steps, so
    // the phase-local step index differs from the action's position in
    // `actions` for both of them: a step index counted as an action ordinal —
    // per phase or across the case — would report 0 and 1 instead of 1 and 2.
    let before_each = BeforeEach::new(vec![
        write_step("setup.txt", "s"),
        action("true"),
        assert_exit(0),
    ])
    .unwrap();
    let script = Script {
        before_each: Some(before_each),
        cases: vec![Case {
            name: "acts after two writes".to_string(),
            steps: vec![
                write_step("a.txt", "x"),
                write_step("b.txt", "y"),
                action("true"),
                assert_exit(0),
            ],
        }],
    };
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert!(
        matches!(result.cases[0].status, CaseStatus::Pass),
        "{:?}",
        result.cases[0].status
    );
    assert_eq!(
        result.cases[0]
            .actions
            .iter()
            .map(|action| action.origin)
            .collect::<Vec<_>>(),
        vec![
            StepOrigin::new(StepPhase::BeforeEach, 1),
            StepOrigin::new(StepPhase::Case, 2),
        ]
    );
}

#[test]
fn the_body_entry_checkpoint_keeps_workspace_state_and_drops_process_evidence() {
    // The case body's first assertion sees the file the setup action created,
    // but a process expectation there has no action of its own to describe:
    // the last setup action's exit code must not answer for the case body.
    // `BeforeEach` is not `Clone`, so each script builds its own.
    let setup =
        || BeforeEach::new(vec![action("echo setup > from-setup.txt"), assert_exit(0)]).unwrap();
    let workspace_first = Script {
        before_each: Some(setup()),
        cases: vec![Case {
            name: "observes the setup action's workspace effect".to_string(),
            steps: vec![assert_file_exists_step("from-setup.txt")],
        }],
    };
    let result = evaluate(
        &workspace_first,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert!(
        matches!(result.cases[0].status, CaseStatus::Pass),
        "{:?}",
        result.cases[0].status
    );

    let process_first = Script {
        before_each: Some(setup()),
        cases: vec![Case {
            name: "cannot reach the setup action's exit code".to_string(),
            steps: vec![assert_exit(0)],
        }],
    };
    let result = evaluate(
        &process_first,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    let CaseStatus::ScriptError(script_error) = &result.cases[0].status else {
        panic!(
            "expected CaseStatus::ScriptError, got {:?}",
            result.cases[0].status
        );
    };
    assert_eq!(
        script_error.diagnostic_code,
        Some(DiagnosticCode::SemanticExpectationRequiresAction)
    );
    assert_eq!(
        script_error.origin,
        Some(StepOrigin::new(StepPhase::Case, 0))
    );
}

#[test]
fn binding_provenance_numbers_actions_across_the_whole_concrete_case() {
    // Provenance uses the concrete case's own action numbering, not a
    // phase-relative counter: the `before_each` binding names action 0 and the
    // case body binding names action 1. Index 0 alone would not tell the two
    // numbering schemes apart, so both are asserted.
    let before_each = BeforeEach::new(vec![
        action("printf 'from-setup'"),
        binding_step("captured"),
    ])
    .unwrap();
    let script = Script {
        before_each: Some(before_each),
        cases: vec![Case {
            name: "uses both bindings".to_string(),
            steps: vec![
                action("printf 'from-body'"),
                binding_step("from_body"),
                Step::AssertionBlock(
                    AssertionBlock::new(vec![
                        stdout_text_equals_binding("captured"),
                        stdout_text_equals_binding("from_body"),
                    ])
                    .unwrap(),
                ),
            ],
        }],
    };
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    // The setup output differs from the body output, so the first expectation
    // fails and the second passes — what matters here is the provenance each
    // one reports.
    assert!(matches!(result.cases[0].status, CaseStatus::Fail));
    assert_eq!(result.cases[0].actions.len(), 2);
    let expectations = &result.cases[0].assertion_blocks[0].expectations;
    assert_eq!(
        binding_provenance(&expectations[0]),
        ("captured".to_string(), 0),
        "a before_each binding names the setup action"
    );
    assert_eq!(
        binding_provenance(&expectations[1]),
        ("from_body".to_string(), 1),
        "a case body binding names the case body action"
    );
}

/// The binding name and provenance action index a `stdout text_equals &name`
/// expectation reports.
fn binding_provenance(expectation: &ExpectationResult) -> (String, usize) {
    let ExpectationKind::StdoutTextEquals {
        expected_source: TextValueProvenance::Binding { name, source },
        ..
    } = &expectation.kind
    else {
        panic!(
            "expected a binding-sourced stdout text_equals, got {:?}",
            expectation.kind
        );
    };
    (name.clone(), source.action_index)
}

#[test]
fn a_before_each_assertion_failure_fails_only_its_own_concrete_case() {
    // `false` makes the setup assertion fail for both cases. Each concrete
    // case fails on its own; neither runs its body, and the first failure does
    // not stop the second case from being set up and reported.
    let before_each = BeforeEach::new(vec![action("false"), assert_exit(0)]).unwrap();
    let script = Script {
        before_each: Some(before_each),
        cases: vec![
            Case {
                name: "first".to_string(),
                steps: vec![action("touch body-ran.txt"), assert_exit(0)],
            },
            Case {
                name: "second".to_string(),
                steps: vec![action("touch body-ran.txt"), assert_exit(0)],
            },
        ],
    };
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert_eq!(result.cases.len(), 2);
    for case in &result.cases {
        assert!(matches!(case.status, CaseStatus::Fail), "{:?}", case.status);
        // Only the setup action ran: the case body never started.
        assert_eq!(case.actions.len(), 1);
        assert_eq!(case.assertion_blocks.len(), 1);
        assert_eq!(
            case.assertion_blocks[0].origin,
            StepOrigin::new(StepPhase::BeforeEach, 1)
        );
    }
    assert_eq!(result.exit_code(), 1);
}

#[test]
fn before_each_write_failure_is_attributed_to_the_before_each_phase() {
    // Two `before_each` writes to the same path: the second violates the
    // create-only overwrite policy. The failure belongs to the concrete case
    // that was running the setup, and its origin names the `before_each`
    // phase and the failing step's position within that block.
    let before_each = BeforeEach::new(vec![
        write_step("a.txt", "first"),
        write_step("a.txt", "second"),
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
        runtime_error
            .message
            .contains("before_each write step at step 2"),
        "message must name the failing before_each step: {}",
        runtime_error.message
    );
    assert_eq!(
        runtime_error.origin,
        Some(StepOrigin::new(StepPhase::BeforeEach, 1))
    );
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
    assert_eq!(
        runtime_error.origin,
        Some(StepOrigin::new(StepPhase::Case, 1))
    );
    assert_eq!(
        runtime_error.diagnostic_code,
        Some(DiagnosticCode::StepWriteTargetExists)
    );
    assert_eq!(result.cases[0].side_effects_executed, 1);

    let initialization_script = make_script(vec![Case {
        name: "initialization failure".to_string(),
        steps: vec![assert_file_exists_step("unused.txt")],
    }]);
    let result = super::super::execution::evaluate_with(
        &initialization_script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
        || Err(std::io::Error::other("workspace unavailable")),
        super::super::execution::build_case_execution_environment,
    );
    let CaseStatus::RuntimeError(runtime_error) = &result.cases[0].status else {
        panic!("expected CaseStatus::RuntimeError");
    };
    assert!(
        runtime_error
            .message
            .contains("failed to create isolated case workspace"),
        "message must identify case-workspace initialization: {}",
        runtime_error.message
    );

    let result = super::super::execution::evaluate_with(
        &initialization_script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
        crate::workspace::Workspace::new,
        |_, _, _| Err(std::io::Error::other("shim unavailable")),
    );
    let CaseStatus::RuntimeError(runtime_error) = &result.cases[0].status else {
        panic!("expected CaseStatus::RuntimeError");
    };
    assert!(
        runtime_error
            .message
            .contains("failed to set up registered command shims"),
        "message must identify command-shim initialization: {}",
        runtime_error.message
    );
}

/// Parser-side scope validation guarantees a `write` step's binding references
/// resolve, so this state is unreachable from any accepted script. The runtime
/// still reports it as a runtime error rather than panicking, and only a
/// hand-built model can reach that branch.
#[test]
fn a_write_step_whose_binding_never_resolves_is_a_runtime_error() {
    let script = make_script(vec![Case {
        name: "unresolvable write content".to_string(),
        steps: vec![
            action("true"),
            Step::SideEffect(SideEffectingStep::WriteFile(WriteFileStep {
                path: WorkspacePath::parse("out.txt").unwrap(),
                mode: None,
                content: TextValueExpression::Binding(crate::model::BindingReference {
                    name: "never_captured".to_string(),
                    reference_span: crate::model::LocatedSpan {
                        start: 0,
                        end: 0,
                        line: 1,
                        column: 1,
                    },
                }),
            })),
            assert_exit(0),
        ],
    }]);

    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );

    assert_eq!(result.exit_code(), 3);
    let CaseStatus::RuntimeError(runtime_error) = &result.cases[0].status else {
        panic!(
            "expected CaseStatus::RuntimeError, got {:?}",
            result.cases[0].status
        );
    };
    assert!(
        runtime_error
            .message
            .contains("could not resolve its content"),
        "message must identify the write step's content: {}",
        runtime_error.message
    );
    assert_eq!(
        runtime_error.diagnostic_code.map(|code| code.as_str()),
        Some("semantic.binding.undefined")
    );
}

// Pins the evaluator-to-workspace seam for a write step's `mode`: the step's
// own mode must reach `Workspace::write_file`, not a hardcoded `None`. Observed
// through an action rather than by inspecting the workspace directly, because
// the workspace is internal to the evaluator and dropped with the case.
#[test]
#[cfg(unix)]
fn write_step_mode_reaches_the_workspace() {
    let script = single_case(vec![
        write_step_with_mode(
            "bin/tool",
            "#!/bin/sh\n",
            Some(FileMode::from_bits(0o755).unwrap()),
        ),
        action("test -x bin/tool"),
        assert_exit(0),
    ]);

    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );

    assert!(
        matches!(result.cases[0].status, CaseStatus::Pass),
        "the written file must be executable: {:?}",
        result.cases[0].status
    );
}

// The companion to the test above: without a mode the same content is not
// executable, so the assertion above cannot pass for a reason unrelated to the
// mode.
#[test]
#[cfg(unix)]
fn write_step_without_a_mode_leaves_the_file_non_executable() {
    let script = single_case(vec![
        write_step("bin/tool", "#!/bin/sh\n"),
        action("test -x bin/tool"),
        assert_exit(0),
    ]);

    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );

    assert!(
        matches!(result.cases[0].status, CaseStatus::Fail),
        "expected the executable check to fail: {:?}",
        result.cases[0].status
    );
}
