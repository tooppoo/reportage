//! Instance processing for JSON snapshot normalization (issue #114).
//!
//! Separate from `snapshot_normalization.rs` because the two phases fail for different reasons: a
//! failure here is about a document, a failure there is about a schema, and which of the two a
//! suite is looking at should be readable from the test target that reports it.
//!
//! Plans are prepared from small hand-built schemas rather than assembled instruction by
//! instruction, so that what is applied is what the documented traversal actually produces. The
//! maintained contract schemas are exercised the same way at the end, over documents shaped like
//! the ones the harness will normalize.
//!
//! See docs/adr/20260723T160117Z_json-schema-driven-snapshot-normalization-foundation.md.

use serde_json::{Value, json};

#[path = "support/json_schema.rs"]
mod json_schema;
#[path = "support/snapshot_normalization/mod.rs"]
mod snapshot_normalization;

use json_schema::{JSON_REPORT, RUN_RESULT, SchemaVariant};
use snapshot_normalization::{
    ApplicationError, ApplicationErrorKind, InstanceToken, NormalizationPlan, apply, prepare,
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

fn prepared(schema: &Value) -> NormalizationPlan {
    prepare(schema).unwrap_or_else(|error| panic!("schema preparation must succeed: {error}"))
}

fn normalized(schema: &Value, document: Value) -> Value {
    apply(&prepared(schema), document)
        .unwrap_or_else(|error| panic!("normalization must succeed: {error}"))
}

fn rejected(schema: &Value, document: Value) -> ApplicationError {
    match apply(&prepared(schema), document) {
        Ok(document) => panic!("normalization must fail, but it produced {document}"),
        Err(error) => error,
    }
}

/// A schema whose `tool.version` is annotated, the shape both contract schemas use.
fn versioned_tool() -> Value {
    json!({
        "type": "object",
        "properties": {
            "tool": { "type": "object", "properties": { "version": annotated("<VERSION>") } }
        }
    })
}

// ---------------------------------------------------------------------------
// Replacement
// ---------------------------------------------------------------------------

#[test]
fn an_annotated_root_is_replaced() {
    assert_eq!(
        normalized(&annotated("<VERSION>"), json!("0.0.7")),
        json!("<VERSION>")
    );
}

#[test]
fn annotated_values_are_replaced_through_properties_arrays_and_both_at_once() {
    let schema = json!({
        "type": "object",
        "properties": {
            "artifactRoot": annotated("<ARTIFACT_ROOT>"),
            "tool": { "type": "object", "properties": { "version": annotated("<VERSION>") } },
            "runs": { "type": "array", "items": annotated("<RUN>") },
            "tests": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "evidence": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": { "path": annotated("<PATH>") }
                            }
                        }
                    }
                }
            }
        }
    });
    let document = json!({
        "artifactRoot": "/tmp/run-1",
        "tool": { "name": "reportage", "version": "0.0.7" },
        "runs": ["a", "b"],
        "tests": [
            { "evidence": [{ "path": "x", "size": 1 }, { "path": "y", "size": 2 }] },
            { "evidence": [] }
        ]
    });

    assert_eq!(
        normalized(&schema, document),
        json!({
            "artifactRoot": "<ARTIFACT_ROOT>",
            "tool": { "name": "reportage", "version": "<VERSION>" },
            "runs": ["<RUN>", "<RUN>"],
            "tests": [
                { "evidence": [{ "path": "<PATH>", "size": 1 }, { "path": "<PATH>", "size": 2 }] },
                { "evidence": [] }
            ]
        }),
        "an empty array has every element normalized, and values no instruction reaches keep what \
         was observed"
    );
}

#[test]
fn every_scalar_kind_may_be_replaced() {
    // `null` included: a property that is present and null is a value the document has, so an
    // annotation applies to it. Whether it may be absent is the schema's `required`, not this.
    for observed in [json!("0.0.7"), json!(7), json!(true), json!(null)] {
        assert_eq!(
            normalized(
                &versioned_tool(),
                json!({ "tool": { "version": observed } })
            ),
            json!({ "tool": { "version": "<VERSION>" } })
        );
    }
}

#[test]
fn a_missing_property_leaves_the_document_alone() {
    // Requiredness belongs to contract validation (issue #192), which runs first. Nothing about an
    // annotation says the property has to be there.
    let document = json!({ "tool": { "name": "reportage" } });
    assert_eq!(
        normalized(&versioned_tool(), document.clone()),
        document,
        "an optional property the document does not have is not a normalization failure"
    );

    assert_eq!(
        normalized(&versioned_tool(), json!({})),
        json!({}),
        "nor is one whose whole container is absent"
    );
}

#[test]
fn the_replacement_value_is_written_as_the_literal_string_the_annotation_carries() {
    // No interpolation, no token resolution, no escape expansion: whatever the schema author wrote
    // is what the snapshot gets. `<VERSION>` is a naming convention, not syntax.
    let placeholder = r"<${tool.version}>\n\t~0 <VERSION> %s {}";
    assert_eq!(
        normalized(&annotated(placeholder), json!("0.0.7")),
        Value::String(placeholder.to_string())
    );
}

