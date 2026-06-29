//! Guards the pest grammar against drifting away from real Reportage scripts.
//!
//! Unit tests in `parser.rs` exercise the grammar against inline string
//! literals. This test instead parses every checked-in `.repor` fixture
//! under `examples/` and `e2e/`, so a grammar change that breaks real
//! scripts fails `cargo test` (and therefore `just check`) without needing
//! a separate CI job or the full CLI binary.

use std::fs;
use std::path::Path;

#[test]
fn all_repor_fixtures_parse_successfully() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    let patterns = ["examples/**/*.repor", "e2e/**/*.repor"];
    let mut checked = 0;

    for pattern in patterns {
        let full_pattern = root.join(pattern);
        let full_pattern = full_pattern
            .to_str()
            .expect("fixture pattern path must be valid UTF-8");

        for entry in glob::glob(full_pattern).expect("invalid glob pattern") {
            let path = entry.expect("glob entry must be readable");
            let source = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()));

            reportage_core::parser::parse(&source).unwrap_or_else(|e| {
                panic!("grammar failed to parse fixture {}: {e}", path.display())
            });

            checked += 1;
        }
    }

    assert!(
        checked > 0,
        "no .repor fixtures were found under examples/ or e2e/; \
         the glob patterns may be stale"
    );
}
