//! Semantic rule registry — the inventory of semantic rules known to exist in Reportage.
//!
//! This module owns semantic rule identity ([`SemanticRuleId`]) and the coverage obligations attached to each known semantic rule.
//! It is not the runtime implementation of semantic behavior: parser construction, semantic validation, and evaluation remain owned by `parser`, `semantic`, `evaluator`, and the domain model.
//!
//! The registry is a coverage inventory. It answers:
//!
//! - which semantic rules are known to exist;
//! - which category each rule belongs to;
//! - whether a rule must have a semantic spec, conformance cases, and generated docs;
//! - which diagnostics and syntax rules are related to the rule for cross-reference checks.
//!
//! [`SemanticRuleId::as_str`] is the canonical string representation used by spec JSON, generated docs, and coverage checks.
//! [`DiagnosticCode`] remains the canonical identity for diagnostics, and `reportage.pest` remains the canonical syntax definition.
//! The registry links these identities, but it does not make diagnostic codes or pest grammar rules subordinate to semantic rules.
//!
//! `spec/language/semantics/*.json` remains the source of truth for each rule's normative fields and conformance cases; this registry does not duplicate that detail.
//! See docs/adr/20260708T065700Z_semantic-rule-coverage-registry.md for the full rationale, including how cross-references are verified in CI.

use crate::diagnostic::DiagnosticCode;
use std::fmt;

/// The broad classification of a semantic rule. `runner-lifecycle`, `artifact`, and `diagnostic`
/// concerns are owned elsewhere (see the ADR) and are deliberately not represented here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleCategory {
    Assertion,
    LogicalComposition,
    ValueReference,
}

/// Whether a rule's evaluation behavior exists in the runtime today. This is not the coverage
/// check's primary judgment; it is auxiliary context surfaced in failure messages and inventory
/// output. See the `spec_required` / `conformance_required` / `docs_required` fields for the
/// fields the coverage check actually gates on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImplementationStatus {
    Implemented,
    Planned,
    Deferred,
}

/// The identity of a semantic rule known to exist in Reportage.
///
/// This enum is the source of truth for semantic rule identity.
/// Rust code that needs to name a semantic rule must reference a variant, not a string literal; [`SemanticRuleId::as_str`] is the only place the string form is defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SemanticRuleId {
    AssertionExitEquals,
    AssertionStdoutContains,
    AssertionStderrContains,
    AssertionStdoutEmpty,
    AssertionStderrEmpty,
    AssertionStdoutContentsEquals,
    AssertionStderrContentsEquals,
    AssertionFileExists,
    AssertionFileContains,
    AssertionFileContentsEquals,
    AssertionFileTextEquals,
    AssertionDirExists,
    AssertionDirContains,
    LogicalCompositionExpectationNot,
    LogicalCompositionExpectationAll,
    LogicalCompositionExpectationAny,
    ValueReferenceWorkspacePathResolve,
    ValueReferenceFilePathValidate,
    ValueReferenceDirEntryNameValidate,
    ValueReferenceFixtureReferenceResolve,
    ValueReferenceFileContentsReferenceResolve,
    ValueReferenceLiteralKindMismatch,
}

