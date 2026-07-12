//! Stable, machine-readable diagnostic identity for parser and validator errors.
//!
//! See docs2/reference/diagnostics.md for the naming convention, compatibility policy, and the split between `code` (stable), `message` (improvable), `location`, and `details` (auxiliary, weaker stability).

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
    /// `file <"path"> exists` observed a missing path.
    AssertionFileExistsMissing,
    /// `file <"path"> exists` observed a path that is not a regular file (e.g. a directory).
    AssertionFileExistsNotAFile,
    /// `file <"path"> contains "<text>"` observed a path that is not a readable UTF-8 regular file (missing, a directory, unreadable, or non-UTF-8 content).
    AssertionFileContainsPreconditionUnmet,
    /// `file <"path"> contains "<text>"` observed a readable UTF-8 regular file that does not contain the expected substring.
    AssertionFileContainsMismatch,
    /// `file <"path"> contents_equals <expected>` observed actual bytes that did not
    /// byte-for-byte match the expected bytes.
    AssertionFileContentsEqualsMismatch,
    /// `file <"path"> contents_equals <expected>` observed a missing actual path.
    AssertionFileContentsEqualsActualMissing,
    /// `file <"path"> contents_equals <expected>` observed an actual path that is not a
    /// regular file (e.g. a directory).
    AssertionFileContentsEqualsActualNotARegularFile,
    /// `file <"path"> contents_equals <expected>` observed an actual regular file that
    /// could not be read.
    AssertionFileContentsEqualsActualUnreadable,
    /// `stdout contents_equals <expected>` observed captured stdout that did not
    /// byte-for-byte match the expected bytes.
    AssertionStdoutContentsEqualsMismatch,
    /// `stderr contents_equals <expected>` observed captured stderr that did not
    /// byte-for-byte match the expected bytes.
    AssertionStderrContentsEqualsMismatch,
    /// `file <"path"> text_equals <text_literal>` observed actual bytes that did not
    /// byte-for-byte match the expected `TextValue`'s UTF-8 bytes.
    AssertionFileTextEqualsMismatch,
    /// `file <"path"> text_equals <text_literal>` observed a missing actual path.
    AssertionFileTextEqualsActualMissing,
    /// `file <"path"> text_equals <text_literal>` observed an actual path that is not a
    /// regular file (e.g. a directory).
    AssertionFileTextEqualsActualNotARegularFile,
    /// `file <"path"> text_equals <text_literal>` observed an actual regular file that
    /// could not be read.
    AssertionFileTextEqualsActualUnreadable,
    /// `stdout text_equals <text_literal>` observed captured stdout that did not
    /// byte-for-byte match the expected `TextValue`'s UTF-8 bytes.
    AssertionStdoutTextEqualsMismatch,
    /// `stderr text_equals <text_literal>` observed captured stderr that did not
    /// byte-for-byte match the expected `TextValue`'s UTF-8 bytes.
    AssertionStderrTextEqualsMismatch,
    /// A `dir <"path"> contains "<name>"` entry name was empty.
    SemanticDirEntryNameEmpty,
    /// A `dir <"path"> contains "<name>"` entry name contained a path separator (`/`).
    SemanticDirEntryNamePathSeparator,
    /// A `dir <"path"> contains "<name>"` entry name was `.` or `..`.
    SemanticDirEntryNameDotEntry,
    /// A `dir <"path"> contains "<name>"` entry name contained a control character.
    SemanticDirEntryNameControlChar,
    /// `dir <"path"> exists` observed a missing path.
    AssertionDirExistsMissing,
    /// `dir <"path"> exists` observed a path that is not a directory (e.g. a regular file).
    AssertionDirExistsNotADirectory,
    /// `dir <"path"> contains "<name>"` observed a subject path that does not exist.
    AssertionDirContainsSubjectMissing,
    /// `dir <"path"> contains "<name>"` observed a subject path that is not a directory.
    AssertionDirContainsSubjectNotADirectory,
    /// `dir <"path"> contains "<name>"` observed a directory that does not contain an entry named `<name>`.
    AssertionDirContainsEntryMissing,
    /// `dir <"path"> contains "<name>"` observed a directory whose entries could not be read (e.g. a permission error).
    AssertionDirContainsSubjectUnreadable,
    /// A `not` / `all` / `any` logical composition block contains zero expectation expressions.
    /// See docs2/reference/semantic-diagnostics.md.
    SemanticExpectationEmptyBlock,
    /// An assertion block evaluates a process expectation (`exit`, `stdout`, `stderr`) before
    /// any `$` action has run, so there is no last action result to compare against.
    SemanticExpectationRequiresAction,
    /// `exit <code>` observed an actual exit code that did not match the expected code.
    AssertionExitMismatch,
    /// `stdout contains "<text>"` observed captured stdout that did not contain the expected substring.
    AssertionStdoutContainsMismatch,
    /// `stderr contains "<text>"` observed captured stderr that did not contain the expected substring.
    AssertionStderrContainsMismatch,
    /// `stdout is empty` observed non-empty captured stdout.
    AssertionStdoutNotEmpty,
    /// `stderr is empty` observed non-empty captured stderr.
    AssertionStderrNotEmpty,
    /// A `write` step's workspace path was empty.
    SemanticWorkspacePathEmpty,
    /// A `write` step's workspace path was absolute.
    SemanticWorkspacePathAbsolute,
    /// A `write` step's workspace path contained a `.` or `..` segment.
    SemanticWorkspacePathDotSegment,
    /// A literal of one kind (string literal / workspace path literal /
    /// fixture reference literal) appeared in an argument position whose
    /// signature requires a different kind, e.g. `file "out.txt" exists`
    /// where the `file` subject requires a workspace path literal.
    /// See docs2/reference/semantic-diagnostics.md.
    SemanticLiteralKindMismatch,
    /// A heredoc literal (in a `write` step or a `file ... contains`
    /// expectation) contains a non-blank body line that is indented less
    /// than the closing fence.
    ParseHeredocLiteralShallowIndent,
    /// An `@"<path>"` fixture reference literal's path was empty.
    SemanticFixtureReferenceEmpty,
    /// An `@"<path>"` fixture reference literal's path was absolute.
    SemanticFixtureReferenceAbsolute,
    /// An `@"<path>"` fixture reference literal's path contained a `.` or `..` segment.
    SemanticFixtureReferenceDotSegment,
    /// A fixture reference's resolved source file does not exist.
    SemanticFixtureReferenceMissing,
    /// A fixture reference's resolved source exists but is not a regular file (e.g. a directory).
    SemanticFixtureReferenceNotARegularFile,
    /// A fixture reference's resolved source lies outside the referencing
    /// `*.repor` file's directory once canonicalized (e.g. escaped via a symlink),
    /// even though the raw path contained no `.` / `..` segment.
    SemanticFixtureReferenceEscapesReporDirectory,
    /// A `contents_equals` expected `WorkspacePath` does not exist. Unlike a missing
    /// `file` subject (an assertion failure), this is a test-definition error: the
    /// expected value itself could not be sourced.
    SemanticFileContentsReferenceMissing,
    /// A `contents_equals` expected `WorkspacePath` exists but is not a regular file
    /// (e.g. a directory). A test-definition error, not an assertion failure.
    SemanticFileContentsReferenceNotARegularFile,
    /// A `contents_equals` expected `WorkspacePath` is a regular file but could not
    /// be read. A test-definition error, not an assertion failure.
    SemanticFileContentsReferenceReadError,
    /// A `write` step's target path already existed; create-only writes reject this.
    StepWriteTargetExists,
    /// A `write` step's target path had a regular file somewhere in its parent path.
    StepWriteParentNotADirectory,
    /// A `write` step failed due to an OS-level I/O error.
    StepWriteIoError,
    /// A document block contains no documentation field.
    ParseDocumentBlockEmpty,
    /// A document block declares the same documentation field more than once.
    ParseDocumentBlockDuplicateField,
    /// A document block's `order` field value overflows the supported non-negative integer range.
    ParseDocumentBlockInvalidOrder,
    /// A source contains more than one `document file` block.
    ParseDocumentFileDuplicate,
    /// A `document file` block appears after the source's first case block.
    ParseDocumentFileAfterCase,
}

