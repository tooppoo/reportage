//! Output directory validation and existing-output-preserving replacement.
//!
//! [`OutputDirectory`] is the single owner of the `--out-dir` rules: they are
//! independent of format and layout, so adding a format or layout must not
//! change them. Replacement is process-failure safe (an existing document is
//! either fully replaced or left untouched); power-loss durability and
//! `fsync` are out of scope in v0. See
//! docs/adr/20260723T070556Z_documentation-generation-command.md.

use std::io::Write;
use std::path::{Component, Path, PathBuf};

#[derive(Debug)]
pub enum OutputError {
    /// `--out-dir` exists but is not a directory (a regular file, or another
    /// non-directory filesystem object).
    NotADirectory(PathBuf),
    /// `--out-dir` exists but is a symlink; v0 requires a real directory.
    SymlinkOutputDirectory(PathBuf),
    /// Creating the output directory failed at the OS level.
    CreateFailed { path: PathBuf, message: String },
    /// A layout produced a relative output path that could leave the output
    /// root (absolute, `..`, or root/prefix components). This is an internal
    /// contract violation, surfaced as an error instead of writing outside
    /// the root.
    InvalidRelativePath(String),
    /// The output path exists but is a directory or a symlink, which is never
    /// silently replaced.
    ExistingOutputNotReplaceable(PathBuf),
    /// Creating or writing the temporary file failed at the OS level. The
    /// existing document, if any, is untouched.
    WriteFailed { path: PathBuf, message: String },
    /// Replacing the output with the fully written temporary file failed.
    /// The existing document, if any, is untouched.
    ReplaceFailed { path: PathBuf, message: String },
}