impl SemanticRuleId {
    /// Every variant, in declaration order, for iteration by coverage checks.
    ///
    /// When adding a variant, add it here too; `all_lists_every_variant_once_in_declaration_order` fails on any non-trailing omission, duplicate, or ordering drift.
    pub const ALL: &'static [SemanticRuleId] = &[
        Self::AssertionExitEquals,
        Self::AssertionStdoutContains,
        Self::AssertionStderrContains,
        Self::AssertionStdoutEmpty,
        Self::AssertionStderrEmpty,
        Self::AssertionStdoutContentsEquals,
        Self::AssertionStderrContentsEquals,
        Self::AssertionFileExists,
        Self::AssertionFileContains,
        Self::AssertionFileContentsEquals,
        Self::AssertionFileTextEquals,
        Self::AssertionDirExists,
        Self::AssertionDirContains,
        Self::LogicalCompositionExpectationNot,
        Self::LogicalCompositionExpectationAll,
        Self::LogicalCompositionExpectationAny,
        Self::ValueReferenceWorkspacePathResolve,
        Self::ValueReferenceFilePathValidate,
        Self::ValueReferenceDirEntryNameValidate,
        Self::ValueReferenceFixtureReferenceResolve,
        Self::ValueReferenceFileContentsReferenceResolve,
        Self::ValueReferenceLiteralKindMismatch,
    ];

    /// The canonical string representation of this rule id.
    ///
    /// Spec JSON `id` fields, generated docs section headings, and coverage checks all match against this string.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AssertionExitEquals => "assertion.exit.equals",
            Self::AssertionStdoutContains => "assertion.stdout.contains",
            Self::AssertionStderrContains => "assertion.stderr.contains",
            Self::AssertionStdoutEmpty => "assertion.stdout.empty",
            Self::AssertionStderrEmpty => "assertion.stderr.empty",
            Self::AssertionStdoutContentsEquals => "assertion.stdout.contents_equals",
            Self::AssertionStderrContentsEquals => "assertion.stderr.contents_equals",
            Self::AssertionFileExists => "assertion.file.exists",
            Self::AssertionFileContains => "assertion.file.contains",
            Self::AssertionFileContentsEquals => "assertion.file.contents_equals",
            Self::AssertionFileTextEquals => "assertion.file.text_equals",
            Self::AssertionDirExists => "assertion.dir.exists",
            Self::AssertionDirContains => "assertion.dir.contains",
            Self::LogicalCompositionExpectationNot => "logical-composition.expectation.not",
            Self::LogicalCompositionExpectationAll => "logical-composition.expectation.all",
            Self::LogicalCompositionExpectationAny => "logical-composition.expectation.any",
            Self::ValueReferenceWorkspacePathResolve => "value-reference.workspace-path.resolve",
            Self::ValueReferenceFilePathValidate => "value-reference.file-path.validate",
            Self::ValueReferenceDirEntryNameValidate => "value-reference.dir-entry-name.validate",
            Self::ValueReferenceFixtureReferenceResolve => {
                "value-reference.fixture-reference.resolve"
            }
            Self::ValueReferenceFileContentsReferenceResolve => {
                "value-reference.file-contents-reference.resolve"
            }
            Self::ValueReferenceLiteralKindMismatch => "value-reference.literal.kind-mismatch",
        }
    }
}

impl fmt::Display for SemanticRuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How a diagnostic code relates to the semantic rule whose entry references it.
///
/// The relation kind decides which naming check CI applies to the cross-reference; see `tests/semantic_rule_coverage.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelatedDiagnostic {
    /// A diagnostic code owned by exactly this rule.
    /// Its string form must be `<rule-id>.<reason>` with a non-empty reason, and no other registry entry may reference it.
    RuleOwned(DiagnosticCode),
    /// A diagnostic code shared by two or more semantic rules, e.g. the empty-block diagnostic of `not` / `all` / `any`.
    /// Exempt from the rule-id prefix check; CI instead verifies that at least two registry entries reference it.
    Shared(DiagnosticCode),
    /// A diagnostic code emitted by validating or resolving a value that this rule describes.
    ///
    /// These codes live in the `semantic.*` namespace defined by docs/semantic-diagnostics.md, because the same validation also fires outside the rule's own assertion position (e.g. `semantic.workspace_path.*` from a `write` step's path, `semantic.literal.kind_mismatch` from any argument position).
    /// Renaming them to `<rule-id>.<reason>` would misattribute those emissions to one syntactic position, so they are exempt from the rule-id prefix check; CI instead verifies the `semantic.` namespace.
    SemanticValidation(DiagnosticCode),
}

impl RelatedDiagnostic {
    pub const fn code(self) -> DiagnosticCode {
        match self {
            Self::RuleOwned(code) | Self::Shared(code) | Self::SemanticValidation(code) => code,
        }
    }
}

/// One semantic rule's coverage-requirement record.
///
/// `related_diagnostic_codes` references [`DiagnosticCode`] values directly, so a diagnostic rename or removal that misses the registry fails to compile instead of going stale.
/// `related_syntax_rule` stays a plain pest grammar rule name because grammar rule identity is owned by `reportage.pest`, not by Rust code; its existence is verified against the grammar file in CI.
#[derive(Debug, Clone, Copy)]
pub struct SemanticRuleEntry {
    pub id: SemanticRuleId,
    pub category: RuleCategory,
    pub implementation_status: ImplementationStatus,
    pub spec_required: bool,
    pub conformance_required: bool,
    pub docs_required: bool,
    pub related_syntax_rule: Option<&'static str>,
    pub related_diagnostic_codes: &'static [RelatedDiagnostic],
}

use ImplementationStatus::Implemented;
use RelatedDiagnostic::{RuleOwned, SemanticValidation, Shared};
use RuleCategory::{Assertion, LogicalComposition, ValueReference};

