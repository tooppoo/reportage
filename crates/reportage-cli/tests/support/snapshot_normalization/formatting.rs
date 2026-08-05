//! Deterministic snapshot formatting: how a normalized document is written for comparison.
//!
//! A stage of its own, after normalization and never mixed into it. Normalization decides which
//! values a snapshot may not depend on; formatting decides how the result is spelled. Keeping them
//! apart is what lets a diff be read as one or the other — a changed value or a changed layout —
//! rather than as both at once.
//!
//! The rules are fixed by
//! docs/adr/20260723T160117Z_json-schema-driven-snapshot-normalization-foundation.md: object keys
//! sorted recursively, array order preserved, two-space indentation, LF, exactly one trailing
//! newline, numbers serialized by `serde_json` rather than as they were spelled in the input, and
//! UTF-8 without ASCII escaping beyond what JSON requires.
//!
//! This is deliberately not RFC 8785 canonical JSON. A snapshot is read and reviewed by people, so
//! it is pretty-printed; the project needs the output to be stable, not byte-canonical.

use serde_json::{Map, Value};

/// Formats `document` as the text a snapshot is compared against.
///
/// The output ends with exactly one newline: `serde_json` writes none, and a file whose last line
/// has no terminator is what makes every tool that appends one produce a spurious diff.
pub fn format_snapshot(document: &Value) -> String {
    let mut text = serde_json::to_string_pretty(&with_sorted_keys(document))
        .expect("a `Value` holds only string keys and finite numbers, so it always serializes");
    text.push('\n');
    text
}

/// Rebuilds `value` with every object's members in ascending key order, recursively.
///
/// Sorting here rather than leaving it to `serde_json::Map` is required, not a duplicate of what
/// the map already does: the normalization foundation asks that object ordering be made explicit,
/// which is to say a property of this code rather than of a dependency's choice of map. Concretely,
/// which map backs `Value::Object` is a cargo feature — `preserve_order` swaps the sorted
/// `BTreeMap` for an insertion-ordered `IndexMap` — and any normal or dev dependency of the crates
/// built together can enable it, which would otherwise reorder every snapshot in this repository.
///
/// Members are inserted in the order they must come out in, which is the order both backing maps
/// produce: a sorted map keeps it by construction, an insertion-ordered one by insertion.
///
/// Ascending is by Unicode scalar value, which is what `str` comparison is. It is not a locale
/// collation, so `Z` precedes `a` and `é` follows both; a snapshot must not read differently on a
/// machine with a different locale.
///
/// Array order is untouched. Position carries meaning in every array these documents hold — the
/// order tests ran, the order actions were taken — so sorting one would destroy what it records.
fn with_sorted_keys(value: &Value) -> Value {
    match value {
        Value::Object(members) => {
            let mut names: Vec<&String> = members.keys().collect();
            names.sort();
            let mut sorted = Map::with_capacity(names.len());
            for name in names {
                sorted.insert(name.clone(), with_sorted_keys(&members[name]));
            }
            Value::Object(sorted)
        }
        Value::Array(elements) => Value::Array(elements.iter().map(with_sorted_keys).collect()),
        scalar => scalar.clone(),
    }
}
