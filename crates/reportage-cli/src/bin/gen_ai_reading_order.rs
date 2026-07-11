//! Generates `docs/ai/reading-order.generated.md` from `reportage_cli::references::DOCUMENTS`.
//!
//! `DOCUMENTS` is also the source `reportage references --format=json` reads
//! (`src/references.rs`); this binary is deliberately the only other reader, so the reading
//! order can never carry a second, driftable definition. See issue #142 and
//! docs/adr/20260709T090000Z_ai-documentation-guide-structure.md.

use std::env;
use std::fs;
use std::path::Path;

use reportage_cli::references::{DOCUMENTS, DocumentEntry};

/// Directory `docs/ai/reading-order.generated.md` lives in, as repository-root-relative path
/// components. Links in the generated file are relative to this directory, not to the
/// repository root, so the file stays readable when opened directly from a checkout.
const OUTPUT_DIR: &[&str] = &["docs", "ai"];

/// Rewrites a repository-root-relative `path` (e.g. `"docs/syntax.md"`) into a path relative to
/// [`OUTPUT_DIR`] (e.g. `"../syntax.md"`), by walking the shared directory prefix.
fn relative_link(path: &str) -> String {
    let components: Vec<&str> = path.split('/').collect();
    let dirs = &components[..components.len() - 1];

    let shared = OUTPUT_DIR
        .iter()
        .zip(dirs.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let ascents = std::iter::repeat_n("..", OUTPUT_DIR.len() - shared);
    ascents
        .chain(components[shared..].iter().copied())
        .collect::<Vec<_>>()
        .join("/")
}

fn render_entry(doc: &DocumentEntry) -> String {
    let link = relative_link(doc.path);
    let mut out = format!("## {}\n\n", doc.title);
    out.push_str(&format!("- Link: [{}]({})\n", doc.title, link));
    out.push_str(&format!("- Path: `{}`\n", doc.path));
    out.push_str(&format!("- Role: {}\n", doc.role));
    if !doc.note.is_empty() {
        out.push_str(&format!("- Note: {}\n", doc.note));
    }
    out.push('\n');
    out
}

fn render_docs(documents: &[DocumentEntry]) -> String {
    let mut out = String::new();
    out.push_str(
        "GENERATED FILE: do not edit directly. Regenerate with `just ai-docs-gen`.\n(see [crates/reportage-cli/src/bin/gen_ai_reading_order.rs](../../crates/reportage-cli/src/bin/gen_ai_reading_order.rs))\n\n",
    );
    out.push_str("# AI reading order\n\n");
    out.push_str(
        "This is the recommended reading order for AI agents authoring, editing, or reviewing `.repor` files. It is generated from the same `DOCUMENTS` table `reportage references --format=json` reads (`crates/reportage-cli/src/references.rs`), so this list and that command's `documents[]` field never drift apart. See [`docs/ai/README.md`](README.md) for how to use this list.\n\n",
    );
    out.push_str(
        "`role` and `note` below are internal reading-order metadata. They are not part of the `reportage references --format=json` output contract (`spec/output/references-index/schema.json`), which carries only `id`, `title`, `path`, and `urls`.\n\n",
    );

    for doc in documents {
        out.push_str(&render_entry(doc));
    }

    out
}

fn main() {
    let output = env::args()
        .nth(1)
        .unwrap_or_else(|| "docs/ai/reading-order.generated.md".to_string());
    let docs = render_docs(DOCUMENTS);
    fs::write(Path::new(&output), docs)
        .unwrap_or_else(|e| panic!("cannot write {}: {}", output, e));
}
