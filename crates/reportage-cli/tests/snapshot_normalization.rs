//! Schema preparation for JSON snapshot normalization, with static local `$ref` (issue #193).
//!
//! Two kinds of case appear here, and they establish different things:
//!
//! - hand-built schemas, which are how a rejected form can be exercised at all. The maintained
//!   contract schemas are deliberately inside the supported profile, so nothing they contain
//!   demonstrates that an unsupported reference, a cycle, or a nested `$id` is caught;
//! - the maintained contract schemas themselves, which are what the harness will actually prepare
//!   (issue #114) and are therefore the only cases that show the supported profile is wide enough
//!   for the documents it exists for.
//!
//! See docs/adr/20260729T182026Z_static-local-reference-resolution-for-snapshot-normalization.md.

use serde_json::{Value, json};

#[path = "support/json_schema.rs"]
mod json_schema;
#[path = "support/snapshot_normalization/mod.rs"]
mod snapshot_normalization;

use json_schema::{JSON_REPORT, RUN_RESULT, SchemaVariant};
use snapshot_normalization::{
    ANNOTATION_KEYWORD, InstanceSegment, NormalizationPlan, Operation, PreparationError,
    PreparationErrorKind, SchemaLocation, prepare, resolve,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A string schema annotated to be replaced by `placeholder`.
fn annotated(placeholder: &str) -> Value {
    json!({
        "type": "string",
        "x-reportage-snapshot": { "operation": "replace", "value": placeholder }
    })
}

/// A schema whose single property `x` is the reference under test.
///
/// Every rejected-reference case shares this shape so that the expected diagnostic location is the
/// same [`REFERENCE_LOCATION`] throughout and each case shows only the reference that is at issue.
fn schema_referring_to(reference: Value) -> Value {
    json!({
        "type": "object",
        "properties": { "x": { "$ref": reference } },
        "$defs": { "Tool": { "type": "object" } }
    })
}

/// Where [`schema_referring_to`] puts the reference under test.
const REFERENCE_LOCATION: &str = "/properties/x/$ref";

/// A JSON object built from member names that are not known at compile time.
fn object<'a>(members: impl IntoIterator<Item = (&'a str, Value)>) -> Value {
    Value::Object(
        members
            .into_iter()
            .map(|(name, value)| (name.to_string(), value))
            .collect(),
    )
}

fn prepared(schema: &Value) -> NormalizationPlan {
    prepare(schema).unwrap_or_else(|error| panic!("schema preparation must succeed: {error}"))
}

fn rejected(schema: &Value) -> PreparationError {
    match prepare(schema) {
        Ok(plan) => panic!(
            "schema preparation must fail, but it produced {} instruction(s)",
            plan.instructions().len()
        ),
        Err(error) => error,
    }
}

/// The plan as `(instance target, placeholder)` pairs, sorted so a case states what is normalized
/// without also pinning traversal order.
fn normalized(plan: &NormalizationPlan) -> Vec<(String, String)> {
    let mut pairs: Vec<(String, String)> = plan
        .instructions()
        .iter()
        .map(|instruction| {
            assert_eq!(
                instruction.operation(),
                Operation::Replace,
                "`replace` is the only operation this profile defines"
            );
            (
                instruction.target().to_string(),
                instruction.value().to_string(),
            )
        })
        .collect();
    pairs.sort();
    pairs
}

// ---------------------------------------------------------------------------
// Supported resolution
// ---------------------------------------------------------------------------

#[test]
fn a_reference_reaches_the_annotations_of_the_definition_it_names() {
    let schema = json!({
        "type": "object",
        "properties": { "tool": { "$ref": "#/$defs/Tool" } },
        "$defs": {
            "Tool": {
                "type": "object",
                "properties": { "name": { "type": "string" }, "version": annotated("<VERSION>") }
            }
        }
    });

    let plan = prepared(&schema);
    assert_eq!(
        normalized(&plan),
        vec![("/tool/version".to_string(), "<VERSION>".to_string())]
    );
    assert_eq!(
        plan.instructions()[0].source().as_pointer(),
        "/$defs/Tool/properties/version",
        "the instruction must point at the annotated definition, not at the referring property"
    );
}

#[test]
fn following_a_reference_does_not_move_the_instance_location() {
    // The annotation is on the referenced schema itself rather than below it, so the instruction's
    // target is whatever the reference alone produced.
    let schema = json!({
        "type": "object",
        "properties": { "version": { "$ref": "#/$defs/Version" } },
        "$defs": { "Version": annotated("<VERSION>") }
    });

    assert_eq!(
        normalized(&prepared(&schema)),
        vec![("/version".to_string(), "<VERSION>".to_string())]
    );
}

