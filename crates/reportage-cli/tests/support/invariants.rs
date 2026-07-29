//! Domain invariants of a run document that JSON Schema cannot express.
//!
//! JSON Schema constrains one document's shape and values. These checks constrain relations: a
//! reference resolving to a target elsewhere in the same document, a count agreeing with what it
//! counts, a recorded path naming a file that exists. A document can satisfy every published
//! constraint and still violate all of them.
//!
//! Keeping them apart from schema validation is what makes each failure legible: a schema
//! violation means the producer broke the published contract, a violation here means the producer
//! wrote a self-inconsistent document that the contract was never able to rule out. See
//! docs/adr/20260728T092956Z_json-contract-validation-policy.md.
//!
//! The checks apply to both JSON contracts, because the fields they relate — `diagnostics`,
//! `summary`, `tests` — are the ones the stdout document and the canonical manifest share.

#![allow(dead_code)]

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

/// Every `diagnosticRef` in the document must name a `diagnostics[].id` in the same document.
///
/// The schema types `diagnosticRef` as a string and cannot say what it points at, so a renderer
/// that emitted an id for a diagnostic it never added would produce a document that validates and
/// still cannot be followed by a consumer.
pub fn assert_diagnostic_refs_resolve(document: &Value, context: &str) {
    let ids: BTreeSet<&str> = array(document, "diagnostics", context)
        .iter()
        .map(|diagnostic| string(diagnostic, "id", context))
        .collect();

    for (pointer, reference) in collect_diagnostic_refs(document, context) {
        assert!(
            ids.contains(reference.as_str()),
            "{context}: diagnosticRef `{reference}` at {pointer} names no diagnostic in the same document (known ids: {ids:?})"
        );
    }
}

/// A logical composition's children must never carry their own `diagnosticRef`.
///
/// Diagnostic attribution is composition-level: only the composition node that failed produces a
/// diagnostic, so a child carrying one would attribute the same failure twice. The schema declares
/// `diagnosticRef` on every expectation kind and has no way to forbid it in this one position.
///
/// Returns how many failed compositions were inspected. Only a failed composition has a diagnostic
/// to misattribute, so a caller that never sees one has not tested anything; see
/// [`assert_failed_logical_compositions_were_inspected`].
#[must_use]
pub fn assert_logical_children_have_no_diagnostic_ref(document: &Value, context: &str) -> usize {
    let mut failed_compositions = 0;

    for (pointer, expectation) in collect_expectations(document, context) {
        if expectation.get("kind").and_then(Value::as_str) != Some("logical") {
            continue;
        }
        if expectation.get("status").and_then(Value::as_str) == Some("failed") {
            failed_compositions += 1;
        }
        let children = expectation
            .get("children")
            .and_then(Value::as_array)
            .unwrap_or_else(|| {
                panic!("{context}: a logical expectation at {pointer} has no children array")
            });
        for (index, child) in children.iter().enumerate() {
            assert!(
                child.get("diagnosticRef").is_none(),
                "{context}: the child at {pointer}/children/{index} carries its own diagnosticRef; only the composition itself may be attributed a diagnostic"
            );
        }
    }

    failed_compositions
}

/// Guards [`assert_logical_children_have_no_diagnostic_ref`] against passing vacuously.
///
/// The invariant is unobservable in a run where no logical composition failed: with no diagnostic
/// to attribute, no child could carry one however the renderer behaved. A fixture set that stops
/// producing a failing composition would silently turn the check into a no-op, so the absence of
/// one is itself a failure. Mirrors `evidence_files_match_their_manifest_references`'s guard
/// against an accidentally empty loop.
pub fn assert_failed_logical_compositions_were_inspected(inspected: usize) {
    assert!(
        inspected > 0,
        "no fixture produced a failed logical composition, so the diagnosticRef attribution invariant was not exercised; add a fixture whose `all` / `any` / `not` composition fails"
    );
}

/// The `summary` counts must agree with the concrete results in the same document.
///
/// `errors` is checked as a lower bound rather than an equality: it counts file-level errors as
/// well as error-status tests, and a file that fails before any case exists contributes an error
/// with no test entry to compare against. The status relations below are what pin it from the
/// other side.
pub fn assert_summary_agrees_with_results(document: &Value, context: &str) {
    let tests = array(document, "tests", context);
    let summary = document
        .get("summary")
        .unwrap_or_else(|| panic!("{context}: the document has no summary"));

    let actions: u64 = tests
        .iter()
        .map(|test| array(test, "actions", context).len() as u64)
        .sum();
    let assertions: u64 = tests
        .iter()
        .map(|test| array(test, "assertions", context).len() as u64)
        .sum();
    let scripts = tests
        .iter()
        .filter_map(|test| test.get("path").and_then(Value::as_str))
        .collect::<BTreeSet<_>>()
        .len() as u64;

    assert_eq!(
        count(summary, "actions", context),
        actions,
        "{context}: summary.actions"
    );
    assert_eq!(
        count(summary, "assertions", context),
        assertions,
        "{context}: summary.assertions"
    );
    assert_eq!(
        count(summary, "scripts", context),
        scripts,
        "{context}: summary.scripts"
    );
    assert_eq!(
        count(summary, "passed", context),
        tests_with_status(tests, "passed"),
        "{context}: summary.passed"
    );
    assert_eq!(
        count(summary, "failed", context),
        tests_with_status(tests, "failed"),
        "{context}: summary.failed"
    );
    assert!(
        count(summary, "errors", context) >= tests_with_status(tests, "error"),
        "{context}: summary.errors must count at least every error-status test"
    );

    // The top-level status is defined as error > failed > passed over these same counts, so it is
    // derivable from them; a document where it is not means one of the two was computed from
    // something else.
    let status = string(document, "status", context);
    let expected_status = if count(summary, "errors", context) > 0 {
        "error"
    } else if count(summary, "failed", context) > 0 {
        "failed"
    } else {
        "passed"
    };
    assert_eq!(
        status, expected_status,
        "{context}: the top-level status must follow error > failed > passed over the summary counts"
    );
}

