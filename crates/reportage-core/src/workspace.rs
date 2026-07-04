//! Per-concrete-case isolated workspace.
//!
//! Each concrete case runs in its own workspace directory: `$` actions run
//! with it as their working directory, `write` steps write into it, and file
//! expectations resolve paths against it. See docs/semantics.md — Workspace
//! lifecycle and Write step.

use std::path::Path;

use crate::diagnostic::DiagnosticCode;
use crate::model::WorkspacePath;

/// An isolated case workspace, backed by a temporary directory that is
/// removed when the workspace is dropped.
///
/// See docs/semantics.md — Cleanup and preservation: v0 does not offer
/// workspace preservation, so unconditional cleanup on drop is correct.
pub struct Workspace {
    dir: tempfile::TempDir,
}

/// Error writing a file into a workspace via a `write` step.
///
/// This is the runtime step error classification for side-effecting steps.
/// See docs/semantics.md — Write step, and the accompanying ADR.
#[derive(Debug)]
pub enum WriteFileError {
    /// The target path already exists (file, directory, or symlink).
    /// `write` is create-only and never silently overwrites.
    TargetAlreadyExists,
    /// A regular file exists somewhere along the target's parent path,
    /// so the parent directories cannot be created.
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
            WriteFileError::ParentNotADirectory => {
                write!(f, "a regular file exists along the target's parent path")
            }
            WriteFileError::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for WriteFileError {}

impl WriteFileError {
    /// The stable, machine-readable diagnostic code for this error.
    /// See docs/diagnostics.md.
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
    /// are created automatically, unless a regular file already occupies
    /// part of that parent path.
    pub fn write_file(&self, path: &WorkspacePath, content: &str) -> Result<(), WriteFileError> {
        if self.parent_has_regular_file(path) {
            return Err(WriteFileError::ParentNotADirectory);
        }

        let target = self.root().join(path.as_str());
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(WriteFileError::Io)?;
        }

        use std::io::Write as _;
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
        {
            Ok(mut file) => file
                .write_all(content.as_bytes())
                .map_err(WriteFileError::Io),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                Err(WriteFileError::TargetAlreadyExists)
            }
            Err(e) => Err(WriteFileError::Io(e)),
        }
    }

    /// Returns true if a regular file (not a directory) already occupies
    /// one of `path`'s ancestor components under the workspace root.
    ///
    /// Checked explicitly, rather than inferred from `create_dir_all`'s
    /// error kind, so the classification does not depend on platform-
    /// specific `io::ErrorKind` variants.
    fn parent_has_regular_file(&self, path: &WorkspacePath) -> bool {
        let mut ancestor = self.root().to_path_buf();
        let rel = Path::new(path.as_str());
        let mut components: Vec<_> = rel.components().collect();
        // The last component is the file name itself, not a parent directory.
        components.pop();
        for component in components {
            ancestor.push(component);
            if std::fs::symlink_metadata(&ancestor).is_ok_and(|meta| meta.is_file()) {
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

    #[test]
    fn workspace_is_removed_when_dropped() {
        let workspace = Workspace::new().unwrap();
        let root = workspace.root().to_path_buf();
        assert!(root.exists());
        drop(workspace);
        assert!(!root.exists());
    }
}
