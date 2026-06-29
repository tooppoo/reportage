use std::path::{Path, PathBuf};

use clap::Parser;
use reportage_core::{
    artifact::ArtifactWriter,
    config, evaluator,
    executor::ExecutionEnvironment,
    result::{CaseStatus, ExpectationKind, FileErrorKind, RunResult},
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

    let writer = ArtifactWriter::for_run(Path::new(".reportage"));
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
                    match &expectation.kind {
                        ExpectationKind::Exit { expected, actual } => {
                            eprintln!(
                                "  assertion block at step {}: expected exit {expected}, got {actual}",
                                block.step_index + 1,
                            );
                        }
                        ExpectationKind::StdoutContains { expected, actual } => {
                            eprintln!(
                                "  assertion block at step {}: stdout does not contain {:?}",
                                block.step_index + 1,
                                expected,
                            );
                            eprintln!("    actual stdout: {:?}", actual);
                        }
                        ExpectationKind::StderrContains { expected, actual } => {
                            eprintln!(
                                "  assertion block at step {}: stderr does not contain {:?}",
                                block.step_index + 1,
                                expected,
                            );
                            eprintln!("    actual stderr: {:?}", actual);
                        }
                        ExpectationKind::StdoutEmpty { actual } => {
                            eprintln!(
                                "  assertion block at step {}: expected stdout to be empty",
                                block.step_index + 1,
                            );
                            eprintln!("    actual stdout: {:?}", actual);
                        }
                        ExpectationKind::StderrEmpty { actual } => {
                            eprintln!(
                                "  assertion block at step {}: expected stderr to be empty",
                                block.step_index + 1,
                            );
                            eprintln!("    actual stderr: {:?}", actual);
                        }
                    }
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
