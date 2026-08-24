//! Per-concrete-case isolated workspace.
//!
//! Each concrete case runs in its own workspace directory: `$` actions run
//! with it as their working directory, `write` steps write into it, and file
//! expectations resolve paths against it. See docs/reference/execution-model.md —
//! Workspace lifecycle, and docs/reference/semantics.md — Write step.

use std::path::Path;

use crate::diagnostic::DiagnosticCode;
use crate::model::{FileMode, WorkspacePath};

/// Applies a permission bit set to an already-open file.
///
/// Indirected through a function pointer only so a test can substitute a
/// failing one; see [`Workspace::write_file_applying`].
type ApplyMode = fn(&std::fs::File, FileMode) -> std::io::Result<()>;

/// An isolated case workspace, backed by a temporary directory that is
/// removed when the workspace is dropped.
///
/// See docs/reference/execution-model.md — Cleanup and preservation: v0 does not offer
/// workspace preservation, so unconditional cleanup on drop is correct.
pub struct Workspace {
    dir: tempfile::TempDir,
}

/// Error writing a file into a workspace via a `write` step.
///
/// This is the runtime step error classification for side-effecting steps.
/// See docs/reference/semantics.md — Write step, and the accompanying ADR.
#[derive(Debug)]
pub enum WriteFileError {
    /// The target path already exists (file, directory, or symlink).
    /// `write` is create-only and never silently overwrites.
    TargetAlreadyExists,
    /// Something other than a plain directory (a regular file, a symlink,
    /// or another special file type) already occupies part of the target's
    /// parent path, so the parent directories cannot be created.
    ///
    /// A symlink is rejected here rather than followed: an earlier `$`
    /// action could otherwise plant a symlink to an arbitrary external
    /// directory inside the workspace (`$ ln -s /tmp escape`), and a later
    /// `write` step through it would silently write outside the isolated
    /// workspace.
    ParentNotADirectory,
    /// An OS-level I/O error occurred while creating directories or writing the file.
    Io(std::io::Error),
    /// The requested file mode could not be applied to the file before it was
    /// published at the target path.
    ///
    /// Separate from [`WriteFileError::Io`] only so the message can name the
    /// mode as the failing part and repeat the value that was refused; both
    /// classify as `step.write.io_error`.
    SetMode {
        mode: FileMode,
        error: std::io::Error,
    },
}

impl std::fmt::Display for WriteFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WriteFileError::TargetAlreadyExists => {
                write!(f, "target path already exists; write is create-only")
            }
            WriteFileError::ParentNotADirectory => write!(
                f,
                "the target's parent path is blocked by a file, symlink, or other non-directory entry"
            ),
            WriteFileError::Io(e) => write!(f, "I/O error: {e}"),
            WriteFileError::SetMode { mode, error } => write!(
                f,
                "I/O error while setting the file mode to 0o{:03o}: {error}",
                mode.bits()
            ),
        }
    }
}

impl std::error::Error for WriteFileError {}

impl WriteFileError {
    /// The stable, machine-readable diagnostic code for this error.
    /// See docs/reference/diagnostics.md.
    pub const fn code(&self) -> DiagnosticCode {
        match self {
            WriteFileError::TargetAlreadyExists => DiagnosticCode::StepWriteTargetExists,
            WriteFileError::ParentNotADirectory => DiagnosticCode::StepWriteParentNotADirectory,
            WriteFileError::Io(_) | WriteFileError::SetMode { .. } => {
                DiagnosticCode::StepWriteIoError
            }
        }
    }
}

impl Workspace {
    /// Creates a fresh, empty isolated workspace backed by a new temporary directory.
    pub fn new() -> std::io::Result<Self> {
        Ok(Self {
            dir: tempfile::TempDir::new()?,
        })
    }

    /// The workspace root directory. `$` actions run with this as their
    /// working directory; file expectations and `write` steps resolve
    /// paths relative to it.
    pub fn root(&self) -> &Path {
        self.dir.path()
    }

    /// Writes `content` to `path`, resolved against the workspace root.
    ///
    /// Create-only: rejects a target that already exists (file, directory,
    /// or symlink) rather than silently overwriting it. Parent directories
    /// are created automatically, unless something other than a plain
    /// directory already occupies part of that parent path.
    ///
    /// `content` is written to a temporary file in the same parent
    /// directory first, then atomically persisted to `target` only if
    /// `target` does not already exist. This keeps a write that fails
    /// partway through from ever leaving a partially-written file visible
    /// at `target` — the create-only guarantee and the file's content
    /// become visible together, or not at all.
    ///
    /// `mode`, when given, is the target file's final permission bits. It is
    /// applied to the temporary file before that file is published, so the
    /// same all-or-nothing rule covers the mode: a file visible at `target`
    /// always already carries the requested mode. `None` leaves whatever
    /// mode the temporary file was created with, preserving the behavior of
    /// every `write` step that does not name one. `mode` applies only to the
    /// target file; parent directories created above are left alone.
    pub fn write_file(
        &self,
        path: &WorkspacePath,
        content: &str,
        mode: Option<FileMode>,
    ) -> Result<(), WriteFileError> {
        self.write_file_applying(
            path,
            content,
            mode.map(|mode| (mode, set_file_mode as ApplyMode)),
        )
    }

