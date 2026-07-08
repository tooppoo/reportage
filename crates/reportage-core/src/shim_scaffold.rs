//! `reportage shim scaffold`: generate a coverage-integration shim file from a static builtin
//! template.
//!
//! This is a scaffold, not a managed resource: reportage renders a template into a file once
//! and never touches it again. It does not detect coverage tools, package managers, or
//! project state, and it does not resolve or verify `--entry-point` against the filesystem.
//! See `docs/shim-scaffold.md` and the ADR at
//! `docs/adr/20260708T062146Z_shim-scaffold-command.md`.
//!
//! [`TemplateRegistry::builtin`] ships `typescript-c8-tsx` (added by #128) and `golang` (added by #129).
//! This module's own tests also exercise template resolution and rendering through a
//! locally-defined test-fixture template, so the scaffolding pipeline itself (validation,
//! lookup, rendering, output-path policy, permissions) has coverage that does not depend on any
//! particular real template's content.

use std::path::PathBuf;

use crate::shell_quote::single_quote;

/// The minimal per-render context available to a template in v0.
///
/// `entry_point` is never checked against the filesystem: existence and meaning are left to the
/// template's own documentation (see docs/shim-scaffold.md — Template model). Only its lexical
/// safety is validated here, since it is embedded into generated files verbatim.
#[derive(Debug, Clone)]
pub struct TemplateContext {
    entry_point: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TemplateContextError {
    EntryPointContainsNul,
    EntryPointContainsNewline,
    EntryPointContainsCarriageReturn,
}

impl std::fmt::Display for TemplateContextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TemplateContextError::EntryPointContainsNul => {
                write!(f, "--entry-point must not contain NUL bytes")
            }
            TemplateContextError::EntryPointContainsNewline => {
                write!(
                    f,
                    "--entry-point must not contain line feed (LF) characters"
                )
            }
            TemplateContextError::EntryPointContainsCarriageReturn => {
                write!(
                    f,
                    "--entry-point must not contain carriage return (CR) characters"
                )
            }
        }
    }
}

impl std::error::Error for TemplateContextError {}

impl TemplateContext {
    /// Builds a context from a raw `--entry-point` value.
    ///
    /// Rejects NUL, LF, and CR so that no template can be tricked into emitting a file with an
    /// injected line or a truncated C string. A NUL byte can never actually reach this function
    /// through `std::env::args()` (the OS cannot represent one in argv), so that branch is only
    /// reachable from direct callers such as this module's own tests; it stays as defense in
    /// depth for embedders that build a context without going through argv.
    pub fn new(entry_point: String) -> Result<Self, TemplateContextError> {
        if entry_point.contains('\0') {
            return Err(TemplateContextError::EntryPointContainsNul);
        }
        if entry_point.contains('\n') {
            return Err(TemplateContextError::EntryPointContainsNewline);
        }
        if entry_point.contains('\r') {
            return Err(TemplateContextError::EntryPointContainsCarriageReturn);
        }
        Ok(Self { entry_point })
    }

    pub fn entry_point(&self) -> &str {
        &self.entry_point
    }
}

/// A renderable shim template.
///
/// Builtin templates are plain Rust functions embedded in the binary (see
/// [`TemplateRegistry::builtin`]). This trait is the seam a future external-template loader
/// (reading template files from disk — out of scope for v0, see docs/shim-scaffold.md —
/// Non-goals) would implement against instead: [`TemplateRegistry`] and [`scaffold`] only ever
/// see `&dyn ShimTemplate`, and never assume the renderer is a builtin Rust function.
pub trait ShimTemplate: Send + Sync {
    fn render(&self, ctx: &TemplateContext) -> String;
}

impl<F> ShimTemplate for F
where
    F: Fn(&TemplateContext) -> String + Send + Sync,
{
    fn render(&self, ctx: &TemplateContext) -> String {
        self(ctx)
    }
}

/// A name-resolved set of templates the scaffold command can render.
///
/// Registration is a plain `Vec`, not a `HashMap`: v0's template count is small (2 today) and [`TemplateRegistry::available_names`] wants a stable sorted order for diagnostics regardless of registration order.
pub struct TemplateRegistry {
    entries: Vec<(String, Box<dyn ShimTemplate>)>,
}

impl TemplateRegistry {
    pub fn new(entries: Vec<(String, Box<dyn ShimTemplate>)>) -> Self {
        Self { entries }
    }

    /// The registry the CLI uses.
    /// `--template` values other than the ones registered here are "unknown".
    pub fn builtin() -> Self {
        Self::new(vec![
            (
                "typescript-c8-tsx".to_string(),
                Box::new(typescript_c8_tsx_template) as Box<dyn ShimTemplate>,
            ),
            (
                "golang".to_string(),
                Box::new(golang_template) as Box<dyn ShimTemplate>,
            ),
        ])
    }

    pub fn resolve(&self, name: &str) -> Option<&dyn ShimTemplate> {
        self.entries
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, t)| t.as_ref())
    }

    /// Sorted template names, for use in "unknown template" diagnostics.
    pub fn available_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.entries.iter().map(|(n, _)| n.as_str()).collect();
        names.sort_unstable();
        names
    }
}

