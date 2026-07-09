//! The `reportage docs` documentation discovery command (issue #137).
//!
//! Prints the versioned documentation URL index for the running binary: the runtime version, the `v{version}` tag derived from it, and a human URL / AI-readable URL pair per document.
//! This command is a side-effect-free URL index. It never embeds document bodies, executes scripts, loads config, writes artifacts, touches `.reportage/`, or performs network access.
//! Tag existence and URL reachability are release-process concerns, deliberately not checked here.
//! See docs/adr/20260708T180000Z_ai-documentation-discovery-core-path.md.
//!
//! The `--format=json` document printed here is its own contract (`spec/output/docs-index/schema.json`), independent from the run report contract in `render::json` even though both spell the flag `--format=json`.

use serde_json::json;

/// Version of the `reportage docs --format=json` stdout contract (`spec/output/docs-index/schema.json`).
/// Independent from the run report and artifact result contract versions.
const DOCS_INDEX_SCHEMA_VERSION: u32 = 1;

/// Binary/tool name, not the `reportage-cli` package name: consumers resolve documentation for the command they invoke.
const TOOL_NAME: &str = "reportage";

const GITHUB_REPO: &str = "tooppoo/reportage";

/// The validation command AI agents should run after editing a `.repor` file.
/// This must stay an invocation that exists in the current CLI: `reportage check` does not exist yet, so the index points at a plain `--format=json` run (see issue #137).
const VALIDATION_COMMAND: &str = "reportage <file.repor> --format=json";

struct DocumentEntry {
    /// Stable identifier: must survive title or path renames, and must stay unique within [`DOCUMENTS`].
    id: &'static str,
    title: &'static str,
    /// Repository-root-relative path. Must exist in the repository; enforced by `tests/docs_index.rs`, not at runtime.
    path: &'static str,
}

/// The docs index, in the recommended reading order for AI consumers.
/// The order is part of the output contract: reorder deliberately, never incidentally.
const DOCUMENTS: &[DocumentEntry] = &[
    DocumentEntry {
        id: "syntax",
        title: "Syntax reference",
        path: "docs/syntax.md",
    },
    DocumentEntry {
        id: "semantics",
        title: "Semantics",
        path: "docs/semantics.md",
    },
    DocumentEntry {
        id: "semantic-rules",
        title: "Semantic rule catalog",
        path: "docs/language/semantic-rules.md",
    },
    DocumentEntry {
        id: "diagnostics",
        title: "Diagnostics",
        path: "docs/diagnostics.md",
    },
    DocumentEntry {
        id: "execution-model",
        title: "Execution model",
        path: "docs/execution-model.md",
    },
    DocumentEntry {
        id: "exit-codes",
        title: "Exit codes",
        path: "docs/exit-codes.md",
    },
    DocumentEntry {
        id: "configuration",
        title: "Configuration",
        path: "docs/configuration.md",
    },
    DocumentEntry {
        id: "json-report",
        title: "JSON execution report contract",
        path: "spec/output/json-report/README.md",
    },
    DocumentEntry {
        id: "run-result",
        title: "Run result artifact contract",
        path: "spec/artifacts/run-result/README.md",
    },
];

/// The reportage version this binary was built as; the docs tag is derived from it mechanically, even for dev/prerelease builds where the tag may not exist (accepted in v0, see issue #137).
fn runtime_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn docs_tag(version: &str) -> String {
    format!("v{version}")
}

fn human_url(tag: &str, path: &str) -> String {
    format!("https://github.com/{GITHUB_REPO}/blob/{tag}/{path}")
}

fn ai_url(tag: &str, path: &str) -> String {
    format!("https://raw.githubusercontent.com/{GITHUB_REPO}/{tag}/{path}")
}

pub fn render_human() {
    let version = runtime_version();
    let tag = docs_tag(version);

    println!("{TOOL_NAME} {version} (docs tag: {tag})");
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
    let tag = docs_tag(version);

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
        "schema_version": DOCS_INDEX_SCHEMA_VERSION,
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
        serde_json::to_string_pretty(&document).expect("docs index serialization should not fail")
    );
}