#[test]
fn annotations_are_reached_through_nested_objects_and_array_items() {
    let schema = json!({
        "type": "object",
        "properties": {
            "tests": {
                "type": "array",
                "items": { "$ref": "#/$defs/Test" }
            }
        },
        "$defs": {
            "Test": {
                "type": "object",
                "properties": {
                    "evidence": {
                        "type": "object",
                        "properties": { "path": annotated("<PATH>") }
                    }
                }
            }
        }
    });

    assert_eq!(
        normalized(&prepared(&schema)),
        vec![("/tests/*/evidence/path".to_string(), "<PATH>".to_string())]
    );
}

#[test]
fn an_array_element_target_is_a_structural_segment_not_a_pointer_token() {
    // The rendered `/tests/*/version` is a diagnostic form: `*` is a legal property name, so what
    // "every element of this array" means has to be readable from the value itself.
    let schema = json!({
        "type": "object",
        "properties": {
            "tests": { "type": "array", "items": { "$ref": "#/$defs/Test" } }
        },
        "$defs": {
            "Test": { "type": "object", "properties": { "version": annotated("<VERSION>") } }
        }
    });

    let plan = prepared(&schema);
    assert_eq!(
        plan.instructions()[0].target().segments(),
        [
            InstanceSegment::Property("tests".to_string()),
            InstanceSegment::ArrayElement,
            InstanceSegment::Property("version".to_string()),
        ]
    );
}

#[test]
fn one_definition_reused_across_locations_yields_one_instruction_per_location() {
    let schema = json!({
        "type": "object",
        "properties": {
            "tool": { "$ref": "#/$defs/Versioned" },
            "shim": { "$ref": "#/$defs/Versioned" },
            "plugins": { "type": "array", "items": { "$ref": "#/$defs/Versioned" } }
        },
        "$defs": {
            "Versioned": {
                "type": "object",
                "properties": { "version": annotated("<VERSION>") }
            }
        }
    });

    assert_eq!(
        normalized(&prepared(&schema)),
        vec![
            ("/plugins/*/version".to_string(), "<VERSION>".to_string()),
            ("/shim/version".to_string(), "<VERSION>".to_string()),
            ("/tool/version".to_string(), "<VERSION>".to_string()),
        ]
    );
}

#[test]
fn escaped_and_unicode_definition_names_resolve() {
    // `/` and `~` cannot appear raw in a JSON Pointer token, so a definition named with either is
    // only reachable through its escape; a non-ASCII name needs no encoding at all.
    //
    // `~01` is the case that pins the decoding order: decoding `~0` first would read the `~` it
    // produces as the start of another escape and resolve to the definition named `/` instead of
    // the one named `~1`. That wrong name is also defined, so a mis-decode normalizes the wrong
    // field rather than failing as unresolved. `~0~1` covers a name holding both characters.
    let schema = json!({
        "type": "object",
        "properties": {
            "slashed": { "$ref": "#/$defs/with~1slash" },
            "tilde": { "$ref": "#/$defs/with~0tilde" },
            "escaped_tilde_one": { "$ref": "#/$defs/~01" },
            "escaped_both": { "$ref": "#/$defs/~0~1" },
            "unicode": { "$ref": "#/$defs/バージョン" },
            "empty": { "$ref": "#/$defs/" }
        },
        "$defs": {
            "with/slash": annotated("<SLASH>"),
            "with~tilde": annotated("<TILDE>"),
            "~1": annotated("<TILDE_ONE>"),
            "/": annotated("<WRONG_TILDE_ONE>"),
            "~/": annotated("<TILDE_SLASH>"),
            "バージョン": annotated("<UNICODE>"),
            "": annotated("<EMPTY>")
        }
    });

    assert_eq!(
        normalized(&prepared(&schema)),
        vec![
            ("/empty".to_string(), "<EMPTY>".to_string()),
            ("/escaped_both".to_string(), "<TILDE_SLASH>".to_string()),
            ("/escaped_tilde_one".to_string(), "<TILDE_ONE>".to_string()),
            ("/slashed".to_string(), "<SLASH>".to_string()),
            ("/tilde".to_string(), "<TILDE>".to_string()),
            ("/unicode".to_string(), "<UNICODE>".to_string()),
        ]
    );
}

#[test]
fn an_escaped_definition_name_is_reported_re_encoded() {
    // The source location has to be a pointer into the document, so a name containing `/` or `~`
    // must be written back escaped rather than concatenated raw. Cycle identity compares these
    // locations, so an unescaped one would also make two different definitions look like one.
    for (reference, name, pointer) in [
        ("#/$defs/with~1slash", "with/slash", "/$defs/with~1slash"),
        ("#/$defs/~0~1", "~/", "/$defs/~0~1"),
    ] {
        let schema = json!({
            "type": "object",
            "properties": { "escaped": { "$ref": reference } },
            "$defs": object([(name, annotated("<PLACEHOLDER>"))])
        });

        assert_eq!(
            prepared(&schema).instructions()[0].source().as_pointer(),
            pointer
        );
    }
}