impl Default for TemplateRegistry {
    fn default() -> Self {
        Self::builtin()
    }
}

/// A single `reportage shim scaffold` invocation, already collected from CLI arguments.
///
/// The CLI layer collapses "flag not given" and "flag given as an empty string" into the same
/// empty `String`/`PathBuf` before constructing this, so [`scaffold`] validates both cases with
/// one check instead of tracking `Option` at this layer.
#[derive(Debug, Clone)]
pub struct ScaffoldRequest {
    pub template: String,
    pub entry_point: String,
    pub out: PathBuf,
    pub force: bool,
}

/// A single defect in an argument's shape: empty/missing, or (for `--entry-point`) lexically
/// unsafe. See [`ScaffoldError::InvalidRequest`] for why these are collected rather than
/// reported one at a time.
#[derive(Debug, PartialEq, Eq)]
pub enum RequestViolation {
    EmptyTemplate,
    EmptyEntryPoint,
    InvalidEntryPoint(TemplateContextError),
    EmptyOut,
}

impl std::fmt::Display for RequestViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestViolation::EmptyTemplate => write!(f, "--template must not be empty"),
            RequestViolation::EmptyEntryPoint => write!(f, "--entry-point must not be empty"),
            RequestViolation::InvalidEntryPoint(e) => write!(f, "{e}"),
            RequestViolation::EmptyOut => write!(f, "--out must not be empty"),
        }
    }
}

impl std::error::Error for RequestViolation {}

#[derive(Debug)]
pub enum ScaffoldError {
    /// One or more of `--template`/`--entry-point`/`--out` was empty, missing, or (for
    /// `--entry-point`) lexically unsafe.
    ///
    /// Every such violation present in a single invocation is collected here and reported
    /// together, rather than reporting only the first one found: a caller who fixes the
    /// reported problem and reruns should not be met with a second, previously-hidden problem
    /// the first `scaffold` call already could have told them about.
    InvalidRequest(Vec<RequestViolation>),
    UnknownTemplate {
        requested: String,
        available: Vec<String>,
    },
    OutIsSymlink(PathBuf),
    OutIsDirectory(PathBuf),
    OutAlreadyExists(PathBuf),
    Io(std::io::Error),
}

impl std::fmt::Display for ScaffoldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScaffoldError::InvalidRequest(violations) => {
                let joined = violations
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ");
                write!(f, "{joined}")
            }
            ScaffoldError::UnknownTemplate {
                requested,
                available,
            } => {
                if available.is_empty() {
                    write!(
                        f,
                        "unknown template '{requested}': no templates are currently registered"
                    )
                } else {
                    write!(
                        f,
                        "unknown template '{requested}': available templates are: {}",
                        available.join(", ")
                    )
                }
            }
            ScaffoldError::OutIsSymlink(path) => write!(
                f,
                "--out path '{}' is a symlink: reportage refuses to write a shim file through a symlink, with or without --force",
                path.display()
            ),
            ScaffoldError::OutIsDirectory(path) => write!(
                f,
                "--out path '{}' is a directory: a shim file cannot be created there, with or without --force",
                path.display()
            ),
            ScaffoldError::OutAlreadyExists(path) => write!(
                f,
                "--out path '{}' already exists: use --force to overwrite it",
                path.display()
            ),
            ScaffoldError::Io(e) => write!(f, "failed to write shim: {e}"),
        }
    }
}

impl std::error::Error for ScaffoldError {}