/// Every `tests[].path` must name a file that exists.
///
/// Paths are recorded relative to the working directory the run was started from, so `workspace`
/// is that directory.
pub fn assert_test_paths_exist(document: &Value, workspace: &Path, context: &str) {
    for test in array(document, "tests", context) {
        let Some(path) = test.get("path").and_then(Value::as_str) else {
            continue;
        };
        let resolved = workspace.join(path);
        assert!(
            resolved.is_file(),
            "{context}: tests[].path `{path}` names no file under {}",
            workspace.display()
        );
    }
}

// ---------------------------------------------------------------------------
// Document traversal
// ---------------------------------------------------------------------------

/// Every `diagnosticRef` in the document, paired with the JSON Pointer it sits at.
///
/// Both the assertion node and its expectation tree may carry one, so this walks the assertion
/// objects and every expectation nested under them rather than a fixed set of locations.
fn collect_diagnostic_refs(document: &Value, context: &str) -> Vec<(String, String)> {
    let mut refs = Vec::new();

    for (test_index, test) in array(document, "tests", context).iter().enumerate() {
        for (assertion_index, assertion) in array(test, "assertions", context).iter().enumerate() {
            let pointer = format!("/tests/{test_index}/assertions/{assertion_index}");
            if let Some(reference) = assertion.get("diagnosticRef").and_then(Value::as_str) {
                refs.push((pointer.clone(), reference.to_owned()));
            }
            collect_refs_in_expectation(
                assertion.get("expectation"),
                &format!("{pointer}/expectation"),
                &mut refs,
            );
        }
    }

    refs
}

fn collect_refs_in_expectation(
    expectation: Option<&Value>,
    pointer: &str,
    refs: &mut Vec<(String, String)>,
) {
    let Some(expectation) = expectation else {
        return;
    };
    if let Some(reference) = expectation.get("diagnosticRef").and_then(Value::as_str) {
        refs.push((pointer.to_owned(), reference.to_owned()));
    }
    if let Some(children) = expectation.get("children").and_then(Value::as_array) {
        for (index, child) in children.iter().enumerate() {
            collect_refs_in_expectation(Some(child), &format!("{pointer}/children/{index}"), refs);
        }
    }
}

/// Every expectation node in the document, including the ones nested inside logical compositions.
fn collect_expectations<'a>(document: &'a Value, context: &str) -> Vec<(String, &'a Value)> {
    let mut expectations = Vec::new();

    for (test_index, test) in array(document, "tests", context).iter().enumerate() {
        for (assertion_index, assertion) in array(test, "assertions", context).iter().enumerate() {
            if let Some(expectation) = assertion.get("expectation") {
                collect_nested_expectations(
                    expectation,
                    format!("/tests/{test_index}/assertions/{assertion_index}/expectation"),
                    &mut expectations,
                );
            }
        }
    }

    expectations
}

fn collect_nested_expectations<'a>(
    expectation: &'a Value,
    pointer: String,
    expectations: &mut Vec<(String, &'a Value)>,
) {
    if let Some(children) = expectation.get("children").and_then(Value::as_array) {
        for (index, child) in children.iter().enumerate() {
            collect_nested_expectations(child, format!("{pointer}/children/{index}"), expectations);
        }
    }
    expectations.push((pointer, expectation));
}

// Each helper takes `context` for the same reason every assertion message above does: these tests
// loop over every fixture, so a malformed document has to say which fixture produced it.

fn array<'a>(value: &'a Value, key: &str, context: &str) -> &'a Vec<Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{context}: `{key}` must be an array"))
}

fn string<'a>(value: &'a Value, key: &str, context: &str) -> &'a str {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{context}: `{key}` must be a string"))
}

fn count(summary: &Value, key: &str, context: &str) -> u64 {
    summary
        .get(key)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("{context}: summary.{key} must be a non-negative integer"))
}

fn tests_with_status(tests: &[Value], status: &str) -> u64 {
    tests
        .iter()
        .filter(|test| test.get("status").and_then(Value::as_str) == Some(status))
        .count() as u64
}