#[test]
fn a_boolean_definition_is_terminal_and_normalizes_nothing() {
    // `true` and `false` are schemas, so they are legitimate reference targets, but neither carries
    // annotations or subschemas. Whether anything can satisfy `false` is a validation question.
    let schema = json!({
        "type": "object",
        "properties": {
            "anything": { "$ref": "#/$defs/Anything" },
            "nothing": { "$ref": "#/$defs/Nothing" }
        },
        "$defs": { "Anything": true, "Nothing": false }
    });

    assert!(prepared(&schema).instructions().is_empty());
}

// ---------------------------------------------------------------------------
// Traversal boundary
// ---------------------------------------------------------------------------

#[test]
fn subtrees_below_unsupported_keywords_are_not_inspected() {
    // Normalization does not enter these keywords, so it never has to interpret what is inside
    // them. Rejecting the document over a form found only there would make normalization support a
    // constraint on parts of the contract normalization has no opinion about.
    let schema = json!({
        "type": "object",
        "properties": {
            "expectation": {
                "oneOf": [
                    { "$ref": "https://example.com/other.json#/$defs/Remote" },
                    { "$dynamicRef": "#meta" },
                    { "$id": "https://example.com/nested.json", "type": "object" },
                    { "$ref": "#/$defs/Tool", "description": "sibling" }
                ]
            },
            "extras": {
                "additionalProperties": { "$ref": "#/$defs/Cycle" },
                "patternProperties": { "^x-": annotated("<IGNORED>") }
            }
        },
        "$defs": {
            "Tool": { "type": "object" },
            "Cycle": { "properties": { "self": { "$ref": "#/$defs/Cycle" } } }
        }
    });

    assert!(
        prepared(&schema).instructions().is_empty(),
        "an annotation reachable only through an unsupported keyword is ignored, not collected"
    );
}

#[test]
fn definitions_no_supported_reference_reaches_are_not_inspected() {
    let schema = json!({
        "type": "object",
        "properties": { "tool": { "$ref": "#/$defs/Tool" } },
        "$defs": {
            "Tool": { "type": "object", "properties": { "version": annotated("<VERSION>") } },
            "Unreached": {
                "$id": "https://example.com/unreached.json",
                "$dynamicAnchor": "meta",
                "properties": {
                    "broken": { "$ref": "../elsewhere.json" },
                    "loop": { "$ref": "#/$defs/Unreached" }
                }
            }
        }
    });

    assert_eq!(
        normalized(&prepared(&schema)),
        vec![("/tool/version".to_string(), "<VERSION>".to_string())]
    );
}

#[test]
fn a_position_that_holds_no_schema_is_skipped_rather_than_rejected() {
    // The same value is an error when a reference resolves to it and merely skipped when traversal
    // descends into it. A reference must be checked, because it is how a definition can be named at
    // all; a keyword holding a non-schema is a malformed document, which contract validation
    // decides (issue #192) and normalization does not restate.
    let schema = json!({
        "type": "object",
        "properties": {
            "text": "not a schema",
            "tuple": { "type": "array", "items": [{ "type": "string" }] },
            "tool": { "$ref": "#/$defs/Tool" }
        },
        "$defs": { "Tool": { "type": "object", "properties": { "version": annotated("<V>") } } }
    });

    assert_eq!(
        normalized(&prepared(&schema)),
        vec![("/tool/version".to_string(), "<V>".to_string())]
    );
}

// ---------------------------------------------------------------------------
// Unsupported reference forms
// ---------------------------------------------------------------------------

#[test]
fn references_outside_the_supported_grammar_are_rejected() {
    // Each case is a spelling whose resolution would need machinery this profile does not have:
    // base-URI rebasing, external retrieval, plain-name anchors, deep pointers, or percent-decoding.
    let cases: &[(&str, &str)] = &[
        ("#", "the whole document, not a definition"),
        ("#tool", "a plain-name anchor fragment"),
        ("#/properties/x", "a pointer outside `$defs`"),
        ("#/$defs", "`$defs` itself rather than a definition"),
        ("#/$defs/Tool/properties/name", "deeper than a direct entry"),
        ("#/definitions/Tool", "the Draft 7 definitions keyword"),
        ("https://example.com/schema.json#/$defs/Tool", "remote"),
        ("other.json#/$defs/Tool", "an external file"),
        ("./other.json", "a relative path"),
        ("#/$defs/Tool%20Name", "percent-encoded"),
        ("#/$defs/%23", "percent-encoded"),
        ("#/$defs/Tool~2Name", "a malformed escape"),
        ("#/$defs/Tool~", "a truncated escape"),
    ];

    for (reference, why) in cases {
        let error = rejected(&schema_referring_to(json!(reference)));
        assert_eq!(
            error.kind(),
            PreparationErrorKind::UnsupportedReferenceForm,
            "`{reference}` is {why} and must be rejected as an unsupported form: {error}"
        );
        assert_eq!(error.location().as_pointer(), REFERENCE_LOCATION);
        assert_eq!(
            error.value(),
            Some(&json!(reference)),
            "the diagnostic must quote the reference that was rejected"
        );
    }
}

