//! Entry point for repository maintenance tasks. Keep this thin: command behaviour belongs in
//! the library modules, where it can be tested without spawning a process. What is left here is
//! covered by the process-level tests in `tests/cli.rs`, and this file is inside the coverage
//! gate, so logic added here has to be exercised.
//!
//! Exit codes are the tool's automation contract and are defined by
//! [`xtask::output::Category`]: 0 success, 1 internal, 2 usage, 3 input, 4 filesystem,
//! 5 conflict. Usage errors come from clap, which already exits with 2.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};

use xtask::output::{OutputFormat, Rendered, render};
use xtask::schema_artifacts;

#[derive(Parser)]
#[command(name = "xtask", about = "Repository maintenance tasks for reportage")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Manage the generated public JSON Schema artifacts under `spec/`.
    SchemaArtifacts {
        #[command(subcommand)]
        action: SchemaArtifactsAction,
    },
}

#[derive(Subcommand)]
enum SchemaArtifactsAction {
    /// Generate each public `schema.json` from its `schema.internal.json`.
    Gen {
        /// Report what would change without writing any file.
        #[arg(long)]
        dry_run: bool,
        #[command(flatten)]
        common: CommonArgs,
    },
    /// Verify each committed public `schema.json` matches its `schema.internal.json`.
    Check {
        #[command(flatten)]
        common: CommonArgs,
    },
}

#[derive(Args)]
struct CommonArgs {
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,
    /// Repository root to operate on.
    ///
    /// Hidden because the `just` recipes never pass it. It exists so tests can drive the real
    /// binary against a synthetic root and observe the process exit code, which is this tool's
    /// automation contract and cannot be verified by calling the library directly.
    #[arg(long, hide = true)]
    root: Option<PathBuf>,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Text,
    Json,
}

impl From<Format> for OutputFormat {
    fn from(format: Format) -> Self {
        match format {
            Format::Text => OutputFormat::Text,
            Format::Json => OutputFormat::Json,
        }
    }
}

fn main() -> ExitCode {
    let (action, common) = match Cli::parse().command {
        Command::SchemaArtifacts { action } => match action {
            SchemaArtifactsAction::Gen { dry_run, common } => (Some(dry_run), common),
            SchemaArtifactsAction::Check { common } => (None, common),
        },
    };

    let root = common
        .root
        .unwrap_or_else(schema_artifacts::repository_root);
    let report = match action {
        Some(dry_run) => schema_artifacts::generate(&root, dry_run),
        None => schema_artifacts::check(&root),
    };

    emit(render(&report, common.format.into()))
}

fn emit(rendered: Rendered) -> ExitCode {
    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();
    let _ = stdout.write_all(rendered.stdout.as_bytes());
    let _ = stdout.flush();
    let _ = stderr.write_all(rendered.stderr.as_bytes());
    let _ = stderr.flush();

    ExitCode::from(u8::try_from(rendered.exit_code).unwrap_or(1))
}
