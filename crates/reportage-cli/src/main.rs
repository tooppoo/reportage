use std::path::Path;

use clap::Parser;
use reportage_core::{
    artifact::ArtifactWriter,
    evaluator, parser,
    result::{AssertionKind, CaseStatus},
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
            CaseStatus::ValidationError(_) => "ERROR",
            CaseStatus::RuntimeError(_) => "ERROR",
        };
        println!("{tag}  {}", case.name);

        match &case.status {
            CaseStatus::ValidationError(msg) | CaseStatus::RuntimeError(msg) => {
                eprintln!("  {msg}");
            }
            _ => {}
        }

        for assertion in &case.assertions {
            if !assertion.passed {
                match assertion.kind {
                    AssertionKind::Exit { expected, actual } => {
                        eprintln!(
                            "  assertion failed at step {}: expected exit {expected}, got {actual}",
                            assertion.step_index + 1,
                        );
                    }
                }
            }
        }
    }

    std::process::exit(result.exit_code());
}