#[test]
fn a_non_string_reference_is_rejected() {
    for reference in [json!(42), json!(null), json!(["#/$defs/Tool"]), json!({})] {
        let error = rejected(&schema_referring_to(reference.clone()));
        assert_eq!(error.kind(), PreparationErrorKind::NonStringReference);
        assert_eq!(error.location().as_pointer(), REFERENCE_LOCATION);
        assert_eq!(
            error.value(),
            Some(&reference),
            "a non-string value must still appear in the diagnostic"
        );
    }
}

#[test]
fn a_definitions_container_that_is_not_an_object_is_rejected() {
    // Distinct from "no such definition": there is nowhere to look one up, which is a defect in the
    // document rather than in the reference.
    for container in [json!([]), json!("definitions"), json!(null)] {
        let schema = json!({
            "type": "object",
            "properties": { "x": { "$ref": "#/$defs/Tool" } },
            "$defs": container
        });
        let error = rejected(&schema);
        assert_eq!(
            error.kind(),
            PreparationErrorKind::InvalidReferenceContainer
        );
        assert_eq!(error.location().as_pointer(), REFERENCE_LOCATION);
        assert_eq!(error.value(), Some(&json!("#/$defs/Tool")));
    }
}

#[test]
fn a_reference_to_a_missing_definition_is_rejected() {
    let without_defs = json!({
        "type": "object",
        "properties": { "x": { "$ref": "#/$defs/Tool" } }
    });
    let without_member = json!({
        "type": "object",
        "properties": { "x": { "$ref": "#/$defs/Missing" } },
        "$defs": { "Tool": { "type": "object" } }
    });

    for (schema, reference) in [
        (without_defs, "#/$defs/Tool"),
        (without_member, "#/$defs/Missing"),
    ] {
        let error = rejected(&schema);
        assert_eq!(error.kind(), PreparationErrorKind::UnresolvedReference);
        assert_eq!(error.location().as_pointer(), REFERENCE_LOCATION);
        assert_eq!(error.value(), Some(&json!(reference)));
    }
}

#[test]
fn a_reference_that_resolves_to_something_other_than_a_schema_is_rejected() {
    // The type check is what stops a reference from targeting an arbitrary JSON object — a
    // `properties` map, an annotation — which would then be walked as though it were a schema.
    for target in [
        json!("not a schema"),
        json!(7),
        json!(null),
        json!([{ "type": "object" }]),
    ] {
        let schema = json!({
            "type": "object",
            "properties": { "x": { "$ref": "#/$defs/Tool" } },
            "$defs": { "Tool": target }
        });
        let error = rejected(&schema);
        assert_eq!(error.kind(), PreparationErrorKind::InvalidResolvedTarget);
        assert_eq!(error.location().as_pointer(), REFERENCE_LOCATION);
        assert_eq!(error.value(), Some(&json!("#/$defs/Tool")));
    }
}

// ---------------------------------------------------------------------------
// Reached-node compatibility
// ---------------------------------------------------------------------------

#[test]
fn a_root_identifier_is_accepted() {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://example.com/schema.json",
        "type": "object",
        "properties": { "version": annotated("<VERSION>") }
    });

    assert_eq!(
        normalized(&prepared(&schema)),
        vec![("/version".to_string(), "<VERSION>".to_string())]
    );
}

#[test]
fn an_identifier_below_the_root_is_rejected() {
    let schema = json!({
        "$id": "https://example.com/schema.json",
        "type": "object",
        "properties": { "tool": { "$ref": "#/$defs/Tool" } },
        "$defs": {
            "Tool": { "$id": "https://example.com/tool.json", "type": "object" }
        }
    });

    let error = rejected(&schema);
    assert_eq!(error.kind(), PreparationErrorKind::NestedIdentifier);
    assert_eq!(error.location().as_pointer(), "/$defs/Tool/$id");
    assert_eq!(error.value(), Some(&json!("https://example.com/tool.json")));
}

#[test]
fn dynamic_references_and_anchors_are_rejected_where_traversal_reaches_them() {
    for keyword in ["$dynamicRef", "$dynamicAnchor"] {
        let schema = json!({
            "type": "object",
            "properties": { "x": object([(keyword, json!("#meta")), ("type", json!("object"))]) }
        });
        let error = rejected(&schema);
        assert_eq!(error.kind(), PreparationErrorKind::DynamicReference);
        assert_eq!(
            error.location().as_pointer(),
            format!("/properties/x/{keyword}")
        );
        assert_eq!(error.value(), Some(&json!("#meta")));
    }
}

