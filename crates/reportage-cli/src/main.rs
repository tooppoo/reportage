mod render;

use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use reportage_cli::references;
use reportage_core::{
    artifact::{ArtifactWriter, RunId},
    config, evaluator,
    executor::ExecutionEnvironment,
    result::ExecutionReport,
    shim::{CommandName, CommandRegistry, ExecutableInvocation},
    shim_scaffold::{ScaffoldError, ScaffoldRequest, TemplateRegistry, scaffold},
    suite,
};

use render::{OutputRenderer, human::HumanRenderer, json::JsonRenderer};

/// Output format for the run result.
///
/// `Json` is the structured execution report described in issue #75: a single JSON document
/// on CLI stdout, projected from the always-written `result.json` artifact manifest (#102).
/// See `render::json` for the projection and the CLI-stdout-vs-captured-output contract.
#[derive(Clone, Copy, Default, clap::ValueEnum)]
enum OutputFormat {
    #[default]
    Human,
    Json,
}

#[derive(Parser)]
#[command(
    name = "reportage",
    about = "Run reportage test scripts",
    version,
    override_usage = "reportage [OPTIONS] [SUBCOMMAND]...",
    // Deliberately a pointer, not a URL list: help stays short and the URL set has a single
    // owner (`reportage references`). See docs/adr/20260708T180000Z_ai-documentation-discovery-core-path.md.
    after_help = "Documentation:\n  \
        Run `reportage references` to list versioned documentation URLs.\n  \
        Run `reportage references --format=json` for a machine-readable reference index."
)]
struct Cli {
    /// Tooling subcommand. When present, no test scripts are run: see each subcommand's own
    /// help for its behavior. A bare filename that happens to match a subcommand name (e.g. a
    /// script literally named `shim`) cannot be run positionally; pass its path with a `./`
    /// prefix or a directory component to disambiguate.
    #[command(subcommand)]
    command: Option<Commands>,

    /// Explicit script paths to execute. Cannot be combined with --config.
    scripts: Vec<PathBuf>,

    /// Path to the config file. Defaults to ./reportage.kdl when no scripts are given.
    #[arg(long)]
    config: Option<PathBuf>,

    /// Output format for the run result: `human` (default) or `json`.
    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    format: OutputFormat,

    /// Fixed artifact run id, for internal self-testing / development only.
    ///
    /// Not a public stable interface: hidden from `--help`, and not documented as a normal CLI feature.
    /// See docs/planning/TBD.md — "Self-test run ID control".
    #[arg(long = "debug-run-id", hide = true)]
    debug_run_id: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Coverage-integration shim tooling. See docs/reference/shim-scaffold.md.
    Shim(ShimArgs),

    /// List versioned documentation URLs for this reportage version.
    References(ReferencesArgs),

    // Registered as a subcommand so `docs` can never be taken as a positional script path,
    // but hidden from normal help until the real command ships: it is not a feature to
    // advertise yet. The explicit `about` is the only text an explicit `reportage help docs`
    // may show; `long_about = None` keeps this comment from ever rendering there.
    #[command(
        hide = true,
        about = "Reserved for a future documentation generation command; not implemented yet",
        long_about = None
    )]
    Docs(DocsArgs),
}

#[derive(Parser)]
struct ReferencesArgs {
    /// Output format for the reference index: `human` (default) or `json`.
    #[arg(long, value_enum, default_value_t = ReferencesFormat::Human)]
    format: ReferencesFormat,
}

/// Output format for the `references` subcommand.
///
/// A separate enum from the run result's [`OutputFormat`] on purpose: both spell `--format=json`,
/// but the reference index document (`spec/output/references-index/schema.json`) and the run
/// report document (`spec/output/json-report/schema.json`) are independent contracts. See issue #137.
#[derive(Clone, Copy, Default, clap::ValueEnum)]
enum ReferencesFormat {
    #[default]
    Human,
    Json,
}

