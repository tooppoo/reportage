use super::*;

#[test]
fn file_text_equals_passes_when_actual_bytes_match_quoted_expected() {
    let script = single_case(vec![
        write_step("actual.txt", "hello"),
        assert_file_text_equals("actual.txt", "hello"),
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
fn file_text_equals_passes_when_actual_bytes_match_heredoc_expected() {
    // Runtime semantics are transparent to literal form: a heredoc expected value compares
    // identically to the same text written as a quoted string literal.
    let script = single_case(vec![
        write_step("actual.txt", "hello\nworld\n"),
        assert_file_text_equals_heredoc("actual.txt", "hello\nworld\n"),
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
fn file_text_equals_passes_when_both_sides_are_empty() {
    let script = single_case(vec![
        write_step("actual.txt", ""),
        assert_file_text_equals("actual.txt", ""),
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
fn file_text_equals_fails_on_single_byte_mismatch() {
    let script = single_case(vec![
        write_step("actual.txt", "hello"),
        assert_file_text_equals("actual.txt", "hellp"),
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
    let ExpectationKind::FileTextEquals { observation, .. } = &expectation.kind else {
        panic!("expected ExpectationKind::FileTextEquals");
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
fn file_text_equals_fails_on_heredoc_mismatch() {
    // Mirrors `file_text_equals_fails_on_single_byte_mismatch`, but with a heredoc expected
    // value: failure classification and `expected_source` must both reflect the heredoc
    // literal form, not just the quoted-string form the sibling test covers.
    let script = single_case(vec![
        write_step("actual.txt", "hello\nworld\n"),
        assert_file_text_equals_heredoc("actual.txt", "hello\nWORLD\n"),
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
    let ExpectationKind::FileTextEquals {
        expected_source,
        observation,
        ..
    } = &expectation.kind
    else {
        panic!("expected ExpectationKind::FileTextEquals");
    };
    assert_eq!(
        *expected_source,
        TextEqualsExpectedSource::Heredoc("hello\nWORLD\n".to_string())
    );
    let ContentsEqualsObservation::Compared(comparison) = observation else {
        panic!("expected ContentsEqualsObservation::Compared");
    };
    assert!(matches!(
        comparison.outcome,
        ContentsEqualsOutcome::Mismatch(_)
    ));
}

#[test]
fn file_text_equals_detects_missing_trailing_newline_as_mismatch() {
    let script = single_case(vec![
        write_step("actual.txt", "hello"),
        assert_file_text_equals("actual.txt", "hello\n"),
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
fn file_text_equals_detects_crlf_vs_lf_as_mismatch() {
    let script = single_case(vec![
        write_step("actual.txt", "hello\n"),
        assert_file_text_equals("actual.txt", "hello\r\n"),
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
fn file_text_equals_detects_unicode_normalization_difference_as_mismatch() {
    // NFC "é" (U+00E9) vs. NFD "e" + combining acute (U+0065 U+0301): visually identical,
    // distinct UTF-8 bytes. text_equals performs no normalization of any kind.
    let nfc = "caf\u{00e9}";
    let nfd = "cafe\u{0301}";
    let script = single_case(vec![
        write_step("actual.txt", nfc),
        assert_file_text_equals("actual.txt", nfd),
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
fn file_text_equals_missing_actual_is_assertion_failure_not_script_error() {
    let script = single_case(vec![assert_file_text_equals("does-not-exist.txt", "hello")]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert!(matches!(result.cases[0].status, CaseStatus::Fail));
    let expectation = &result.cases[0].assertion_blocks[0].expectations[0];
    let ExpectationKind::FileTextEquals { observation, .. } = &expectation.kind else {
        panic!("expected ExpectationKind::FileTextEquals");
    };
    assert_eq!(*observation, ContentsEqualsObservation::ActualMissing);
}

#[test]
fn file_text_equals_actual_directory_is_assertion_failure_not_script_error() {
    let script = single_case(vec![
        action("mkdir a-dir"),
        assert_file_text_equals("a-dir", "hello"),
    ]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    assert!(matches!(result.cases[0].status, CaseStatus::Fail));
    let expectation = &result.cases[0].assertion_blocks[0].expectations[0];
    let ExpectationKind::FileTextEquals { observation, .. } = &expectation.kind else {
        panic!("expected ExpectationKind::FileTextEquals");
    };
    assert_eq!(
        *observation,
        ContentsEqualsObservation::ActualNotRegularFile
    );
}

#[test]
fn file_text_equals_expected_source_reflects_literal_kind() {
    let script = single_case(vec![
        write_step("actual.txt", "hello"),
        assert_file_text_equals("actual.txt", "hello"),
    ]);
    let result = evaluate(
        &script,
        &default_env(),
        Path::new("test.repor"),
        &default_commands(),
    );
    let expectation = &result.cases[0].assertion_blocks[0].expectations[0];
    let ExpectationKind::FileTextEquals {
        expected_source, ..
    } = &expectation.kind
    else {
        panic!("expected ExpectationKind::FileTextEquals");
    };
    assert_eq!(
        *expected_source,
        TextEqualsExpectedSource::Quoted("hello".to_string())
    );
}

// ─── `before_each` case-local setup (#70) ──────────────────────────────
