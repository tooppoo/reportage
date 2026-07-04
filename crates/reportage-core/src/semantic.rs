//! Semantic validation of the normalized expectation model.
//!
//! A semantic error means the script parses successfully, but a normalized expectation violates a policy the evaluator must reject *before* evidence comparison begins.
//! This is distinct from a parse error (the script text itself is malformed) and from an assertion failure (the expectation is valid, but observed evidence does not satisfy it).
//!
//! See docs/semantic-diagnostics.md and docs/adr/20260702T133734Z_semantic-and-assertion-diagnostic-model.md.

use crate::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticDetails};

/// A semantic error detected while validating a normalized expectation.
#[derive(Debug, PartialEq)]
pub enum SemanticError {
    /// A file assertion path was absolute; only relative paths are accepted.
    AbsoluteFilePath(String),
    /// A file assertion path contained a `.` or `..` path segment.
    DotSegmentFilePath(String),
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
        }
    }

    /// Converts this error into the struct-based diagnostic model.
    pub fn to_diagnostic(&self) -> Diagnostic {
        let raw_value = match self {
            SemanticError::AbsoluteFilePath(path) | SemanticError::DotSegmentFilePath(path) => {
                path.clone()
            }
        };

        Diagnostic {
            code: self.code(),
            message: self.to_string(),
            location: None,
            details: DiagnosticDetails {
                pest_message: None,
                raw_value: Some(raw_value),
            },
        }
    }
}

/// Validates a file assertion path against reportage's path policy.
///
/// - Absolute paths are rejected.
/// - `.` and `..` path segments are rejected.
///
/// This centralizes path policy validation for the `file "<path>"` subject so every predicate (`exists`, `contains`, and future predicates) shares the same rejection rule.
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
}
