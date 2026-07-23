//! Generated-document conformance for `reportage docs` (issue #170).
//!
//! Each scenario under `tests/fixtures/docs/<scenario>/sources/` is copied
//! into a temp working directory and generated through the real binary; the
//! produced `index.txt` must match the committed
//! `tests/fixtures/docs/<scenario>/index.snapshot.txt` byte for byte. The
//! snapshots double as the inspectable generated example documents required
//! by the issue. Refresh with `UPDATE_DOCS_SNAPSHOTS=1`, mirroring
//! `json_report_fixtures.rs`'s convention. The output contains no volatile
//! fields (no versions, no absolute paths), so no normalization is applied.
//!
//! Scenarios that need filesystem shapes a committed fixture cannot carry
//! (CRLF line endings, which `.gitattributes` would normalize) are built
//! in-test instead.

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
    repo_root().join("tests/fixtures/docs")
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

fn assert_matches_snapshot(scenario: &str, generated: &str) {
    let snapshot_path = fixture_dir().join(scenario).join("index.snapshot.txt");
    if update_snapshots() {
        std::fs::write(&snapshot_path, generated).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&snapshot_path).unwrap_or_else(|_| {
        panic!(
            "missing snapshot {}; run `UPDATE_DOCS_SNAPSHOTS=1 cargo test -p reportage-cli --test docs_generation` to create or refresh snapshots",
            snapshot_path.display()
        )
    });
    assert_eq!(
        generated, expected,
        "generated document for scenario '{scenario}' does not match its snapshot; \
         refresh deliberately with `UPDATE_DOCS_SNAPSHOTS=1 cargo test -p reportage-cli --test docs_generation`"
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

// --- committed fixture scenarios -----------------------------------------

/// The representative example from issue #170: one documented file with one
/// documented case, fixing the full block layout of the plain text contract.
#[test]
fn representative_scenario_matches_snapshot() {
    let dir = TempDir::new().unwrap();
    seed_scenario(&dir, "representative");
    let generated = generate(&dir);
    assert_matches_snapshot("representative", &generated);
}

/// Multiple groups and files: declared order before undeclared, order
/// ascending, path tie-break, case-sensitive group ordering, fallback title /
/// group / case titles, a zero-case file, heredoc descriptions, and a source
/// without a final newline.
#[test]
fn mixed_scenario_matches_snapshot() {
    let dir = TempDir::new().unwrap();
    seed_scenario(&dir, "mixed");
    let generated = generate(&dir);
    assert_matches_snapshot("mixed", &generated);
}

// --- in-test scenarios ----------------------------------------------------

const CRLF_SOURCE: &str = "document file {\r\n  title \"CRLF file\"\r\n}\r\n\r\ncase \"crlf case\" {\r\n  $ true\r\n\r\n  assert {\r\n    exit 0\r\n  }\r\n}\r\n";

/// CRLF sources generate an LF-only document; the source block content is
/// otherwise unchanged.
#[test]
fn crlf_sources_are_normalized_to_lf_in_the_generated_document() {
    let dir = TempDir::new().unwrap();
    dir.child("sources/crlf.repor")
        .write_str(CRLF_SOURCE)
        .unwrap();

    let generated = generate(&dir);
    assert!(
        !generated.contains('\r'),
        "generated document must not contain CR bytes"
    );
    assert!(generated.contains(
        "Reportage source\n    case \"crlf case\" {\n      $ true\n\n      assert {\n        exit 0\n      }\n    }\n"
    ));
}

/// The same sources selected by different patterns and lexical routes are
/// documented exactly once, and repeated runs produce identical bytes.
#[test]
fn deduplication_and_determinism() {
    let dir = TempDir::new().unwrap();
    seed_scenario(&dir, "representative");
    std::fs::create_dir(dir.child("sub").path()).unwrap();

    reportage(&dir)
        .args([
            "docs",
            "sources/*.repor",
            "sources/file-assertions.repor",
            "sub/../sources/*.repor",
            "--out-dir",
            "generated",
        ])
        .assert()
        .success();
    let first = std::fs::read_to_string(dir.child("generated/index.txt").path()).unwrap();
    assert_eq!(
        first.matches("Source path").count(),
        1,
        "the deduplicated source must appear exactly once"
    );

    reportage(&dir)
        .args([
            "docs",
            "sources/*.repor",
            "sources/file-assertions.repor",
            "sub/../sources/*.repor",
            "--out-dir",
            "generated",
        ])
        .assert()
        .success();
    let second = std::fs::read_to_string(dir.child("generated/index.txt").path()).unwrap();
    assert_eq!(first, second);
}

/// The document ends with exactly one LF and carries no trailing whitespace,
/// independent of whether sources end with a final newline.
#[test]
fn document_tail_and_whitespace_contract() {
    let dir = TempDir::new().unwrap();
    seed_scenario(&dir, "mixed");

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
