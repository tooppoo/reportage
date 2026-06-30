//! Syntax conformance fixtures for the production `parse()` entrypoint.
//!
//! These tests intentionally avoid raw pest parser access. They lock down which
//! checked-in scripts are accepted or rejected after grammar parsing plus
//! parser construction validation.

use std::fs;
use std::path::{Path, PathBuf};

use reportage_core::parser::{ParseError, parse};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture_paths(kind: &str) -> Vec<PathBuf> {
    let pattern = repo_root()
        .join(format!("tests/fixtures/syntax/{kind}/*.repor"))
        .to_str()
        .expect("fixture glob path must be valid UTF-8")
        .to_string();

    let mut paths = glob::glob(&pattern)
        .expect("syntax fixture glob pattern must be valid")
        .map(|entry| entry.expect("syntax fixture path must be readable"))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn read_fixture(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("failed to read syntax fixture {}: {e}", path.display()))
}

fn fixture_stem(path: &Path) -> &str {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .expect("syntax fixture file name must be valid UTF-8")
}

#[test]
fn valid_syntax_fixtures_parse_successfully() {
    let paths = fixture_paths("valid");
    assert!(
        !paths.is_empty(),
        "expected at least one valid syntax fixture"
    );

    for path in paths {
        let source = read_fixture(&path);
        parse(&source).unwrap_or_else(|e| {
            panic!(
                "valid syntax fixture {} must parse successfully: {e}",
                path.display()
            )
        });
    }
}

#[test]
fn invalid_syntax_fixtures_are_rejected() {
    let paths = fixture_paths("invalid");
    assert!(
        !paths.is_empty(),
        "expected at least one invalid syntax fixture"
    );

    for path in paths {
        let source = read_fixture(&path);
        let err = match parse(&source) {
            Ok(_) => panic!("invalid syntax fixture {} must be rejected", path.display()),
            Err(err) => err,
        };

        match fixture_stem(&path) {
            "empty_action" | "whitespace_only_action" => {
                assert!(matches!(err, ParseError::EmptyAction { .. }));
            }
            "empty_case_block" => {
                assert!(matches!(err, ParseError::EmptyCase { .. }));
            }
            "case_without_assertion_block" => {
                assert!(matches!(err, ParseError::MissingAssertionBlock { .. }));
            }
            "exit_code_out_of_range" => {
                assert!(matches!(err, ParseError::InvalidExitCode { .. }));
            }
            _ => {}
        }
    }
}