/// Arguments to the reserved `docs` subcommand.
///
/// The reserved command has no interface yet, so every invocation — whatever flags or values
/// follow `docs`, including `--help` and the old `--format=json` — must reach the same
/// not-implemented error instead of a clap parse error or a help screen. Swallowing all trailing
/// tokens keeps that guarantee until the documentation generation command replaces this stub.
#[derive(Parser)]
#[command(disable_help_flag = true)]
struct DocsArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
    _args: Vec<String>,
}

#[derive(Parser)]
struct ShimArgs {
    #[command(subcommand)]
    command: ShimCommand,
}

#[derive(Subcommand)]
enum ShimCommand {
    /// Render a builtin template into a shim file. See docs/reference/shim-scaffold.md.
    Scaffold(ScaffoldArgs),
}

#[derive(Parser)]
struct ScaffoldArgs {
    /// Name of the builtin template to render.
    #[arg(long)]
    template: Option<String>,

    /// Value embedded into the rendered template as its entry point. Not checked against the
    /// filesystem; see docs/reference/shim-scaffold.md — Template model.
    #[arg(long = "entry-point")]
    entry_point: Option<String>,

    /// Destination path for the generated shim file.
    #[arg(long)]
    out: Option<String>,

    /// Overwrite an existing regular file at `--out`. Has no effect when `--out` is a
    /// directory or a symlink: those are always rejected. See docs/reference/shim-scaffold.md — Output
    /// path policy.
    #[arg(long)]
    force: bool,
}

