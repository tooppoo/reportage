//! `reportage shim scaffold`: generate a coverage-integration shim file from a static builtin
//! template.
//!
//! This is a scaffold, not a managed resource: reportage renders a template into a file once
//! and never touches it again. It does not detect coverage tools, package managers, or
//! project state, and it does not resolve or verify `--entry-point` against the filesystem.
//! See `docs/shim-scaffold.md` and the ADR at
//! `docs/adr/20260708T062146Z_shim-scaffold-command.md`.
//!
//! v0 ships no builtin templates ([`TemplateRegistry::builtin`] is empty); #128 and #129 add
//! `typescript-c8-tsx` and `golang`. This module's own tests exercise template resolution and
//! rendering through a locally-defined test-fixture template so the scaffolding pipeline itself
//! (validation, lookup, rendering, output-path policy, permissions) is covered ahead of those
//! templates landing.

use std::path::PathBuf;

#[cfg(test)]
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

#[derive(Debug)]
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
/// Registration is a plain `Vec`, not a `HashMap`: v0's template count is small (0 today, a
/// handful once #128/#129 land) and [`TemplateRegistry::available_names`] wants a stable sorted
/// order for diagnostics regardless of registration order.
pub struct TemplateRegistry {
    entries: Vec<(String, Box<dyn ShimTemplate>)>,
}

impl TemplateRegistry {
    pub fn new(entries: Vec<(String, Box<dyn ShimTemplate>)>) -> Self {
        Self { entries }
    }

    /// The registry the CLI uses. Empty in v0: `--template` is therefore always "unknown"
    /// against this registry until #128 (`typescript-c8-tsx`) and #129 (`golang`) add entries.
    pub fn builtin() -> Self {
        Self::new(vec![])
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

#[derive(Debug)]
pub enum ScaffoldError {
    EmptyTemplate,
    EmptyEntryPoint,
    InvalidEntryPoint(TemplateContextError),
    EmptyOut,
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
            ScaffoldError::EmptyTemplate => write!(f, "--template must not be empty"),
            ScaffoldError::EmptyEntryPoint => write!(f, "--entry-point must not be empty"),
            ScaffoldError::InvalidEntryPoint(e) => write!(f, "{e}"),
            ScaffoldError::EmptyOut => write!(f, "--out must not be empty"),
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
                "refusing to write to '{}': it is a symlink",
                path.display()
            ),
            ScaffoldError::OutIsDirectory(path) => write!(
                f,
                "refusing to write to '{}': it is a directory",
                path.display()
            ),
            ScaffoldError::OutAlreadyExists(path) => write!(
                f,
                "'{}' already exists; use --force to overwrite",
                path.display()
            ),
            ScaffoldError::Io(e) => write!(f, "failed to write shim: {e}"),
        }
    }
}

impl std::error::Error for ScaffoldError {}

/// Renders `request.template` and writes it to `request.out`.
///
/// Validation order is: empty/missing arguments, entry-point lexical safety, the output-path
/// policy (see docs/shim-scaffold.md — Output path policy), then template resolution. The
/// output-path policy is checked before template resolution deliberately, and is read-only (no
/// directory is created and nothing is written yet) — so an unknown `--template` never masks an
/// `--out` conflict the caller also needs to fix, and checking it costs nothing when the
/// template turns out to be unknown anyway. Nothing is written to disk until every check
/// (including template resolution) has passed.
pub fn scaffold(
    request: &ScaffoldRequest,
    registry: &TemplateRegistry,
) -> Result<(), ScaffoldError> {
    if request.template.is_empty() {
        return Err(ScaffoldError::EmptyTemplate);
    }
    if request.entry_point.is_empty() {
        return Err(ScaffoldError::EmptyEntryPoint);
    }
    let ctx = TemplateContext::new(request.entry_point.clone())
        .map_err(ScaffoldError::InvalidEntryPoint)?;
    if request.out.as_os_str().is_empty() {
        return Err(ScaffoldError::EmptyOut);
    }

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

/// A shell-script test-fixture template used only by this module's own tests, to exercise
/// rendering (including safe entry-point quoting) and the project-ownership notice contract
/// ahead of #128/#129 landing real builtin templates.
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
    fn builtin_registry_has_no_templates_in_v0() {
        assert_eq!(
            TemplateRegistry::builtin().available_names(),
            Vec::<&str>::new()
        );
    }

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
            Err(ScaffoldError::EmptyTemplate)
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
            Err(ScaffoldError::EmptyEntryPoint)
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
            Err(ScaffoldError::InvalidEntryPoint(
                TemplateContextError::EntryPointContainsNewline
            ))
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
            Err(ScaffoldError::EmptyOut)
        ));
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
        let err = scaffold(&request, &TemplateRegistry::builtin()).unwrap_err();
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