/// Renders `request.template` and writes it to `request.out`.
///
/// Validation order is: empty/missing arguments (all at once, see below), entry-point lexical
/// safety, the output-path policy (see docs/shim-scaffold.md — Output path policy), then
/// template resolution. The output-path policy is checked before template resolution
/// deliberately, and is read-only (no directory is created and nothing is written yet) — so an
/// unknown `--template` never masks an `--out` conflict the caller also needs to fix, and
/// checking it costs nothing when the template turns out to be unknown anyway. Nothing is
/// written to disk until every check (including template resolution) has passed.
pub fn scaffold(
    request: &ScaffoldRequest,
    registry: &TemplateRegistry,
) -> Result<(), ScaffoldError> {
    // `--template`/`--entry-point`/`--out` are independent of each other, so every violation
    // among them is collected and reported in one `InvalidRequest`, instead of returning on the
    // first one found: a caller fixing one problem at a time would otherwise have to run
    // `scaffold` again just to learn about the next one.
    let mut violations = Vec::new();

    if request.template.is_empty() {
        violations.push(RequestViolation::EmptyTemplate);
    }

    let entry_point_ctx = if request.entry_point.is_empty() {
        violations.push(RequestViolation::EmptyEntryPoint);
        None
    } else {
        match TemplateContext::new(request.entry_point.clone()) {
            Ok(ctx) => Some(ctx),
            Err(e) => {
                violations.push(RequestViolation::InvalidEntryPoint(e));
                None
            }
        }
    };

    if request.out.as_os_str().is_empty() {
        violations.push(RequestViolation::EmptyOut);
    }

    if !violations.is_empty() {
        return Err(ScaffoldError::InvalidRequest(violations));
    }
    let ctx = entry_point_ctx.expect("empty/invalid --entry-point would have been collected above");

    // `symlink_metadata` (not `metadata`) so a symlink is detected as such even when it points
    // at a regular file or is dangling: v0 refuses to write through a symlink regardless of
    // `--force`, since the eventual write target is not the path the user named.
    //
    // This check and the later `std::fs::write` are not one atomic operation: something else
    // could replace `request.out` with a symlink in between. Closing that window (e.g. opening
    // the destination with a no-follow flag) needs a platform-specific dependency this v0
    // foundation deliberately doesn't take on for a single-user local CLI's narrow race window;
    // see the ADR at docs/adr/20260708T062146Z_shim-scaffold-command.md for the accepted
    // trade-off.
    match std::fs::symlink_metadata(&request.out) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(ScaffoldError::OutIsSymlink(request.out.clone()));
            }
            if meta.is_dir() {
                return Err(ScaffoldError::OutIsDirectory(request.out.clone()));
            }
            if !request.force {
                return Err(ScaffoldError::OutAlreadyExists(request.out.clone()));
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(ScaffoldError::Io(e)),
    }

    let template =
        registry
            .resolve(&request.template)
            .ok_or_else(|| ScaffoldError::UnknownTemplate {
                requested: request.template.clone(),
                available: registry
                    .available_names()
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
            })?;

    if let Some(parent) = request.out.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(ScaffoldError::Io)?;
    }

    let content = template.render(&ctx);
    std::fs::write(&request.out, content).map_err(ScaffoldError::Io)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&request.out)
            .map_err(ScaffoldError::Io)?
            .permissions();
        // OR in only the owner-execute bit: group/other bits are left exactly as file creation
        // (and umask) produced them, so scaffold never grants non-owner execute permission.
        let mode = perms.mode() | 0o100;
        perms.set_mode(mode);
        std::fs::set_permissions(&request.out, perms).map_err(ScaffoldError::Io)?;
    }

    Ok(())
}

/// The `typescript-c8-tsx` builtin template: a POSIX `sh` shim that runs a TypeScript entry
/// point under `tsx`, wrapped in `c8` for Node.js/V8 coverage collection.
///
/// This is an initial scaffold, not a guarantee of a working TypeScript execution setup: see
/// docs/shim-scaffold.md — `typescript-c8-tsx` template for the assumptions and limitations a
/// project may need to adjust after generation (package manager, `tsx` vs. `ts-node` or a
/// custom loader, running built JavaScript instead of source, c8 reporter/output
/// configuration). `entry_point` is embedded through [`single_quote`], the same POSIX
/// single-quoting `fixture_shell_template`'s tests exercise, so this template does not
/// reimplement its own quoting.
fn typescript_c8_tsx_template(ctx: &TemplateContext) -> String {
    format!(
        "#!/bin/sh\n\
         set -eu\n\
         \n\
         # Scaffolded by reportage.\n\
         # This file is owned by this project after generation.\n\
         # Edit the c8 / tsx invocation below to match this project.\n\
         #\n\
         # Recommended:\n\
         # - Manage c8 and tsx in this project's package.json.\n\
         # - Pin dependency versions through this project's package manager.\n\
         #\n\
         # Notes:\n\
         # - A command wired up through reportage runs with the case workspace as its working\n\
         #   directory, not this file's directory, so this shim cd's into its own directory\n\
         #   first: entry_point and --reports-dir below are resolved from there. If entry_point\n\
         #   or this project's node_modules live somewhere else relative to this file, adjust\n\
         #   the cd target and these paths to match.\n\
         # - npx may depend on npm's package resolution behavior.\n\
         # - --clean=false keeps coverage from earlier invocations in --reports-dir instead of\n\
         #   erasing it: a suite typically invokes this shim once per test case, and c8's\n\
         #   default --clean=true would otherwise leave only the last invocation's coverage in\n\
         #   the report. This project is still responsible for clearing --reports-dir before a\n\
         #   fresh suite run, since this shim has no way to know when one run ends and the next\n\
         #   begins.\n\
         \n\
         CDPATH= cd -- \"$(dirname -- \"$0\")\"\n\
         \n\
         entry_point={entry_point}\n\
         \n\
         exec npx c8 \\\n\
         \x20\x20--clean=false \\\n\
         \x20\x20--reporter=text \\\n\
         \x20\x20--reporter=lcov \\\n\
         \x20\x20--reports-dir coverage/reportage \\\n\
         \x20\x20npx tsx \"$entry_point\" \"$@\"\n",
        entry_point = single_quote(ctx.entry_point()),
    )
}

