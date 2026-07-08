//! Semantic rule coverage registry — the inventory of semantic rules known to exist in Reportage.
//!
//! This registry is a spec coverage inventory, not runtime implementation. It does not implement
//! evaluation behavior; the parser (`parser.rs`), evaluator (`evaluator.rs`), and model (`model.rs`)
//! own that. This registry only enumerates which semantic rules exist, which category each belongs
//! to, and whether each one is required to have a semantic spec, conformance cases, and generated
//! docs. `spec/language/semantics/*.json` remains the source of truth for a rule's normative fields
//! and conformance cases; this registry does not duplicate that detail. See
//! docs/adr/20260708T065700Z_semantic-rule-coverage-registry.md for the full rationale, including
//! why this is not derived directly from `Expectation`/AST/parser shapes and why it is not (yet) a
//! JSON registry.

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

/// One semantic rule's coverage-requirement record.
///
/// `related_syntax_rule` and `related_diagnostic_codes` are optional cross-references — pest
/// grammar rule names and `DiagnosticCode::as_str()` strings, respectively — kept as plain
/// `&'static str` rather than typed handles so this module never depends on `parser`, `model`, or
/// `diagnostic`. A dependency in that direction would make the registry couple to implementation
/// shape, which is exactly what it must not do (see the ADR).
#[derive(Debug, Clone, Copy)]
pub struct SemanticRuleEntry {
    pub id: &'static str,
    pub category: RuleCategory,
    pub implementation_status: ImplementationStatus,
    pub spec_required: bool,
    pub conformance_required: bool,
    pub docs_required: bool,
    pub related_syntax_rule: Option<&'static str>,
    pub related_diagnostic_codes: &'static [&'static str],
}

use ImplementationStatus::Implemented;
use RuleCategory::{Assertion, LogicalComposition, ValueReference};

