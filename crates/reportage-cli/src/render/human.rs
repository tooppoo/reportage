//! The human-readable console renderer.
//!
//! This preserves the console output that `reportage run` produced before
//! runner and output rendering were separated (see #76): it is a renderer
//! over `ExecutionReport`, not a source of truth in its own right.

use reportage_core::contents_diagnostic::mismatch_context;
use reportage_core::result::{
    CaseStatus, ContentsEqualsComparison, ContentsEqualsExpectedSource, ContentsEqualsObservation,
    ContentsEqualsOutcome, DirContainsObservation, DirExistsObservation, ExecutionReport,
    ExpectationKind, ExpectationResult, FileContentObservation, FileErrorKind,
    FileExistsObservation, StepOrigin, StepPhase, TextValueProvenance,
};

use super::OutputRenderer;

pub struct HumanRenderer;

impl OutputRenderer for HumanRenderer {
    fn render(&self, report: &ExecutionReport) {
        print_results(report);
    }
}

fn print_results(result: &ExecutionReport) {
    if result.is_noop() {
        println!("NO-OP  no cases found; nothing was executed");
    }

    for error in &result.file_errors {
        let kind = match &error.kind {
            FileErrorKind::ReadError(_) => "READ ERROR",
            FileErrorKind::ParseError { .. } => "PARSE ERROR",
        };
        eprintln!("{kind}  {}", error.source_path.display());
        let (message, diagnostic_code) = match &error.kind {
            FileErrorKind::ReadError(msg) => (msg.as_str(), None),
            FileErrorKind::ParseError {
                message,
                diagnostic_code,
                location: _,
            } => (message.as_str(), Some(*diagnostic_code)),
        };
        eprintln!("  {message}");
        if let Some(code) = diagnostic_code {
            eprintln!("    diagnostic code: {}", code.as_str());
        }
    }

    for case in &result.cases {
        let tag = match &case.status {
            CaseStatus::Pass => "PASS",
            CaseStatus::Fail => "FAIL",
            CaseStatus::ScriptError(_) => "ERROR",
            CaseStatus::RuntimeError(_) => "ERROR",
        };

        let label = match &case.source_path {
            Some(path) => format!("{}  {} :: {}", tag, path.display(), case.name),
            None => format!("{tag}  {}", case.name),
        };
        println!("{label}");

        match &case.status {
            CaseStatus::ScriptError(err) => {
                eprintln!("  {}", err.message);
                if let Some(code) = err.diagnostic_code {
                    eprintln!("    diagnostic code: {}", code.as_str());
                }
            }
            CaseStatus::RuntimeError(err) => {
                eprintln!("  {}", err.message);
                if let Some(code) = err.diagnostic_code {
                    eprintln!("    diagnostic code: {}", code.as_str());
                }
            }
            _ => {}
        }

        for block in &case.assertion_blocks {
            for expectation in &block.expectations {
                if !expectation.passed {
                    print_failed_expectation(&step_label(block.origin), expectation);
                }
            }
        }

        // For failing cases, show observed shim invocations so the resolved shim path and target invocation are visible in diagnostics.
        // See ADR 20260628T210000Z_shim-invocation-event-side-channel.
        if matches!(&case.status, CaseStatus::Fail | CaseStatus::RuntimeError(_)) {
            for action in &case.actions {
                for ev in &action.shim_invocations {
                    eprintln!(
                        "  shim invoked for '{}': {} -> {}",
                        ev.command_name,
                        ev.shim_path.display(),
                        ev.target.program.display()
                    );
                    if !ev.target.args.is_empty() {
                        eprintln!("    target args: {:?}", ev.target.args);
                    }
                }
                for warning in &action.shim_event_parse_warnings {
                    eprintln!("  shim event warning: {warning}");
                }
            }
        }
    }
}