impl std::fmt::Display for OutputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputError::NotADirectory(path) => {
                write!(
                    f,
                    "output directory '{}' exists but is not a directory",
                    path.display()
                )
            }
            OutputError::SymlinkOutputDirectory(path) => {
                write!(
                    f,
                    "output directory '{}' is a symlink; a regular directory is required",
                    path.display()
                )
            }
            OutputError::CreateFailed { path, message } => {
                write!(
                    f,
                    "cannot create output directory '{}': {message}",
                    path.display()
                )
            }
            OutputError::InvalidRelativePath(path) => {
                write!(
                    f,
                    "internal error: layout produced output path '{path}' outside the output directory"
                )
            }
            OutputError::ExistingOutputNotReplaceable(path) => {
                write!(
                    f,
                    "output path '{}' exists but is not a regular file; it is not replaced",
                    path.display()
                )
            }
            OutputError::WriteFailed { path, message } => {
                write!(
                    f,
                    "cannot write generated document '{}': {message}",
                    path.display()
                )
            }
            OutputError::ReplaceFailed { path, message } => {
                write!(
                    f,
                    "cannot replace generated document '{}': {message}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for OutputError {}

/// A validated output root. Constructing it is the only way to write
/// generated documents, so every write path goes through the same
/// containment and replacement rules.
#[derive(Debug)]
pub struct OutputDirectory(PathBuf);

impl OutputDirectory {
    /// Validates `path` as the output root, creating it (recursively) when it
    /// does not exist.
    ///
    /// Callers must invoke this only after source resolution, loading,
    /// Catalog construction, and rendering have succeeded: a failing
    /// generation must not create directories.
    pub fn prepare(path: &Path) -> Result<Self, OutputError> {
        match std::fs::symlink_metadata(path) {
            Ok(metadata) => {
                let file_type = metadata.file_type();
                if file_type.is_symlink() {
                    Err(OutputError::SymlinkOutputDirectory(path.to_path_buf()))
                } else if !file_type.is_dir() {
                    Err(OutputError::NotADirectory(path.to_path_buf()))
                } else {
                    Ok(Self(path.to_path_buf()))
                }
            }
            Err(_) => {
                std::fs::create_dir_all(path).map_err(|e| OutputError::CreateFailed {
                    path: path.to_path_buf(),
                    message: e.to_string(),
                })?;
                Ok(Self(path.to_path_buf()))
            }
        }
    }

    /// Writes one generated document at `relative_path` under the root using
    /// existing-output-preserving replacement:
    ///
    /// 1. the whole document is written to a temporary file in the same
    ///    directory as the target,
    /// 2. only after the write fully succeeds, the target is replaced with a
    ///    platform-appropriate rename,
    /// 3. any failure before the replacement leaves an existing target
    ///    untouched, and the temporary file is removed best-effort (on drop).
    ///
    /// An existing regular file is overwritten; a directory or symlink at the
    /// target is an error. Unrelated files in the output directory are never
    /// touched.
    pub fn write_document(
        &self,
        relative_path: &str,
        contents: &str,
    ) -> Result<PathBuf, OutputError> {
        validate_relative_path(relative_path)?;
        let target = self.0.join(relative_path);

        if let Ok(metadata) = std::fs::symlink_metadata(&target) {
            let file_type = metadata.file_type();
            if !file_type.is_file() {
                return Err(OutputError::ExistingOutputNotReplaceable(target));
            }
        }

        let write_error = |e: std::io::Error| OutputError::WriteFailed {
            path: target.clone(),
            message: e.to_string(),
        };
        let parent = target.parent().unwrap_or(&self.0);
        let mut temp = tempfile::Builder::new()
            .prefix(".reportage-docs-")
            .suffix(".tmp")
            .tempfile_in(parent)
            .map_err(write_error)?;
        temp.write_all(contents.as_bytes()).map_err(write_error)?;
        temp.flush().map_err(write_error)?;

        temp.persist(&target)
            .map_err(|e| OutputError::ReplaceFailed {
                path: target.clone(),
                message: e.error.to_string(),
            })?;

        Ok(target)
    }
}

/// Enforces the layout output path contract: only normal components, so the
/// joined path can never leave the output root.
fn validate_relative_path(relative_path: &str) -> Result<(), OutputError> {
    let path = Path::new(relative_path);
    let only_normal_components = !relative_path.is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
    if only_normal_components {
        Ok(())
    } else {
        Err(OutputError::InvalidRelativePath(relative_path.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_a_missing_output_directory_recursively() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("generated/docs");

        let output = OutputDirectory::prepare(&root).unwrap();
        assert!(root.is_dir());

        let written = output.write_document("index.txt", "content\n").unwrap();
        assert_eq!(std::fs::read_to_string(written).unwrap(), "content\n");
    }

    #[test]
    fn rejects_a_regular_file_as_output_directory() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("blocker");
        std::fs::write(&root, "").unwrap();

        let err = OutputDirectory::prepare(&root).unwrap_err();
        assert!(matches!(err, OutputError::NotADirectory(_)));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlink_as_output_directory() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let err = OutputDirectory::prepare(&link).unwrap_err();
        assert!(matches!(err, OutputError::SymlinkOutputDirectory(_)));
    }

    #[test]
    fn creation_failure_is_reported_as_create_failed() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, "").unwrap();

        let err = OutputDirectory::prepare(&blocker.join("out")).unwrap_err();
        assert!(matches!(err, OutputError::CreateFailed { .. }));
    }

    #[test]
    fn overwrites_an_existing_regular_file() {
        let dir = tempfile::tempdir().unwrap();
        let output = OutputDirectory::prepare(dir.path()).unwrap();
        std::fs::write(dir.path().join("index.txt"), "old").unwrap();

        output.write_document("index.txt", "new\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("index.txt")).unwrap(),
            "new\n"
        );
    }

    #[test]
    fn rejects_a_directory_at_the_output_path() {
        let dir = tempfile::tempdir().unwrap();
        let output = OutputDirectory::prepare(dir.path()).unwrap();
        std::fs::create_dir(dir.path().join("index.txt")).unwrap();

        let err = output.write_document("index.txt", "new\n").unwrap_err();
        assert!(matches!(err, OutputError::ExistingOutputNotReplaceable(_)));
        assert!(dir.path().join("index.txt").is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlink_at_the_output_path_and_preserves_its_target() {
        let dir = tempfile::tempdir().unwrap();
        let output = OutputDirectory::prepare(dir.path()).unwrap();
        let real = dir.path().join("real.txt");
        std::fs::write(&real, "untouched").unwrap();
        std::os::unix::fs::symlink(&real, dir.path().join("index.txt")).unwrap();

        let err = output.write_document("index.txt", "new\n").unwrap_err();
        assert!(matches!(err, OutputError::ExistingOutputNotReplaceable(_)));
        assert_eq!(std::fs::read_to_string(&real).unwrap(), "untouched");
    }

    #[test]
    fn unrelated_files_and_temporaries_do_not_remain() {
        let dir = tempfile::tempdir().unwrap();
        let output = OutputDirectory::prepare(dir.path()).unwrap();
        std::fs::write(dir.path().join("unrelated.txt"), "keep").unwrap();

        output.write_document("index.txt", "content\n").unwrap();

        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        assert!(names.contains(&"unrelated.txt".to_string()));
        assert!(names.contains(&"index.txt".to_string()));
        assert!(
            names.iter().all(|name| !name.ends_with(".tmp")),
            "no temporary file may remain: {names:?}"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("unrelated.txt")).unwrap(),
            "keep"
        );
    }

    /// The replacement guarantee: a temp-file write failure leaves an
    /// existing document byte-identical and leaves no temporary residue.
    #[cfg(unix)]
    #[test]
    fn write_failure_preserves_the_existing_document() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let output = OutputDirectory::prepare(dir.path()).unwrap();
        std::fs::write(dir.path().join("index.txt"), "old content").unwrap();

        // r-x on the output directory: creating the temporary file fails.
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o555)).unwrap();

        // Privileged users bypass permission checks; skip rather than assert
        // the wrong thing there.
        let write_blocked = std::fs::write(dir.path().join(".probe"), "").is_err();
        let result = if write_blocked {
            Some(output.write_document("index.txt", "new content\n"))
        } else {
            let _ = std::fs::remove_file(dir.path().join(".probe"));
            None
        };

        // Restore permissions before TempDir cleanup regardless of outcome.
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();

        if let Some(result) = result {
            let err = result.unwrap_err();
            assert!(
                matches!(err, OutputError::WriteFailed { .. }),
                "expected WriteFailed, got {err:?}"
            );
            assert_eq!(
                std::fs::read_to_string(dir.path().join("index.txt")).unwrap(),
                "old content"
            );
            let leftovers: Vec<_> = std::fs::read_dir(dir.path())
                .unwrap()
                .map(|entry| entry.unwrap().file_name().into_string().unwrap())
                .filter(|name| name != "index.txt")
                .collect();
            assert!(
                leftovers.is_empty(),
                "no temporary file may remain: {leftovers:?}"
            );
        }
    }

    #[test]
    fn rejects_relative_paths_that_could_escape_the_root() {
        let dir = tempfile::tempdir().unwrap();
        let output = OutputDirectory::prepare(dir.path()).unwrap();

        for relative in ["../escape.txt", "/absolute.txt", "a/../../b.txt", ""] {
            let err = output.write_document(relative, "x").unwrap_err();
            assert!(
                matches!(err, OutputError::InvalidRelativePath(_)),
                "path {relative:?} must be rejected"
            );
        }
    }
}
