//! Semantic rule coverage check.
//!
//! Verifies that `reportage_core::semantic_rule_registry::SEMANTIC_RULE_REGISTRY` (the source of
//! truth for "which semantic rules exist" and "what coverage each one requires") and
//! `spec/language/semantics/*.json` (the source of truth for each rule's normative fields and
//! conformance cases) do not drift apart. See
//! docs/adr/20260708T065700Z_semantic-rule-coverage-registry.md.
//!
//! This module intentionally does not re-validate spec file schema shape; `semantic_specs.rs`
//! already owns that. It only checks registry/spec correspondence and the required-flag
//! implications the registry itself declares.

use reportage_core::semantic_rule_registry::SEMANTIC_RULE_REGISTRY;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct SpecSummary {
    id: String,
    #[serde(rename = "conformanceCases", default)]
    conformance_cases: Vec<serde_json::Value>,
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

#[test]
fn registry_ids_are_unique() {
    let mut seen = BTreeSet::new();
    for entry in SEMANTIC_RULE_REGISTRY {
        assert!(
            seen.insert(entry.id),
            "duplicate semantic rule id '{}' in SEMANTIC_RULE_REGISTRY",
            entry.id
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
        .filter(|entry| entry.spec_required && !spec_ids.contains(entry.id))
        .map(|entry| entry.id)
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
    let registry_ids: BTreeSet<&str> = SEMANTIC_RULE_REGISTRY.iter().map(|e| e.id).collect();

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
    let specs_by_id: std::collections::BTreeMap<String, SpecSummary> = load_spec_summaries()
        .into_iter()
        .map(|(_, summary)| (summary.id.clone(), summary))
        .collect();

    let missing: Vec<&str> = SEMANTIC_RULE_REGISTRY
        .iter()
        .filter(|entry| entry.conformance_required)
        .filter(|entry| {
            specs_by_id
                .get(entry.id)
                .is_none_or(|summary| summary.conformance_cases.is_empty())
        })
        .map(|entry| entry.id)
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
        .map(|entry| entry.id)
        .collect();

    assert!(
        missing.is_empty(),
        "semantic rule(s) marked docs_required=true are missing a '## <id>' section in {}: {:?}. \
         Run 'just semantic-docs-gen' after adding the missing spec file(s).",
        docs_path.display(),
        missing
    );
}
