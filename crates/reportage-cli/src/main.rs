use std::path::{Path, PathBuf};

use clap::Parser;
use reportage_core::{
    artifact::{ArtifactWriter, RunId},
    config, evaluator,
    executor::ExecutionEnvironment,
    result::{
        CaseStatus, ExpectationKind, ExpectationResult, FileContentObservation, FileErrorKind,
        FileExistsObservation, RunResult,
    },
    suite,
};

#[derive(Parser)]
#[command(
    name = "reportage",
    about = "Run reportage test scripts",
    override_usage = "reportage [OPTIONS] [SUBCOMMAND]..."
)]
struct Cli {
    /// Explicit script paths to execute. Cannot be combined with --config.
    scripts: Vec<PathBuf>,

    /// Path to the config file. Defaults to ./reportage.kdl when no scripts are given.
    #[arg(long)]
    config: Option<PathBuf>,

    /// Fixed artifact run id, for internal self-testing / development only.
    ///
    /// Not a public stable interface: hidden from `--help`, and not documented
    /// as a normal CLI feature. See docs/TBD.md — "Self-test run ID control".
    #[arg(long = "debug-run-id", hide = true)]
    debug_run_id: Option<String>,
}

enum InvocationMode {
    /// One or more explicit script paths; no config file required.
    ExplicitScripts(Vec<PathBuf>),
    /// Discover test files via a config file.
    Config(PathBuf),
}

fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => match e.kind() {
            clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                e.print().expect("error writing help");
                std::process::exit(0);
            }
            _ => {
                e.print().expect("error writing error");
                std::process::exit(4);
            }
        },
    };

    let mode = determine_mode(&cli);

    let result = match mode {
        InvocationMode::ExplicitScripts(scripts) => run_scripts(scripts),
        InvocationMode::Config(config_path) => run_with_config(config_path),
    };

    let writer = match &cli.debug_run_id {
        Some(raw_id) => {
            let run_id = match RunId::new(raw_id.clone()) {
                Ok(id) => id,
                Err(e) => {
                    eprintln!("error: invalid --debug-run-id: {e}");
                    std::process::exit(3);
                }
            };
            match ArtifactWriter::for_fixed_run(Path::new(".reportage"), &run_id) {
                Ok(writer) => writer,
                Err(e) => {
                    eprintln!("error: {e}");
                    std::process::exit(3);
                }
            }
        }
        None => ArtifactWriter::for_run(Path::new(".reportage")),
    };
    if let Err(e) = writer.write(&result) {
        // Artifact generation is required by default; write failures are runtime
        // infrastructure errors, not optional conditions the caller can ignore.
        // Continuing here would let CI report success with no artifact evidence.
        // See docs/artifacts.md and ADR 20260627T100400Z_generate-artifacts-by-default.
        eprintln!("error: failed to write artifacts: {e}");
        std::process::exit(3);
    }

    print_results(&result);

    std::process::exit(result.exit_code());
}

fn determine_mode(cli: &Cli) -> InvocationMode {
    match (&cli.config, cli.scripts.is_empty()) {
        (Some(_), false) => {
            // --config combined with explicit scripts is rejected in v0.
            // See ADR 20260628_reject-combined-config-and-scripts.
            eprintln!(
                "error: --config cannot be combined with explicit script arguments in v0; \
                 use either 'reportage --config <path>' or 'reportage <script>...'"
            );
            std::process::exit(3);
        }
        (Some(config_path), true) => InvocationMode::Config(config_path.clone()),
        (None, false) => InvocationMode::ExplicitScripts(cli.scripts.clone()),
        (None, true) => InvocationMode::Config(PathBuf::from("reportage.kdl")),
    }
}

/// Runs one or more explicitly-specified scripts through the pre-execution validation phase.
fn run_scripts(scripts: Vec<PathBuf>) -> RunResult {
    let (validated, file_errors) = suite::load_and_validate(&scripts);

    if !file_errors.is_empty() {
        return RunResult {
            cases: vec![],
            file_errors,
        };
    }

    let env = ExecutionEnvironment::default();
    let mut all_cases = Vec::new();
    for file in validated {
        let mut run = evaluator::evaluate(&file.script, &env);
        for case in &mut run.cases {
            case.source_path = Some(file.source_path.clone());
        }
        all_cases.extend(run.cases);
    }

    RunResult {
        cases: all_cases,
        file_errors: vec![],
    }
}

/// Loads and validates a config file, discovers test files, then runs them.
fn run_with_config(config_path: PathBuf) -> RunResult {
    let config_source = match std::fs::read_to_string(&config_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read config '{}': {e}", config_path.display());
            std::process::exit(3);
        }
    };

    let config = match config::parse_config(&config_source) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(3);
        }
    };

    let base_dir = config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    let discovered = match suite::discover_files(&base_dir, &config.tests.paths) {
        Ok(files) => files,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    };

    run_scripts(discovered)
}

fn print_results(result: &RunResult) {
    if result.is_noop() {
        println!("NO-OP  no cases found; nothing was executed");
    }

    for error in &result.file_errors {
        let kind = match &error.kind {
            FileErrorKind::ReadError(_) => "READ ERROR",
            FileErrorKind::ParseError(_) => "PARSE ERROR",
        };
        eprintln!("{kind}  {}", error.source_path.display());
        let message = match &error.kind {
            FileErrorKind::ReadError(msg) | FileErrorKind::ParseError(msg) => msg,
        };
        eprintln!("  {message}");
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
            CaseStatus::ScriptError(msg) | CaseStatus::RuntimeError(msg) => {
                eprintln!("  {msg}");
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

        // For failing cases, show observed shim invocations so the resolved
        // shim path and target invocation are visible in diagnostics.
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

/// Prints why one failed top-level expectation within an assertion block did
/// not hold.
///
/// Recurses into a `not` / `all` / `any` composition's children, printing
/// every child's own detail rather than filtering by the child's own
/// pass/fail state. This matters for `not`: when a `not` block fails, that
/// means its (grouped) contents *held* — none of its children individually
/// failed — so filtering for failed children would print nothing and lose
/// the information needed to explain the negation's failure. Always
/// recursing into every child, described in its own held/did-not-hold
/// terms, keeps `all` / `any` failures explainable too, without needing a
/// separate per-operator rule for which children are "responsible".
fn print_failed_expectation(step_index: usize, expectation: &ExpectationResult) {
    print_expectation_detail(step_index, expectation);
    if let Some(code) = expectation.kind.failure_diagnostic_code() {
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
            eprintln!("    actual stdout: {:?}", actual);
        }
        ExpectationKind::StderrContains { expected, actual } => {
            let verb = if held { "contains" } else { "does not contain" };
            eprintln!(
                "  assertion block at step {}: stderr {verb} {:?}",
                step_index + 1,
                expected,
            );
            eprintln!("    actual stderr: {:?}", actual);
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
            eprintln!("    actual stdout: {:?}", actual);
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
            eprintln!("    actual stderr: {:?}", actual);
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