#[test]
fn a_reference_object_with_any_other_member_is_rejected() {
    // Siblings are neither ignored nor merged: in Draft 2020-12 they are evaluated alongside the
    // referenced schema, so ignoring one would silently drop it. This is why an annotation belongs
    // in the referenced definition rather than beside the reference.
    let siblings: &[(&str, Value)] = &[
        ("type", json!("object")),
        ("description", json!("what this is")),
        ("$comment", json!("a note")),
        (
            "x-reportage-snapshot",
            json!({ "operation": "replace", "value": "<V>" }),
        ),
        ("unknownKeyword", json!(true)),
    ];

    for (sibling, value) in siblings {
        let schema = json!({
            "type": "object",
            "properties": {
                "x": object([("$ref", json!("#/$defs/Tool")), (sibling, value.clone())])
            },
            "$defs": { "Tool": { "type": "object" } }
        });
        let error = rejected(&schema);
        assert_eq!(
            error.kind(),
            PreparationErrorKind::ReferenceSibling,
            "`{sibling}` beside `$ref` must be rejected: {error}"
        );
        assert_eq!(error.location().as_pointer(), REFERENCE_LOCATION);
        assert!(
            error.to_string().contains(sibling),
            "the diagnostic must name the sibling that was found: {error}"
        );
    }
}

#[test]
fn the_document_root_may_not_carry_an_identifier_beside_a_reference() {
    // The root `$id` allowance is about where a document declares its own identity, not an
    // exemption from the sibling rule. `$defs` is left out so `$id` is the only sibling there is,
    // which is what makes the diagnostic naming `$id` mean the rule did not exempt it.
    let schema = json!({
        "$id": "https://example.com/schema.json",
        "$ref": "#/$defs/Root"
    });

    let error = rejected(&schema);
    assert_eq!(error.kind(), PreparationErrorKind::ReferenceSibling);
    assert_eq!(error.location().as_pointer(), "/$ref");
    assert!(
        error.to_string().contains("$id"),
        "the diagnostic must name `$id` as the sibling that was found: {error}"
    );
}

#[test]
fn a_sibling_is_reported_as_a_sibling_rather_than_as_the_keyword_it_is() {
    // Rule order is a contract, not an incidental: in a reference object the other members have no
    // agreed meaning here, so `$id` and `$dynamicRef` beside `$ref` are sibling defects. Reporting
    // either as a nested `$id` or a dynamic reference would name a repair that does not apply.
    for keyword in ["$id", "$dynamicRef"] {
        let schema = json!({
            "type": "object",
            "properties": {
                "x": object([
                    ("$ref", json!("#/$defs/Tool")),
                    (keyword, json!("https://example.com/tool.json")),
                ])
            },
            "$defs": { "Tool": { "type": "object" } }
        });

        let error = rejected(&schema);
        assert_eq!(
            error.kind(),
            PreparationErrorKind::ReferenceSibling,
            "`{keyword}` beside `$ref` is a sibling defect: {error}"
        );
        assert_eq!(error.location().as_pointer(), REFERENCE_LOCATION);
    }
}

#[test]
fn a_tuple_prefixed_array_is_rejected() {
    // `items` is traversed as describing every element. With `prefixItems` present it describes
    // only the elements after the prefix, so an annotation below it would be collected for
    // positions it does not describe. Required by the normalization foundation ADR.
    let schema = json!({
        "type": "object",
        "properties": {
            "pair": {
                "type": "array",
                "prefixItems": [{ "type": "string" }],
                "items": annotated("<REST>")
            }
        }
    });

    let error = rejected(&schema);
    assert_eq!(error.kind(), PreparationErrorKind::TupleItems);
    assert_eq!(
        error.location().as_pointer(),
        "/properties/pair/prefixItems"
    );
}

// ---------------------------------------------------------------------------
// Cycles
// ---------------------------------------------------------------------------

#[test]
fn a_direct_self_cycle_is_rejected() {
    let schema = json!({
        "type": "object",
        "properties": { "node": { "$ref": "#/$defs/Node" } },
        "$defs": {
            "Node": {
                "type": "object",
                "properties": { "next": { "$ref": "#/$defs/Node" } }
            }
        }
    });

    let error = rejected(&schema);
    assert_eq!(error.kind(), PreparationErrorKind::ReferenceCycle);
    assert_eq!(
        error.location().as_pointer(),
        "/$defs/Node/properties/next/$ref",
        "the location must be the reference that closed the cycle"
    );

    let cycle = error
        .cycle_detail()
        .expect("a cycle error must carry its cycle detail");
    assert_eq!(cycle.start().as_pointer(), "/$defs/Node");
    let chain: Vec<(String, String)> = cycle
        .chain()
        .iter()
        .map(|step| (step.reference().as_pointer(), step.target().as_pointer()))
        .collect();
    assert_eq!(
        chain,
        vec![(
            "/properties/node/$ref".to_string(),
            "/$defs/Node".to_string()
        )],
        "the chain must show how the active expansion was entered"
    );
}