/// The full semantic rule inventory, one entry per [`SemanticRuleId`] variant.
///
/// `#[doc(hidden)]` because this is consumed by integration tests, the docs generator, and CI
/// checks within this workspace, not by external `reportage-core` consumers; it is not part of the
/// crate's stable public API. `assertion.exit.equals` / `assertion.stdout.contains` /
/// `assertion.stderr.contains` had specs before #101. #101 added specs for the `assertion`,
/// `logical-composition`, and `value-reference` rules in its scope (see
/// spec/language/semantics/README.md), so those entries now have `spec_required`,
/// `conformance_required`, and `docs_required` all `true` too. `assertion.stdout.contents_equals`,
/// `assertion.stderr.contents_equals`, `value-reference.file-path.validate`, and
/// `value-reference.dir-entry-name.validate` were explicitly out of #101's scope and remain known
/// rules awaiting a semantic spec.
#[doc(hidden)]
pub const SEMANTIC_RULE_REGISTRY: &[SemanticRuleEntry] = &[
    SemanticRuleEntry {
        id: SemanticRuleId::AssertionExitEquals,
        category: Assertion,
        implementation_status: Implemented,
        spec_required: true,
        conformance_required: true,
        docs_required: true,
        related_syntax_rule: Some("exit_exp"),
        related_diagnostic_codes: &[RuleOwned(DiagnosticCode::AssertionExitMismatch)],
    },
    SemanticRuleEntry {
        id: SemanticRuleId::AssertionStdoutContains,
        category: Assertion,
        implementation_status: Implemented,
        spec_required: true,
        conformance_required: true,
        docs_required: true,
        related_syntax_rule: Some("stdout_exp"),
        related_diagnostic_codes: &[RuleOwned(DiagnosticCode::AssertionStdoutContainsMismatch)],
    },
    SemanticRuleEntry {
        id: SemanticRuleId::AssertionStderrContains,
        category: Assertion,
        implementation_status: Implemented,
        spec_required: true,
        conformance_required: true,
        docs_required: true,
        related_syntax_rule: Some("stderr_exp"),
        related_diagnostic_codes: &[RuleOwned(DiagnosticCode::AssertionStderrContainsMismatch)],
    },
    SemanticRuleEntry {
        id: SemanticRuleId::AssertionStdoutEmpty,
        category: Assertion,
        implementation_status: Implemented,
        spec_required: true,
        conformance_required: true,
        docs_required: true,
        related_syntax_rule: Some("stdout_exp"),
        related_diagnostic_codes: &[RuleOwned(DiagnosticCode::AssertionStdoutNotEmpty)],
    },
    SemanticRuleEntry {
        id: SemanticRuleId::AssertionStderrEmpty,
        category: Assertion,
        implementation_status: Implemented,
        spec_required: true,
        conformance_required: true,
        docs_required: true,
        related_syntax_rule: Some("stderr_exp"),
        related_diagnostic_codes: &[RuleOwned(DiagnosticCode::AssertionStderrNotEmpty)],
    },
    SemanticRuleEntry {
        id: SemanticRuleId::AssertionStdoutContentsEquals,
        category: Assertion,
        implementation_status: Implemented,
        spec_required: false,
        conformance_required: false,
        docs_required: false,
        related_syntax_rule: Some("output_contents_equals"),
        related_diagnostic_codes: &[RuleOwned(
            DiagnosticCode::AssertionStdoutContentsEqualsMismatch,
        )],
    },
    SemanticRuleEntry {
        id: SemanticRuleId::AssertionStderrContentsEquals,
        category: Assertion,
        implementation_status: Implemented,
        spec_required: false,
        conformance_required: false,
        docs_required: false,
        related_syntax_rule: Some("output_contents_equals"),
        related_diagnostic_codes: &[RuleOwned(
            DiagnosticCode::AssertionStderrContentsEqualsMismatch,
        )],
    },
    SemanticRuleEntry {
        id: SemanticRuleId::AssertionFileExists,
        category: Assertion,
        implementation_status: Implemented,
        spec_required: true,
        conformance_required: true,
        docs_required: true,
        related_syntax_rule: Some("file_exp"),
        related_diagnostic_codes: &[
            RuleOwned(DiagnosticCode::AssertionFileExistsMissing),
            RuleOwned(DiagnosticCode::AssertionFileExistsNotAFile),
        ],
    },
    SemanticRuleEntry {
        id: SemanticRuleId::AssertionFileContains,
        category: Assertion,
        implementation_status: Implemented,
        spec_required: true,
        conformance_required: true,
        docs_required: true,
        related_syntax_rule: Some("file_exp"),
        related_diagnostic_codes: &[
            RuleOwned(DiagnosticCode::AssertionFileContainsPreconditionUnmet),
            RuleOwned(DiagnosticCode::AssertionFileContainsMismatch),
        ],
    },
    SemanticRuleEntry {
        id: SemanticRuleId::AssertionFileContentsEquals,
        category: Assertion,
        implementation_status: Implemented,
        spec_required: true,
        conformance_required: true,
        docs_required: true,
        related_syntax_rule: Some("file_exp"),
        related_diagnostic_codes: &[
            RuleOwned(DiagnosticCode::AssertionFileContentsEqualsMismatch),
            RuleOwned(DiagnosticCode::AssertionFileContentsEqualsActualMissing),
            RuleOwned(DiagnosticCode::AssertionFileContentsEqualsActualNotARegularFile),
            RuleOwned(DiagnosticCode::AssertionFileContentsEqualsActualUnreadable),
        ],
    },
    SemanticRuleEntry {
        id: SemanticRuleId::AssertionFileTextEquals,
        category: Assertion,
        implementation_status: Implemented,
        spec_required: true,
        conformance_required: true,
        docs_required: true,
        related_syntax_rule: Some("file_exp"),
        related_diagnostic_codes: &[
            RuleOwned(DiagnosticCode::AssertionFileTextEqualsMismatch),
            RuleOwned(DiagnosticCode::AssertionFileTextEqualsActualMissing),
            RuleOwned(DiagnosticCode::AssertionFileTextEqualsActualNotARegularFile),
            RuleOwned(DiagnosticCode::AssertionFileTextEqualsActualUnreadable),
        ],
    },
    SemanticRuleEntry {
        id: SemanticRuleId::AssertionDirExists,
        category: Assertion,
        implementation_status: Implemented,
        spec_required: true,
        conformance_required: true,
        docs_required: true,
        related_syntax_rule: Some("dir_exp"),
        related_diagnostic_codes: &[
            RuleOwned(DiagnosticCode::AssertionDirExistsMissing),
            RuleOwned(DiagnosticCode::AssertionDirExistsNotADirectory),
        ],
    },
    SemanticRuleEntry {
        id: SemanticRuleId::AssertionDirContains,
        category: Assertion,
        implementation_status: Implemented,
        spec_required: true,
        conformance_required: true,
        docs_required: true,
        related_syntax_rule: Some("dir_exp"),
        related_diagnostic_codes: &[
            RuleOwned(DiagnosticCode::AssertionDirContainsSubjectMissing),
            RuleOwned(DiagnosticCode::AssertionDirContainsSubjectNotADirectory),
            RuleOwned(DiagnosticCode::AssertionDirContainsEntryMissing),
            RuleOwned(DiagnosticCode::AssertionDirContainsSubjectUnreadable),
        ],
    },
    SemanticRuleEntry {
        id: SemanticRuleId::LogicalCompositionExpectationNot,
        category: LogicalComposition,
        implementation_status: Implemented,
        spec_required: true,
        conformance_required: true,
        docs_required: true,
        related_syntax_rule: Some("not_block"),
        related_diagnostic_codes: &[Shared(DiagnosticCode::SemanticExpectationEmptyBlock)],
    },
    SemanticRuleEntry {
        id: SemanticRuleId::LogicalCompositionExpectationAll,
        category: LogicalComposition,
        implementation_status: Implemented,
        spec_required: true,
        conformance_required: true,
        docs_required: true,
        related_syntax_rule: Some("all_block"),
        related_diagnostic_codes: &[Shared(DiagnosticCode::SemanticExpectationEmptyBlock)],
    },
    SemanticRuleEntry {
        id: SemanticRuleId::LogicalCompositionExpectationAny,
        category: LogicalComposition,
        implementation_status: Implemented,
        spec_required: true,
        conformance_required: true,
        docs_required: true,
        related_syntax_rule: Some("any_block"),
        related_diagnostic_codes: &[Shared(DiagnosticCode::SemanticExpectationEmptyBlock)],
    },
    SemanticRuleEntry {
        // `dir <"path">`'s subject path reuses this exact rule (via `WorkspacePath::parse` and
        // the same three diagnostic codes) rather than defining a separate dir-path rule; see
        // `semantic::validate_dir_path`.
        id: SemanticRuleId::ValueReferenceWorkspacePathResolve,
        category: ValueReference,
        implementation_status: Implemented,
        spec_required: true,
        conformance_required: true,
        docs_required: true,
        related_syntax_rule: Some("workspace_path_literal"),
        related_diagnostic_codes: &[
            SemanticValidation(DiagnosticCode::SemanticWorkspacePathEmpty),
            SemanticValidation(DiagnosticCode::SemanticWorkspacePathAbsolute),
            SemanticValidation(DiagnosticCode::SemanticWorkspacePathDotSegment),
        ],
    },
    SemanticRuleEntry {
        // Distinct from `value-reference.workspace-path.resolve`: `file <"path">`'s subject
        // path is validated by `semantic::validate_file_path`, a separate, narrower check (no
        // leading `/`, no `.`/`..` segments) with its own diagnostic codes, not by
        // `WorkspacePath::parse`. See docs/adr/20260704T112155Z_subject-first-file-assertion-syntax.md.
        id: SemanticRuleId::ValueReferenceFilePathValidate,
        category: ValueReference,
        implementation_status: Implemented,
        spec_required: false,
        conformance_required: false,
        docs_required: false,
        related_syntax_rule: Some("file_exp"),
        related_diagnostic_codes: &[
            SemanticValidation(DiagnosticCode::SemanticFilePathAbsolute),
            SemanticValidation(DiagnosticCode::SemanticFilePathDotSegment),
        ],
    },
    SemanticRuleEntry {
        id: SemanticRuleId::ValueReferenceDirEntryNameValidate,
        category: ValueReference,
        implementation_status: Implemented,
        spec_required: false,
        conformance_required: false,
        docs_required: false,
        related_syntax_rule: Some("dir_contains"),
        related_diagnostic_codes: &[
            SemanticValidation(DiagnosticCode::SemanticDirEntryNameEmpty),
            SemanticValidation(DiagnosticCode::SemanticDirEntryNamePathSeparator),
            SemanticValidation(DiagnosticCode::SemanticDirEntryNameDotEntry),
            SemanticValidation(DiagnosticCode::SemanticDirEntryNameControlChar),
        ],
    },
    SemanticRuleEntry {
        id: SemanticRuleId::ValueReferenceFixtureReferenceResolve,
        category: ValueReference,
        implementation_status: Implemented,
        spec_required: true,
        conformance_required: true,
        docs_required: true,
        related_syntax_rule: Some("fixture_reference_literal"),
        related_diagnostic_codes: &[
            SemanticValidation(DiagnosticCode::SemanticFixtureReferenceEmpty),
            SemanticValidation(DiagnosticCode::SemanticFixtureReferenceAbsolute),
            SemanticValidation(DiagnosticCode::SemanticFixtureReferenceDotSegment),
            SemanticValidation(DiagnosticCode::SemanticFixtureReferenceMissing),
            SemanticValidation(DiagnosticCode::SemanticFixtureReferenceNotARegularFile),
            SemanticValidation(DiagnosticCode::SemanticFixtureReferenceEscapesReporDirectory),
        ],
    },
    SemanticRuleEntry {
        id: SemanticRuleId::ValueReferenceFileContentsReferenceResolve,
        category: ValueReference,
        implementation_status: Implemented,
        spec_required: true,
        conformance_required: true,
        docs_required: true,
        related_syntax_rule: None,
        related_diagnostic_codes: &[
            SemanticValidation(DiagnosticCode::SemanticFileContentsReferenceMissing),
            SemanticValidation(DiagnosticCode::SemanticFileContentsReferenceNotARegularFile),
            SemanticValidation(DiagnosticCode::SemanticFileContentsReferenceReadError),
        ],
    },
    SemanticRuleEntry {
        id: SemanticRuleId::ValueReferenceLiteralKindMismatch,
        category: ValueReference,
        implementation_status: Implemented,
        spec_required: true,
        conformance_required: true,
        docs_required: true,
        related_syntax_rule: Some("value_literal"),
        related_diagnostic_codes: &[SemanticValidation(
            DiagnosticCode::SemanticLiteralKindMismatch,
        )],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_matches_as_str() {
        for id in SemanticRuleId::ALL {
            assert_eq!(id.to_string(), id.as_str());
        }
    }

    /// `as usize` yields the declaration index of a fieldless variant, so requiring `ALL[i] as usize == i` proves `ALL` matches declaration order with no duplicate and no gap.
    /// A variant appended after the last `ALL` element is the one omission this cannot see; the doc comment on `ALL` covers that case.
    #[test]
    fn all_lists_every_variant_once_in_declaration_order() {
        for (index, id) in SemanticRuleId::ALL.iter().enumerate() {
            assert_eq!(
                *id as usize, index,
                "SemanticRuleId::ALL out of sync at index {index} ({id})"
            );
        }
    }
}
