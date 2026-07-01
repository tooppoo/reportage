//! Stable, machine-readable diagnostic identity for parser and validator errors.
//!
//! See docs/diagnostics.md for the naming convention, compatibility policy, and
//! the split between `code` (stable), `message` (improvable), `location`, and
//! `details` (auxiliary, weaker stability).

use std::fmt;

/// A stable, machine-readable identifier for a diagnostic.
///
/// The string form (see [`DiagnosticCode::as_str`]) is the external contract
/// that tests and tooling depend on. It is intentionally independent of the
/// Rust error enum variant names that produce it, so internal error types can
/// be restructured without renaming published codes.
///
/// Codes use a dot-separated `<domain>.<reason>` namespace, e.g.
/// `parse.invalid_exit_code`. Renaming or removing an existing code is a
/// breaking change; adding a new code is not.
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
}

impl DiagnosticCode {
    /// The stable external string representation of this code.
    ///
    /// This is the identifier tests and tooling must depend on instead of
    /// `Display` message text or internal enum variant names.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ParseSyntax => "parse.syntax",
            Self::ParseEmptyCase => "parse.empty_case",
            Self::ParseMissingAssertionBlock => "parse.missing_assertion_block",
            Self::ParseEmptyAction => "parse.empty_action",
            Self::ParseInvalidExitCode => "parse.invalid_exit_code",
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
/// `column` is not always available (e.g. some parse-domain validation
/// errors only know the line a construct started on).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticLocation {
    pub line: usize,
    pub column: Option<usize>,
}

/// Auxiliary information attached to a diagnostic.
///
/// Unlike `code`, the contents of `details` do not carry a strong stability
/// guarantee. In particular, pest-derived message text and expected-token
/// summaries are grammar-dependent and must not be treated as a stable API by
/// tests or tooling; depend on `code` instead.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiagnosticDetails {
    /// Raw message text from the pest grammar error, when the diagnostic
    /// originates from a syntax error.
    pub pest_message: Option<String>,
    /// The offending raw value (e.g. an out-of-range exit code literal or a
    /// case name), when relevant to the diagnostic.
    pub raw_value: Option<String>,
}

/// A machine-readable diagnostic produced by parsing or validating a script.
///
/// `code` is the stable identifier. `message` is a human-facing string that
/// may be improved over time without being a breaking change. `location` and
/// `details` provide position and auxiliary context respectively.
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
        ] {
            assert_eq!(code.to_string(), code.as_str());
        }
    }
}
