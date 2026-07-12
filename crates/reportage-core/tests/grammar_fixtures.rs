//! Guards the pest grammar against drifting away from real Reportage scripts.
//!
//! Unit tests in `parser.rs` exercise the grammar against inline string literals.
//! This test instead parses every checked-in `.repor` fixture in the valid corpus (see `support::repor_corpus_paths`), so a grammar change that breaks real scripts fails `cargo test` (and therefore `just check`) without needing a separate CI job or the full CLI binary.

use std::fs;

mod support;

#[test]
fn all_repor_fixtures_parse_successfully() {
    for path in support::repor_corpus_paths() {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()));

        reportage_core::parser::parse(&source)
            .unwrap_or_else(|e| panic!("grammar failed to parse fixture {}: {e}", path.display()));
    }
}
