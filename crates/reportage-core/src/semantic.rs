//! Semantic validation of the normalized expectation model.
//!
//! A semantic error means the script parses successfully, but a normalized expectation violates a policy the evaluator must reject *before* evidence comparison begins.
//! This is distinct from a parse error (the script text itself is malformed) and from an assertion failure (the expectation is valid, but observed evidence does not satisfy it).
//!
//! See docs/semantic-diagnostics.md and docs/adr/20260702T133734Z_semantic-and-assertion-diagnostic-model.md.

use crate::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticDetails};
use crate::model::{WorkspacePath, WorkspacePathError};

/// A semantic error detected while validating a normalized expectation.
#[derive(Debug, PartialEq)]
pub enum SemanticError {
    /// A file assertion path was absolute; only relative paths are accepted.
    AbsoluteFilePath(String),
    /// A file assertion path contained a `.` or `..` path segment.
    DotSegmentFilePath(String),
    /// A dir assertion subject path failed `WorkspacePath` validation (empty,
    /// absolute, or containing a `.` / `..` segment) — the same subject path
    /// rule `file` assertion paths follow.
    InvalidDirPath {
        path: String,
        reason: WorkspacePathError,
    },
    /// A `dir <"path"> contains "<name>"` entry name was empty.
    EmptyDirEntryName(String),
    /// A `dir <"path"> contains "<name>"` entry name contained a path separator (`/`).
    PathSeparatorDirEntryName(String),
    /// A `dir <"path"> contains "<name>"` entry name was `.` or `..`.
    DotEntryDirEntryName(String),
    /// A `dir <"path"> contains "<name>"` entry name contained a control character.
    ControlCharDirEntryName(String),
}

impl std::fmt::Display for SemanticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SemanticError::AbsoluteFilePath(path) => write!(
                f,
                "file assertion path '{path}' must be relative; absolute paths are rejected"
            ),
            SemanticError::DotSegmentFilePath(path) => write!(
                f,
                "file assertion path '{path}' must not contain '.' or '..' segments"
            ),
            SemanticError::InvalidDirPath { path, reason } => {
                let reason_text = match reason {
                    WorkspacePathError::Empty => "must not be empty",
                    WorkspacePathError::Absolute => "must be relative; absolute paths are rejected",
                    WorkspacePathError::DotSegment => "must not contain '.' or '..' segments",
                };
                write!(f, "directory assertion path '{path}' {reason_text}")
            }
            SemanticError::EmptyDirEntryName(_) => {
                write!(f, "directory entry name must not be empty")
            }
            SemanticError::PathSeparatorDirEntryName(name) => write!(
                f,
                "directory entry name '{name}' must not contain a path separator ('/')"
            ),
            SemanticError::DotEntryDirEntryName(name) => {
                write!(f, "directory entry name '{name}' must not be '.' or '..'")
            }
            SemanticError::ControlCharDirEntryName(name) => write!(
                f,
                "directory entry name {name:?} must not contain control characters"
            ),
        }
    }
}

impl std::error::Error for SemanticError {}

impl SemanticError {
    /// The stable, machine-readable diagnostic code for this error.
    ///
    /// See docs/diagnostics.md and docs/semantic-diagnostics.md.
    pub const fn code(&self) -> DiagnosticCode {
        match self {
            SemanticError::AbsoluteFilePath(_) => DiagnosticCode::SemanticFilePathAbsolute,
            SemanticError::DotSegmentFilePath(_) => DiagnosticCode::SemanticFilePathDotSegment,
            SemanticError::InvalidDirPath { reason, .. } => match reason {
                WorkspacePathError::Empty => DiagnosticCode::SemanticWorkspacePathEmpty,
                WorkspacePathError::Absolute => DiagnosticCode::SemanticWorkspacePathAbsolute,
                WorkspacePathError::DotSegment => DiagnosticCode::SemanticWorkspacePathDotSegment,
            },
            SemanticError::EmptyDirEntryName(_) => DiagnosticCode::SemanticDirEntryNameEmpty,
            SemanticError::PathSeparatorDirEntryName(_) => {
                DiagnosticCode::SemanticDirEntryNamePathSeparator
            }
            SemanticError::DotEntryDirEntryName(_) => DiagnosticCode::SemanticDirEntryNameDotEntry,
            SemanticError::ControlCharDirEntryName(_) => {
                DiagnosticCode::SemanticDirEntryNameControlChar
            }
        }
    }

