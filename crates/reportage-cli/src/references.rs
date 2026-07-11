//! The `reportage references` reference discovery command (issue #137, renamed from `docs` in issue #166).
//!
//! Prints the versioned documentation URL index for the running binary: the runtime version, the git tag derived from it (identical to the version, no `v` prefix), and a human URL / AI-readable URL pair per document.
//! This command is a side-effect-free URL index. It never embeds document bodies, executes scripts, loads config, writes artifacts, touches `.reportage/`, or performs network access.
//! Tag existence and URL reachability are release-process concerns, deliberately not checked here.
//! See docs/adr/20260708T180000Z_ai-documentation-discovery-core-path.md.
//!
//! The `--format=json` document printed here is its own contract (`spec/output/references-index/schema.json`), independent from the run report contract in `render::json` even though both spell the flag `--format=json`.
//!
//! [`DOCUMENTS`] doubles as the source for `docs/ai/reading-order.generated.md`
//! (`src/bin/gen_ai_reading_order.rs`, issue #142), so `role` and `note` are declared here even
//! though [`render_json`] never serializes them: keeping one source avoids a second, driftable
//! reading-order definition. See docs/adr/20260709T090000Z_ai-documentation-guide-structure.md.

use serde_json::json;

/// Version of the `reportage references --format=json` stdout contract (`spec/output/references-index/schema.json`).
/// Independent from the run report and artifact result contract versions.
const REFERENCES_INDEX_SCHEMA_VERSION: u32 = 1;

/// Binary/tool name, not the `reportage-cli` package name: consumers resolve documentation for the command they invoke.
const TOOL_NAME: &str = "reportage";

const GITHUB_REPO: &str = "tooppoo/reportage";

/// The validation command AI agents should run after editing a `.repor` file.
/// This must stay an invocation that exists in the current CLI: `reportage check` does not exist yet, so the index points at a plain `--format=json` run (see issue #137).
const VALIDATION_COMMAND: &str = "reportage <file.repor> --format=json";

pub struct DocumentEntry {
    /// Stable identifier: must survive title or path renames, and must stay unique within [`DOCUMENTS`].
    pub id: &'static str,
    pub title: &'static str,
    /// Repository-root-relative path. Must exist in the repository; enforced by `tests/references_index.rs`, not at runtime.
    pub path: &'static str,
    /// Short description of this document's job in the reading order.
    /// Internal-only: rendered into `docs/ai/reading-order.generated.md`, never into `reportage references --format=json` (see the module-level comment and issue #142).
    pub role: &'static str,
    /// A caution worth surfacing next to `role` when an AI reads this document, e.g. that it is generated. Empty when `role` already says everything needed. Same internal-only status as `role`.
    pub note: &'static str,
}

