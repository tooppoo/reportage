//! Entry point for repository maintenance tasks. Keep this thin: everything worth testing
//! belongs in the library modules, because this file is excluded from coverage enforcement.
//!
//! Exit codes are the tool's automation contract and are defined by
//! [`xtask::output::Category`]: 0 success, 1 internal, 2 usage, 3 input, 4 filesystem,
//! 5 conflict. Usage errors come from clap, which already exits with 2.

use std::io::Write;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};

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
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Verify each committed public `schema.json` matches its `schema.internal.json`.
    Check {
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
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
    let cli = Cli::parse();
    let root = schema_artifacts::repository_root();

    let (report, format) = match cli.command {
        Command::SchemaArtifacts {
            action: SchemaArtifactsAction::Gen { dry_run, format },
        } => (schema_artifacts::generate(&root, dry_run), format),
        Command::SchemaArtifacts {
            action: SchemaArtifactsAction::Check { format },
        } => (schema_artifacts::check(&root), format),
    };

    emit(render(&report, format.into()))
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
