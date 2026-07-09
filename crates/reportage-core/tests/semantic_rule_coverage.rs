//! Semantic rule coverage check.
//!
//! Verifies that `reportage_core::semantic_rule_registry::SEMANTIC_RULE_REGISTRY` (the source of
//! truth for "which semantic rules exist" and "what coverage each one requires") and
//! `spec/language/semantics/*.json` (the source of truth for each rule's normative fields and
//! conformance cases) do not drift apart, and that the registry's cross-references into
//! `DiagnosticCode` and `reportage.pest` follow the rules declared by their relation kind. See
//! docs/adr/20260708T065700Z_semantic-rule-coverage-registry.md.
//!
//! This module intentionally does not re-validate spec file schema shape; `semantic_specs.rs`
//! already owns that. It only checks registry/spec correspondence, the required-flag implications
//! the registry itself declares, and the cross-reference conventions.

use reportage_core::diagnostic::DiagnosticCode;
use reportage_core::semantic_rule_registry::{
    RelatedDiagnostic, SEMANTIC_RULE_REGISTRY, SemanticRuleId,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct SpecSummary {
    id: String,
    #[serde(rename = "conformanceCases", default)]
    conformance_cases: Vec<ConformanceCaseSummary>,
}

#[derive(Debug, Deserialize)]
struct ConformanceCaseSummary {
    description: String,
    #[serde(rename = "expectedDiagnosticCode", default)]
    expected_diagnostic_code: Option<String>,
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate must live under crates/")
        .parent()
        .expect("crates/ must live under workspace root")
        .to_path_buf()
}

fn spec_dir() -> PathBuf {
    workspace_root().join("spec/language/semantics")
}

fn generated_docs_path() -> PathBuf {
    workspace_root().join("docs/language/semantic-rules.md")
}

fn grammar_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/reportage.pest")
}

fn load_spec_summaries() -> Vec<(PathBuf, SpecSummary)> {
    let dir = spec_dir();
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", dir.display(), e))
        .filter_map(|entry| {
            let path = entry.expect("dir entry error").path();
            if path.extension().is_some_and(|e| e == "json")
                && path.file_name().is_some_and(|n| n != "schema.json")
            {
                Some(path)
            } else {
                None
            }
        })
        .collect();
    paths.sort();

    paths
        .into_iter()
        .map(|path| {
            let src = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("cannot read {}: {}", path.display(), e));
            let summary: SpecSummary = serde_json::from_str(&src)
                .unwrap_or_else(|e| panic!("failed to deserialize {}: {}", path.display(), e));
            (path, summary)
        })
        .collect()
}

/// Rule names defined by the grammar, extracted line-by-line.
///
/// Every rule definition in `reportage.pest` starts at column zero as `<name> = ...`; rule bodies
/// only continue on indented lines, so scanning line starts is sufficient and avoids a pest
/// meta-grammar dependency.
fn pest_rule_names() -> BTreeSet<String> {
    let path = grammar_path();
    let src = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", path.display(), e));
    src.lines()
        .filter_map(|line| {
            let (candidate, _) = line.split_once('=')?;
            let candidate = candidate.trim_end();
            let is_identifier = !candidate.is_empty()
                && candidate
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_');
            (candidate == candidate.trim_start() && is_identifier).then(|| candidate.to_string())
        })
        .collect()
}

#[test]
fn registry_has_exactly_one_entry_per_semantic_rule_id() {
    let mut seen = BTreeSet::new();
    for entry in SEMANTIC_RULE_REGISTRY {
        assert!(
            seen.insert(entry.id),
            "duplicate semantic rule id '{}' in SEMANTIC_RULE_REGISTRY",
            entry.id
        );
    }
    let missing: Vec<&SemanticRuleId> = SemanticRuleId::ALL
        .iter()
        .filter(|id| !seen.contains(id))
        .collect();
    assert!(
        missing.is_empty(),
        "SemanticRuleId variant(s) have no SEMANTIC_RULE_REGISTRY entry: {missing:?}"
    );
}

#[test]
fn semantic_rule_id_strings_are_unique() {
    let mut seen = BTreeSet::new();
    for id in SemanticRuleId::ALL {
        assert!(
            seen.insert(id.as_str()),
            "duplicate SemanticRuleId string '{id}'"
        );
    }
}

#[test]
fn diagnostic_code_strings_are_unique() {
    let mut seen = BTreeSet::new();
    for code in DiagnosticCode::ALL {
        assert!(
            seen.insert(code.as_str()),
            "duplicate DiagnosticCode string '{code}'"
        );
    }
}

#[test]
fn conformance_required_implies_spec_required() {
    for entry in SEMANTIC_RULE_REGISTRY {
        if entry.conformance_required {
            assert!(
                entry.spec_required,
                "semantic rule '{}' has conformance_required=true but spec_required=false; \
                 conformance_required implies spec_required",
                entry.id
            );
        }
    }
}

