//! Per-concrete-case isolated workspace.
//!
//! Each concrete case runs in its own workspace directory: `$` actions run
//! with it as their working directory, `write` steps write into it, and file
//! expectations resolve paths against it. See docs2/reference/execution-model.md —
//! Workspace lifecycle, and docs2/reference/semantics.md — Write step.

use std::path::Path;

use crate::diagnostic::DiagnosticCode;
use crate::model::WorkspacePath;

/// An isolated case workspace, backed by a temporary directory that is
/// removed when the workspace is dropped.
///
/// See docs2/reference/execution-model.md — Cleanup and preservation: v0 does not offer
/// workspace preservation, so unconditional cleanup on drop is correct.
pub struct Workspace {
    dir: tempfile::TempDir,
}

/// Error writing a file into a workspace via a `write` step.
///
/// This is the runtime step error classification for side-effecting steps.
/// See docs2/reference/semantics.md — Write step, and the accompanying ADR.
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
        }
    }
}

impl std::error::Error for WriteFileError {}

impl WriteFileError {
    /// The stable, machine-readable diagnostic code for this error.
    /// See docs2/reference/diagnostics.md.
    pub const fn code(&self) -> DiagnosticCode {
        match self {
            WriteFileError::TargetAlreadyExists => DiagnosticCode::StepWriteTargetExists,
            WriteFileError::ParentNotADirectory => DiagnosticCode::StepWriteParentNotADirectory,
            WriteFileError::Io(_) => DiagnosticCode::StepWriteIoError,
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
    pub fn write_file(&self, path: &WorkspacePath, content: &str) -> Result<(), WriteFileError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_file_creates_file_with_content() {
        let workspace = Workspace::new().unwrap();
        let path = WorkspacePath::parse("a.txt").unwrap();
        workspace.write_file(&path, "hello\n").unwrap();
        let content = std::fs::read_to_string(workspace.root().join("a.txt")).unwrap();
        assert_eq!(content, "hello\n");
    }

    #[test]
    fn write_file_creates_parent_directories() {
        let workspace = Workspace::new().unwrap();
        let path = WorkspacePath::parse("nested/dir/a.txt").unwrap();
        workspace.write_file(&path, "hi").unwrap();
        assert!(workspace.root().join("nested/dir/a.txt").is_file());
    }

    #[test]
    fn write_file_rejects_existing_target() {
        let workspace = Workspace::new().unwrap();
        let path = WorkspacePath::parse("a.txt").unwrap();
        workspace.write_file(&path, "first").unwrap();
        let err = workspace.write_file(&path, "second").unwrap_err();
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
        let err = workspace.write_file(&path, "x").unwrap_err();
        assert!(matches!(err, WriteFileError::TargetAlreadyExists));
    }

    #[test]
    fn write_file_rejects_regular_file_in_parent_path() {
        let workspace = Workspace::new().unwrap();
        std::fs::write(workspace.root().join("blocker"), b"i am a file").unwrap();
        let path = WorkspacePath::parse("blocker/child.txt").unwrap();
        let err = workspace.write_file(&path, "x").unwrap_err();
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
        let err = workspace.write_file(&path, "leaked").unwrap_err();
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
        let err = workspace.write_file(&path, "x").unwrap_err();
        assert!(matches!(err, WriteFileError::ParentNotADirectory));
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