/// The `golang` builtin template: a POSIX `sh` shim that builds a coverage-instrumented Go binary via `go build -cover`, then execs it with `GOCOVERDIR` set so a normal-exit or `os.Exit` run writes Go coverage data to that directory.
///
/// Go coverage instrumentation is a build-time flag, not a runtime one, so this shim rebuilds the binary on every invocation rather than exec-ing a pre-built one; see docs/shim-scaffold.md — `golang` template for why, and for what a project that wants a build-once workflow needs to edit after generation.
///
/// `entry_point` is embedded through [`single_quote`], the same quoting `typescript_c8_tsx_template` uses, so this template does not reimplement its own quoting.
///
/// `work_dir` and `cover_dir` are fixed initial values, not derived from `--out`: v0's `TemplateContext` carries only `entry_point` (see [`TemplateContext`]), so this template has no project-specific destination to embed here even if it wanted to.
fn golang_template(ctx: &TemplateContext) -> String {
    format!(
        "#!/bin/sh\n\
         set -eu\n\
         \n\
         # Scaffolded by reportage.\n\
         # This file is owned by this project after generation.\n\
         # Edit the go build target and coverage paths to match this project.\n\
         #\n\
         # By default, `go build -cover` instruments packages in the main module.\n\
         # It does not include standard library packages or external dependencies by default.\n\
         # Add `-coverpkg` if this project needs a different instrumentation scope.\n\
         #\n\
         # Go coverage data is written when the program returns normally from main\n\
         # or exits via os.Exit. If the program terminates by unrecovered panic\n\
         # or fatal exception, coverage data from that run may be lost.\n\
         #\n\
         # A command wired up through reportage runs with the case workspace as its working\n\
         # directory, not this file's directory, so this shim cd's into its own directory\n\
         # first: entry_point, work_dir, and cover_dir below are resolved from there. If\n\
         # entry_point or this project's go.mod live somewhere else relative to this file,\n\
         # adjust the cd target and these paths to match.\n\
         \n\
         CDPATH= cd -- \"$(dirname -- \"$0\")\"\n\
         \n\
         entry_point={entry_point}\n\
         work_dir='.reportage/shims/go'\n\
         bin_path=\"$work_dir/app\"\n\
         cover_dir='coverage/reportage/go'\n\
         \n\
         mkdir -p \"$work_dir\" \"$cover_dir\"\n\
         \n\
         go build -cover -o \"$bin_path\" \"$entry_point\"\n\
         \n\
         GOCOVERDIR=\"$cover_dir\" exec \"$bin_path\" \"$@\"\n",
        entry_point = single_quote(ctx.entry_point()),
    )
}

