//! Shared helpers for reportage-core integration tests.

use std::path::{Path, PathBuf};

/// Every checked-in `.repor` fixture that must parse successfully.
///
/// This is the single definition of the valid fixture corpus;
/// corpus-wide guards (`grammar_fixtures.rs`, `source_model.rs`) must all
/// enumerate through here so a newly added fixture directory strengthens
/// every guard at once.
/// `tests/fixtures/syntax/invalid/` is deliberately absent: those fixtures
/// are rejected inputs, covered by `syntax_conformance.rs`.
pub fn repor_corpus_paths() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let patterns = [
        "examples/**/*.repor",
        "e2e/**/*.repor",
        "editors/vscode/examples/*.repor",
        "tests/fixtures/syntax/valid/*.repor",
    ];

    let mut paths = Vec::new();
    for pattern in patterns {
        let full_pattern = root.join(pattern);
        let full_pattern = full_pattern
            .to_str()
            .expect("fixture pattern path must be valid UTF-8");
        for entry in glob::glob(full_pattern).expect("invalid glob pattern") {
            paths.push(entry.expect("glob entry must be readable"));
        }
    }

    assert!(
        !paths.is_empty(),
        "no .repor fixtures were found; the corpus glob patterns may be stale"
    );
    paths
}