/// Names the assertion block a failure came from, as the prefix of every line
/// describing it.
///
/// A case running `before_each` has two independently numbered step lists, so
/// the step number alone is ambiguous: `before_each` step 1 and case body step
/// 1 are different steps of the same concrete case. The phase disambiguates
/// them. A case body label stays exactly what it was before `before_each` held
/// assertions, so existing output and expectations on it are unchanged.
///
/// Step numbers are 1-based here; `StepOrigin::step_index` is 0-based.
fn step_label(origin: StepOrigin) -> String {
    let step = origin.step_index + 1;
    match origin.phase {
        StepPhase::BeforeEach => format!("before_each assertion block at step {step}"),
        StepPhase::Case => format!("assertion block at step {step}"),
    }
}

/// Prints why one failed top-level expectation within an assertion block did not hold.
///
/// Recurses into a `not` / `all` / `any` composition's children, printing every child's own detail rather than filtering by the child's own pass/fail state.
/// This matters for `not`: when a `not` block fails, that means its (grouped) contents *held* — none of its children individually failed — so filtering for failed children would print nothing and lose the information needed to explain the negation's failure.
/// Always recursing into every child, described in its own held/did-not-hold terms, keeps `all` / `any` failures explainable too, without needing a separate per-operator rule for which children are "responsible".
fn print_failed_expectation(block: &str, expectation: &ExpectationResult) {
    print_expectation_detail(block, expectation);
    if let Some(code) = expectation.failure_diagnostic_code() {
        eprintln!("    diagnostic code: {}", code.as_str());
    }
}