#[test]
fn docs_required_implies_spec_required() {
    for entry in SEMANTIC_RULE_REGISTRY {
        if entry.docs_required {
            assert!(
                entry.spec_required,
                "semantic rule '{}' has docs_required=true but spec_required=false; \
                 docs_required implies spec_required because generated docs are built from specs",
                entry.id
            );
        }
    }
}

#[test]
fn spec_required_registry_entries_have_a_spec_file() {
    let spec_ids: BTreeSet<String> = load_spec_summaries()
        .into_iter()
        .map(|(_, summary)| summary.id)
        .collect();

    let missing: Vec<&str> = SEMANTIC_RULE_REGISTRY
        .iter()
        .filter(|entry| entry.spec_required && !spec_ids.contains(entry.id.as_str()))
        .map(|entry| entry.id.as_str())
        .collect();

    assert!(
        missing.is_empty(),
        "semantic rule(s) marked spec_required=true in SEMANTIC_RULE_REGISTRY are missing a spec \
         file under {}: {:?}",
        spec_dir().display(),
        missing
    );
}

#[test]
fn spec_files_all_have_a_registry_entry() {
    let registry_ids: BTreeSet<&str> = SEMANTIC_RULE_REGISTRY
        .iter()
        .map(|e| e.id.as_str())
        .collect();

    let excess: Vec<String> = load_spec_summaries()
        .into_iter()
        .filter(|(_, summary)| !registry_ids.contains(summary.id.as_str()))
        .map(|(path, summary)| format!("{} (id={})", path.display(), summary.id))
        .collect();

    // A spec file with no matching registry entry would let the spec directory scan become an
    // implicit second source of truth for rule existence, which the registry exists to prevent.
    assert!(
        excess.is_empty(),
        "semantic spec file(s) under {} have no corresponding entry in SEMANTIC_RULE_REGISTRY: {:?}",
        spec_dir().display(),
        excess
    );
}

#[test]
fn conformance_required_entries_have_conformance_cases() {
    let specs_by_id: BTreeMap<String, SpecSummary> = load_spec_summaries()
        .into_iter()
        .map(|(_, summary)| (summary.id.clone(), summary))
        .collect();

    let missing: Vec<&str> = SEMANTIC_RULE_REGISTRY
        .iter()
        .filter(|entry| entry.conformance_required)
        .filter(|entry| {
            specs_by_id
                .get(entry.id.as_str())
                .is_none_or(|summary| summary.conformance_cases.is_empty())
        })
        .map(|entry| entry.id.as_str())
        .collect();

    assert!(
        missing.is_empty(),
        "semantic rule(s) marked conformance_required=true have no non-empty conformanceCases in \
         their spec file: {:?}",
        missing
    );
}

#[test]
fn docs_required_entries_appear_in_generated_catalog() {
    let docs_path = generated_docs_path();
    let docs = fs::read_to_string(&docs_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", docs_path.display(), e));

    let missing: Vec<&str> = SEMANTIC_RULE_REGISTRY
        .iter()
        .filter(|entry| entry.docs_required)
        .filter(|entry| !docs.contains(&format!("## {}\n", entry.id)))
        .map(|entry| entry.id.as_str())
        .collect();

    assert!(
        missing.is_empty(),
        "semantic rule(s) marked docs_required=true are missing a '## <id>' section in {}: {:?}. \
         Run 'just semantic-docs-gen' after adding the missing spec file(s).",
        docs_path.display(),
        missing
    );
}

#[test]
fn rule_owned_diagnostic_codes_use_rule_id_prefix_with_nonempty_reason() {
    for entry in SEMANTIC_RULE_REGISTRY {
        for related in entry.related_diagnostic_codes {
            let RelatedDiagnostic::RuleOwned(code) = related else {
                continue;
            };
            let prefix = format!("{}.", entry.id);
            let reason = code.as_str().strip_prefix(&prefix);
            // `starts_with(rule_id)` alone would accept `assertion.file.existsX`; the separator
            // dot and a non-empty reason are part of the convention.
            assert!(
                reason.is_some_and(|reason| !reason.is_empty()),
                "rule-owned diagnostic '{}' of semantic rule '{}' must be named \
                 '<rule-id>.<reason>' with a non-empty reason",
                code,
                entry.id
            );
        }
    }
}

/// Which registry entries reference each diagnostic code, with the relation kind used at each
/// reference site. `RelatedDiagnostic` carries no lifetime, so `Copy` comparison is enough.
fn diagnostic_references() -> BTreeMap<&'static str, Vec<(SemanticRuleId, &'static str)>> {
    let mut references: BTreeMap<&'static str, Vec<(SemanticRuleId, &'static str)>> =
        BTreeMap::new();
    for entry in SEMANTIC_RULE_REGISTRY {
        for related in entry.related_diagnostic_codes {
            let kind = match related {
                RelatedDiagnostic::RuleOwned(_) => "RuleOwned",
                RelatedDiagnostic::Shared(_) => "Shared",
                RelatedDiagnostic::SemanticValidation(_) => "SemanticValidation",
            };
            references
                .entry(related.code().as_str())
                .or_default()
                .push((entry.id, kind));
        }
    }
    references
}