impl DiagnosticCode {
    /// Every variant, in declaration order, for crate-internal verification.
    ///
    /// `#[non_exhaustive]` keeps downstream `match` expressions open, but uniqueness and cross-reference checks (`diagnostic.rs` unit tests, `tests/semantic_rule_coverage.rs`) need a full enumeration to iterate.
    /// When adding a variant, add it here too; `all_lists_every_variant_once_in_declaration_order` fails on any non-trailing omission, duplicate, or ordering drift.
    pub const ALL: &'static [DiagnosticCode] = &[
        Self::ParseSyntax,
        Self::ParseEmptyCase,
        Self::ParseMissingAssertionBlock,
        Self::ParseEmptyAction,
        Self::ParseInvalidExitCode,
        Self::SemanticFilePathAbsolute,
        Self::SemanticFilePathDotSegment,
        Self::AssertionFileExistsMissing,
        Self::AssertionFileExistsNotAFile,
        Self::AssertionFileContainsPreconditionUnmet,
        Self::AssertionFileContainsMismatch,
        Self::AssertionFileContentsEqualsMismatch,
        Self::AssertionFileContentsEqualsActualMissing,
        Self::AssertionFileContentsEqualsActualNotARegularFile,
        Self::AssertionFileContentsEqualsActualUnreadable,
        Self::AssertionStdoutContentsEqualsMismatch,
        Self::AssertionStderrContentsEqualsMismatch,
        Self::AssertionFileTextEqualsMismatch,
        Self::AssertionFileTextEqualsActualMissing,
        Self::AssertionFileTextEqualsActualNotARegularFile,
        Self::AssertionFileTextEqualsActualUnreadable,
        Self::AssertionStdoutTextEqualsMismatch,
        Self::AssertionStderrTextEqualsMismatch,
        Self::SemanticDirEntryNameEmpty,
        Self::SemanticDirEntryNamePathSeparator,
        Self::SemanticDirEntryNameDotEntry,
        Self::SemanticDirEntryNameControlChar,
        Self::AssertionDirExistsMissing,
        Self::AssertionDirExistsNotADirectory,
        Self::AssertionDirContainsSubjectMissing,
        Self::AssertionDirContainsSubjectNotADirectory,
        Self::AssertionDirContainsEntryMissing,
        Self::AssertionDirContainsSubjectUnreadable,
        Self::SemanticExpectationEmptyBlock,
        Self::SemanticExpectationRequiresAction,
        Self::AssertionExitMismatch,
        Self::AssertionStdoutContainsMismatch,
        Self::AssertionStderrContainsMismatch,
        Self::AssertionStdoutNotEmpty,
        Self::AssertionStderrNotEmpty,
        Self::SemanticWorkspacePathEmpty,
        Self::SemanticWorkspacePathAbsolute,
        Self::SemanticWorkspacePathDotSegment,
        Self::SemanticLiteralKindMismatch,
        Self::ParseHeredocLiteralShallowIndent,
        Self::SemanticFixtureReferenceEmpty,
        Self::SemanticFixtureReferenceAbsolute,
        Self::SemanticFixtureReferenceDotSegment,
        Self::SemanticFixtureReferenceMissing,
        Self::SemanticFixtureReferenceNotARegularFile,
        Self::SemanticFixtureReferenceEscapesReporDirectory,
        Self::SemanticFileContentsReferenceMissing,
        Self::SemanticFileContentsReferenceNotARegularFile,
        Self::SemanticFileContentsReferenceReadError,
        Self::StepWriteTargetExists,
        Self::StepWriteParentNotADirectory,
        Self::StepWriteIoError,
        Self::ParseDocumentBlockEmpty,
        Self::ParseDocumentBlockDuplicateField,
        Self::ParseDocumentBlockInvalidOrder,
        Self::ParseDocumentFileDuplicate,
        Self::ParseDocumentFileAfterCase,
    ];

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
            Self::AssertionFileExistsMissing => "assertion.file.exists.missing",
            Self::AssertionFileExistsNotAFile => "assertion.file.exists.not_regular_file",
            Self::AssertionFileContainsPreconditionUnmet => {
                "assertion.file.contains.precondition_unmet"
            }
            Self::AssertionFileContainsMismatch => "assertion.file.contains.mismatch",
            Self::AssertionFileContentsEqualsMismatch => "assertion.file.contents_equals.mismatch",
            Self::AssertionFileContentsEqualsActualMissing => {
                "assertion.file.contents_equals.actual_missing"
            }
            Self::AssertionFileContentsEqualsActualNotARegularFile => {
                "assertion.file.contents_equals.actual_not_regular_file"
            }
            Self::AssertionFileContentsEqualsActualUnreadable => {
                "assertion.file.contents_equals.actual_unreadable"
            }
            Self::AssertionStdoutContentsEqualsMismatch => {
                "assertion.stdout.contents_equals.mismatch"
            }
            Self::AssertionStderrContentsEqualsMismatch => {
                "assertion.stderr.contents_equals.mismatch"
            }
            Self::AssertionFileTextEqualsMismatch => "assertion.file.text_equals.mismatch",
            Self::AssertionFileTextEqualsActualMissing => {
                "assertion.file.text_equals.actual_missing"
            }
            Self::AssertionFileTextEqualsActualNotARegularFile => {
                "assertion.file.text_equals.actual_not_regular_file"
            }
            Self::AssertionFileTextEqualsActualUnreadable => {
                "assertion.file.text_equals.actual_unreadable"
            }
            Self::AssertionStdoutTextEqualsMismatch => "assertion.stdout.text_equals.mismatch",
            Self::AssertionStderrTextEqualsMismatch => "assertion.stderr.text_equals.mismatch",
            Self::SemanticDirEntryNameEmpty => "semantic.dir_entry_name.empty",
            Self::SemanticDirEntryNamePathSeparator => "semantic.dir_entry_name.path_separator",
            Self::SemanticDirEntryNameDotEntry => "semantic.dir_entry_name.dot_entry",
            Self::SemanticDirEntryNameControlChar => "semantic.dir_entry_name.control_char",
            Self::AssertionDirExistsMissing => "assertion.dir.exists.missing",
            Self::AssertionDirExistsNotADirectory => "assertion.dir.exists.not_directory",
            Self::AssertionDirContainsSubjectMissing => "assertion.dir.contains.subject_missing",
            Self::AssertionDirContainsSubjectNotADirectory => {
                "assertion.dir.contains.subject_not_directory"
            }
            Self::AssertionDirContainsEntryMissing => "assertion.dir.contains.entry_missing",
            Self::AssertionDirContainsSubjectUnreadable => {
                "assertion.dir.contains.subject_unreadable"
            }
            Self::SemanticExpectationEmptyBlock => "semantic.expectation.empty_block",
            Self::SemanticExpectationRequiresAction => "semantic.expectation.requires_action",
            Self::AssertionExitMismatch => "assertion.exit.equals.mismatch",
            Self::AssertionStdoutContainsMismatch => "assertion.stdout.contains.mismatch",
            Self::AssertionStderrContainsMismatch => "assertion.stderr.contains.mismatch",
            Self::AssertionStdoutNotEmpty => "assertion.stdout.empty.not_empty",
            Self::AssertionStderrNotEmpty => "assertion.stderr.empty.not_empty",
            Self::SemanticWorkspacePathEmpty => "semantic.workspace_path.empty",
            Self::SemanticWorkspacePathAbsolute => "semantic.workspace_path.absolute",
            Self::SemanticWorkspacePathDotSegment => "semantic.workspace_path.dot_segment",
            Self::SemanticLiteralKindMismatch => "semantic.literal.kind_mismatch",
            Self::ParseHeredocLiteralShallowIndent => "parse.heredoc_literal.shallow_indent",
            Self::SemanticFixtureReferenceEmpty => "semantic.fixture_reference.empty",
            Self::SemanticFixtureReferenceAbsolute => "semantic.fixture_reference.absolute",
            Self::SemanticFixtureReferenceDotSegment => "semantic.fixture_reference.dot_segment",
            Self::SemanticFixtureReferenceMissing => "semantic.fixture_reference.missing",
            Self::SemanticFixtureReferenceNotARegularFile => {
                "semantic.fixture_reference.not_a_regular_file"
            }
            Self::SemanticFixtureReferenceEscapesReporDirectory => {
                "semantic.fixture_reference.escapes_repor_directory"
            }
            Self::SemanticFileContentsReferenceMissing => {
                "semantic.file_contents_reference.missing"
            }
            Self::SemanticFileContentsReferenceNotARegularFile => {
                "semantic.file_contents_reference.not_regular_file"
            }
            Self::SemanticFileContentsReferenceReadError => {
                "semantic.file_contents_reference.read_error"
            }
            Self::StepWriteTargetExists => "step.write.target_exists",
            Self::StepWriteParentNotADirectory => "step.write.parent_not_a_directory",
            Self::StepWriteIoError => "step.write.io_error",
            Self::ParseDocumentBlockEmpty => "parse.document_block.empty",
            Self::ParseDocumentBlockDuplicateField => "parse.document_block.duplicate_field",
            Self::ParseDocumentBlockInvalidOrder => "parse.document_block.invalid_order",
            Self::ParseDocumentFileDuplicate => "parse.document_file.duplicate",
            Self::ParseDocumentFileAfterCase => "parse.document_file.after_case",
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
    /// The literal kind the position's signature requires (e.g. `WorkspacePath`), for literal kind mismatch diagnostics.
    pub expected_kind: Option<String>,
    /// The literal kind the script actually wrote (e.g. `StringLiteral`), for literal kind mismatch diagnostics.
    pub actual_kind: Option<String>,
    /// The suggested replacement (e.g. `<"out.txt">`, or a description such as "a string literal or heredoc literal"), for literal kind mismatch diagnostics.
    pub suggestion: Option<String>,
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
        for code in DiagnosticCode::ALL {
            assert_eq!(code.to_string(), code.as_str());
        }
    }

    /// `as usize` yields the declaration index of a fieldless variant, so requiring `ALL[i] as usize == i` proves `ALL` matches declaration order with no duplicate and no gap.
    /// A variant appended after the last `ALL` element is the one omission this cannot see; the doc comment on `ALL` covers that case.
    #[test]
    fn all_lists_every_variant_once_in_declaration_order() {
        for (index, code) in DiagnosticCode::ALL.iter().enumerate() {
            assert_eq!(
                *code as usize, index,
                "DiagnosticCode::ALL out of sync at index {index} ({code})"
            );
        }
    }

    #[test]
    fn as_str_values_are_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for code in DiagnosticCode::ALL {
            assert!(
                seen.insert(code.as_str()),
                "duplicate DiagnosticCode string '{code}'"
            );
        }
    }
}