fn print_expectation_detail(block: &str, expectation: &ExpectationResult) {
    let held = expectation.passed;
    match &expectation.kind {
        ExpectationKind::Exit { expected, actual } => {
            eprintln!("  {block}: expected exit {expected}, got {actual}",);
        }
        ExpectationKind::StdoutContains {
            expected_source,
            actual,
        } => {
            let verb = if held { "contains" } else { "does not contain" };
            eprintln!(
                "  {block}: stdout {verb} {}",
                format_text_equals_source(expected_source),
            );
            // Lossy decode is display-only here; the canonical actual value stays raw bytes.
            eprintln!("    actual stdout: {:?}", String::from_utf8_lossy(actual));
        }
        ExpectationKind::StderrContains {
            expected_source,
            actual,
        } => {
            let verb = if held { "contains" } else { "does not contain" };
            eprintln!(
                "  {block}: stderr {verb} {}",
                format_text_equals_source(expected_source),
            );
            eprintln!("    actual stderr: {:?}", String::from_utf8_lossy(actual));
        }
        ExpectationKind::StdoutEmpty { actual } => {
            let phrase = if held {
                "is empty"
            } else {
                "was expected to be empty"
            };
            eprintln!("  {block}: stdout {phrase}",);
            eprintln!("    actual stdout: {:?}", String::from_utf8_lossy(actual));
        }
        ExpectationKind::StderrEmpty { actual } => {
            let phrase = if held {
                "is empty"
            } else {
                "was expected to be empty"
            };
            eprintln!("  {block}: stderr {phrase}",);
            eprintln!("    actual stderr: {:?}", String::from_utf8_lossy(actual));
        }
        ExpectationKind::FileExists { path, observation } => {
            let reason = match observation {
                FileExistsObservation::RegularFile => "it exists",
                FileExistsObservation::Missing => "it does not exist",
                FileExistsObservation::NotRegularFile => {
                    "it is not a regular file (e.g. a directory)"
                }
            };
            eprintln!("  {block}: file {:?} — {reason}", path,);
        }
        ExpectationKind::FileContains {
            path,
            expected_source,
            observation,
        } => {
            let expected = format_text_equals_source(expected_source);
            let reason = match observation {
                FileContentObservation::Found => format!("its content contains {expected}"),
                FileContentObservation::NotFound => {
                    format!("its content does not contain {expected}")
                }
                FileContentObservation::Missing => "it does not exist".to_string(),
                FileContentObservation::NotRegularFile => {
                    "it is not a regular file (e.g. a directory)".to_string()
                }
                FileContentObservation::Unreadable => "it could not be read".to_string(),
                FileContentObservation::NotUtf8 => "its content is not valid UTF-8".to_string(),
            };
            eprintln!("  {block}: file {:?} — {reason}", path,);
        }
        ExpectationKind::FileContentsEquals {
            path,
            expected_source,
            observation,
        } => {
            let source_display = format_expected_source(expected_source);
            match observation {
                ContentsEqualsObservation::Compared(comparison) => print_byte_comparison_detail(
                    block,
                    &format!("file {path:?}"),
                    "contents_equals",
                    &source_display,
                    comparison,
                ),
                ContentsEqualsObservation::ActualMissing => eprintln!(
                    "  {block}: file {:?} contents_equals {source_display} — it does not exist",
                    path,
                ),
                ContentsEqualsObservation::ActualNotRegularFile => eprintln!(
                    "  {block}: file {:?} contents_equals {source_display} — it is not a regular file (e.g. a directory)",
                    path,
                ),
                ContentsEqualsObservation::ActualUnreadable => eprintln!(
                    "  {block}: file {:?} contents_equals {source_display} — it could not be read",
                    path,
                ),
            }
        }
        ExpectationKind::FileTextEquals {
            path,
            expected_source,
            observation,
        } => {
            let source_display = format_text_equals_source(expected_source);
            match observation {
                ContentsEqualsObservation::Compared(comparison) => print_byte_comparison_detail(
                    block,
                    &format!("file {path:?}"),
                    "text_equals",
                    &source_display,
                    comparison,
                ),
                ContentsEqualsObservation::ActualMissing => eprintln!(
                    "  {block}: file {:?} text_equals {source_display} — it does not exist",
                    path,
                ),
                ContentsEqualsObservation::ActualNotRegularFile => eprintln!(
                    "  {block}: file {:?} text_equals {source_display} — it is not a regular file (e.g. a directory)",
                    path,
                ),
                ContentsEqualsObservation::ActualUnreadable => eprintln!(
                    "  {block}: file {:?} text_equals {source_display} — it could not be read",
                    path,
                ),
            }
        }
        ExpectationKind::StdoutContentsEquals {
            expected_source,
            comparison,
        } => print_byte_comparison_detail(
            block,
            "stdout",
            "contents_equals",
            &format_expected_source(expected_source),
            comparison,
        ),
        ExpectationKind::StderrContentsEquals {
            expected_source,
            comparison,
        } => print_byte_comparison_detail(
            block,
            "stderr",
            "contents_equals",
            &format_expected_source(expected_source),
            comparison,
        ),
        ExpectationKind::StdoutTextEquals {
            expected_source,
            comparison,
        } => print_byte_comparison_detail(
            block,
            "stdout",
            "text_equals",
            &format_text_equals_source(expected_source),
            comparison,
        ),
        ExpectationKind::StderrTextEquals {
            expected_source,
            comparison,
        } => print_byte_comparison_detail(
            block,
            "stderr",
            "text_equals",
            &format_text_equals_source(expected_source),
            comparison,
        ),
        ExpectationKind::DirExists { path, observation } => {
            let reason = match observation {
                DirExistsObservation::Directory => "it exists",
                DirExistsObservation::Missing => "it does not exist",
                DirExistsObservation::NotADirectory => {
                    "it is not a directory (e.g. a regular file)"
                }
            };
            eprintln!("  {block}: dir {:?} — {reason}", path,);
        }
        ExpectationKind::DirContains {
            path,
            expected_entry,
            observation,
        } => {
            let reason = match observation {
                DirContainsObservation::Found => {
                    format!("it contains entry {expected_entry:?}")
                }
                DirContainsObservation::EntryMissing => {
                    format!("it does not contain entry {expected_entry:?}")
                }
                DirContainsObservation::SubjectMissing => "it does not exist".to_string(),
                DirContainsObservation::SubjectNotADirectory => {
                    "it is not a directory (e.g. a regular file)".to_string()
                }
                DirContainsObservation::SubjectUnreadable => "it could not be read".to_string(),
            };
            eprintln!("  {block}: dir {:?} — {reason}", path,);
        }
        ExpectationKind::Logical { operator, children } => {
            let status = if held { "held" } else { "did not hold" };
            eprintln!("  {block}: '{}' block {status}", operator.keyword(),);
            for child in children {
                print_failed_expectation(block, child);
            }
        }
    }
}