#[test]
fn each_diagnostic_code_uses_one_relation_kind_across_the_registry() {
    for (code, references) in diagnostic_references() {
        let kinds: BTreeSet<&str> = references.iter().map(|(_, kind)| *kind).collect();
        assert_eq!(
            kinds.len(),
            1,
            "diagnostic '{code}' is referenced with conflicting relation kinds: {references:?}"
        );
    }
}

#[test]
fn rule_owned_diagnostic_codes_are_referenced_by_exactly_one_rule() {
    for (code, references) in diagnostic_references() {
        if references.iter().any(|(_, kind)| *kind == "RuleOwned") {
            assert_eq!(
                references.len(),
                1,
                "rule-owned diagnostic '{code}' is referenced by multiple semantic rules \
                 ({references:?}); a diagnostic shared between rules must use Shared"
            );
        }
    }
}

#[test]
fn shared_diagnostic_codes_are_referenced_by_at_least_two_rules() {
    for (code, references) in diagnostic_references() {
        if references.iter().any(|(_, kind)| *kind == "Shared") {
            assert!(
                references.len() >= 2,
                "shared diagnostic '{code}' is referenced by only {references:?}; a diagnostic \
                 owned by a single rule must use RuleOwned or SemanticValidation"
            );
        }
    }
}

#[test]
fn semantic_validation_diagnostic_codes_stay_in_semantic_namespace() {
    for entry in SEMANTIC_RULE_REGISTRY {
        for related in entry.related_diagnostic_codes {
            let RelatedDiagnostic::SemanticValidation(code) = related else {
                continue;
            };
            // The `semantic.*` namespace (docs/semantic-diagnostics.md) is what justifies the
            // exemption from the rule-id prefix convention; a code outside it must be RuleOwned
            // or Shared instead.
            assert!(
                code.as_str().starts_with("semantic."),
                "semantic-validation diagnostic '{}' of semantic rule '{}' must live in the \
                 'semantic.*' namespace",
                code,
                entry.id
            );
        }
    }
}

#[test]
fn semantic_validation_diagnostic_codes_are_referenced_by_exactly_one_rule() {
    for (code, references) in diagnostic_references() {
        if references
            .iter()
            .any(|(_, kind)| *kind == "SemanticValidation")
        {
            assert_eq!(
                references.len(),
                1,
                "semantic-validation diagnostic '{code}' is referenced by multiple semantic \
                 rules ({references:?}); a diagnostic shared between rules must use Shared"
            );
        }
    }
}

#[test]
fn related_syntax_rules_exist_in_grammar() {
    let rule_names = pest_rule_names();
    assert!(
        !rule_names.is_empty(),
        "no rule definitions found in {}; the extraction in pest_rule_names() no longer matches \
         the grammar file's layout",
        grammar_path().display()
    );

    let stale: Vec<String> = SEMANTIC_RULE_REGISTRY
        .iter()
        .filter_map(|entry| {
            let syntax_rule = entry.related_syntax_rule?;
            (!rule_names.contains(syntax_rule))
                .then(|| format!("{} (related_syntax_rule={syntax_rule})", entry.id))
        })
        .collect();

    assert!(
        stale.is_empty(),
        "related_syntax_rule value(s) do not exist in {}: {:?}",
        grammar_path().display(),
        stale
    );
}

#[test]
fn spec_expected_diagnostics_are_known_codes_and_respect_rule_ownership() {
    let known_codes: BTreeSet<&str> = DiagnosticCode::ALL.iter().map(|c| c.as_str()).collect();
    let rule_owned_codes: BTreeMap<&str, &str> = SEMANTIC_RULE_REGISTRY
        .iter()
        .flat_map(|entry| {
            entry.related_diagnostic_codes.iter().filter_map(|related| {
                let RelatedDiagnostic::RuleOwned(code) = related else {
                    return None;
                };
                Some((code.as_str(), entry.id.as_str()))
            })
        })
        .collect();

    let mut violations = Vec::new();
    for (path, summary) in load_spec_summaries() {
        for case in &summary.conformance_cases {
            let Some(code) = case.expected_diagnostic_code.as_deref() else {
                continue;
            };
            // First tier: a spec must not reference a diagnostic code that does not exist, e.g.
            // one left behind by a rename.
            if !known_codes.contains(code) {
                violations.push(format!(
                    "{} case '{}' expects unknown diagnostic code '{}'",
                    path.display(),
                    case.description,
                    code
                ));
                continue;
            }
            // Second tier: a rule-owned diagnostic may only be expected by its owning rule's
            // spec. Shared and semantic-validation diagnostics legitimately appear in other
            // rules' specs (e.g. `semantic.literal.kind_mismatch` in an assertion rule's
            // invalid-expected-value case), so they are not constrained here.
            if let Some(owner) = rule_owned_codes.get(code) {
                if *owner != summary.id {
                    violations.push(format!(
                        "{} case '{}' expects '{}' which is rule-owned by '{}', not by '{}'",
                        path.display(),
                        case.description,
                        code,
                        owner,
                        summary.id
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "spec conformance case(s) reference diagnostic codes inconsistently with the registry: \
         {violations:#?}"
    );
}
