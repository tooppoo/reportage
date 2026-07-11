//! Conformance tests for `reportage references --format=json`, the reference index
//! (issue #137, renamed from `reportage docs` in issue #166).
//!
//! Two deliberately separate concerns, mirroring the split documented in
//! `spec/output/references-index/README.md`:
//!
//! - **Schema validation**: the stdout document is deserialised into typed Rust structs marked
//!   `#[serde(deny_unknown_fields)]`, the same CI-enforcement approach as
//!   `json_report_fixtures.rs`. `spec/output/references-index/schema.json` is the authoritative
//!   external contract these structs mirror.
//! - **Repository consistency**: every `documents[].path` must exist in this repository. This
//!   is a property of the repository state, not of the output structure, so it is a separate
//!   test with a distinct failure message rather than a schema constraint.
//!
//! Neither check touches the network: tag existence and URL reachability are release-process
//! concerns, out of scope by design (see
//! `docs/adr/20260708T180000Z_ai-documentation-discovery-core-path.md`).

// Serde-populated struct fields are read through assertions only. Mirrors json_report_fixtures.rs.
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::path::Path;

use assert_cmd::Command;
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Typed representation of the reference index document (schema validation)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReferencesIndexDocument {
    schema_version: u32,
    tool: Tool,
    documents: Vec<Document>,
    validation: Validation,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tool {
    name: String,
    version: String,
    tag: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    id: String,
    title: String,
    path: String,
    urls: Urls,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Urls {
    human: String,
    ai: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Validation {
    command: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn workspace_root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// Runs `reportage references --format=json` and parses its stdout as the reference index
/// document.
///
/// Deserialisation happens straight from the full stdout string, so anything printed around
/// the JSON document (prose, progress, warnings) fails the parse: this asserts the
/// single-JSON-document stdout contract, not just JSON validity somewhere in the stream.
fn references_index() -> ReferencesIndexDocument {
    let output = Command::cargo_bin("reportage")
        .expect("cargo-built reportage binary should exist")
        .args(["references", "--format=json"])
        .output()
        .expect("failed to run reportage references --format=json");

    assert!(
        output.status.success(),
        "reportage references --format=json should exit 0, got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(
        output.stderr.is_empty(),
        "reportage references --format=json should not write to stderr:\n{}",
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("stdout was not a single valid reference index document: {e}\n{stdout}")
    })
}

// ---------------------------------------------------------------------------
// Schema validation (output contract)
// ---------------------------------------------------------------------------

#[test]
fn references_index_document_conforms_to_the_contract() {
    let doc = references_index();

    assert_eq!(doc.schema_version, 1);
    assert_eq!(doc.tool.name, "reportage");
    assert_eq!(doc.tool.version, env!("CARGO_PKG_VERSION"));
    // The tag convention has no `v` prefix: tool.tag == tool.version (issue #166).
    assert_eq!(doc.tool.tag, doc.tool.version);

    assert!(
        !doc.documents.is_empty(),
        "reference index should list at least one document"
    );

    for document in &doc.documents {
        assert_eq!(
            document.urls.human,
            format!(
                "https://github.com/tooppoo/reportage/blob/{}/{}",
                doc.tool.tag, document.path
            ),
            "human URL of '{}' should be the GitHub blob URL for the tag",
            document.id,
        );
        assert_eq!(
            document.urls.ai,
            format!(
                "https://raw.githubusercontent.com/tooppoo/reportage/{}/{}",
                doc.tool.tag, document.path
            ),
            "AI URL of '{}' should be the raw.githubusercontent.com URL for the tag",
            document.id,
        );
        assert!(
            !document.title.is_empty(),
            "document '{}' should have a title",
            document.id,
        );
    }

    let ids: BTreeSet<&str> = doc.documents.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(
        ids.len(),
        doc.documents.len(),
        "documents[].id values must be unique"
    );

    // The sequence itself is contract: it is the recommended reading order for AI consumers
    // (issue #137). Reordering must be a deliberate change made both in the DOCUMENTS table
    // (crates/reportage-cli/src/references.rs) and here, never a refactoring side effect.
    let ordered_ids: Vec<&str> = doc.documents.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(
        ordered_ids,
        [
            "ai-readme",
            "ai-quick-reference",
            "syntax",
            "syntax-conformance",
            "semantics",
            "semantic-rules",
            "diagnostics",
            "semantic-diagnostics",
            "execution-model",
            "exit-codes",
            "configuration",
            "artifacts",
            "json-report",
            "run-result",
            "ai-generation-rules",
            "ai-validation-flow",
            "ai-common-mistakes",
        ],
        "documents[] order is the recommended reading order and must only change deliberately"
    );

    assert_eq!(
        doc.validation.command,
        "reportage <file.repor> --format=json"
    );
}

// ---------------------------------------------------------------------------
// Repository consistency (path existence)
// ---------------------------------------------------------------------------

#[test]
fn every_indexed_document_path_exists_in_the_repository() {
    let root = workspace_root();
    for document in references_index().documents {
        let path = root.join(&document.path);
        assert!(
            path.is_file(),
            "reference index entry '{}' points at '{}', which does not exist in the repository; \
             update the DOCUMENTS table in crates/reportage-cli/src/references.rs or restore the \
             file",
            document.id,
            document.path,
        );
    }
}

/// `validation.command` must reference an invocation that exists in this CLI.
/// The current value is a `reportage <script> --format=json` run; this exercises that exact
/// shape against the built binary so the index can never advertise a command that has drifted
/// away from the CLI (e.g. a not-yet-implemented `reportage check`).
#[test]
fn advertised_validation_command_is_a_real_invocation() {
    let command = references_index().validation.command;
    let placeholder = "<file.repor>";
    assert!(
        command.contains(placeholder),
        "validation.command '{command}' should carry the {placeholder} placeholder"
    );

    let temp = assert_fs::TempDir::new().expect("failed to create temp dir");
    let script = temp.path().join("probe.repor");
    std::fs::write(
        &script,
        "case \"probe\" {\n  $ true\n  assert {\n    exit 0\n  }\n}\n",
    )
    .expect("failed to write probe script");

    let mut parts = command.split_whitespace();
    let bin = parts
        .next()
        .expect("validation.command should not be empty");
    assert_eq!(bin, "reportage");
    let args: Vec<String> = parts
        .map(|arg| {
            if arg == placeholder {
                script.display().to_string()
            } else {
                arg.to_string()
            }
        })
        .collect();

    Command::cargo_bin("reportage")
        .expect("cargo-built reportage binary should exist")
        .args(&args)
        .current_dir(temp.path())
        .assert()
        .success();
}