#[test]
fn indirect_cycles_are_rejected() {
    let two_node = json!({
        "type": "object",
        "properties": { "a": { "$ref": "#/$defs/A" } },
        "$defs": {
            "A": { "properties": { "b": { "$ref": "#/$defs/B" } } },
            "B": { "properties": { "a": { "$ref": "#/$defs/A" } } }
        }
    });
    let three_node = json!({
        "type": "object",
        "properties": { "a": { "$ref": "#/$defs/A" } },
        "$defs": {
            "A": { "properties": { "b": { "$ref": "#/$defs/B" } } },
            "B": { "properties": { "c": { "$ref": "#/$defs/C" } } },
            "C": { "properties": { "a": { "$ref": "#/$defs/A" } } }
        }
    });

    for (schema, closing, chain_length) in [
        (two_node, "/$defs/B/properties/a/$ref", 2),
        (three_node, "/$defs/C/properties/a/$ref", 3),
    ] {
        let error = rejected(&schema);
        assert_eq!(error.kind(), PreparationErrorKind::ReferenceCycle);
        assert_eq!(error.location().as_pointer(), closing);

        let cycle = error.cycle_detail().expect("a cycle error carries detail");
        assert_eq!(cycle.start().as_pointer(), "/$defs/A");
        assert_eq!(
            cycle.chain().len(),
            chain_length,
            "the chain must hold every expansion that was still active"
        );
        assert_eq!(
            cycle.chain()[0].target().as_pointer(),
            "/$defs/A",
            "the chain is ordered outermost first"
        );
    }
}

#[test]
fn a_cycle_that_starts_below_the_outermost_expansion_reports_where_it_starts() {
    // `A` is expanded on the way in but is not part of the loop: the cycle is `B` -> `C` -> `B`.
    // The reported start is the target that was re-entered, not simply the first active expansion.
    let schema = json!({
        "type": "object",
        "properties": { "a": { "$ref": "#/$defs/A" } },
        "$defs": {
            "A": { "properties": { "b": { "$ref": "#/$defs/B" } } },
            "B": { "properties": { "c": { "$ref": "#/$defs/C" } } },
            "C": { "properties": { "b": { "$ref": "#/$defs/B" } } }
        }
    });

    let error = rejected(&schema);
    let cycle = error.cycle_detail().expect("a cycle error carries detail");
    assert_eq!(error.location().as_pointer(), "/$defs/C/properties/b/$ref");
    assert_eq!(cycle.start().as_pointer(), "/$defs/B");
    assert_eq!(
        cycle
            .chain()
            .iter()
            .map(|step| step.target().as_pointer())
            .collect::<Vec<_>>(),
        ["/$defs/A", "/$defs/B", "/$defs/C"],
        "the chain must hold the whole active expansion, including the part before the cycle"
    );
}

#[test]
fn a_definition_reached_twice_without_nesting_is_not_a_cycle() {
    // Cycle identity is "still being expanded", not "seen before": a definition used by two
    // properties of the same wrapper, and again further down, terminates every time.
    let schema = json!({
        "type": "object",
        "properties": { "outer": { "$ref": "#/$defs/Wrapper" } },
        "$defs": {
            "Wrapper": {
                "type": "object",
                "properties": {
                    "left": { "$ref": "#/$defs/Leaf" },
                    "right": { "$ref": "#/$defs/Leaf" },
                    "nested": {
                        "type": "object",
                        "properties": { "deep": { "$ref": "#/$defs/Leaf" } }
                    }
                }
            },
            "Leaf": { "type": "object", "properties": { "version": annotated("<VERSION>") } }
        }
    });

    assert_eq!(
        normalized(&prepared(&schema)),
        vec![
            ("/outer/left/version".to_string(), "<VERSION>".to_string()),
            (
                "/outer/nested/deep/version".to_string(),
                "<VERSION>".to_string()
            ),
            ("/outer/right/version".to_string(), "<VERSION>".to_string()),
        ]
    );
}

// ---------------------------------------------------------------------------
// Annotations reached through references
// ---------------------------------------------------------------------------

#[test]
fn a_malformed_annotation_in_a_referenced_definition_is_rejected() {
    // Strictness is the point: JSON Schema ignores unknown keywords, so a misspelled annotation
    // member would otherwise leave a volatile value in the snapshot with nothing reporting it.
    let cases: &[(&str, Value)] = &[
        ("not an object", json!("replace")),
        (
            "an unknown member",
            json!({ "operation": "replace", "value": "<V>", "when": "always" }),
        ),
        ("no operation", json!({ "value": "<V>" })),
        ("no value", json!({ "operation": "replace" })),
        (
            "an unknown operation",
            json!({ "operation": "remove", "value": "<V>" }),
        ),
        (
            "a non-string value",
            json!({ "operation": "replace", "value": 1 }),
        ),
    ];

    for (why, annotation) in cases {
        let schema = json!({
            "type": "object",
            "properties": { "tool": { "$ref": "#/$defs/Tool" } },
            "$defs": { "Tool": { "type": "string", "x-reportage-snapshot": annotation } }
        });
        let error = rejected(&schema);
        assert_eq!(
            error.kind(),
            PreparationErrorKind::InvalidAnnotation,
            "an annotation with {why} must be rejected: {error}"
        );
        assert!(
            error
                .location()
                .as_pointer()
                .starts_with("/$defs/Tool/x-reportage-snapshot"),
            "the diagnostic must point into the annotation that has to be edited: {error}"
        );
    }
}

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