#[test]
fn one_plan_normalizes_every_document_it_is_applied_to() {
    // Preparation is per schema, not per document, which is what lets a suite prepare once and
    // normalize each fixture with the same plan.
    let plan = prepared(&versioned_tool());

    let normalized: Vec<Value> = [
        json!({ "tool": { "version": "0.0.7" } }),
        json!({ "tool": { "version": "0.0.8" } }),
        json!({ "tool": {} }),
    ]
    .into_iter()
    .map(|document| apply(&plan, document).expect("normalization must succeed"))
    .collect();

    assert_eq!(
        normalized,
        vec![
            json!({ "tool": { "version": "<VERSION>" } }),
            json!({ "tool": { "version": "<VERSION>" } }),
            json!({ "tool": {} }),
        ]
    );
}

// ---------------------------------------------------------------------------
// Instance shape failures
// ---------------------------------------------------------------------------

#[test]
fn a_property_step_onto_something_other_than_an_object_is_rejected() {
    // `null` among them: on the way to a target it is a container that is not there, not the
    // absent optional property that is a no-op. Normalization cannot read `type`, so it cannot tell
    // a contract-legal null container from an illegal one, and skipping either would leave a
    // volatile value in the snapshot with nothing saying why.
    for observed in [json!("reportage 0.0.7"), json!(null), json!(7), json!([])] {
        let error = rejected(&versioned_tool(), json!({ "tool": observed }));

        assert_eq!(error.kind(), ApplicationErrorKind::NonObjectContainer);
        assert_eq!(error.instance().as_pointer(), "/tool");
        assert_eq!(error.target().to_string(), "/tool/version");
        assert_eq!(
            error.source().as_pointer(),
            "/properties/tool/properties/version"
        );
    }
}

#[test]
fn an_every_element_step_onto_something_other_than_an_array_is_rejected() {
    let schema = json!({
        "type": "object",
        "properties": { "tests": { "type": "array", "items": annotated("<TEST>") } }
    });
    let error = rejected(&schema, json!({ "tests": { "0": "a" } }));

    assert_eq!(error.kind(), ApplicationErrorKind::NonArrayContainer);
    assert_eq!(error.instance().as_pointer(), "/tests");
    assert_eq!(error.target().to_string(), "/tests/*");
}

#[test]
fn replacing_an_object_or_an_array_is_rejected() {
    // A placeholder string in place of either would erase the shape the snapshot exists to protect,
    // so it is reported instead of written — including at the document root.
    for observed in [json!({ "major": 0 }), json!([0, 0, 7])] {
        let nested = rejected(
            &versioned_tool(),
            json!({ "tool": { "version": observed } }),
        );
        assert_eq!(nested.kind(), ApplicationErrorKind::ContainerTarget);
        assert_eq!(nested.instance().as_pointer(), "/tool/version");

        let root = rejected(&annotated("<VERSION>"), observed.clone());
        assert_eq!(root.kind(), ApplicationErrorKind::ContainerTarget);
        assert!(
            root.instance().is_document_root(),
            "the whole document is the position that could not be replaced"
        );
    }
}

#[test]
fn a_failure_inside_an_array_names_the_element_it_happened_at() {
    // The pattern the instruction carries cannot say which element: `/tests/*` is every one of
    // them. Only the walk knows, which is why the diagnostic carries both.
    let schema = json!({
        "type": "object",
        "properties": {
            "tests": {
                "type": "array",
                "items": { "type": "object", "properties": { "name": annotated("<NAME>") } }
            }
        }
    });
    let document = json!({
        "tests": [{ "name": "first" }, { "name": "second" }, { "name": { "text": "third" } }]
    });

    let error = rejected(&schema, document);
    assert_eq!(error.instance().as_pointer(), "/tests/2/name");
    assert_eq!(
        error.instance().tokens(),
        [
            InstanceToken::Property("tests".to_string()),
            InstanceToken::Index(2),
            InstanceToken::Property("name".to_string()),
        ]
    );
    assert_eq!(error.target().to_string(), "/tests/*/name");
}

#[test]
fn a_container_and_a_descendant_of_it_can_never_both_be_normalized() {
    // Instructions are applied in place to one document, so an annotation on a container and one
    // below it are not independent the way two unrelated targets are. They cannot both succeed, and
    // no order of the two produces a document instead of a failure: replacing the container needs
    // it to be a scalar, and a scalar has nothing below it to reach.
    let schema = json!({
        "type": "object",
        "properties": {
            "tool": {
                "type": "object",
                "x-reportage-snapshot": { "operation": "replace", "value": "<TOOL>" },
                "properties": { "version": annotated("<VERSION>") }
            }
        }
    });

    let container = rejected(&schema, json!({ "tool": { "version": "0.0.7" } }));
    assert_eq!(container.kind(), ApplicationErrorKind::ContainerTarget);
    assert_eq!(container.instance().as_pointer(), "/tool");

    let scalar = rejected(&schema, json!({ "tool": null }));
    assert_eq!(scalar.kind(), ApplicationErrorKind::NonObjectContainer);
    assert_eq!(
        scalar.target().to_string(),
        "/tool/version",
        "the container was replaced, and the instruction below it then had nothing to descend into"
    );
}

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

