//! Stable, machine-readable diagnostic identity for parser and validator errors.
//!
//! See docs/diagnostics.md for the naming convention, compatibility policy, and the split between `code` (stable), `message` (improvable), `location`, and `details` (auxiliary, weaker stability).

use std::fmt;

/// A stable, machine-readable identifier for a diagnostic.
///
/// The string form (see [`DiagnosticCode::as_str`]) is the external contract that tests and tooling depend on.
/// It is intentionally independent of the Rust error enum variant names that produce it, so internal error types can be restructured without renaming published codes.
///
/// Codes use a dot-separated `<domain>.<reason>` namespace, e.g. `parse.invalid_exit_code`.
/// Renaming or removing an existing code is a breaking change; adding a new code is not.
///
/// Marked `#[non_exhaustive]` so that adding a new code does not break downstream `match` expressions that would otherwise have to be exhaustive over every variant.
/// Downstream code must include a wildcard (`_`) arm when matching.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticCode {
    /// A pest grammar syntax error, wrapped without a more specific code.
    ParseSyntax,
    /// A case block contains no steps.
    ParseEmptyCase,
    /// A case block contains no assertion block.
    ParseMissingAssertionBlock,
    /// An action step's command is empty after trimming whitespace.
    ParseEmptyAction,
    /// An exit code expectation falls outside `0..=255`.
    ParseInvalidExitCode,
    /// A file assertion path is absolute; only relative paths are accepted.
    SemanticFilePathAbsolute,
    /// A file assertion path contains a `.` or `..` segment.
    SemanticFilePathDotSegment,
    /// `file "<path>" exists` observed a missing path.
    AssertionFileExistsMissing,
    /// `file "<path>" exists` observed a path that is not a regular file (e.g. a directory).
    AssertionFileExistsNotAFile,
    /// `file "<path>" contains "<text>"` observed a path that is not a readable UTF-8 regular file (missing, a directory, unreadable, or non-UTF-8 content).
    AssertionFileContainsPreconditionUnmet,
    /// `file "<path>" contains "<text>"` observed a readable UTF-8 regular file that does not contain the expected substring.
    AssertionFileContainsMismatch,
    /// A `dir "<path>" contains "<name>"` entry name was empty.
    SemanticDirEntryNameEmpty,
    /// A `dir "<path>" contains "<name>"` entry name contained a path separator (`/`).
    SemanticDirEntryNamePathSeparator,
    /// A `dir "<path>" contains "<name>"` entry name was `.` or `..`.
    SemanticDirEntryNameDotEntry,
    /// A `dir "<path>" contains "<name>"` entry name contained a control character.
    SemanticDirEntryNameControlChar,
    /// `dir "<path>" exists` observed a missing path.
    AssertionDirExistsMissing,
    /// `dir "<path>" exists` observed a path that is not a directory (e.g. a regular file).
    AssertionDirExistsNotADirectory,
    /// `dir "<path>" contains "<name>"` observed a subject path that does not exist.
    AssertionDirContainsSubjectMissing,
    /// `dir "<path>" contains "<name>"` observed a subject path that is not a directory.
    AssertionDirContainsSubjectNotADirectory,
    /// `dir "<path>" contains "<name>"` observed a directory that does not contain an entry named `<name>`.
    AssertionDirContainsEntryMissing,
    /// A `not` / `all` / `any` logical composition block contains zero expectation expressions.
    /// See docs/semantic-diagnostics.md.
    SemanticExpectationEmptyBlock,
    /// A `write` step's workspace path was empty.
    SemanticWorkspacePathEmpty,
    /// A `write` step's workspace path was absolute.
    SemanticWorkspacePathAbsolute,
    /// A `write` step's workspace path contained a `.` or `..` segment.
    SemanticWorkspacePathDotSegment,
    /// A `write` step's fenced raw text block contains a non-blank body line
    /// that is indented less than the closing fence.
    ParseRawBlockShallowIndent,
    /// A `write` step's target path already existed; create-only writes reject this.
    StepWriteTargetExists,
    /// A `write` step's target path had a regular file somewhere in its parent path.
    StepWriteParentNotADirectory,
    /// A `write` step failed due to an OS-level I/O error.
    StepWriteIoError,
}