/// The full semantic rule inventory.
///
/// `#[doc(hidden)]` because this is consumed by integration tests, the docs generator, and CI
/// checks within this workspace, not by external `reportage-core` consumers; it is not part of the
/// crate's stable public API. Only `assertion.exit.equals` / `assertion.stdout.contains` /
/// `assertion.stderr.contains` have specs today, so they are the only entries with `spec_required`,
/// `conformance_required`, and `docs_required` all `true`. Every other entry here is a known rule
/// awaiting a semantic spec (#101); flipping its required flags to `true` is #101's job, not this
/// registry's.
#[doc(hidden)]
pub const SEMANTIC_RULE_REGISTRY: &[SemanticRuleEntry] = &[
    SemanticRuleEntry {
        id: "assertion.exit.equals",
        category: Assertion,
        implementation_status: Implemented,
        spec_required: true,
        conformance_required: true,
        docs_required: true,
        related_syntax_rule: Some("exit_exp"),
        related_diagnostic_codes: &["assertion.exit.mismatch"],
    },
    SemanticRuleEntry {
        id: "assertion.stdout.contains",
        category: Assertion,
        implementation_status: Implemented,
        spec_required: true,
        conformance_required: true,
        docs_required: true,
        related_syntax_rule: Some("stdout_exp"),
        related_diagnostic_codes: &["assertion.stdout.contains_mismatch"],
    },
    SemanticRuleEntry {
        id: "assertion.stderr.contains",
        category: Assertion,
        implementation_status: Implemented,
        spec_required: true,
        conformance_required: true,
        docs_required: true,
        related_syntax_rule: Some("stderr_exp"),
        related_diagnostic_codes: &["assertion.stderr.contains_mismatch"],
    },
    SemanticRuleEntry {
        id: "assertion.stdout.empty",
        category: Assertion,
        implementation_status: Implemented,
        spec_required: false,
        conformance_required: false,
        docs_required: false,
        related_syntax_rule: Some("stdout_exp"),
        related_diagnostic_codes: &["assertion.stdout.not_empty"],
    },
    SemanticRuleEntry {
        id: "assertion.stderr.empty",
        category: Assertion,
        implementation_status: Implemented,
        spec_required: false,
        conformance_required: false,
        docs_required: false,
        related_syntax_rule: Some("stderr_exp"),
        related_diagnostic_codes: &["assertion.stderr.not_empty"],
    },
    SemanticRuleEntry {
        id: "assertion.stdout.contents_equals",
        category: Assertion,
        implementation_status: Implemented,
        spec_required: false,
        conformance_required: false,
        docs_required: false,
        related_syntax_rule: Some("output_contents_equals"),
        related_diagnostic_codes: &["assertion.stdout.contents_equals_mismatch"],
    },
    SemanticRuleEntry {
        id: "assertion.stderr.contents_equals",
        category: Assertion,
        implementation_status: Implemented,
        spec_required: false,
        conformance_required: false,
        docs_required: false,
        related_syntax_rule: Some("output_contents_equals"),
        related_diagnostic_codes: &["assertion.stderr.contents_equals_mismatch"],
    },
    SemanticRuleEntry {
        id: "assertion.file.exists",
        category: Assertion,
        implementation_status: Implemented,
        spec_required: false,
        conformance_required: false,
        docs_required: false,
        related_syntax_rule: Some("file_exp"),
        related_diagnostic_codes: &[
            "assertion.file.exists_missing",
            "assertion.file.exists_not_a_file",
        ],
    },
    SemanticRuleEntry {
        id: "assertion.file.contains",
        category: Assertion,
        implementation_status: Implemented,
        spec_required: false,
        conformance_required: false,
        docs_required: false,
        related_syntax_rule: Some("file_exp"),
        related_diagnostic_codes: &[
            "assertion.file.contains_precondition_unmet",
            "assertion.file.contains_mismatch",
        ],
    },
    SemanticRuleEntry {
        id: "assertion.file.contents_equals",
        category: Assertion,
        implementation_status: Implemented,
        spec_required: false,
        conformance_required: false,
        docs_required: false,
        related_syntax_rule: Some("file_exp"),
        related_diagnostic_codes: &[
            "assertion.file.contents_equals_mismatch",
            "assertion.file.contents_equals_actual_missing",
            "assertion.file.contents_equals_actual_not_a_regular_file",
            "assertion.file.contents_equals_actual_unreadable",
        ],
    },
    SemanticRuleEntry {
        id: "assertion.file.text_equals",
        category: Assertion,
        implementation_status: Implemented,
        spec_required: false,
        conformance_required: false,
        docs_required: false,
        related_syntax_rule: Some("file_exp"),
        related_diagnostic_codes: &[
            "assertion.file.text_equals_mismatch",
            "assertion.file.text_equals_actual_missing",
            "assertion.file.text_equals_actual_not_a_regular_file",
            "assertion.file.text_equals_actual_unreadable",
        ],
    },
    SemanticRuleEntry {
        id: "assertion.dir.exists",
        category: Assertion,
        implementation_status: Implemented,
        spec_required: false,
        conformance_required: false,
        docs_required: false,
        related_syntax_rule: Some("dir_exp"),
        related_diagnostic_codes: &[
            "assertion.dir.exists_missing",
            "assertion.dir.exists_not_directory",
        ],
    },
    SemanticRuleEntry {
        id: "assertion.dir.contains",
        category: Assertion,
        implementation_status: Implemented,
        spec_required: false,
        conformance_required: false,
        docs_required: false,
        related_syntax_rule: Some("dir_exp"),
        related_diagnostic_codes: &[
            "assertion.dir.contains_subject_missing",
            "assertion.dir.contains_subject_not_directory",
            "assertion.dir.contains_entry_missing",
            "assertion.dir.contains_subject_unreadable",
        ],
    },
    SemanticRuleEntry {
        id: "logical-composition.expectation.not",
        category: LogicalComposition,
        implementation_status: Implemented,
        spec_required: false,
        conformance_required: false,
        docs_required: false,
        related_syntax_rule: Some("not_block"),
        related_diagnostic_codes: &["semantic.expectation.empty_block"],
    },
    SemanticRuleEntry {
        id: "logical-composition.expectation.all",
        category: LogicalComposition,
        implementation_status: Implemented,
        spec_required: false,
        conformance_required: false,
        docs_required: false,
        related_syntax_rule: Some("all_block"),
        related_diagnostic_codes: &["semantic.expectation.empty_block"],
    },
    SemanticRuleEntry {
        id: "logical-composition.expectation.any",
        category: LogicalComposition,
        implementation_status: Implemented,
        spec_required: false,
        conformance_required: false,
        docs_required: false,
        related_syntax_rule: Some("any_block"),
        related_diagnostic_codes: &["semantic.expectation.empty_block"],
    },
    SemanticRuleEntry {
        id: "value-reference.workspace-path.resolve",
        category: ValueReference,
        implementation_status: Implemented,
        spec_required: false,
        conformance_required: false,
        docs_required: false,
        related_syntax_rule: Some("workspace_path_literal"),
        related_diagnostic_codes: &[
            "semantic.workspace_path.empty",
            "semantic.workspace_path.absolute",
            "semantic.workspace_path.dot_segment",
        ],
    },
    SemanticRuleEntry {
        id: "value-reference.fixture-reference.resolve",
        category: ValueReference,
        implementation_status: Implemented,
        spec_required: false,
        conformance_required: false,
        docs_required: false,
        related_syntax_rule: Some("fixture_reference_literal"),
        related_diagnostic_codes: &[
            "semantic.fixture_reference.empty",
            "semantic.fixture_reference.absolute",
            "semantic.fixture_reference.dot_segment",
            "semantic.fixture_reference.missing",
            "semantic.fixture_reference.not_a_regular_file",
            "semantic.fixture_reference.escapes_repor_directory",
        ],
    },
    SemanticRuleEntry {
        id: "value-reference.file-contents-reference.resolve",
        category: ValueReference,
        implementation_status: Implemented,
        spec_required: false,
        conformance_required: false,
        docs_required: false,
        related_syntax_rule: None,
        related_diagnostic_codes: &[
            "semantic.file_contents_reference.missing",
            "semantic.file_contents_reference.not_regular_file",
            "semantic.file_contents_reference.read_error",
        ],
    },
    SemanticRuleEntry {
        id: "value-reference.literal.kind-mismatch",
        category: ValueReference,
        implementation_status: Implemented,
        spec_required: false,
        conformance_required: false,
        docs_required: false,
        related_syntax_rule: Some("value_literal"),
        related_diagnostic_codes: &["semantic.literal.kind_mismatch"],
    },
];
