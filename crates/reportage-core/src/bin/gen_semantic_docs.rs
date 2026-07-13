use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticSpec {
    #[serde(rename = "$schema")]
    _schema: String,
    #[serde(rename = "schemaVersion")]
    _schema_version: u32,
    id: String,
    category: String,
    syntax: String,
    normative: serde_json::Map<String, serde_json::Value>,
    #[serde(rename = "conformanceCases")]
    conformance_cases: Vec<ConformanceCase>,
}

/// `assertion` and `checkpoint` are absent on a "parser case" (a conformance case verified
/// against the production parser rather than the evaluator, for rules that concern
/// acceptance/rejection of syntax or a literal rather than a checkpoint comparison). See
/// spec/language/semantics/README.md.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConformanceCase {
    description: String,
    #[serde(rename = "assertionSource")]
    assertion_source: String,
    #[serde(default)]
    assertion: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(default)]
    checkpoint: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(rename = "expectedResult")]
    expected_result: String,
    #[serde(rename = "expectedDiagnosticCode")]
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

fn load_spec_files() -> Vec<(PathBuf, SemanticSpec)> {
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

    let mut specs: Vec<(PathBuf, SemanticSpec)> = paths
        .into_iter()
        .map(|path| {
            let src = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("cannot read {}: {}", path.display(), e));
            let spec: SemanticSpec = serde_json::from_str(&src)
                .unwrap_or_else(|e| panic!("failed to deserialize {}: {}", path.display(), e));
            (path, spec)
        })
        .collect();
    specs.sort_by(|(left_path, left), (right_path, right)| {
        left.id
            .cmp(&right.id)
            .then_with(|| left_path.cmp(right_path))
    });
    specs
}

fn markdown_escape(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('\r', "\\r")
        .replace('\n', "<br>")
}

fn inline_code(input: &str) -> String {
    format!("`{}`", input.replace('`', "\\`"))
}

fn json_inline(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => inline_code(&format!("{s:?}")),
        serde_json::Value::Number(_) | serde_json::Value::Bool(_) | serde_json::Value::Null => {
            inline_code(&value.to_string())
        }
        _ => inline_code(
            &serde_json::to_string(value).expect("serializing semantic spec value must succeed"),
        ),
    }
}

fn string_literal_code(input: &str) -> String {
    inline_code(&serde_json::to_string(input).expect("serializing string must succeed"))
}

fn render_value(value: &serde_json::Value) -> String {
    markdown_escape(&json_inline(value))
}

fn render_map_table(fields: &serde_json::Map<String, serde_json::Value>) -> String {
    let mut out = String::from("| Field | Value |\n|---|---|\n");
    let mut keys: Vec<&String> = fields.keys().collect();
    keys.sort();
    for key in keys {
        out.push_str(&format!(
            "| {} | {} |\n",
            markdown_escape(key),
            render_value(&fields[key])
        ));
    }
    out
}

fn render_docs(specs: &[(PathBuf, SemanticSpec)]) -> String {
    let mut out = String::new();
    out.push_str(
        "GENERATED FILE: do not edit directly. Regenerate with `just semantic-docs-gen`.\n(see [crates/reportage-core/src/bin/gen_semantic_docs.rs](../../crates/reportage-core/src/bin/gen_semantic_docs.rs))\n\n",
    );
    out.push_str("# Semantic Rules\n\n");
    out.push_str("This is the generated semantic rule catalog. It is generated from `spec/language/semantics/*.json`. The JSON specs are the source of truth for each rule's normative fields and conformance cases; this catalog is a read-only view of that content.\n\n");
    out.push_str("The inventory of which semantic rules exist, and which ones require a spec, conformance cases, or a catalog entry here, is owned separately by the Rust const registry (`reportage_core::semantic_rule_registry::SEMANTIC_RULE_REGISTRY`), checked in CI by `just semantic-rule-coverage-check`. See [spec/language/semantics/README.md](../../spec/language/semantics/README.md) and [docs/adr/20260708T065700Z_semantic-rule-coverage-registry.md](../adr/20260708T065700Z_semantic-rule-coverage-registry.md) for the full source-of-truth split.\n\n");
    out.push_str("The conformance case lists below are read-only views derived from the JSON specs. Change the JSON specs, then regenerate this file.\n\n");
    out.push_str("Semantic conformance verifies the expected pass/fail result by passing the normalized assertion representation and checkpoint data from each JSON case to the semantic evaluator. Parser/source consistency is checked separately. The diagnostic code contract is defined in [`semantic-diagnostics.md`](semantic-diagnostics.md); expected diagnostic code checks remain optional until semantic conformance enables code verification. Cases without diagnostic codes are verified by pass/fail result only.\n\n");

    for (path, spec) in specs {
        let relative = path
            .strip_prefix(workspace_root())
            .unwrap_or(path)
            .display()
            .to_string();
        out.push_str(&format!("## {}\n\n", spec.id));
        out.push_str(&format!("- Source: `{}`\n", relative));
        out.push_str(&format!("- Syntax form: `{}`\n", spec.syntax));
        out.push_str(&format!("- Category: `{}`\n\n", spec.category));

        out.push_str("### Normative Fields\n\n");
        out.push_str(&render_map_table(&spec.normative));
        out.push('\n');

        out.push_str("### Conformance Cases\n\n");
        out.push_str("| Description | Assertion source | Normalized assertion | Checkpoint | Expected result | Expected diagnostic |\n");
        out.push_str("|---|---|---|---|---|---|\n");
        for case in &spec.conformance_cases {
            let expected_diagnostic = case
                .expected_diagnostic_code
                .as_deref()
                .map(inline_code)
                .unwrap_or_else(|| "-".to_string());
            let render_optional_map = |map: &Option<serde_json::Map<String, serde_json::Value>>| {
                map.as_ref()
                    .map(|m| render_value(&serde_json::Value::Object(m.clone())))
                    .unwrap_or_else(|| "-".to_string())
            };
            out.push_str(&format!(
                "| {} | {} | {} | {} | `{}` | {} |\n",
                markdown_escape(&case.description),
                markdown_escape(&string_literal_code(&case.assertion_source)),
                render_optional_map(&case.assertion),
                render_optional_map(&case.checkpoint),
                markdown_escape(&case.expected_result),
                markdown_escape(&expected_diagnostic)
            ));
        }
        out.push('\n');
    }

    out
}

fn main() {
    let output = env::args()
        .nth(1)
        .unwrap_or_else(|| "docs/reference/semantic-rules.md".to_string());
    let docs = render_docs(&load_spec_files());
    fs::write(Path::new(&output), docs)
        .unwrap_or_else(|e| panic!("cannot write {}: {}", output, e));
}