    /// Converts this error into the struct-based diagnostic model.
    pub fn to_diagnostic(&self) -> Diagnostic {
        let raw_value = match self {
            SemanticError::AbsoluteFilePath(path) | SemanticError::DotSegmentFilePath(path) => {
                path.clone()
            }
            SemanticError::InvalidDirPath { path, .. } => path.clone(),
            SemanticError::EmptyDirEntryName(name)
            | SemanticError::PathSeparatorDirEntryName(name)
            | SemanticError::DotEntryDirEntryName(name)
            | SemanticError::ControlCharDirEntryName(name) => name.clone(),
        };

        Diagnostic {
            code: self.code(),
            message: self.to_string(),
            location: None,
            details: DiagnosticDetails {
                raw_value: Some(raw_value),
                ..Default::default()
            },
        }
    }
}

/// Validates a file assertion path against reportage's path policy.
///
/// - Absolute paths are rejected.
/// - `.` and `..` path segments are rejected.
///
/// This centralizes path policy validation for the `file <"path">` subject so every predicate (`exists`, `contains`, and future predicates) shares the same rejection rule.
/// See docs/adr/20260704T112155Z_subject-first-file-assertion-syntax.md.
pub fn validate_file_path(path: &str) -> Result<(), SemanticError> {
    if path.starts_with('/') {
        return Err(SemanticError::AbsoluteFilePath(path.to_string()));
    }
    for segment in path.split('/') {
        if segment == "." || segment == ".." {
            return Err(SemanticError::DotSegmentFilePath(path.to_string()));
        }
    }
    Ok(())
}

/// Validates a dir assertion subject path against the same `WorkspacePath` subject path rule
/// used elsewhere in reportage (relative, non-empty, no `.` / `..` segments).
///
/// Reuses `WorkspacePath::parse` directly rather than reimplementing the rule, so a `dir`
/// subject path and a `write` step path can never silently drift apart.
/// See docs/adr/20260706T000000Z_subject-first-directory-assertion-syntax.md.
pub fn validate_dir_path(path: &str) -> Result<(), SemanticError> {
    WorkspacePath::parse(path)
        .map(|_| ())
        .map_err(|reason| SemanticError::InvalidDirPath {
            path: path.to_string(),
            reason,
        })
}

