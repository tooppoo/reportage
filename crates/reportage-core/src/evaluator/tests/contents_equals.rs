use super::*;

#[test]
fn file_contents_equals_passes_when_actual_and_expected_workspace_bytes_match() {
    let script = single_case(vec![
        write_step("actual.txt", "hello"),
        write_step("expected.txt", "hello"),
        assert_file_contents_equals_workspace("actual.txt", "expected.txt"),
    ]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert!(matches!(result.cases[0].status, CaseStatus::Pass));
}

#[test]
fn file_contents_equals_passes_when_both_workspace_files_are_empty() {
    let script = single_case(vec![
        write_step("actual.txt", ""),
        write_step("expected.txt", ""),
        assert_file_contents_equals_workspace("actual.txt", "expected.txt"),
    ]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert!(matches!(result.cases[0].status, CaseStatus::Pass));
}

#[test]
fn file_contents_equals_fails_on_single_byte_mismatch() {
    let script = single_case(vec![
        write_step("actual.txt", "hello"),
        write_step("expected.txt", "hellp"),
        assert_file_contents_equals_workspace("actual.txt", "expected.txt"),
    ]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert!(matches!(result.cases[0].status, CaseStatus::Fail));
    let expectation = &result.cases[0].assertion_blocks[0].expectations[0];
    assert!(!expectation.passed);
    let ExpectationKind::FileContentsEquals { observation, .. } = &expectation.kind else {
        panic!("expected ExpectationKind::FileContentsEquals");
    };
    let ContentsEqualsObservation::Compared(comparison) = observation else {
        panic!("expected ContentsEqualsObservation::Compared");
    };
    assert_eq!(
        comparison.outcome,
        ContentsEqualsOutcome::Mismatch(crate::result::ContentsMismatch {
            actual_len: 5,
            expected_len: 5,
            first_diff_offset: 4,
        })
    );
}

#[test]
fn file_contents_equals_detects_missing_trailing_newline_as_mismatch() {
    let script = single_case(vec![
        write_step("actual.txt", "hello"),
        write_step("expected.txt", "hello\n"),
        assert_file_contents_equals_workspace("actual.txt", "expected.txt"),
    ]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert!(matches!(result.cases[0].status, CaseStatus::Fail));
}

#[test]
fn file_contents_equals_detects_crlf_vs_lf_as_mismatch() {
    let script = single_case(vec![
        write_step("actual.txt", "hello\n"),
        write_step("expected.txt", "hello\r\n"),
        assert_file_contents_equals_workspace("actual.txt", "expected.txt"),
    ]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert!(matches!(result.cases[0].status, CaseStatus::Fail));
}

#[test]
fn file_contents_equals_missing_actual_is_assertion_failure_not_script_error() {
    let script = single_case(vec![
        write_step("expected.txt", "hello"),
        assert_file_contents_equals_workspace("does-not-exist.txt", "expected.txt"),
    ]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert!(matches!(result.cases[0].status, CaseStatus::Fail));
    let expectation = &result.cases[0].assertion_blocks[0].expectations[0];
    let ExpectationKind::FileContentsEquals { observation, .. } = &expectation.kind else {
        panic!("expected ExpectationKind::FileContentsEquals");
    };
    assert_eq!(*observation, ContentsEqualsObservation::ActualMissing);
}

#[test]
fn file_contents_equals_actual_directory_is_assertion_failure_not_script_error() {
    let script = single_case(vec![
        action("mkdir a-dir"),
        write_step("expected.txt", "hello"),
        assert_file_contents_equals_workspace("a-dir", "expected.txt"),
    ]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert!(matches!(result.cases[0].status, CaseStatus::Fail));
    let expectation = &result.cases[0].assertion_blocks[0].expectations[0];
    let ExpectationKind::FileContentsEquals { observation, .. } = &expectation.kind else {
        panic!("expected ExpectationKind::FileContentsEquals");
    };
    assert_eq!(
        *observation,
        ContentsEqualsObservation::ActualNotRegularFile
    );
}

#[test]
fn file_contents_equals_missing_expected_workspace_path_is_script_error_exit_two() {
    let script = single_case(vec![
        write_step("actual.txt", "hello"),
        assert_file_contents_equals_workspace("actual.txt", "does-not-exist.txt"),
    ]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert_eq!(result.exit_code(), 2);
    let CaseStatus::ScriptError(err) = &result.cases[0].status else {
        panic!("expected CaseStatus::ScriptError");
    };
    assert_eq!(
        err.diagnostic_code,
        Some(DiagnosticCode::SemanticFileContentsReferenceMissing)
    );

    let fixture_dir = tempfile::TempDir::new().unwrap();
    let checkpoint = Checkpoint::initial(
        fixture_dir.path().to_path_buf(),
        fixture_dir.path().to_path_buf(),
    );
    let expectation = Expectation::File(FileExpectation {
        path: "actual.txt".to_string(),
        matcher: FileMatcher::ContentsEquals(FileContentsReference::Fixture(
            FixtureReference::parse("missing.fixture").unwrap(),
        )),
    });
    let error = evaluate_expectation_at_checkpoint(&expectation, &checkpoint).unwrap_err();
    assert_eq!(
        error.diagnostic_code,
        DiagnosticCode::SemanticFixtureReferenceMissing
    );
}

#[test]
fn file_contents_equals_expected_workspace_path_is_directory_is_script_error() {
    let script = single_case(vec![
        action("mkdir expected-dir"),
        write_step("actual.txt", "hello"),
        assert_file_contents_equals_workspace("actual.txt", "expected-dir"),
    ]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert_eq!(result.exit_code(), 2);
    let CaseStatus::ScriptError(err) = &result.cases[0].status else {
        panic!("expected CaseStatus::ScriptError");
    };
    assert_eq!(
        err.diagnostic_code,
        Some(DiagnosticCode::SemanticFileContentsReferenceNotARegularFile)
    );
}

#[test]
fn stdout_contents_equals_passes_on_matching_bytes() {
    let script = single_case(vec![
        write_step("expected.txt", "hello"),
        action("printf hello"),
        assert_stdout_contents_equals_workspace("expected.txt"),
    ]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert!(matches!(result.cases[0].status, CaseStatus::Pass));
}

#[test]
fn stdout_contents_equals_fails_on_mismatched_bytes() {
    let script = single_case(vec![
        write_step("expected.txt", "world"),
        action("printf hello"),
        assert_stdout_contents_equals_workspace("expected.txt"),
    ]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert!(matches!(result.cases[0].status, CaseStatus::Fail));
}

#[test]
fn stderr_contents_equals_passes_on_matching_bytes() {
    let script = single_case(vec![
        write_step("expected.txt", "oops"),
        action("printf oops 1>&2"),
        assert_stderr_contents_equals_workspace("expected.txt"),
    ]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert!(matches!(result.cases[0].status, CaseStatus::Pass));
}

#[test]
fn stdout_contents_equals_missing_expected_workspace_path_is_script_error_exit_two() {
    let script = single_case(vec![
        action("printf hello"),
        assert_stdout_contents_equals_workspace("does-not-exist.txt"),
    ]);
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
fn contents_equals_script_error_inside_logical_composition_aborts_the_case() {
    // A `contents_equals` expected-value error nested inside `all { ... }` must abort the
    // whole case as a script error, not be swallowed as an ordinary failing child.
    let script = single_case(vec![
        action("true"),
        write_step("actual.txt", "hello"),
        Step::AssertionBlock(
            AssertionBlock::new(vec![logical(
                LogicalOperator::All,
                vec![
                    exit_exp(0),
                    Expectation::File(FileExpectation {
                        path: "actual.txt".to_string(),
                        matcher: FileMatcher::ContentsEquals(FileContentsReference::Workspace(
                            WorkspacePath::parse("does-not-exist.txt").unwrap(),
                        )),
                    }),
                ],
            )])
            .unwrap(),
        ),
    ]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert_eq!(result.exit_code(), 2);
    assert!(matches!(result.cases[0].status, CaseStatus::ScriptError(_)));
}

// ─── `text_equals` comparison evaluation (#88) ──────────────────────────
