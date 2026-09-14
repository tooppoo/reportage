//! Generated-document conformance for `reportage docs` (issue #257): the
//! product-facing projection.
//!
//! The scenario under `tests/fixtures/docs-product/<scenario>/sources/` is a
//! stand-in for the real thing this projection exists for — a product whose
//! `.repor` files describe the configuration its users write, the commands
//! they run, and what those commands are verified to produce. It is copied
//! into a temp working directory and generated through the real binary; the
//! produced `index.txt` / `index.md` must match the committed
//! `index.snapshot.txt` / `index.snapshot.md` byte for byte. The snapshots
//! double as the inspectable generated example documents the issue requires.
//! Refresh with `UPDATE_DOCS_SNAPSHOTS=1`, the same convention
//! `docs_generation.rs` uses. The output contains no volatile fields (no
//! versions, no absolute paths), so no normalization is applied.
//!
//! `docs_generation.rs` is the counterpart for `docs-reportage`; keeping the
//! two suites apart is what keeps each projection's contract pinned to the
//! subcommand that owns it.

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use assert_fs::TempDir;
use assert_fs::prelude::*;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root must exist")
        .to_path_buf()
}

fn fixture_dir() -> PathBuf {
    repo_root().join("tests/fixtures/docs-product")
}

fn update_snapshots() -> bool {
    std::env::var_os("UPDATE_DOCS_SNAPSHOTS").is_some()
}

fn reportage(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("reportage").unwrap();
    cmd.current_dir(dir);
    cmd
}

/// Copies `<scenario>/sources/**` into `sources/` inside the temp dir.
fn seed_scenario(dir: &TempDir, scenario: &str) {
    let sources = fixture_dir().join(scenario).join("sources");
    copy_tree(&sources, dir.child("sources").path());
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

fn assert_matches_snapshot(scenario: &str, snapshot_name: &str, generated: &str) {
    let snapshot_path = fixture_dir().join(scenario).join(snapshot_name);
    if update_snapshots() {
        std::fs::write(&snapshot_path, generated).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&snapshot_path).unwrap_or_else(|_| {
        panic!(
            "missing snapshot {}; run `UPDATE_DOCS_SNAPSHOTS=1 cargo test -p reportage-cli --test docs_product_generation` to create or refresh snapshots",
            snapshot_path.display()
        )
    });
    assert_eq!(
        generated, expected,
        "generated document for scenario '{scenario}' does not match its snapshot; \
         refresh deliberately with `UPDATE_DOCS_SNAPSHOTS=1 cargo test -p reportage-cli --test docs_product_generation`"
    );
}

fn generate(dir: &TempDir) -> String {
    reportage(dir)
        .args(["docs", "sources/**/*.repor", "--out-dir", "generated"])
        .assert()
        .success()
        .stdout("generated: generated/index.txt\n");
    std::fs::read_to_string(dir.child("generated/index.txt").path()).unwrap()
}

fn generate_markdown(dir: &TempDir) -> String {
    reportage(dir)
        .args([
            "docs",
            "sources/**/*.repor",
            "--out-dir",
            "generated",
            "--format",
            "markdown",
        ])
        .assert()
        .success()
        .stdout("generated: generated/index.md\n");
    std::fs::read_to_string(dir.child("generated/index.md").path()).unwrap()
}

/// The representative scenario: prepared files with heredoc content, single
/// and repeated commands, several assertion checkpoints in one example, a
/// `before_each` projected as preparation, and a failing-command example.
#[test]
fn enozunu_like_scenario_matches_snapshot() {
    let dir = TempDir::new().unwrap();
    seed_scenario(&dir, "enozunu-like");
    let generated = generate(&dir);
    assert_matches_snapshot("enozunu-like", "index.snapshot.txt", &generated);
}

/// The same scenario through `--format markdown`: fixes the heading hierarchy,
/// table of contents, explicit anchors, step blocks, and verified-outcome
/// lists of the product Markdown contract.
#[test]
fn enozunu_like_markdown_scenario_matches_snapshot() {
    let dir = TempDir::new().unwrap();
    seed_scenario(&dir, "enozunu-like");
    let generated = generate_markdown(&dir);
    assert_matches_snapshot("enozunu-like", "index.snapshot.md", &generated);
}

/// The generated document never shows the DSL it came from. Asserted on the
/// whole snapshot scenario rather than a minimal one, so a wrapper leaking
/// from any step kind is caught.
#[test]
fn the_generated_documents_expose_no_reportage_construct() {
    let dir = TempDir::new().unwrap();
    seed_scenario(&dir, "enozunu-like");

    for document in [generate(&dir), generate_markdown(&dir)] {
        for construct in [
            "case \"",
            "assert {",
            "before_each",
            "write <",
            "```reportage",
            ".repor",
        ] {
            assert!(
                !document.contains(construct),
                "product output must not contain {construct:?}:\n{document}"
            );
        }
    }
}

/// The document ends with exactly one LF and carries no trailing whitespace,
/// independent of the line structure of the file contents it embeds.
#[test]
fn document_tail_and_whitespace_contract() {
    let dir = TempDir::new().unwrap();
    seed_scenario(&dir, "enozunu-like");

    let generated = generate(&dir);
    assert!(generated.ends_with('\n'));
    assert!(!generated.ends_with("\n\n"));
    for line in generated.lines() {
        assert_eq!(
            line,
            line.trim_end(),
            "no generated line may carry trailing whitespace"
        );
    }
}

/// The two subcommands document the same sources differently: `docs` states
/// what the product does, `docs-reportage` reproduces the scenario source.
/// Their outputs must not be interchangeable.
#[test]
fn the_two_subcommands_produce_different_documents_from_the_same_sources() {
    let dir = TempDir::new().unwrap();
    seed_scenario(&dir, "enozunu-like");

    let product = generate(&dir);
    reportage(&dir)
        .args([
            "docs-reportage",
            "sources/**/*.repor",
            "--out-dir",
            "generated",
            "--index-file-name",
            "source.txt",
        ])
        .assert()
        .success();
    let source = std::fs::read_to_string(dir.child("generated/source.txt").path()).unwrap();

    assert_ne!(product, source);
    assert!(source.contains("Reportage source"));
    assert!(source.contains("case \"init writes a lock file\" {"));
    assert!(product.contains("Command\n  demo init"));
}

/// `docs` parses sources but never executes them and never writes artifacts.
#[test]
fn docs_does_not_execute_sources_or_write_artifacts() {
    let dir = TempDir::new().unwrap();
    dir.child("sources/marker.repor")
        .write_str(
            "case \"would create a marker\" {\n  $ touch marker.txt\n\n  assert {\n    exit 0\n  }\n}\n",
        )
        .unwrap();

    generate(&dir);
    assert!(
        !dir.child("marker.txt").path().exists(),
        "the case action must not run"
    );
    assert!(
        !dir.child(".reportage").path().exists(),
        "no execution artifact may be written"
    );
}
