//! Deterministic snapshot formatting (issue #114).
//!
//! Its own test target, like the two normalization phases, because formatting answers a different
//! question from either: not which values a snapshot may depend on, but how the result is written.
//!
//! Most cases pin exact output text. Formatting is a contract about bytes — indentation, line
//! endings, the final newline — and an assertion over a parsed value cannot see any of them.
//!
//! See docs/adr/20260723T160117Z_json-schema-driven-snapshot-normalization-foundation.md.

use serde_json::{Value, json};

#[path = "support/snapshot_normalization/mod.rs"]
mod snapshot_normalization;

use snapshot_normalization::format_snapshot;

/// Parses `text` as the snapshot reader would.
fn parsed(text: &str) -> Value {
    serde_json::from_str(text)
        .unwrap_or_else(|error| panic!("formatted output must parse: {error}"))
}

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

#[test]
fn the_output_is_two_space_indented_with_lf_and_one_trailing_newline() {
    let formatted = format_snapshot(&json!({
        "tool": { "name": "reportage" },
        "tests": [{ "status": "passed" }]
    }));

    assert_eq!(
        formatted,
        concat!(
            "{\n",
            "  \"tests\": [\n",
            "    {\n",
            "      \"status\": \"passed\"\n",
            "    }\n",
            "  ],\n",
            "  \"tool\": {\n",
            "    \"name\": \"reportage\"\n",
            "  }\n",
            "}\n",
        )
    );
    assert!(
        !formatted.contains('\r'),
        "line endings must be LF, so a snapshot file does not depend on the platform that wrote it"
    );
}

#[test]
fn the_output_ends_with_exactly_one_newline() {
    // One, so a file whose last line has no terminator does not give every tool that appends one a
    // spurious diff; not two, because the second is invisible in review and survives forever.
    for document in [json!({}), json!([]), json!("text"), json!(0), json!(null)] {
        let formatted = format_snapshot(&document);
        assert!(formatted.ends_with('\n'), "{formatted:?}");
        assert!(!formatted.ends_with("\n\n"), "{formatted:?}");
    }
}

#[test]
fn empty_objects_and_arrays_stay_on_one_line() {
    assert_eq!(
        format_snapshot(&json!({ "diagnostics": [], "summary": {} })),
        "{\n  \"diagnostics\": [],\n  \"summary\": {}\n}\n"
    );
}

// ---------------------------------------------------------------------------
// Ordering
// ---------------------------------------------------------------------------

#[test]
fn object_keys_are_sorted_at_every_depth() {
    let formatted = format_snapshot(&json!({
        "tool": { "version": "0.0.7", "name": "reportage" },
        "artifactRoot": "<ARTIFACT_ROOT>",
        "tests": [{ "status": "passed", "id": "t1" }]
    }));

    assert_eq!(
        formatted,
        concat!(
            "{\n",
            "  \"artifactRoot\": \"<ARTIFACT_ROOT>\",\n",
            "  \"tests\": [\n",
            "    {\n",
            "      \"id\": \"t1\",\n",
            "      \"status\": \"passed\"\n",
            "    }\n",
            "  ],\n",
            "  \"tool\": {\n",
            "    \"name\": \"reportage\",\n",
            "    \"version\": \"0.0.7\"\n",
            "  }\n",
            "}\n",
        ),
        "keys must be sorted inside arrays and inside nested objects, not only at the root"
    );
}

#[test]
fn keys_are_ordered_by_code_point_and_not_by_a_locale_collation() {
    // `Z` before `a` because uppercase ASCII sorts first, `é` after both because it is above ASCII.
    // A locale-aware collation would interleave them, and a snapshot must not read differently on a
    // machine configured differently.
    let formatted = format_snapshot(&json!({ "a": 1, "é": 2, "Z": 3, "B": 4 }));

    assert_eq!(
        formatted,
        "{\n  \"B\": 4,\n  \"Z\": 3,\n  \"a\": 1,\n  \"é\": 2\n}\n"
    );
}

#[test]
fn array_order_is_preserved() {
    // Position is the record in every array these documents hold: the order tests ran, the order
    // actions were taken. Sorting one would destroy what it says.
    let document = json!({ "tests": ["c", "a", "b"], "summary": [3, 1, 2] });

    assert_eq!(
        parsed(&format_snapshot(&document)),
        document,
        "no array may be reordered"
    );
    assert!(format_snapshot(&document).contains("\"c\",\n    \"a\",\n    \"b\""));
}

// ---------------------------------------------------------------------------
// Values
// ---------------------------------------------------------------------------

#[test]
fn numbers_are_written_by_serde_json_and_not_as_they_were_spelled() {
    // The lexical form of a number is not part of what a snapshot pins: `1e3` and `1000.0` are the
    // same value, and a document is only ever compared after being parsed and written back.
    let document =
        parsed(r#"{ "exponent": 1e3, "trailing": 1.50, "integer": 7, "big": 9007199254740993 }"#);

    assert_eq!(
        format_snapshot(&document),
        concat!(
            "{\n",
            "  \"big\": 9007199254740993,\n",
            "  \"exponent\": 1000.0,\n",
            "  \"integer\": 7,\n",
            "  \"trailing\": 1.5\n",
            "}\n",
        )
    );
}

#[test]
fn text_above_ascii_is_written_as_utf8_rather_than_escaped() {
    // Escaping would make a snapshot unreadable exactly where a human most needs to read it: a
    // diagnostic message or a captured stream in a language other than English.
    let formatted = format_snapshot(&json!({ "message": "検証に失敗しました ✅" }));

    assert_eq!(
        formatted,
        "{\n  \"message\": \"検証に失敗しました ✅\"\n}\n"
    );
    assert!(!formatted.contains("\\u"));
}

#[test]
fn what_json_requires_escaping_is_still_escaped() {
    // The rule is "no ASCII escaping beyond what JSON requires", not "no escaping": a control
    // character written raw would not parse.
    assert_eq!(
        format_snapshot(&json!({ "captured": "line\ttab\nnewline \"quoted\" \\ \u{1}" })),
        "{\n  \"captured\": \"line\\ttab\\nnewline \\\"quoted\\\" \\\\ \\u0001\"\n}\n"
    );
}

// ---------------------------------------------------------------------------
// Properties of the whole
// ---------------------------------------------------------------------------

#[test]
fn formatting_changes_nothing_but_the_writing() {
    // Reordering members is not editing the document: a JSON object is unordered, so the value read
    // back is the value that went in. This is what keeps formatting a stage that decides nothing
    // about content, and normalization the only stage that does.
    let document = json!({
        "tool": { "version": "<VERSION>", "name": "reportage" },
        "tests": [{ "id": "t2", "actions": [] }, { "id": "t1", "actions": [{ "exitCode": 0 }] }],
        "noop": false,
        "location": null
    });

    assert_eq!(parsed(&format_snapshot(&document)), document);
}

#[test]
fn formatting_is_idempotent() {
    // A snapshot is written by one run and compared by the next. If formatting its own output could
    // change it, a suite could fail against a snapshot it had just written.
    let document = json!({
        "b": [{ "z": 1, "a": [2, 1] }],
        "a": { "": "empty key", "~/": "escaped in a pointer, plain here" }
    });

    let once = format_snapshot(&document);
    assert_eq!(format_snapshot(&parsed(&once)), once);
}
