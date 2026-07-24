use super::*;

#[test]
fn stdout_empty_passes_on_zero_bytes() {
    let checkpoint = checkpoint_after_output(vec![], vec![]);
    let result =
        evaluate_expectation_at_checkpoint(&stdout_empty_expectation(), &checkpoint).unwrap();
    assert!(result.passed);
}

// Whitespace-only output is still output: `empty` must observe zero bytes, not
// "nothing but whitespace". Regression coverage for the `.trim().is_empty()` bug.
#[test]
fn stdout_empty_fails_on_single_space() {
    let checkpoint = checkpoint_after_output(b" ".to_vec(), vec![]);
    let result =
        evaluate_expectation_at_checkpoint(&stdout_empty_expectation(), &checkpoint).unwrap();
    assert!(!result.passed);
}

#[test]
fn stdout_empty_fails_on_tab() {
    let checkpoint = checkpoint_after_output(b"\t".to_vec(), vec![]);
    let result =
        evaluate_expectation_at_checkpoint(&stdout_empty_expectation(), &checkpoint).unwrap();
    assert!(!result.passed);
}

#[test]
fn stdout_empty_fails_on_lf() {
    let checkpoint = checkpoint_after_output(b"\n".to_vec(), vec![]);
    let result =
        evaluate_expectation_at_checkpoint(&stdout_empty_expectation(), &checkpoint).unwrap();
    assert!(!result.passed);
}

#[test]
fn stdout_empty_fails_on_crlf() {
    let checkpoint = checkpoint_after_output(b"\r\n".to_vec(), vec![]);
    let result =
        evaluate_expectation_at_checkpoint(&stdout_empty_expectation(), &checkpoint).unwrap();
    assert!(!result.passed);
}

#[test]
fn stdout_empty_fails_on_bare_cr() {
    let checkpoint = checkpoint_after_output(b"\r".to_vec(), vec![]);
    let result =
        evaluate_expectation_at_checkpoint(&stdout_empty_expectation(), &checkpoint).unwrap();
    assert!(!result.passed);
}

#[test]
fn stderr_empty_fails_on_whitespace_only() {
    let checkpoint = checkpoint_after_output(vec![], b" \t\r\n".to_vec());
    let result =
        evaluate_expectation_at_checkpoint(&stderr_empty_expectation(), &checkpoint).unwrap();
    assert!(!result.passed);
}

#[test]
fn stdout_contains_matches_substring_in_non_utf8_output() {
    // 0xff is invalid UTF-8 in any position. A lossy decode at capture time would have
    // replaced it with U+FFFD before this match ever ran; raw byte matching must not do that.
    let mut stdout = b"ok".to_vec();
    stdout.push(0xff);
    let checkpoint = checkpoint_after_output(stdout, vec![]);
    let expectation = Expectation::Stdout(crate::model::OutputExpectation {
        matcher: OutputMatcher::Contains("ok".to_string()),
    });
    let result = evaluate_expectation_at_checkpoint(&expectation, &checkpoint).unwrap();
    assert!(result.passed);
}

// ─── stdout/stderr `text_equals` evaluation ─────────────────────────────

fn stdout_text_equals_expectation(literal: TextLiteral) -> Expectation {
    Expectation::Stdout(crate::model::OutputExpectation {
        matcher: OutputMatcher::TextEquals(literal),
    })
}

fn stderr_text_equals_expectation(literal: TextLiteral) -> Expectation {
    Expectation::Stderr(crate::model::OutputExpectation {
        matcher: OutputMatcher::TextEquals(literal),
    })
}

#[test]
fn stdout_text_equals_passes_on_byte_for_byte_match() {
    let checkpoint = checkpoint_after_output(b"hello\n".to_vec(), vec![]);
    let expectation = stdout_text_equals_expectation(TextLiteral::Quoted("hello\n".to_string()));
    let result = evaluate_expectation_at_checkpoint(&expectation, &checkpoint).unwrap();
    assert!(result.passed);
    let ExpectationKind::StdoutTextEquals {
        expected_source, ..
    } = &result.kind
    else {
        panic!("expected ExpectationKind::StdoutTextEquals");
    };
    assert_eq!(
        *expected_source,
        TextEqualsExpectedSource::Quoted("hello\n".to_string())
    );
}

#[test]
fn stdout_text_equals_detects_missing_trailing_newline_as_mismatch() {
    let checkpoint = checkpoint_after_output(b"hello\n".to_vec(), vec![]);
    let expectation = stdout_text_equals_expectation(TextLiteral::Quoted("hello".to_string()));
    let result = evaluate_expectation_at_checkpoint(&expectation, &checkpoint).unwrap();
    assert!(!result.passed);
    let ExpectationKind::StdoutTextEquals { comparison, .. } = &result.kind else {
        panic!("expected ExpectationKind::StdoutTextEquals");
    };
    assert_eq!(
        comparison.outcome,
        ContentsEqualsOutcome::Mismatch(crate::result::ContentsMismatch {
            actual_len: 6,
            expected_len: 5,
            first_diff_offset: 5,
        })
    );
}

#[test]
fn stderr_text_equals_heredoc_compares_identically_to_quoted() {
    // String literal and heredoc literal are transparent to the comparison: both resolve
    // to the same TextValue before bytes are compared. See docs/adr — text_equals evaluation.
    let checkpoint = checkpoint_after_output(vec![], b"warn\nline\n".to_vec());
    for literal in [
        TextLiteral::Quoted("warn\nline\n".to_string()),
        TextLiteral::Heredoc("warn\nline\n".to_string()),
    ] {
        let expectation = stderr_text_equals_expectation(literal);
        let result = evaluate_expectation_at_checkpoint(&expectation, &checkpoint).unwrap();
        assert!(result.passed);
    }
}

#[test]
fn stderr_text_equals_heredoc_mismatch_reports_heredoc_expected_source() {
    let checkpoint = checkpoint_after_output(vec![], b"warn\n".to_vec());
    let expectation = stderr_text_equals_expectation(TextLiteral::Heredoc("other\n".to_string()));
    let result = evaluate_expectation_at_checkpoint(&expectation, &checkpoint).unwrap();
    assert!(!result.passed);
    let ExpectationKind::StderrTextEquals {
        expected_source, ..
    } = &result.kind
    else {
        panic!("expected ExpectationKind::StderrTextEquals");
    };
    assert_eq!(
        *expected_source,
        TextEqualsExpectedSource::Heredoc("other\n".to_string())
    );
}

#[test]
fn stdout_text_equals_compares_raw_bytes_against_non_utf8_output() {
    // Raw byte semantics: non-UTF-8 captured output participates in the comparison
    // unmodified, so it can only ever mismatch a (UTF-8) expected TextValue — never panic
    // or lossily decode.
    let mut stdout = b"ok".to_vec();
    stdout.push(0xff);
    let checkpoint = checkpoint_after_output(stdout, vec![]);
    let expectation = stdout_text_equals_expectation(TextLiteral::Quoted("ok".to_string()));
    let result = evaluate_expectation_at_checkpoint(&expectation, &checkpoint).unwrap();
    assert!(!result.passed);
}

#[test]
fn stdout_text_equals_passes_when_output_and_expected_are_both_empty() {
    let checkpoint = checkpoint_after_output(vec![], vec![]);
    let expectation = stdout_text_equals_expectation(TextLiteral::Quoted(String::new()));
    let result = evaluate_expectation_at_checkpoint(&expectation, &checkpoint).unwrap();
    assert!(result.passed);
}