/// A `contents_equals` expected value's source, formatted the way it would appear in source:
/// `<"path">` for a workspace path, `@"path"` for a fixture reference.
fn format_expected_source(source: &ContentsEqualsExpectedSource) -> String {
    match source {
        ContentsEqualsExpectedSource::Workspace(path) => format!("<{path:?}>"),
        ContentsEqualsExpectedSource::Fixture(path) => format!("@{path:?}"),
    }
}

/// A `text_equals` expected value's source, formatted for the subject description line.
///
/// A string literal is rendered compactly, the way it would appear in source. A heredoc
/// literal is rendered as a plain label instead of its full body: the mismatch detail below
/// already carries a bounded, escaped context window and a line number, and printing the
/// full heredoc body here would risk unbounded output. See docs/adr — text_equals evaluation.
///
/// An interpolated literal is rendered as a label for a second reason as well: its resolved
/// value mixes script text with captured process output, so naming the literal keeps the subject
/// line free of it. This bounds the subject line only — a mismatch's own escaped context window
/// below still shows a bounded slice of the expected bytes, exactly as it does for a raw literal.
/// See docs/adr/20260726T060000Z_interpolated-text-literal.md.
fn format_text_equals_source(source: &TextValueProvenance) -> String {
    match source {
        TextValueProvenance::Quoted(value) => format!("{value:?}"),
        TextValueProvenance::Heredoc(_) => "<heredoc literal>".to_string(),
        TextValueProvenance::Binding { name, .. } => format!("&{name}"),
        TextValueProvenance::Interpolated {
            form,
            span,
            references,
        } => TextValueProvenance::describe_interpolated(*form, *span, references),
    }
}

/// Prints a byte-for-byte comparison's outcome, shared by every expectation that carries a
/// `ContentsEqualsComparison` (`contents_equals` and `text_equals`, file and stream subjects).
/// On mismatch, prints only a bounded, escaped context window around the first differing byte —
/// never the full actual/expected bytes. `operator` is the expectation keyword as written in
/// source (`contents_equals` or `text_equals`), so the subject description matches what the
/// author wrote. See `reportage_core::contents_diagnostic` and docs/reference/semantic-diagnostics.md.
fn print_byte_comparison_detail(
    block: &str,
    subject: &str,
    operator: &str,
    source_display: &str,
    comparison: &ContentsEqualsComparison,
) {
    match &comparison.outcome {
        ContentsEqualsOutcome::Match => {
            eprintln!("  {block}: {subject} {operator} {source_display} — bytes match",);
        }
        ContentsEqualsOutcome::Mismatch(mismatch) => {
            let ctx = mismatch_context(&comparison.actual, &comparison.expected, mismatch);
            eprintln!("  {block}: {subject} {operator} {source_display} — bytes differ",);
            eprintln!(
                "    actual length: {}, expected length: {}",
                mismatch.actual_len, mismatch.expected_len,
            );
            eprintln!(
                "    first differing byte at offset {} (line {})",
                mismatch.first_diff_offset, ctx.first_diff_line,
            );
            eprintln!("    actual:   {:?}", ctx.actual_context);
            eprintln!("    expected: {:?}", ctx.expected_context);
        }
    }
}