/// The references index, in the recommended reading order for AI consumers.
/// The order is part of the output contract: reorder deliberately, never incidentally.
pub const DOCUMENTS: &[DocumentEntry] = &[
    DocumentEntry {
        id: "ai-readme",
        title: "AI documentation guide",
        path: "docs/ai/README.md",
        role: "Entrypoint for AI-assisted authoring, editing, and review of .repor files",
        note: "A guide, not a specification: it points at the normative documents below rather than redefining them.",
    },
    DocumentEntry {
        id: "ai-quick-reference",
        title: "AI quick reference",
        path: "docs/ai/quick-reference.md",
        role: "Shortest path to a minimal valid .repor file, for a fast orientation pass",
        note: "Not a full syntax or semantics reference; follow its links for anything beyond the minimal shape.",
    },
    DocumentEntry {
        id: "syntax",
        title: "Syntax reference",
        path: "docs/syntax.md",
        role: "Normative syntax reference",
        note: "Generated from the grammar; a construct absent here is not available, regardless of what seems plausible.",
    },
    DocumentEntry {
        id: "syntax-conformance",
        title: "Syntax conformance fixtures",
        path: "docs/syntax-conformance.md",
        role: "Where the syntax conformance fixtures live: known-valid and known-invalid .repor examples with AST snapshots",
        note: "Describes repository test fixtures; the fixtures under tests/fixtures/syntax/ are the example set itself.",
    },
    DocumentEntry {
        id: "semantics",
        title: "Semantics",
        path: "docs/semantics.md",
        role: "Overview and entrypoint for the semantics documentation set",
        note: "",
    },
    DocumentEntry {
        id: "semantic-rules",
        title: "Semantic rule catalog",
        path: "docs/language/semantic-rules.md",
        role: "Generated catalog of language semantic rules",
        note: "Generated from spec/language/semantics/*.json; do not hand-edit.",
    },
    DocumentEntry {
        id: "diagnostics",
        title: "Diagnostics",
        path: "docs/diagnostics.md",
        role: "Parser and validator diagnostic code reference",
        note: "",
    },
    DocumentEntry {
        id: "semantic-diagnostics",
        title: "Semantic and assertion diagnostics",
        path: "docs/semantic-diagnostics.md",
        role: "Semantic, assertion, and step diagnostic code reference, extending the parse.* model above",
        note: "A specification: parts may not yet be applied to the parser, evaluator, or CLI diagnostic rendering.",
    },
    DocumentEntry {
        id: "execution-model",
        title: "Execution model",
        path: "docs/execution-model.md",
        role: "Runner execution order and case workspace/checkpoint lifecycle",
        note: "",
    },
    DocumentEntry {
        id: "exit-codes",
        title: "Exit codes",
        path: "docs/exit-codes.md",
        role: "Reportage process exit code reference",
        note: "",
    },
    DocumentEntry {
        id: "configuration",
        title: "Configuration",
        path: "docs/configuration.md",
        role: "reportage.kdl config file reference",
        note: "",
    },
    DocumentEntry {
        id: "artifacts",
        title: "Artifacts",
        path: "docs/artifacts.md",
        role: "Artifact bundle overview: the .reportage/runs layout and result.json as the canonical run record",
        note: "reportage <file.repor> --format=json prints a projection derived from result.json, not the artifact document itself.",
    },
    DocumentEntry {
        id: "json-report",
        title: "JSON execution report contract",
        path: "spec/output/json-report/README.md",
        role: "Run JSON output contract",
        note: "",
    },
    DocumentEntry {
        id: "run-result",
        title: "Run result artifact contract",
        path: "spec/artifacts/run-result/README.md",
        role: "Run result artifact JSON contract",
        note: "",
    },
    DocumentEntry {
        id: "ai-generation-rules",
        title: "AI generation rules",
        path: "docs/ai/generation-rules.md",
        role: "Rules and prohibitions for generating or editing .repor files",
        note: "Read after the syntax and semantics references above; it does not repeat their content.",
    },
    DocumentEntry {
        id: "ai-validation-flow",
        title: "AI validation flow",
        path: "docs/ai/validation-flow.md",
        role: "How to validate a .repor file after generating or editing it",
        note: "Only describes commands that exist in this CLI today; see the validation.command field for the current invocation.",
    },
    DocumentEntry {
        id: "ai-common-mistakes",
        title: "AI common mistakes",
        path: "docs/ai/common-mistakes.md",
        role: "Short wrong/correct examples of mistakes AI agents commonly make",
        note: "Points at existing fixtures and generated docs rather than collecting a full example set.",
    },
];

/// The reportage version this binary was built as; the references tag is derived from it mechanically, even for dev/prerelease builds where the tag may not exist (accepted in v0, see issue #137).
fn runtime_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// The git tag URLs point at: identical to `version`, with no `v` prefix, matching the repository's tag convention (issue #166).
fn references_tag(version: &str) -> String {
    version.to_string()
}

fn human_url(tag: &str, path: &str) -> String {
    format!("https://github.com/{GITHUB_REPO}/blob/{tag}/{path}")
}

fn ai_url(tag: &str, path: &str) -> String {
    format!("https://raw.githubusercontent.com/{GITHUB_REPO}/{tag}/{path}")
}

pub fn render_human() {
    let version = runtime_version();
    let tag = references_tag(version);

    println!("{TOOL_NAME} {version} (references tag: {tag})");
    println!();
    println!("Documents (recommended reading order):");
    for doc in DOCUMENTS {
        println!();
        println!("  {}", doc.title);
        println!("    human: {}", human_url(&tag, doc.path));
        println!("    ai:    {}", ai_url(&tag, doc.path));
    }
    println!();
    println!("Validation:");
    println!("  After editing a .repor file, run: {VALIDATION_COMMAND}");
}

pub fn render_json() {
    let version = runtime_version();
    let tag = references_tag(version);

    let documents: Vec<_> = DOCUMENTS
        .iter()
        .map(|doc| {
            json!({
                "id": doc.id,
                "title": doc.title,
                "path": doc.path,
                "urls": {
                    "human": human_url(&tag, doc.path),
                    "ai": ai_url(&tag, doc.path),
                },
            })
        })
        .collect();

    let document = json!({
        "schema_version": REFERENCES_INDEX_SCHEMA_VERSION,
        "tool": {
            "name": TOOL_NAME,
            "version": version,
            "tag": tag,
        },
        "documents": documents,
        "validation": {
            "command": VALIDATION_COMMAND,
        },
    });

    // The JSON mode contract mirrors the run report's: the single JSON document is the only thing on stdout.
    println!(
        "{}",
        serde_json::to_string_pretty(&document)
            .expect("references index serialization should not fail")
    );
}
