use std::path::Path;

use clap::Parser;
use reportage_core::{
    artifact::ArtifactWriter,
    evaluator, parser,
    result::{CaseStatus, ExpectationKind},
};

#[derive(Parser)]
#[command(name = "reportage", about = "Run reportage test scripts")]
struct Cli {
    /// Path to the reportage script file to execute
    script: std::path::PathBuf,
}

fn main() {
    let cli = Cli::parse();

    let source = match std::fs::read_to_string(&cli.script) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read '{}': {e}", cli.script.display());
            std::process::exit(3);
        }
    };

    let script = match parser::parse(&source) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    };

    let result = evaluator::evaluate(&script);

    let writer = ArtifactWriter::for_run(Path::new(".reportage"));
    if let Err(e) = writer.write(&result) {
        // Artifact generation is required by default; write failures are runtime
        // infrastructure errors, not optional conditions the caller can ignore.
        // Continuing here would let CI report success with no artifact evidence.
        // See docs/artifacts.md and ADR 20260627T100400Z_generate-artifacts-by-default.
        eprintln!("error: failed to write artifacts: {e}");
        std::process::exit(3);
    }

    for case in &result.cases {
        let tag = match &case.status {
            CaseStatus::Pass => "PASS",
            CaseStatus::Fail => "FAIL",
            CaseStatus::ScriptError(_) => "ERROR",
            CaseStatus::RuntimeError(_) => "ERROR",
        };
        println!("{tag}  {}", case.name);

        match &case.status {
            CaseStatus::ScriptError(msg) | CaseStatus::RuntimeError(msg) => {
                eprintln!("  {msg}");
            }
            _ => {}
        }

        for block in &case.assertion_blocks {
            for expectation in &block.expectations {
                if !expectation.passed {
                    match expectation.kind {
                        ExpectationKind::Exit { expected, actual } => {
                            eprintln!(
                                "  assertion block at step {}: expected exit {expected}, got {actual}",
                                block.step_index + 1,
                            );
                        }
                    }
                }
            }
        }
    }

    std::process::exit(result.exit_code());
}