/// A shell-script test-fixture template used only by this module's own tests, to exercise
/// rendering (including safe entry-point quoting) and the project-ownership notice contract
/// independently of any particular real template's content.
#[cfg(test)]
fn fixture_shell_template(ctx: &TemplateContext) -> String {
    format!(
        "#!/bin/sh\n\
         # Generated by `reportage shim scaffold`.\n\
         # This file is now owned by your project: edit it freely.\n\
         # reportage does not manage, regenerate, or resync this file after generation.\n\
         exec {} \"$@\"\n",
        single_quote(ctx.entry_point()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // --- TemplateContext ---

    #[test]
    fn plain_entry_point_is_accepted() {
        assert!(TemplateContext::new("./bin/app".to_string()).is_ok());
    }

    #[test]
    fn entry_point_with_quote_is_accepted() {
        assert!(TemplateContext::new("it's/entry.sh".to_string()).is_ok());
    }

    #[test]
    fn entry_point_with_nul_is_rejected() {
        assert!(matches!(
            TemplateContext::new("bad\0entry".to_string()),
            Err(TemplateContextError::EntryPointContainsNul)
        ));
    }

    #[test]
    fn entry_point_with_lf_is_rejected() {
        assert!(matches!(
            TemplateContext::new("bad\nentry".to_string()),
            Err(TemplateContextError::EntryPointContainsNewline)
        ));
    }

    #[test]
    fn entry_point_with_cr_is_rejected() {
        assert!(matches!(
            TemplateContext::new("bad\rentry".to_string()),
            Err(TemplateContextError::EntryPointContainsCarriageReturn)
        ));
    }

    // --- TemplateRegistry ---

    #[test]
    fn registry_resolves_registered_template_by_name() {
        let registry = TemplateRegistry::new(vec![(
            "fixture".to_string(),
            Box::new(fixture_shell_template) as Box<dyn ShimTemplate>,
        )]);
        assert!(registry.resolve("fixture").is_some());
        assert!(registry.resolve("does-not-exist").is_none());
    }

    #[test]
    fn available_names_are_sorted() {
        let registry = TemplateRegistry::new(vec![
            (
                "zzz".to_string(),
                Box::new(fixture_shell_template) as Box<dyn ShimTemplate>,
            ),
            (
                "aaa".to_string(),
                Box::new(fixture_shell_template) as Box<dyn ShimTemplate>,
            ),
        ]);
        assert_eq!(registry.available_names(), vec!["aaa", "zzz"]);
    }

    // --- fixture template rendering ---

    #[test]
    fn fixture_template_quotes_entry_point_containing_single_quote() {
        let ctx = TemplateContext::new("it's/entry.sh".to_string()).unwrap();
        let rendered = fixture_shell_template(&ctx);
        assert!(rendered.contains("exec 'it'\\''s/entry.sh' \"$@\""));
    }

    #[test]
    fn fixture_template_includes_project_ownership_comment() {
        let ctx = TemplateContext::new("./entry.sh".to_string()).unwrap();
        let rendered = fixture_shell_template(&ctx);
        assert!(rendered.contains("now owned by your project"));
    }

    // --- typescript-c8-tsx template rendering ---

    #[test]
    fn typescript_c8_tsx_registers_under_its_template_name() {
        assert!(
            TemplateRegistry::builtin()
                .resolve("typescript-c8-tsx")
                .is_some()
        );
    }

    #[test]
    fn builtin_available_names_include_both_registered_templates_sorted() {
        assert_eq!(
            TemplateRegistry::builtin().available_names(),
            vec!["golang", "typescript-c8-tsx"]
        );
    }

    #[test]
    fn typescript_c8_tsx_embeds_the_entry_point() {
        let ctx = TemplateContext::new("my-app/index.ts".to_string()).unwrap();
        let rendered = typescript_c8_tsx_template(&ctx);
        assert!(rendered.contains("entry_point='my-app/index.ts'"));
    }

    #[test]
    fn typescript_c8_tsx_quotes_an_entry_point_containing_a_space_and_a_single_quote() {
        // Exercises the same `single_quote` helper `fixture_shell_template`'s tests exercise
        // (see `fixture_template_quotes_entry_point_containing_single_quote` above), so this
        // asserts the template reuses #127's quoting rather than embedding the value unquoted.
        let ctx = TemplateContext::new("it's/my app/index.ts".to_string()).unwrap();
        let rendered = typescript_c8_tsx_template(&ctx);
        assert!(rendered.contains("entry_point='it'\\''s/my app/index.ts'"));
    }

    #[test]
    fn typescript_c8_tsx_execs_through_c8_and_tsx() {
        let ctx = TemplateContext::new("my-app/index.ts".to_string()).unwrap();
        let rendered = typescript_c8_tsx_template(&ctx);
        assert!(rendered.contains("exec npx c8"));
        assert!(rendered.contains("npx tsx \"$entry_point\" \"$@\""));
    }

    #[test]
    fn typescript_c8_tsx_resolves_paths_relative_to_its_own_directory() {
        // reportage runs a wired-up command with the case workspace as its working directory,
        // not this shim's own directory, so entry_point/--reports-dir must be resolved against
        // this file's own location rather than left to resolve against whatever directory the
        // shim happened to be invoked from.
        let ctx = TemplateContext::new("my-app/index.ts".to_string()).unwrap();
        let rendered = typescript_c8_tsx_template(&ctx);
        assert!(rendered.contains("CDPATH= cd -- \"$(dirname -- \"$0\")\""));
    }

    #[test]
    fn typescript_c8_tsx_disables_clean_before_run() {
        // A suite typically invokes this shim once per test case, each as a separate `npx c8`
        // process. c8's default --clean=true erases prior coverage before every run, so without
        // this flag only the last invocation's coverage would survive in the final report.
        let ctx = TemplateContext::new("my-app/index.ts".to_string()).unwrap();
        let rendered = typescript_c8_tsx_template(&ctx);
        assert!(rendered.contains("--clean=false"));
    }

    #[test]
    fn typescript_c8_tsx_passes_additional_arguments_through_to_the_entry_point() {
        let ctx = TemplateContext::new("my-app/index.ts".to_string()).unwrap();
        let rendered = typescript_c8_tsx_template(&ctx);
        assert!(rendered.contains("tsx \"$entry_point\" \"$@\""));
    }

    #[test]
    fn typescript_c8_tsx_includes_project_editable_comment() {
        let ctx = TemplateContext::new("./entry.ts".to_string()).unwrap();
        let rendered = typescript_c8_tsx_template(&ctx);
        assert!(rendered.contains("Edit the c8 / tsx invocation below to match this project."));
    }

    #[test]
    fn typescript_c8_tsx_recommends_managing_dependencies_via_package_json() {
        let ctx = TemplateContext::new("./entry.ts".to_string()).unwrap();
        let rendered = typescript_c8_tsx_template(&ctx);
        assert!(rendered.contains("Manage c8 and tsx in this project's package.json."));
    }

    #[test]
    fn typescript_c8_tsx_renders_as_a_syntactically_valid_posix_shell_script() {
        // A behavioral, not merely textual, check: feeds the rendered content to `sh -n`
        // (parse-only, no execution) so a regression that breaks quoting or line-continuation
        // structure fails here even if the substring assertions above happen not to catch it.
        let ctx = TemplateContext::new("it's/my app/index.ts".to_string()).unwrap();
        let rendered = typescript_c8_tsx_template(&ctx);

        let mut child = std::process::Command::new("sh")
            .arg("-n")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .expect("failed to spawn sh -n");
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(rendered.as_bytes())
            .unwrap();
        let status = child.wait().expect("failed to wait on sh -n");
        assert!(
            status.success(),
            "rendered template is not valid POSIX sh:\n{rendered}"
        );
    }

    // --- golang template rendering ---

    #[test]
    fn golang_registers_under_its_template_name() {
        assert!(TemplateRegistry::builtin().resolve("golang").is_some());
    }

    #[test]
    fn golang_embeds_the_entry_point() {
        let ctx = TemplateContext::new("cli.go".to_string()).unwrap();
        let rendered = golang_template(&ctx);
        assert!(rendered.contains("entry_point='cli.go'"));
    }

    #[test]
    fn golang_accepts_a_build_target_that_is_not_a_file_path() {
        // #129's `--entry-point` is a `go build` target, not necessarily a file path: `.` and a package directory are both valid targets, so the template must embed them verbatim rather than assuming a file-path shape.
        let ctx = TemplateContext::new(".".to_string()).unwrap();
        let rendered = golang_template(&ctx);
        assert!(rendered.contains("entry_point='.'"));
    }

    #[test]
    fn golang_quotes_an_entry_point_containing_a_space_and_a_single_quote() {
        // Exercises the same `single_quote` helper `fixture_shell_template`'s tests exercise (see `fixture_template_quotes_entry_point_containing_single_quote` above), so this asserts the template reuses #127's quoting rather than embedding the value unquoted.
        let ctx = TemplateContext::new("it's/my app/cmd".to_string()).unwrap();
        let rendered = golang_template(&ctx);
        assert!(rendered.contains("entry_point='it'\\''s/my app/cmd'"));
    }

    #[test]
    fn golang_resolves_paths_relative_to_its_own_directory() {
        // reportage runs a wired-up command with the case workspace as its working directory,
        // not this shim's own directory, so entry_point/work_dir/cover_dir must be resolved
        // against this file's own location rather than left to resolve against whatever
        // directory the shim happened to be invoked from.
        let ctx = TemplateContext::new("cli.go".to_string()).unwrap();
        let rendered = golang_template(&ctx);
        assert!(rendered.contains("CDPATH= cd -- \"$(dirname -- \"$0\")\""));
    }

    #[test]
    fn golang_builds_with_the_cover_flag() {
        let ctx = TemplateContext::new("cli.go".to_string()).unwrap();
        let rendered = golang_template(&ctx);
        assert!(rendered.contains("go build -cover -o \"$bin_path\" \"$entry_point\""));
    }

    #[test]
    fn golang_sets_gocoverdir_before_running_the_built_binary() {
        let ctx = TemplateContext::new("cli.go".to_string()).unwrap();
        let rendered = golang_template(&ctx);
        assert!(rendered.contains("GOCOVERDIR=\"$cover_dir\" exec \"$bin_path\" \"$@\""));
    }

    #[test]
    fn golang_passes_additional_arguments_through_to_the_built_binary() {
        let ctx = TemplateContext::new("cli.go".to_string()).unwrap();
        let rendered = golang_template(&ctx);
        assert!(rendered.contains("exec \"$bin_path\" \"$@\""));
    }

    #[test]
    fn golang_includes_project_editable_comment() {
        let ctx = TemplateContext::new("cli.go".to_string()).unwrap();
        let rendered = golang_template(&ctx);
        assert!(
            rendered.contains("Edit the go build target and coverage paths to match this project.")
        );
    }

    #[test]
    fn golang_documents_the_default_coverpkg_scope() {
        let ctx = TemplateContext::new("cli.go".to_string()).unwrap();
        let rendered = golang_template(&ctx);
        assert!(rendered.contains("`go build -cover` instruments packages in the main module."));
        assert!(
            rendered.contains(
                "Add `-coverpkg` if this project needs a different instrumentation scope."
            )
        );
    }

    #[test]
    fn golang_documents_coverage_data_loss_on_unrecovered_panic() {
        let ctx = TemplateContext::new("cli.go".to_string()).unwrap();
        let rendered = golang_template(&ctx);
        assert!(rendered.contains("unrecovered panic"));
    }

    #[test]
    fn golang_renders_as_a_syntactically_valid_posix_shell_script() {
        // A behavioral, not merely textual, check: feeds the rendered content to `sh -n` (parse-only, no execution) so a regression that breaks quoting or line-continuation structure fails here even if the substring assertions above happen not to catch it.
        let ctx = TemplateContext::new("it's/my app/cmd".to_string()).unwrap();
        let rendered = golang_template(&ctx);

        let mut child = std::process::Command::new("sh")
            .arg("-n")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .expect("failed to spawn sh -n");
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(rendered.as_bytes())
            .unwrap();
        let status = child.wait().expect("failed to wait on sh -n");
        assert!(
            status.success(),
            "rendered template is not valid POSIX sh:\n{rendered}"
        );
    }

    // --- scaffold(): validation ---

    fn fixture_registry() -> TemplateRegistry {
        TemplateRegistry::new(vec![(
            "fixture".to_string(),
            Box::new(fixture_shell_template) as Box<dyn ShimTemplate>,
        )])
    }

    #[test]
    fn empty_template_is_rejected() {
        let request = ScaffoldRequest {
            template: "".to_string(),
            entry_point: "./entry.sh".to_string(),
            out: PathBuf::from("out.sh"),
            force: false,
        };
        assert!(matches!(
            scaffold(&request, &fixture_registry()),
            Err(ScaffoldError::InvalidRequest(v)) if v == vec![RequestViolation::EmptyTemplate]
        ));
    }

    #[test]
    fn empty_entry_point_is_rejected() {
        let request = ScaffoldRequest {
            template: "fixture".to_string(),
            entry_point: "".to_string(),
            out: PathBuf::from("out.sh"),
            force: false,
        };
        assert!(matches!(
            scaffold(&request, &fixture_registry()),
            Err(ScaffoldError::InvalidRequest(v)) if v == vec![RequestViolation::EmptyEntryPoint]
        ));
    }

    #[test]
    fn entry_point_with_newline_is_rejected() {
        let request = ScaffoldRequest {
            template: "fixture".to_string(),
            entry_point: "a\nb".to_string(),
            out: PathBuf::from("out.sh"),
            force: false,
        };
        assert!(matches!(
            scaffold(&request, &fixture_registry()),
            Err(ScaffoldError::InvalidRequest(v))
                if v == vec![RequestViolation::InvalidEntryPoint(
                    TemplateContextError::EntryPointContainsNewline
                )]
        ));
    }

    #[test]
    fn empty_out_is_rejected() {
        let request = ScaffoldRequest {
            template: "fixture".to_string(),
            entry_point: "./entry.sh".to_string(),
            out: PathBuf::from(""),
            force: false,
        };
        assert!(matches!(
            scaffold(&request, &fixture_registry()),
            Err(ScaffoldError::InvalidRequest(v)) if v == vec![RequestViolation::EmptyOut]
        ));
    }

    #[test]
    fn multiple_simultaneous_violations_are_all_collected() {
        // Combining several independent violations must report every one of them, not just the
        // first one `scaffold` happens to check, so a caller fixing the reported problems does
        // not have to rerun `scaffold` once per remaining violation.
        let request = ScaffoldRequest {
            template: "".to_string(),
            entry_point: "".to_string(),
            out: PathBuf::from(""),
            force: false,
        };
        let err = scaffold(&request, &fixture_registry()).unwrap_err();
        match &err {
            ScaffoldError::InvalidRequest(violations) => {
                assert_eq!(
                    violations,
                    &vec![
                        RequestViolation::EmptyTemplate,
                        RequestViolation::EmptyEntryPoint,
                        RequestViolation::EmptyOut,
                    ]
                );
            }
            other => panic!("expected InvalidRequest, got {other:?}"),
        }
        let message = err.to_string();
        assert!(message.contains("--template must not be empty"));
        assert!(message.contains("--entry-point must not be empty"));
        assert!(message.contains("--out must not be empty"));
    }

    #[test]
    fn empty_template_and_invalid_entry_point_are_both_collected() {
        let request = ScaffoldRequest {
            template: "".to_string(),
            entry_point: "a\nb".to_string(),
            out: PathBuf::from("out.sh"),
            force: false,
        };
        let err = scaffold(&request, &fixture_registry()).unwrap_err();
        match &err {
            ScaffoldError::InvalidRequest(violations) => {
                assert_eq!(
                    violations,
                    &vec![
                        RequestViolation::EmptyTemplate,
                        RequestViolation::InvalidEntryPoint(
                            TemplateContextError::EntryPointContainsNewline
                        ),
                    ]
                );
            }
            other => panic!("expected InvalidRequest, got {other:?}"),
        }
    }

    #[test]
    fn unknown_template_lists_available_names() {
        let request = ScaffoldRequest {
            template: "does-not-exist".to_string(),
            entry_point: "./entry.sh".to_string(),
            out: PathBuf::from("out.sh"),
            force: false,
        };
        let err = scaffold(&request, &fixture_registry()).unwrap_err();
        match err {
            ScaffoldError::UnknownTemplate {
                requested,
                available,
            } => {
                assert_eq!(requested, "does-not-exist");
                assert_eq!(available, vec!["fixture".to_string()]);
            }
            other => panic!("expected UnknownTemplate, got {other:?}"),
        }
    }

    #[test]
    fn unknown_template_against_empty_registry_reports_no_templates() {
        let request = ScaffoldRequest {
            template: "anything".to_string(),
            entry_point: "./entry.sh".to_string(),
            out: PathBuf::from("out.sh"),
            force: false,
        };
        // An explicitly empty registry, not `TemplateRegistry::builtin()`: builtin now carries
        // `typescript-c8-tsx`, so this test constructs the empty case directly to keep covering
        // the "no templates are currently registered" wording.
        let err = scaffold(&request, &TemplateRegistry::new(vec![])).unwrap_err();
        match &err {
            ScaffoldError::UnknownTemplate { available, .. } => {
                assert!(available.is_empty());
            }
            other => panic!("expected UnknownTemplate, got {other:?}"),
        }
        assert!(
            err.to_string()
                .contains("no templates are currently registered")
        );
    }

    // --- scaffold(): output path policy and successful generation ---

    #[test]
    #[cfg(unix)]
    fn successful_scaffold_writes_rendered_content() {
        let dir = TempDir::new().unwrap();
        let out = dir.path().join("shim.sh");
        let request = ScaffoldRequest {
            template: "fixture".to_string(),
            entry_point: "./bin/app".to_string(),
            out: out.clone(),
            force: false,
        };
        scaffold(&request, &fixture_registry()).unwrap();

        let content = std::fs::read_to_string(&out).unwrap();
        assert!(content.contains("exec './bin/app' \"$@\""));
    }

    #[test]
    #[cfg(unix)]
    fn successful_scaffold_creates_missing_parent_directories() {
        let dir = TempDir::new().unwrap();
        let out = dir.path().join("nested").join("dir").join("shim.sh");
        let request = ScaffoldRequest {
            template: "fixture".to_string(),
            entry_point: "./bin/app".to_string(),
            out: out.clone(),
            force: false,
        };
        scaffold(&request, &fixture_registry()).unwrap();
        assert!(out.exists());
    }

    #[test]
    #[cfg(unix)]
    fn successful_scaffold_grants_owner_execute_bit_only() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new().unwrap();
        let out = dir.path().join("shim.sh");
        let request = ScaffoldRequest {
            template: "fixture".to_string(),
            entry_point: "./bin/app".to_string(),
            out: out.clone(),
            force: false,
        };
        scaffold(&request, &fixture_registry()).unwrap();

        let mode = std::fs::metadata(&out).unwrap().permissions().mode();
        assert_ne!(mode & 0o100, 0, "owner execute bit must be granted");
        assert_eq!(
            mode & 0o011,
            0,
            "group/other execute bits must not be granted"
        );
    }

    #[test]
    #[cfg(unix)]
    fn existing_file_is_rejected_by_default() {
        let dir = TempDir::new().unwrap();
        let out = dir.path().join("shim.sh");
        std::fs::write(&out, "old content").unwrap();

        let request = ScaffoldRequest {
            template: "fixture".to_string(),
            entry_point: "./bin/app".to_string(),
            out: out.clone(),
            force: false,
        };
        assert!(matches!(
            scaffold(&request, &fixture_registry()),
            Err(ScaffoldError::OutAlreadyExists(_))
        ));
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "old content");
    }

    #[test]
    #[cfg(unix)]
    fn existing_file_is_overwritten_with_force() {
        let dir = TempDir::new().unwrap();
        let out = dir.path().join("shim.sh");
        std::fs::write(&out, "old content").unwrap();

        let request = ScaffoldRequest {
            template: "fixture".to_string(),
            entry_point: "./bin/app".to_string(),
            out: out.clone(),
            force: true,
        };
        scaffold(&request, &fixture_registry()).unwrap();
        assert!(std::fs::read_to_string(&out).unwrap().contains("./bin/app"));
    }

    #[test]
    #[cfg(unix)]
    fn existing_directory_is_rejected_even_with_force() {
        let dir = TempDir::new().unwrap();
        let out = dir.path().join("shim-dir");
        std::fs::create_dir_all(&out).unwrap();

        let request = ScaffoldRequest {
            template: "fixture".to_string(),
            entry_point: "./bin/app".to_string(),
            out: out.clone(),
            force: true,
        };
        assert!(matches!(
            scaffold(&request, &fixture_registry()),
            Err(ScaffoldError::OutIsDirectory(_))
        ));
    }

    #[test]
    #[cfg(unix)]
    fn existing_symlink_is_rejected_even_with_force() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("target.sh");
        std::fs::write(&target, "target content").unwrap();
        let out = dir.path().join("shim-link.sh");
        std::os::unix::fs::symlink(&target, &out).unwrap();

        let request = ScaffoldRequest {
            template: "fixture".to_string(),
            entry_point: "./bin/app".to_string(),
            out: out.clone(),
            force: true,
        };
        assert!(matches!(
            scaffold(&request, &fixture_registry()),
            Err(ScaffoldError::OutIsSymlink(_))
        ));
        // The symlink itself, and its target, must be untouched.
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "target content");
    }

    #[test]
    #[cfg(unix)]
    fn dangling_symlink_is_rejected() {
        let dir = TempDir::new().unwrap();
        let out = dir.path().join("shim-link.sh");
        std::os::unix::fs::symlink(dir.path().join("does-not-exist"), &out).unwrap();

        let request = ScaffoldRequest {
            template: "fixture".to_string(),
            entry_point: "./bin/app".to_string(),
            out: out.clone(),
            force: true,
        };
        assert!(matches!(
            scaffold(&request, &fixture_registry()),
            Err(ScaffoldError::OutIsSymlink(_))
        ));
    }
}
