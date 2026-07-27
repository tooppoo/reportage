//! Process-level tests for the `xtask` binary.
//!
//! The library tests cover what each command decides; these cover what the process does with
//! that decision. The exit code is the automation contract `just schema-artifacts-check` and CI
//! branch on, and it is produced only in `main.rs`, so nothing else in the suite would catch a
//! regression that made a stale public schema exit zero.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;
use xtask::schema_artifacts::CONTRACTS;

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(args)
        .output()
        .expect("xtask binary runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is UTF-8")
}

fn code(output: &Output) -> Option<i32> {
    output.status.code()
}

/// A root whose committed public schemas disagree with their internal sources.
fn stale_root() -> TempDir {
    let root = TempDir::new().expect("temporary directory");
    for contract in CONTRACTS {
        copy(
            &repository_path(contract.internal_path),
            &root.path().join(contract.internal_path),
        );
        fs::write(root.path().join(contract.public_path), "{}\n").expect("write public schema");
    }
    root
}

fn repository_path(relative: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to.parent().expect("path has a parent")).expect("create directory");
    fs::copy(from, to).expect("copy schema file");
}

#[test]
fn check_exits_zero_on_the_committed_schemas_and_writes_only_to_stdout() {
    let output = run(&["schema-artifacts", "check"]);

    assert_eq!(code(&output), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).contains("up to date"));
    assert!(stderr(&output).is_empty());
}

#[test]
fn check_emits_one_json_envelope_on_stdout() {
    let output = run(&["schema-artifacts", "check", "--format", "json"]);

    assert_eq!(code(&output), Some(0));
    assert!(stderr(&output).is_empty());
    let envelope: Value = serde_json::from_str(&stdout(&output)).expect("one JSON envelope");
    assert_eq!(envelope["status"], "success");
    assert_eq!(envelope["exitCode"], 0);
}

#[test]
fn a_stale_public_schema_exits_five_and_writes_only_to_stderr() {
    let root = stale_root();

    let output = run(&[
        "schema-artifacts",
        "check",
        "--root",
        root.path().to_str().expect("UTF-8 path"),
    ]);

    assert_eq!(code(&output), Some(5));
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("just schema-artifacts-gen"));
}

#[test]
fn a_stale_public_schema_reports_a_conflict_envelope_on_stderr() {
    let root = stale_root();

    let output = run(&[
        "schema-artifacts",
        "check",
        "--root",
        root.path().to_str().expect("UTF-8 path"),
        "--format",
        "json",
    ]);

    assert_eq!(code(&output), Some(5));
    assert!(stdout(&output).is_empty());
    let envelope: Value = serde_json::from_str(&stderr(&output)).expect("one JSON envelope");
    assert_eq!(envelope["status"], "error");
    assert_eq!(envelope["exitCode"], 5);
    assert_eq!(envelope["error"]["category"], "conflict");
}

#[test]
fn a_missing_internal_source_exits_four() {
    let root = TempDir::new().expect("temporary directory");

    let output = run(&[
        "schema-artifacts",
        "check",
        "--root",
        root.path().to_str().expect("UTF-8 path"),
    ]);

    assert_eq!(code(&output), Some(4));
    assert!(stderr(&output).contains("SOURCE_SCHEMA_UNREADABLE"));
}

#[test]
fn generation_against_a_synthetic_root_writes_the_public_schemas() {
    let root = stale_root();

    let output = run(&[
        "schema-artifacts",
        "gen",
        "--root",
        root.path().to_str().expect("UTF-8 path"),
    ]);

    assert_eq!(code(&output), Some(0), "{}", stderr(&output));
    for contract in CONTRACTS {
        let generated =
            fs::read_to_string(root.path().join(contract.public_path)).expect("public schema");
        assert_ne!(generated, "{}\n");
        assert!(!generated.contains("x-reportage-snapshot"));
    }
}

#[test]
fn an_unknown_subcommand_exits_two() {
    let output = run(&["schema-artifacts", "bogus"]);

    assert_eq!(code(&output), Some(2));
    assert!(stdout(&output).is_empty());
}