    /// [`Workspace::write_file`] with the mode-applying step supplied by the
    /// caller.
    ///
    /// Exists so a test can inject a failing mode application. A real `chmod`
    /// on a temporary file this process just created cannot be made to fail
    /// portably, and the property being protected — a rejected mode leaves no
    /// target behind — is a property of *where* the mode is applied, which a
    /// later refactor could silently move past `persist_noclobber`.
    fn write_file_applying(
        &self,
        path: &WorkspacePath,
        content: &str,
        apply_mode: Option<(FileMode, ApplyMode)>,
    ) -> Result<(), WriteFileError> {
        if self.parent_path_is_blocked(path) {
            return Err(WriteFileError::ParentNotADirectory);
        }

        let target = self.root().join(path.as_str());
        let parent = target
            .parent()
            .expect("a workspace-root-joined path always has a parent");
        std::fs::create_dir_all(parent).map_err(WriteFileError::Io)?;

        use std::io::Write as _;
        let mut temp = tempfile::Builder::new()
            .tempfile_in(parent)
            .map_err(WriteFileError::Io)?;
        temp.write_all(content.as_bytes())
            .map_err(WriteFileError::Io)?;

        if let Some((mode, apply)) = apply_mode {
            apply(temp.as_file(), mode).map_err(|error| WriteFileError::SetMode { mode, error })?;
        }

        match temp.persist_noclobber(&target) {
            Ok(_) => Ok(()),
            Err(persist_err) if persist_err.error.kind() == std::io::ErrorKind::AlreadyExists => {
                Err(WriteFileError::TargetAlreadyExists)
            }
            Err(persist_err) => Err(WriteFileError::Io(persist_err.error)),
        }
    }

    /// Returns true if one of `path`'s ancestor directory components already
    /// exists under the workspace root as something other than a plain
    /// directory: a regular file, a symlink (regardless of what it points
    /// to), or another special file type.
    ///
    /// Symlinks are rejected outright rather than followed and checked,
    /// because a symlink planted by an earlier `$` action could otherwise
    /// let a `write` step escape the isolated workspace. Checked explicitly,
    /// rather than inferred from `create_dir_all`'s error kind, so the
    /// classification does not depend on platform-specific `io::ErrorKind`
    /// variants.
    fn parent_path_is_blocked(&self, path: &WorkspacePath) -> bool {
        let mut ancestor = self.root().to_path_buf();
        let rel = Path::new(path.as_str());
        let mut components: Vec<_> = rel.components().collect();
        // The last component is the file name itself, not a parent directory.
        components.pop();
        for component in components {
            ancestor.push(component);
            if std::fs::symlink_metadata(&ancestor).is_ok_and(|meta| !meta.is_dir()) {
                return true;
            }
        }
        false
    }
}

/// Sets `file`'s permission bits to exactly `mode`.
///
/// Applied to the open handle rather than to a path, so no other process can
/// substitute a different file between the write and the mode change. `chmod`
/// assigns the bits verbatim, which is what makes the resulting mode
/// independent of the reportage process's umask — unlike the file *creation*
/// mode, which the kernel masks.
#[cfg(unix)]
fn set_file_mode(file: &std::fs::File, mode: FileMode) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    file.set_permissions(std::fs::Permissions::from_mode(mode.bits()))
}