#[test]
fn every_defect_class_is_reported_under_its_own_classification() {
    // One case per classification, so a caller can act on the classification instead of matching
    // message text, and so no two defects collapse into the same answer.
    let cases: &[(PreparationErrorKind, Value)] = &[
        (
            PreparationErrorKind::NonStringReference,
            schema_referring_to(json!(42)),
        ),
        (
            PreparationErrorKind::UnsupportedReferenceForm,
            schema_referring_to(json!("#/definitions/Tool")),
        ),
        (
            PreparationErrorKind::InvalidReferenceContainer,
            json!({
                "properties": { "x": { "$ref": "#/$defs/Tool" } },
                "$defs": []
            }),
        ),
        (
            PreparationErrorKind::UnresolvedReference,
            schema_referring_to(json!("#/$defs/Missing")),
        ),
        (
            PreparationErrorKind::InvalidResolvedTarget,
            json!({
                "properties": { "x": { "$ref": "#/$defs/Tool" } },
                "$defs": { "Tool": "text" }
            }),
        ),
        (
            PreparationErrorKind::NestedIdentifier,
            json!({
                "properties": { "x": { "$ref": "#/$defs/Tool" } },
                "$defs": { "Tool": { "$id": "https://example.com/tool.json" } }
            }),
        ),
        (
            PreparationErrorKind::DynamicReference,
            json!({ "properties": { "x": { "$dynamicRef": "#meta" } } }),
        ),
        (
            PreparationErrorKind::ReferenceSibling,
            json!({
                "properties": { "x": { "$ref": "#/$defs/Tool", "type": "object" } },
                "$defs": { "Tool": { "type": "object" } }
            }),
        ),
        (
            PreparationErrorKind::TupleItems,
            json!({ "properties": { "x": { "prefixItems": [true], "items": true } } }),
        ),
        (
            PreparationErrorKind::ReferenceCycle,
            json!({
                "properties": { "x": { "$ref": "#/$defs/Loop" } },
                "$defs": { "Loop": { "properties": { "self": { "$ref": "#/$defs/Loop" } } } }
            }),
        ),
        (
            PreparationErrorKind::InvalidAnnotation,
            json!({ "properties": { "x": { "x-reportage-snapshot": "replace" } } }),
        ),
    ];

    for (expected, schema) in cases {
        let error = rejected(schema);
        assert_eq!(error.kind(), *expected, "{error}");
        assert!(
            error.to_string().contains(expected.label()),
            "a rendered diagnostic must name its classification: {error}"
        );
    }

    let covered: Vec<PreparationErrorKind> = cases.iter().map(|(kind, _)| *kind).collect();
    for kind in PreparationErrorKind::ALL {
        assert!(
            covered.contains(&kind),
            "no case produces the `{kind}` classification, so nothing shows it is reachable"
        );
    }
}

#[test]
fn the_classification_inventory_lists_every_variant_once_in_declaration_order() {
    // Without this, a classification left out of `ALL` would make the coverage loop above pass by
    // not asking about it. Mirrors `DiagnosticCode::ALL`'s guard in reportage-core.
    for (index, kind) in PreparationErrorKind::ALL.iter().enumerate() {
        assert_eq!(
            *kind as usize, index,
            "PreparationErrorKind::ALL is out of sync at index {index} ({kind})"
        );
    }
}

#[test]
fn the_document_root_has_the_empty_pointer_as_its_location() {
    // RFC 6901 spells the whole document as the empty string; the human-facing rendering says so in
    // words instead, so a diagnostic about the root does not look like one with a field missing.
    let error = rejected(&json!({ "x-reportage-snapshot": "replace" }));
    assert_eq!(error.location().as_pointer(), "/x-reportage-snapshot");
    assert_eq!(SchemaLocation::root().as_pointer(), "");
    assert_eq!(SchemaLocation::root().to_string(), "<document root>");
}

// ---------------------------------------------------------------------------
// Resolver independence
// ---------------------------------------------------------------------------

#[test]
fn the_resolver_answers_the_same_way_wherever_it_is_called_from() {
    // The resolver takes no traversal state and keeps none, so the definition a reference names is
    // a function of the document alone. Instance-location-dependent results must come from the
    // collector, which is what makes reuse across locations correct.
    let root = json!({
        "$defs": { "Tool": { "type": "object", "properties": { "version": annotated("<V>") } } }
    });
    let reference = json!("#/$defs/Tool");

    let first = resolve(
        &root,
        &reference,
        &SchemaLocation::root()
            .child("properties")
            .child("a")
            .child("$ref"),
    )
    .expect("a supported reference resolves");
    let second = resolve(
        &root,
        &reference,
        &SchemaLocation::root()
            .child("properties")
            .child("b")
            .child("items")
            .child("$ref"),
    )
    .expect("a supported reference resolves");

    assert_eq!(first.location(), second.location());
    assert_eq!(first.target(), second.target());
    assert_eq!(first.location().as_pointer(), "/$defs/Tool");
}