impl DiagnosticCode {
    /// The stable external string representation of this code.
    ///
    /// This is the identifier tests and tooling must depend on instead of `Display` message text or internal enum variant names.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ParseSyntax => "parse.syntax",
            Self::ParseEmptyCase => "parse.empty_case",
            Self::ParseMissingAssertionBlock => "parse.missing_assertion_block",
            Self::ParseEmptyAction => "parse.empty_action",
            Self::ParseInvalidExitCode => "parse.invalid_exit_code",
            Self::SemanticFilePathAbsolute => "semantic.file_path.absolute",
            Self::SemanticFilePathDotSegment => "semantic.file_path.dot_segment",
            Self::AssertionFileExistsMissing => "assertion.file.exists_missing",
            Self::AssertionFileExistsNotAFile => "assertion.file.exists_not_a_file",
            Self::AssertionFileContainsPreconditionUnmet => {
                "assertion.file.contains_precondition_unmet"
            }
            Self::AssertionFileContainsMismatch => "assertion.file.contains_mismatch",
            Self::SemanticDirEntryNameEmpty => "semantic.dir_entry_name.empty",
            Self::SemanticDirEntryNamePathSeparator => "semantic.dir_entry_name.path_separator",
            Self::SemanticDirEntryNameDotEntry => "semantic.dir_entry_name.dot_entry",
            Self::SemanticDirEntryNameControlChar => "semantic.dir_entry_name.control_char",
            Self::AssertionDirExistsMissing => "assertion.dir.exists_missing",
            Self::AssertionDirExistsNotADirectory => "assertion.dir.exists_not_directory",
            Self::AssertionDirContainsSubjectMissing => "assertion.dir.contains_subject_missing",
            Self::AssertionDirContainsSubjectNotADirectory => {
                "assertion.dir.contains_subject_not_directory"
            }
            Self::AssertionDirContainsEntryMissing => "assertion.dir.contains_entry_missing",
            Self::SemanticExpectationEmptyBlock => "semantic.expectation.empty_block",
            Self::SemanticWorkspacePathEmpty => "semantic.workspace_path.empty",
            Self::SemanticWorkspacePathAbsolute => "semantic.workspace_path.absolute",
            Self::SemanticWorkspacePathDotSegment => "semantic.workspace_path.dot_segment",
            Self::ParseRawBlockShallowIndent => "parse.raw_block.shallow_indent",
            Self::StepWriteTargetExists => "step.write.target_exists",
            Self::StepWriteParentNotADirectory => "step.write.parent_not_a_directory",
            Self::StepWriteIoError => "step.write.io_error",
        }
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Line/column position of a diagnostic within source text.
///
/// `column` is not always available (e.g. some parse-domain validation errors only know the line a construct started on).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticLocation {
    pub line: usize,
    pub column: Option<usize>,
}

/// Auxiliary information attached to a diagnostic.
///
/// Unlike `code`, the contents of `details` do not carry a strong stability guarantee.
/// In particular, pest-derived message text and expected-token summaries are grammar-dependent and must not be treated as a stable API by tests or tooling; depend on `code` instead.
///
/// Marked `#[non_exhaustive]` so that adding a new field does not break downstream struct-literal construction or exhaustive field patterns.
/// Downstream code should build one via `DiagnosticDetails::default()` and read fields individually rather than matching on the full field set.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiagnosticDetails {
    /// Raw message text from the pest grammar error, when the diagnostic originates from a syntax error.
    pub pest_message: Option<String>,
    /// The offending raw value (e.g. an out-of-range exit code literal or a case name), when relevant to the diagnostic.
    pub raw_value: Option<String>,
}

/// A machine-readable diagnostic produced by parsing or validating a script.
///
/// `code` is the stable identifier.
/// `message` is a human-facing string that may be improved over time without being a breaking change.
/// `location` and `details` provide position and auxiliary context respectively.
///
/// Marked `#[non_exhaustive]` for the same reason as [`DiagnosticCode`] and [`DiagnosticDetails`]: it keeps future field additions to the diagnostic model itself from being a breaking change for downstream construction or exhaustive matching.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub message: String,
    pub location: Option<DiagnosticLocation>,
    pub details: DiagnosticDetails,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_matches_as_str() {
        for code in [
            DiagnosticCode::ParseSyntax,
            DiagnosticCode::ParseEmptyCase,
            DiagnosticCode::ParseMissingAssertionBlock,
            DiagnosticCode::ParseEmptyAction,
            DiagnosticCode::ParseInvalidExitCode,
            DiagnosticCode::SemanticFilePathAbsolute,
            DiagnosticCode::SemanticFilePathDotSegment,
            DiagnosticCode::AssertionFileExistsMissing,
            DiagnosticCode::AssertionFileExistsNotAFile,
            DiagnosticCode::AssertionFileContainsPreconditionUnmet,
            DiagnosticCode::AssertionFileContainsMismatch,
            DiagnosticCode::SemanticDirEntryNameEmpty,
            DiagnosticCode::SemanticDirEntryNamePathSeparator,
            DiagnosticCode::SemanticDirEntryNameDotEntry,
            DiagnosticCode::SemanticDirEntryNameControlChar,
            DiagnosticCode::AssertionDirExistsMissing,
            DiagnosticCode::AssertionDirExistsNotADirectory,
            DiagnosticCode::AssertionDirContainsSubjectMissing,
            DiagnosticCode::AssertionDirContainsSubjectNotADirectory,
            DiagnosticCode::AssertionDirContainsEntryMissing,
            DiagnosticCode::SemanticExpectationEmptyBlock,
            DiagnosticCode::SemanticWorkspacePathEmpty,
            DiagnosticCode::SemanticWorkspacePathAbsolute,
            DiagnosticCode::SemanticWorkspacePathDotSegment,
            DiagnosticCode::ParseRawBlockShallowIndent,
            DiagnosticCode::StepWriteTargetExists,
            DiagnosticCode::StepWriteParentNotADirectory,
            DiagnosticCode::StepWriteIoError,
        ] {
            assert_eq!(code.to_string(), code.as_str());
        }
    }
}