/// Validates a `dir <"path"> contains "<name>"` entry name.
///
/// `name` must denote a single directory entry, not a nested path: it must be non-empty, must
/// not contain a path separator (`/`), must not be `.` or `..`, and must not contain control
/// characters.
///
/// This is a semantic error, not an assertion failure: the evaluator rejects an invalid entry
/// name before attempting any filesystem evidence comparison.
/// See docs/adr/20260706T000000Z_subject-first-directory-assertion-syntax.md.
pub fn validate_dir_entry_name(name: &str) -> Result<(), SemanticError> {
    if name.is_empty() {
        return Err(SemanticError::EmptyDirEntryName(name.to_string()));
    }
    if name.contains('/') {
        return Err(SemanticError::PathSeparatorDirEntryName(name.to_string()));
    }
    if name == "." || name == ".." {
        return Err(SemanticError::DotEntryDirEntryName(name.to_string()));
    }
    if name.chars().any(|c| c.is_control()) {
        return Err(SemanticError::ControlCharDirEntryName(name.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_path_is_valid() {
        assert!(validate_file_path("a/b.txt").is_ok());
    }

    #[test]
    fn absolute_path_is_rejected() {
        let err = validate_file_path("/etc/passwd").unwrap_err();
        assert_eq!(err, SemanticError::AbsoluteFilePath("/etc/passwd".into()));
        assert_eq!(err.code().as_str(), "semantic.file_path.absolute");
    }

    #[test]
    fn dot_segment_is_rejected() {
        let err = validate_file_path("./a.txt").unwrap_err();
        assert_eq!(err.code().as_str(), "semantic.file_path.dot_segment");
    }

    #[test]
    fn dot_dot_segment_is_rejected() {
        let err = validate_file_path("../a.txt").unwrap_err();
        assert_eq!(
            err,
            SemanticError::DotSegmentFilePath("../a.txt".to_string())
        );
        assert_eq!(err.code().as_str(), "semantic.file_path.dot_segment");
    }

    #[test]
    fn dot_dot_segment_in_middle_is_rejected() {
        let err = validate_file_path("a/../b.txt").unwrap_err();
        assert_eq!(err.code().as_str(), "semantic.file_path.dot_segment");
    }

    #[test]
    fn to_diagnostic_carries_code_and_raw_value() {
        let err = validate_file_path("/tmp/x").unwrap_err();
        let diagnostic = err.to_diagnostic();
        assert_eq!(diagnostic.code.as_str(), "semantic.file_path.absolute");
        assert_eq!(diagnostic.message, err.to_string());
        assert_eq!(diagnostic.location, None);
        assert_eq!(diagnostic.details.raw_value.as_deref(), Some("/tmp/x"));
    }

    // ─── validate_dir_path (#66) ────────────────────────────────────────────

    #[test]
    fn dir_relative_path_is_valid() {
        assert!(validate_dir_path("a/b").is_ok());
    }

    #[test]
    fn dir_empty_path_is_rejected() {
        let err = validate_dir_path("").unwrap_err();
        assert_eq!(err.code().as_str(), "semantic.workspace_path.empty");
    }

    #[test]
    fn dir_absolute_path_is_rejected() {
        let err = validate_dir_path("/etc").unwrap_err();
        assert_eq!(err.code().as_str(), "semantic.workspace_path.absolute");
    }

    #[test]
    fn dir_dot_segment_path_is_rejected() {
        let err = validate_dir_path("a/../b").unwrap_err();
        assert_eq!(err.code().as_str(), "semantic.workspace_path.dot_segment");
    }

    #[test]
    fn dir_path_to_diagnostic_carries_code_and_raw_value() {
        let err = validate_dir_path("/etc").unwrap_err();
        let diagnostic = err.to_diagnostic();
        assert_eq!(diagnostic.code.as_str(), "semantic.workspace_path.absolute");
        assert_eq!(diagnostic.message, err.to_string());
        assert_eq!(diagnostic.details.raw_value.as_deref(), Some("/etc"));
    }

    // ─── validate_dir_entry_name (#66) ──────────────────────────────────────

    #[test]
    fn entry_name_simple_is_valid() {
        assert!(validate_dir_entry_name("result.json").is_ok());
    }

    #[test]
    fn entry_name_empty_is_rejected() {
        let err = validate_dir_entry_name("").unwrap_err();
        assert_eq!(err.code().as_str(), "semantic.dir_entry_name.empty");
    }

    #[test]
    fn entry_name_with_path_separator_is_rejected() {
        let err = validate_dir_entry_name("a/b").unwrap_err();
        assert_eq!(
            err.code().as_str(),
            "semantic.dir_entry_name.path_separator"
        );
    }

    #[test]
    fn entry_name_dot_is_rejected() {
        let err = validate_dir_entry_name(".").unwrap_err();
        assert_eq!(err.code().as_str(), "semantic.dir_entry_name.dot_entry");
    }

    #[test]
    fn entry_name_dot_dot_is_rejected() {
        let err = validate_dir_entry_name("..").unwrap_err();
        assert_eq!(err.code().as_str(), "semantic.dir_entry_name.dot_entry");
    }

    #[test]
    fn entry_name_with_control_char_is_rejected() {
        let err = validate_dir_entry_name("a\nb").unwrap_err();
        assert_eq!(err.code().as_str(), "semantic.dir_entry_name.control_char");
    }

    #[test]
    fn entry_name_to_diagnostic_carries_code_and_raw_value() {
        let err = validate_dir_entry_name("a/b").unwrap_err();
        let diagnostic = err.to_diagnostic();
        assert_eq!(
            diagnostic.code.as_str(),
            "semantic.dir_entry_name.path_separator"
        );
        assert_eq!(diagnostic.message, err.to_string());
        assert_eq!(diagnostic.details.raw_value.as_deref(), Some("a/b"));
    }
}