/// Runs `reportage shim scaffold` and always terminates the process: this subcommand renders a
/// template to a file and exits, without going through the script-execution/report/artifact
/// pipeline the rest of the CLI uses. See docs/reference/shim-scaffold.md.
///
/// `--template`/`--entry-point`/`--out` are `Option<String>` at the clap layer (not
/// `required = true`) specifically so that an omitted flag and an explicitly empty value
/// (`--template ''`) collapse to the same empty string and get one validation path in
/// `reportage_core::shim_scaffold::scaffold`, matching the "empty or unspecified" acceptance
/// criteria for each argument.
fn run_shim_scaffold(args: &ScaffoldArgs) -> ! {
    let request = ScaffoldRequest {
        template: args.template.clone().unwrap_or_default(),
        entry_point: args.entry_point.clone().unwrap_or_default(),
        out: PathBuf::from(args.out.clone().unwrap_or_default()),
        force: args.force,
    };

    match scaffold(&request, &TemplateRegistry::builtin()) {
        Ok(()) => std::process::exit(0),
        Err(ScaffoldError::Io(e)) => {
            eprintln!("error: failed to generate shim: {e}");
            std::process::exit(3);
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    }
}

/// Runs `reportage references` and always terminates the process: like `run_shim_scaffold`, this
/// is a tooling subcommand that must stay outside the script-execution/report/artifact pipeline.
/// It only prints the documentation URL index and exits 0. See `references` (module) and issue #137.
fn run_references(args: &ReferencesArgs) -> ! {
    match args.format {
        ReferencesFormat::Human => references::render_human(),
        ReferencesFormat::Json => references::render_json(),
    }
    std::process::exit(0);
}

/// Rejects the reserved `docs` subcommand and always terminates the process.
///
/// `docs` neither prints the reference index (that is `reportage references` now) nor generates
/// documentation (that command does not exist yet), so succeeding silently would misrepresent
/// both. Exit code 2 follows the shim-scaffold table's "the requested operation could not be
/// treated as valid input" meaning; see docs/reference/exit-codes.md and issue #166.
fn run_docs_reserved() -> ! {
    eprintln!(
        "error: 'reportage docs' is not implemented yet; it is reserved for a future \
         documentation generation command. To list official reference documentation URLs, \
         run 'reportage references'."
    );
    std::process::exit(2);
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

    // Tooling subcommands (`reportage shim scaffold ...`, `reportage references`, the reserved
    // `reportage docs`) exit here and never reach the script-execution/report/artifact pipeline
    // below: they are not test runs, and the artifact-writing exit codes (2/3) further down have
    // no meaning for them.
    match &cli.command {
        Some(Commands::Shim(shim_args)) => match &shim_args.command {
            ShimCommand::Scaffold(args) => run_shim_scaffold(args),
        },
        Some(Commands::References(references_args)) => run_references(references_args),
        Some(Commands::Docs(_)) => run_docs_reserved(),
        None => {}
    }

    let mode = determine_mode(&cli);

    let result = match mode {
        // Explicit script mode never reads a config file, so no commands are ever registered
        // here. Config-based command registration requires `--config` or the default config
        // mode. See docs/reference/configuration.md — Commands.
        InvocationMode::ExplicitScripts(scripts) => {
            run_scripts(scripts, &CommandRegistry::default())
        }
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
        // Artifact generation is required by default; write failures are runtime infrastructure errors, not optional conditions the caller can ignore.
        // Continuing here would let CI report success with no artifact evidence.
        // See docs/reference/artifacts.md and ADR 20260627T100400Z_generate-artifacts-by-default.
        eprintln!("error: failed to write artifacts: {e}");
        std::process::exit(3);
    }

    // `--format=json` must print only the single JSON document to CLI stdout; nothing else
    // (human log, ANSI color, progress output) may share stdout with it. See `render::json`.
    match cli.format {
        OutputFormat::Human => HumanRenderer.render(&result),
        OutputFormat::Json => JsonRenderer::new(writer.run_dir().to_path_buf()).render(&result),
    }

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
fn run_scripts(scripts: Vec<PathBuf>, commands: &CommandRegistry) -> ExecutionReport {
    let (validated, file_errors) = suite::load_and_validate(&scripts);

    if !file_errors.is_empty() {
        return ExecutionReport {
            cases: vec![],
            file_errors,
        };
    }

    let env = ExecutionEnvironment::default();
    let mut all_cases = Vec::new();
    for file in validated {
        let run = evaluator::evaluate(&file.script, &env, &file.source_path, commands);
        all_cases.extend(run.cases);
    }

    ExecutionReport {
        cases: all_cases,
        file_errors: vec![],
    }
}

/// Loads and validates a config file, discovers test files, then runs them.
fn run_with_config(config_path: PathBuf) -> ExecutionReport {
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

    let commands = match resolve_command_registry(&config.commands, &base_dir) {
        Ok(registry) => registry,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(3);
        }
    };

    let discovered = match suite::discover_files(&base_dir, &config.tests.paths) {
        Ok(files) => files,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    };

    run_scripts(discovered, &commands)
}

/// Resolves each configured command's `exec` value into an absolute [`ExecutableInvocation`]
/// target and builds the [`CommandRegistry`] used for this config-driven run.
///
/// `exec` is resolved relative to `base_dir` (the config file's directory) via a lexical
/// absolutization ([`std::path::absolute`]): it does not touch the filesystem, so it neither
/// requires the target executable to already exist nor resolves symlinks in the path. See
/// docs/reference/configuration.md — Commands.
fn resolve_command_registry(
    config: &config::CommandsConfig,
    base_dir: &Path,
) -> Result<CommandRegistry, String> {
    let mut entries = Vec::with_capacity(config.commands.len());
    for command in &config.commands {
        // Command ids are already validated by `config::parse_config`.
        let name = CommandName::new(command.id.clone())
            .expect("command id was already validated during config parsing");

        let relative = base_dir.join(&command.exec);
        let absolute = std::path::absolute(&relative).map_err(|e| {
            format!(
                "failed to resolve exec path '{}' for command '{}': {e}",
                command.exec, command.id
            )
        })?;

        let invocation = ExecutableInvocation::new(absolute, vec![])
            .map_err(|e| format!("invalid exec target for command '{}': {e}", command.id))?;

        entries.push((name, invocation));
    }
    Ok(CommandRegistry::new(entries))
}