/// `write`'s `mode` is defined in POSIX permission-bit terms, which have no
/// faithful Windows equivalent (see docs/adr — no native Windows execution).
/// Reported as a failure rather than ignored: a silently unapplied `mode`
/// would leave a fixture the script declared unreadable or unexecutable
/// looking like it succeeded.
#[cfg(not(unix))]
fn set_file_mode(_file: &std::fs::File, _mode: FileMode) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "write mode requires a POSIX platform",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn write_file_creates_file_with_content() {
        let workspace = Workspace::new().unwrap();
        let path = WorkspacePath::parse("a.txt").unwrap();
        workspace.write_file(&path, "hello\n", None).unwrap();
        let content = std::fs::read_to_string(workspace.root().join("a.txt")).unwrap();
        assert_eq!(content, "hello\n");
    }

    #[test]
    fn write_file_creates_parent_directories() {
        let workspace = Workspace::new().unwrap();
        let path = WorkspacePath::parse("nested/dir/a.txt").unwrap();
        workspace.write_file(&path, "hi", None).unwrap();
        assert!(workspace.root().join("nested/dir/a.txt").is_file());
    }

    #[test]
    fn write_file_rejects_existing_target() {
        let workspace = Workspace::new().unwrap();
        let path = WorkspacePath::parse("a.txt").unwrap();
        workspace.write_file(&path, "first", None).unwrap();
        let err = workspace.write_file(&path, "second", None).unwrap_err();
        assert!(matches!(err, WriteFileError::TargetAlreadyExists));
        assert_eq!(err.code().as_str(), "step.write.target_exists");
        // Not silently overwritten.
        let content = std::fs::read_to_string(workspace.root().join("a.txt")).unwrap();
        assert_eq!(content, "first");
    }

    #[test]
    fn write_file_rejects_existing_directory_target() {
        let workspace = Workspace::new().unwrap();
        std::fs::create_dir_all(workspace.root().join("a-dir")).unwrap();
        let path = WorkspacePath::parse("a-dir").unwrap();
        let err = workspace.write_file(&path, "x", None).unwrap_err();
        assert!(matches!(err, WriteFileError::TargetAlreadyExists));
    }

    #[test]
    fn write_file_rejects_regular_file_in_parent_path() {
        let workspace = Workspace::new().unwrap();
        std::fs::write(workspace.root().join("blocker"), b"i am a file").unwrap();
        let path = WorkspacePath::parse("blocker/child.txt").unwrap();
        let err = workspace.write_file(&path, "x", None).unwrap_err();
        assert!(matches!(err, WriteFileError::ParentNotADirectory));
        assert_eq!(err.code().as_str(), "step.write.parent_not_a_directory");
    }

    // A `$` action can plant a symlink inside the workspace (e.g. `$ ln -s
    // /tmp escape`) before a later `write` step runs. Without rejecting
    // symlink ancestors, `create_dir_all` / file creation would follow that
    // symlink and let `write` escape the isolated workspace entirely.
    #[test]
    #[cfg(unix)]
    fn write_file_rejects_symlink_in_parent_path_instead_of_following_it() {
        let workspace = Workspace::new().unwrap();
        let outside = tempfile::TempDir::new().unwrap();

        std::os::unix::fs::symlink(outside.path(), workspace.root().join("escape")).unwrap();

        let path = WorkspacePath::parse("escape/leaked.txt").unwrap();
        let err = workspace.write_file(&path, "leaked", None).unwrap_err();
        assert!(matches!(err, WriteFileError::ParentNotADirectory));

        // Nothing was written outside the workspace through the symlink.
        assert!(!outside.path().join("leaked.txt").exists());
    }

    #[test]
    #[cfg(unix)]
    fn write_file_rejects_symlink_to_regular_file_in_parent_path() {
        let workspace = Workspace::new().unwrap();
        let real_file = workspace.root().join("real.txt");
        std::fs::write(&real_file, b"i am a file").unwrap();
        std::os::unix::fs::symlink(&real_file, workspace.root().join("link")).unwrap();

        let path = WorkspacePath::parse("link/child.txt").unwrap();
        let err = workspace.write_file(&path, "x", None).unwrap_err();
        assert!(matches!(err, WriteFileError::ParentNotADirectory));
    }

    #[cfg(unix)]
    fn permission_bits(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    // Sampled at the values a script actually reaches for — an executable fake
    // command, an owner-only secret — plus the fully closed and fully open ends
    // of the range.
    #[cfg(unix)]
    #[rstest]
    #[case::executable(0o755)]
    #[case::owner_executable(0o700)]
    #[case::world_readable(0o644)]
    #[case::owner_only(0o600)]
    #[case::closed(0o000)]
    #[case::open(0o777)]
    fn write_file_applies_the_requested_mode_exactly(#[case] requested: u32) {
        let workspace = Workspace::new().unwrap();
        let path = WorkspacePath::parse("bin/tool").unwrap();
        let mode = FileMode::from_bits(requested).unwrap();

        workspace
            .write_file(&path, "#!/bin/sh\n", Some(mode))
            .unwrap();

        assert_eq!(
            permission_bits(&workspace.root().join("bin/tool")),
            requested
        );
    }

    // `0o777` is requested because it covers every bit any umask can mask:
    // whatever the ambient umask is, a `mode` routed through the file
    // *creation* mask instead of `chmod` loses exactly the bits the control
    // file below loses, and this assertion catches it.
    //
    // The one case this cannot detect is a process running with umask `0`,
    // where the two implementations are indistinguishable because nothing is
    // masked at all. The test reports the observed umask on failure rather
    // than asserting a non-zero one, so it never fails for an environment
    // reason; environment-independent coverage of the same criterion belongs
    // to an e2e scenario, which can set the umask of the reportage process it
    // launches.
    #[test]
    #[cfg(unix)]
    fn write_file_does_not_let_the_umask_reduce_the_requested_mode() {
        let workspace = Workspace::new().unwrap();
        // Created the ordinary way, so the kernel masks its `0o666` creation
        // mode down to whatever the ambient umask allows.
        std::fs::write(workspace.root().join("control"), "x").unwrap();
        let umask = 0o666 & !permission_bits(&workspace.root().join("control"));

        let path = WorkspacePath::parse("open.txt").unwrap();
        let mode = FileMode::from_bits(0o777).unwrap();

        workspace.write_file(&path, "x", Some(mode)).unwrap();

        assert_eq!(
            permission_bits(&workspace.root().join("open.txt")),
            0o777,
            "requested mode reduced by the ambient umask 0o{umask:03o}"
        );
    }

    // Pins the mode a `write` step without `mode` produces, so adding `mode`
    // cannot quietly change what every existing script already gets. `0o600`
    // is the mode the temporary file is created with and then keeps: nothing
    // in the no-mode path touches permissions.
    #[test]
    #[cfg(unix)]
    fn write_file_without_a_mode_leaves_the_created_file_owner_only() {
        let workspace = Workspace::new().unwrap();
        let path = WorkspacePath::parse("a.txt").unwrap();

        workspace.write_file(&path, "hi", None).unwrap();

        assert_eq!(permission_bits(&workspace.root().join("a.txt")), 0o600);
    }

    // A `mode` describes the file the step names, not the directories the
    // step had to create to get there. Compared against a directory made the
    // ordinary way in the same process, so the expectation holds whatever the
    // ambient umask is.
    #[test]
    #[cfg(unix)]
    fn write_file_mode_does_not_apply_to_auto_created_parent_directories() {
        let workspace = Workspace::new().unwrap();
        let path = WorkspacePath::parse("nested/dir/tool").unwrap();
        let mode = FileMode::from_bits(0o700).unwrap();
        std::fs::create_dir(workspace.root().join("control")).unwrap();

        workspace.write_file(&path, "x", Some(mode)).unwrap();

        let control = permission_bits(&workspace.root().join("control"));
        assert_eq!(permission_bits(&workspace.root().join("nested")), control);
        assert_eq!(
            permission_bits(&workspace.root().join("nested/dir")),
            control
        );
    }

    // `mode` is not an overwrite escape hatch: create-only still wins, and the
    // existing file keeps the mode it already had.
    #[test]
    #[cfg(unix)]
    fn write_file_with_a_mode_still_refuses_an_existing_target() {
        let workspace = Workspace::new().unwrap();
        let path = WorkspacePath::parse("a.txt").unwrap();
        workspace.write_file(&path, "first", None).unwrap();

        let err = workspace
            .write_file(&path, "second", Some(FileMode::from_bits(0o777).unwrap()))
            .unwrap_err();

        assert!(matches!(err, WriteFileError::TargetAlreadyExists));
        assert_eq!(permission_bits(&workspace.root().join("a.txt")), 0o600);
        let content = std::fs::read_to_string(workspace.root().join("a.txt")).unwrap();
        assert_eq!(content, "first");
    }

    // A refused mode must not publish the file. The content is already written
    // by this point, so applying the mode after `persist_noclobber` instead
    // would leave a target behind carrying permissions the script rejected.
    #[test]
    fn write_file_leaves_no_target_when_the_mode_cannot_be_applied() {
        let workspace = Workspace::new().unwrap();
        let path = WorkspacePath::parse("bin/tool").unwrap();
        let mode = FileMode::from_bits(0o755).unwrap();

        let err = workspace
            .write_file_applying(
                &path,
                "#!/bin/sh\n",
                Some((mode, |_, _| {
                    Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
                })),
            )
            .unwrap_err();

        assert!(matches!(err, WriteFileError::SetMode { .. }));
        assert!(
            !workspace.root().join("bin/tool").exists(),
            "target must not be published when its mode was refused"
        );
    }

    #[test]
    fn set_mode_failure_is_a_write_io_error_that_names_the_refused_mode() {
        let err = WriteFileError::SetMode {
            mode: FileMode::from_bits(0o755).unwrap(),
            error: std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        };

        assert_eq!(err.code().as_str(), "step.write.io_error");
        assert!(
            err.to_string().contains("file mode to 0o755"),
            "message should name the mode as the failing part: {err}"
        );
    }

    #[test]
    fn workspace_is_removed_when_dropped() {
        let workspace = Workspace::new().unwrap();
        let root = workspace.root().to_path_buf();
        assert!(root.exists());
        drop(workspace);
        assert!(!root.exists());
    }
}