#[test]
fn every_failure_class_is_reported_under_its_own_classification() {
    let cases: &[(ApplicationErrorKind, Value, Value)] = &[
        (
            ApplicationErrorKind::NonObjectContainer,
            versioned_tool(),
            json!({ "tool": 7 }),
        ),
        (
            ApplicationErrorKind::NonArrayContainer,
            json!({
                "type": "object",
                "properties": { "tests": { "type": "array", "items": annotated("<TEST>") } }
            }),
            json!({ "tests": 7 }),
        ),
        (
            ApplicationErrorKind::ContainerTarget,
            versioned_tool(),
            json!({ "tool": { "version": [] } }),
        ),
    ];

    for (expected, schema, document) in cases {
        let error = rejected(schema, document.clone());
        assert_eq!(error.kind(), *expected, "{error}");
        assert!(
            error.to_string().contains(expected.label()),
            "a rendered diagnostic must name its classification: {error}"
        );
    }

    let covered: Vec<ApplicationErrorKind> = cases.iter().map(|(kind, _, _)| *kind).collect();
    for kind in ApplicationErrorKind::ALL {
        assert!(
            covered.contains(&kind),
            "no case produces the `{kind}` classification, so nothing shows it is reachable"
        );
    }
}

#[test]
fn the_classification_inventory_lists_every_variant_once_in_declaration_order() {
    // Without this, a classification left out of `ALL` would make the coverage loop above pass by
    // not asking about it. Mirrors `PreparationErrorKind::ALL`'s guard.
    for (index, kind) in ApplicationErrorKind::ALL.iter().enumerate() {
        assert_eq!(
            *kind as usize, index,
            "ApplicationErrorKind::ALL is out of sync at index {index} ({kind})"
        );
    }
}

#[test]
fn a_failure_renders_the_whole_message_a_reader_is_shown() {
    // Pinned as one message rather than as fragments, because that is what it is read as: instance
    // processing has no CLI surface, so a failure reaches a maintainer as the panic message of a
    // failing test. The parts are asserted as values elsewhere; this is about the reading.
    let expected = [
        "non-object container: expected an object to read `version` from, found a string (at /tool)",
        "  normalizing: /tool/version",
        "  requested by: /properties/tool/properties/version",
    ]
    .join("\n");

    assert_eq!(
        rejected(&versioned_tool(), json!({ "tool": "0.0.7" })).to_string(),
        expected
    );
}

#[test]
fn a_failure_at_a_root_position_still_tells_the_document_and_the_schema_apart() {
    // The message that puts both roots in view at once. Carrying an instance position and a schema
    // position is what lets a reader see whether the document or the annotation is wrong, and one
    // marker used for both would take that back exactly where the two are hardest to tell apart.
    let expected = [
        "container target: only a scalar may be replaced, and this position holds an object (at <instance root>)",
        "  normalizing: <instance root>",
        "  requested by: <document root>",
    ]
    .join("\n");

    assert_eq!(
        rejected(&annotated("<VERSION>"), json!({ "major": 0 })).to_string(),
        expected
    );
}

// ---------------------------------------------------------------------------
// The maintained contract schemas
// ---------------------------------------------------------------------------

#[test]
fn the_maintained_contract_schemas_normalize_the_values_their_annotations_name() {
    // The documents are cut down to the annotated fields and enough structure to reach them. That
    // real output satisfies these plans is what the fixture suites will establish once they are
    // moved onto this normalizer.
    let cases = [
        (
            &*JSON_REPORT,
            json!({
                "artifactRoot": ".reportage/runs/run-1",
                "tool": { "name": "reportage", "version": "0.0.7" }
            }),
            json!({
                "artifactRoot": "<ARTIFACT_ROOT>",
                "tool": { "name": "reportage", "version": "<VERSION>" }
            }),
        ),
        (
            &*RUN_RESULT,
            json!({ "tool": { "name": "reportage", "version": "0.0.7" } }),
            json!({ "tool": { "name": "reportage", "version": "<VERSION>" } }),
        ),
    ];

    for (contract, observed, expected) in cases {
        let path = contract.path(SchemaVariant::InternalSource);
        let plan =
            prepare(contract.document(SchemaVariant::InternalSource)).unwrap_or_else(|error| {
                panic!("the internal source schema {path} must prepare: {error}")
            });
        assert_eq!(
            apply(&plan, observed)
                .unwrap_or_else(|error| panic!("{path} must normalize its own shape: {error}")),
            expected
        );
    }
}