#[test]
fn the_resolver_does_not_decide_whether_a_reference_is_a_cycle() {
    // Cycle detection needs the active expansion stack, which only the collector has. Resolving the
    // self-reference of a recursive definition therefore succeeds; the collector is what rejects
    // the document.
    let root = json!({
        "$defs": { "Node": { "properties": { "next": { "$ref": "#/$defs/Node" } } } }
    });

    let resolved = resolve(
        &root,
        &json!("#/$defs/Node"),
        &SchemaLocation::root()
            .child("$defs")
            .child("Node")
            .child("properties")
            .child("next")
            .child("$ref"),
    )
    .expect("the resolver reports only whether the reference names a schema");
    assert_eq!(resolved.location().as_pointer(), "/$defs/Node");
}

// ---------------------------------------------------------------------------
// The maintained contract schemas
// ---------------------------------------------------------------------------

#[test]
fn the_maintained_contract_schemas_prepare_to_the_annotations_they_carry() {
    // These documents are the reason the reference profile exists: both reach their annotations
    // only through `$ref` into `$defs`, both declare a root `$id`, and both use applicator keywords
    // the traversal must step over rather than reject.
    let contracts = [
        (
            &*JSON_REPORT,
            vec![
                ("/artifactRoot", "<ARTIFACT_ROOT>"),
                ("/tool/version", "<VERSION>"),
            ],
        ),
        (&*RUN_RESULT, vec![("/tool/version", "<VERSION>")]),
    ];

    for (contract, expected) in contracts {
        let path = contract.path(SchemaVariant::InternalSource);
        let plan =
            prepare(contract.document(SchemaVariant::InternalSource)).unwrap_or_else(|error| {
                panic!(
                    "the {} internal source schema ({path}) must prepare: {error}",
                    contract.name()
                )
            });
        let expected: Vec<(String, String)> = expected
            .into_iter()
            .map(|(target, placeholder)| (target.to_string(), placeholder.to_string()))
            .collect();
        assert_eq!(
            normalized(&plan),
            expected,
            "{path} normalizes a different set of values than expected; if a field was annotated or \
             an annotation moved, update this expectation together with the snapshot policy"
        );
    }
}

#[test]
fn every_annotation_in_the_maintained_contract_schemas_is_reachable() {
    // An annotation the traversal cannot reach — under `oneOf`, under `patternProperties`, or in an
    // unreferenced definition — is ignored by design, and for an arbitrary schema that is the
    // decided behavior. For these two documents it would be a defect: the annotation would look
    // applied while the volatile value stayed in the snapshot, with nothing saying why. Nothing
    // else notices, because `just schema-artifacts-check` checks where annotations are, not whether
    // normalization gets to them.
    for contract in [&*JSON_REPORT, &*RUN_RESULT] {
        let path = contract.path(SchemaVariant::InternalSource);
        let document = contract.document(SchemaVariant::InternalSource);
        let plan = prepare(document).unwrap_or_else(|error| {
            panic!("the internal source schema {path} must prepare: {error}")
        });

        let reached: Vec<String> = plan
            .instructions()
            .iter()
            .map(|instruction| instruction.source().child(ANNOTATION_KEYWORD).as_pointer())
            .collect();
        let present = annotation_pointers(document);
        assert!(
            !present.is_empty(),
            "{path} is expected to carry annotations; if they moved, this test no longer proves anything"
        );
        for pointer in present {
            assert!(
                reached.contains(&pointer),
                "{path} carries an annotation at {pointer} that normalization traversal never reaches"
            );
        }
    }
}

/// Every `x-reportage-snapshot` member in `document`, as RFC 6901 pointers.
///
/// Deliberately a whole-document walk rather than the traversal under test: it has to see the
/// annotations the traversal would miss.
fn annotation_pointers(document: &Value) -> Vec<String> {
    fn walk(value: &Value, pointer: String, found: &mut Vec<String>) {
        match value {
            Value::Object(members) => {
                for (name, member) in members {
                    let child = format!("{pointer}/{}", name.replace('~', "~0").replace('/', "~1"));
                    if name == ANNOTATION_KEYWORD {
                        found.push(child);
                    } else {
                        walk(member, child, found);
                    }
                }
            }
            Value::Array(elements) => {
                for (index, element) in elements.iter().enumerate() {
                    walk(element, format!("{pointer}/{index}"), found);
                }
            }
            _ => {}
        }
    }

    let mut found = Vec::new();
    walk(document, String::new(), &mut found);
    found
}
