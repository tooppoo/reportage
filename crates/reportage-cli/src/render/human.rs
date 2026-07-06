//! The human-readable console renderer.
//!
//! This preserves the console output that `reportage run` produced before
//! runner and output rendering were separated (see #76): it is a renderer
//! over `ExecutionReport`, not a source of truth in its own right.

use reportage_core::result::{
    CaseStatus, DirContainsObservation, DirExistsObservation, ExecutionReport, ExpectationKind,
    ExpectationResult, FileContentObservation, FileErrorKind, FileExistsObservation,
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
                    print_failed_expectation(block.step_index, expectation);
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

/// Prints why one failed top-level expectation within an assertion block did not hold.
///
/// Recurses into a `not` / `all` / `any` composition's children, printing every child's own detail rather than filtering by the child's own pass/fail state.
/// This matters for `not`: when a `not` block fails, that means its (grouped) contents *held* — none of its children individually failed — so filtering for failed children would print nothing and lose the information needed to explain the negation's failure.
/// Always recursing into every child, described in its own held/did-not-hold terms, keeps `all` / `any` failures explainable too, without needing a separate per-operator rule for which children are "responsible".
fn print_failed_expectation(step_index: usize, expectation: &ExpectationResult) {
    print_expectation_detail(step_index, expectation);
    if let Some(code) = expectation.failure_diagnostic_code() {
        eprintln!("    diagnostic code: {}", code.as_str());
    }
}

fn print_expectation_detail(step_index: usize, expectation: &ExpectationResult) {
    let held = expectation.passed;
    match &expectation.kind {
        ExpectationKind::Exit { expected, actual } => {
            eprintln!(
                "  assertion block at step {}: expected exit {expected}, got {actual}",
                step_index + 1,
            );
        }
        ExpectationKind::StdoutContains { expected, actual } => {
            let verb = if held { "contains" } else { "does not contain" };
            eprintln!(
                "  assertion block at step {}: stdout {verb} {:?}",
                step_index + 1,
                expected,
            );
            // Lossy decode is display-only here; the canonical actual value stays raw bytes.
            eprintln!("    actual stdout: {:?}", String::from_utf8_lossy(actual));
        }
        ExpectationKind::StderrContains { expected, actual } => {
            let verb = if held { "contains" } else { "does not contain" };
            eprintln!(
                "  assertion block at step {}: stderr {verb} {:?}",
                step_index + 1,
                expected,
            );
            eprintln!("    actual stderr: {:?}", String::from_utf8_lossy(actual));
        }
        ExpectationKind::StdoutEmpty { actual } => {
            let phrase = if held {
                "is empty"
            } else {
                "was expected to be empty"
            };
            eprintln!(
                "  assertion block at step {}: stdout {phrase}",
                step_index + 1,
            );
            eprintln!("    actual stdout: {:?}", String::from_utf8_lossy(actual));
        }
        ExpectationKind::StderrEmpty { actual } => {
            let phrase = if held {
                "is empty"
            } else {
                "was expected to be empty"
            };
            eprintln!(
                "  assertion block at step {}: stderr {phrase}",
                step_index + 1,
            );
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
            eprintln!(
                "  assertion block at step {}: file {:?} — {reason}",
                step_index + 1,
                path,
            );
        }
        ExpectationKind::FileContains {
            path,
            expected,
            observation,
        } => {
            let reason = match observation {
                FileContentObservation::Found => format!("its content contains {expected:?}"),
                FileContentObservation::NotFound => {
                    format!("its content does not contain {expected:?}")
                }
                FileContentObservation::Missing => "it does not exist".to_string(),
                FileContentObservation::NotRegularFile => {
                    "it is not a regular file (e.g. a directory)".to_string()
                }
                FileContentObservation::Unreadable => "it could not be read".to_string(),
                FileContentObservation::NotUtf8 => "its content is not valid UTF-8".to_string(),
            };
            eprintln!(
                "  assertion block at step {}: file {:?} — {reason}",
                step_index + 1,
                path,
            );
        }
        ExpectationKind::DirExists { path, observation } => {
            let reason = match observation {
                DirExistsObservation::Directory => "it exists",
                DirExistsObservation::Missing => "it does not exist",
                DirExistsObservation::NotADirectory => {
                    "it is not a directory (e.g. a regular file)"
                }
            };
            eprintln!(
                "  assertion block at step {}: dir {:?} — {reason}",
                step_index + 1,
                path,
            );
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
            eprintln!(
                "  assertion block at step {}: dir {:?} — {reason}",
                step_index + 1,
                path,
            );
        }
        ExpectationKind::Logical { operator, children } => {
            let status = if held { "held" } else { "did not hold" };
            eprintln!(
                "  assertion block at step {}: '{}' block {status}",
                step_index + 1,
                operator.keyword(),
            );
            for child in children {
                print_failed_expectation(step_index, child);
            }
        }
    }
}
