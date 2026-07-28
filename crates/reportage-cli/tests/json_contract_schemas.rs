//! Schema document validity and schema feature coverage for Reportage's two JSON contracts
//! (issue #192).
//!
//! The fixture suites check that real producer output conforms to these schemas. That leaves two
//! gaps this suite closes, both of which follow from representative fixtures being representative
//! rather than exhaustive:
//!
//! - a schema that is itself malformed would make every instance check meaningless, so each
//!   published schema artifact is validated against the Draft 2020-12 meta-schema before anything
//!   is validated against it;
//! - a constraint no fixture happens to violate is never actually exercised, so each JSON Schema
//!   keyword the contracts rely on gets valid and invalid instances constructed directly here.
//!
//! Instances are hand-built rather than produced by the CLI on purpose: making the producer emit a
//! contract violation would mean adding test-only behavior to the runtime, and a keyword can only
//! be shown to bite by feeding it something that must fail.
//!
//! See docs/adr/20260728T092956Z_json-contract-validation-policy.md.

// The base document instantiates every closed shape both contracts define, which nests deeper than
// `json!` unrolls by default. Raised rather than split into assembled fragments: a document that is
// checked against a schema as a whole is easier to read against that schema as one literal.
#![recursion_limit = "512"]

use std::collections::BTreeSet;

use serde_json::{Value, json};

#[path = "support/json_schema.rs"]
mod json_schema;

use json_schema::{Contract, JSON_REPORT, RUN_RESULT, SchemaVariant, compile, numbered};

// ---------------------------------------------------------------------------
// Schema document validity
// ---------------------------------------------------------------------------

#[test]
fn every_contract_schema_artifact_is_a_valid_draft_2020_12_document() {
    for contract in [&*JSON_REPORT, &*RUN_RESULT] {
        for variant in SchemaVariant::ALL {
            let document = contract.document(variant);
            if let Err(error) = jsonschema::draft202012::meta::validate(document) {
                panic!(
                    "{} is not a valid JSON Schema Draft 2020-12 document: {error}\n    schema path: {}",
                    contract.path(variant),
                    error.instance_path(),
                );
            }
        }
    }
}

/// The internal source schema carries `x-reportage-snapshot` normalization metadata that the
/// generated public schema does not (issue #115). Draft 2020-12 ignores unknown keywords, so the
/// annotation must neither break the schema document nor constrain any instance — the latter is
/// what [`internal_and_public_schemas_agree_on_every_feature_case`] establishes.
#[test]
fn the_snapshot_annotation_does_not_break_schema_document_validity() {
    for contract in [&*JSON_REPORT, &*RUN_RESULT] {
        let internal = contract.document(SchemaVariant::InternalSource);
        assert!(
            document_contains_key(internal, "x-reportage-snapshot"),
            "{} is expected to carry x-reportage-snapshot metadata; if the annotation moved, this test no longer proves anything",
            contract.path(SchemaVariant::InternalSource),
        );
        assert!(
            jsonschema::draft202012::meta::validate(internal).is_ok(),
            "{} must stay a valid schema document with its annotations present",
            contract.path(SchemaVariant::InternalSource),
        );
        assert!(
            compile(internal).is_ok(),
            "{} must compile with its annotations present",
            contract.path(SchemaVariant::InternalSource),
        );
    }
}

#[test]
fn a_malformed_schema_fails_before_any_instance_is_validated() {
    // Not a valid schema document: `type` must name a JSON type, and `required` must be an array.
    let malformed = json!({ "type": "trapezoid", "required": "everything" });

    assert!(
        jsonschema::draft202012::meta::validate(&malformed).is_err(),
        "meta-schema validation must reject a malformed schema document",
    );
    assert!(
        compile(&malformed).is_err(),
        "a malformed schema must fail to compile, so no instance can be validated against it",
    );
}

/// External resolution is disabled, so an external `$ref` is a compile failure rather than a
/// network fetch. Contract validation must run identically on a machine with no network access.
#[test]
fn an_external_reference_is_a_failure_rather_than_a_fetch() {
    let error = compile(&json!({ "$ref": "https://example.com/reportage-external.json" }))
        .expect_err("an external reference must not be resolvable");

    assert!(
        error.contains("not resolvable"),
        "the failure must name the unresolved external reference, got: {error}",
    );
}

/// Every `$ref` the published schemas use must be a fragment-only local pointer. This is what
/// makes disabling external resolution a policy rather than a limitation.
#[test]
fn published_schemas_only_use_fragment_only_local_references() {
    for contract in [&*JSON_REPORT, &*RUN_RESULT] {
        for variant in SchemaVariant::ALL {
            for reference in collect_refs(contract.document(variant)) {
                assert!(
                    reference.starts_with("#/"),
                    "{} uses a non-local $ref `{reference}`; contract validation resolves no external resource",
                    contract.path(variant),
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Schema feature tests
// ---------------------------------------------------------------------------

/// A JSON Schema keyword the contracts rely on.
///
/// Every keyword either maps to a variant here, which obliges the case tables to exercise it, or is
/// listed as structural in [`keyword_feature`]. A keyword that is neither fails
/// [`every_keyword_the_schemas_use_maps_to_a_covered_feature`], so adding one to a schema without a
/// case is a test failure rather than a silent coverage gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Feature {
    Const,
    Enum,
    Pattern,
    Minimum,
    Type,
    Items,
    Required,
    AdditionalProperties,
    OneOf,
    Conditional,
    NestedLocalRef,
    ExtensionKeyword,
}

/// The [`Feature`] a schema keyword belongs to, or `None` when the keyword states no constraint on
/// an instance and so has nothing to exercise.
///
/// Returns `Err` for a keyword this suite does not account for at all. That is the coverage-drift
/// signal: a keyword added to a contract is either something an instance can violate, and needs
/// cases, or it is structural, and needs to be recorded as such here.
fn keyword_feature(keyword: &str) -> Result<Option<Feature>, ()> {
    Ok(match keyword {
        "const" => Some(Feature::Const),
        "enum" => Some(Feature::Enum),
        "pattern" => Some(Feature::Pattern),
        "minimum" => Some(Feature::Minimum),
        "type" => Some(Feature::Type),
        "items" => Some(Feature::Items),
        "required" => Some(Feature::Required),
        "additionalProperties" => Some(Feature::AdditionalProperties),
        "oneOf" => Some(Feature::OneOf),
        // `allOf` appears in these schemas only as the wrapper holding `if` / `then` pairs.
        "allOf" | "if" | "then" => Some(Feature::Conditional),
        "$ref" => Some(Feature::NestedLocalRef),
        "x-reportage-snapshot" => Some(Feature::ExtensionKeyword),

        // Structural: identity, documentation, and the containers that hold subschemas. None of
        // these can be violated by an instance, so there is nothing for a case to demonstrate.
        "$schema" | "$id" | "$defs" | "title" | "description" | "properties" => None,

        _ => return Err(()),
    })
}

struct FeatureCase {
    feature: Feature,
    /// What the case demonstrates, phrased so a failure message reads as a sentence.
    description: String,
    instance: Value,
    valid: bool,
}

impl FeatureCase {
    fn new(feature: Feature, description: impl Into<String>, instance: Value, valid: bool) -> Self {
        FeatureCase {
            feature,
            description: description.into(),
            instance,
            valid,
        }
    }
}

/// The expectation kinds whose definitions carry `allOf` / `if` / `then`, and whether the
/// definition also gates on `observed`.
///
/// The two contracts state the same conditional in four definitions independently. Covering only
/// one of them would leave a weakened constraint in any of the other three invisible: producer
/// fixtures only ever emit conforming instances, so nothing else would notice.
const CONDITIONAL_EXPECTATIONS: &[(&str, bool)] = &[
    ("fileContentsEquals", true),
    ("fileTextEquals", true),
    ("stdoutContentsEquals", false),
    ("stdoutTextEquals", false),
];

/// Builds every case for one contract from that contract's own valid base document.
///
/// Cases are expressed as edits to a valid document rather than as standalone fragments, so an
/// "invalid" case fails for exactly the reason it names instead of for an unrelated omission.
fn feature_cases(base: Value, contract_specific: Vec<FeatureCase>) -> Vec<FeatureCase> {
    let logical = expectation_pointer(&base, "logical");

    let mut cases = vec![
        FeatureCase::new(
            Feature::Const,
            "the declared schemaVersion is the only accepted value",
            base.clone(),
            true,
        ),
        FeatureCase::new(
            Feature::Const,
            "a different schemaVersion is rejected",
            set(&base, "/schemaVersion", json!(2)),
            false,
        ),
        FeatureCase::new(
            Feature::Const,
            "an origin kind outside the two declared variants is rejected",
            set(&base, "/diagnostics/0/origin/kind", json!("plugin")),
            false,
        ),
        FeatureCase::new(
            Feature::Enum,
            "another declared top-level status is accepted",
            set(&base, "/status", json!("error")),
            true,
        ),
        FeatureCase::new(
            Feature::Enum,
            "a top-level status outside the enumeration is rejected",
            set(&base, "/status", json!("crashed")),
            false,
        ),
        FeatureCase::new(
            Feature::Type,
            "processExitCode is accepted as an integer",
            set(&base, "/processExitCode", json!(3)),
            true,
        ),
        FeatureCase::new(
            Feature::Type,
            "a stringified processExitCode is rejected",
            set(&base, "/processExitCode", json!("3")),
            false,
        ),
        FeatureCase::new(
            Feature::Items,
            "shim event parse warnings are accepted as strings",
            set(
                &base,
                "/tests/0/actions/0/shimEventParseWarnings",
                json!(["unparsable event line"]),
            ),
            true,
        ),
        FeatureCase::new(
            Feature::Items,
            "a non-string shim event parse warning is rejected",
            set(
                &base,
                "/tests/0/actions/0/shimEventParseWarnings",
                json!([17]),
            ),
            false,
        ),
        FeatureCase::new(
            Feature::Pattern,
            "a dotted lowercase diagnostic code matches the code pattern",
            set(
                &base,
                "/diagnostics/0/code",
                json!("step.write.target_exists"),
            ),
            true,
        ),
        FeatureCase::new(
            Feature::Pattern,
            "an uppercase diagnostic code is rejected",
            set(&base, "/diagnostics/0/code", json!("Parse.Syntax")),
            false,
        ),
        FeatureCase::new(
            Feature::Pattern,
            "an undotted diagnostic code is rejected",
            set(&base, "/diagnostics/0/code", json!("parse")),
            false,
        ),
        FeatureCase::new(
            Feature::Minimum,
            "a zero summary count is accepted",
            set(&base, "/summary/passed", json!(0)),
            true,
        ),
        FeatureCase::new(
            Feature::Minimum,
            "a negative summary count is rejected",
            set(&base, "/summary/passed", json!(-1)),
            false,
        ),
        FeatureCase::new(
            Feature::Minimum,
            "a diagnostic location line below one is rejected",
            set(&base, "/diagnostics/0/location/line", json!(0)),
            false,
        ),
        FeatureCase::new(
            Feature::Required,
            "an optional diagnostic code may be absent",
            remove(&base, "/diagnostics/0/code"),
            true,
        ),
        FeatureCase::new(
            Feature::Required,
            "a missing top-level tests array is rejected",
            remove(&base, "/tests"),
            false,
        ),
        FeatureCase::new(
            Feature::Required,
            "a diagnostic without the always-present location key is rejected",
            remove(&base, "/diagnostics/0/location"),
            false,
        ),
        FeatureCase::new(
            Feature::AdditionalProperties,
            "an unknown top-level field is rejected",
            set(&base, "/runDuration", json!(12)),
            false,
        ),
        FeatureCase::new(
            Feature::AdditionalProperties,
            "an unknown field inside a nested closed object is rejected",
            set(&base, "/tool/commit", json!("abc123")),
            false,
        ),
        FeatureCase::new(
            Feature::AdditionalProperties,
            "a shim invocation may carry fields the renderer does not define",
            set(
                &base,
                "/tests/0/actions/0/shimInvocations",
                json!([{
                    "schemaVersion": 1,
                    "commandName": "git",
                    "shimPath": "shims/git",
                    "target": { "program": "/usr/bin/git", "args": [] },
                    "forwardsCallerArgs": true,
                    "recordedAt": "later addition by the shim runtime"
                }]),
            ),
            true,
        ),
        FeatureCase::new(
            Feature::OneOf,
            "the test-origin variant of a diagnostic origin is accepted",
            set(
                &base,
                "/diagnostics/0/origin",
                json!({ "kind": "test", "test": "test-1" }),
            ),
            true,
        ),
        FeatureCase::new(
            Feature::OneOf,
            "an origin mixing both variants' fields matches neither",
            set(
                &base,
                "/diagnostics/0/origin",
                json!({ "kind": "source", "source": "feature.repor", "test": "test-1" }),
            ),
            false,
        ),
        FeatureCase::new(
            Feature::OneOf,
            "an expectation whose kind is not one of the declared kinds is rejected",
            set(
                &base,
                &expectation_pointer(&base, "exit"),
                json!({ "kind": "exitsQuietly", "status": "passed" }),
            ),
            false,
        ),
        // `Diagnostic.location` and `TextExpectedSource` are the two remaining `oneOf` sites; the
        // four in each contract are covered by a case each, for the reason the conditional is.
        FeatureCase::new(
            Feature::OneOf,
            "the null branch of a diagnostic location is accepted",
            set(&base, "/diagnostics/0/location", json!(null)),
            true,
        ),
        FeatureCase::new(
            Feature::OneOf,
            "a diagnostic location that is neither null nor a location object is rejected",
            set(&base, "/diagnostics/0/location", json!("line 3, column 5")),
            false,
        ),
        FeatureCase::new(
            Feature::OneOf,
            "the binding branch of a text expectation source is accepted",
            set(
                &base,
                &format!(
                    "{}/expectedSource",
                    expectation_pointer(&base, "fileTextEquals")
                ),
                json!({
                    "kind": "binding",
                    "name": "greeting",
                    "actionIndex": 0,
                    "stream": "stdout",
                    "captureMode": "exact"
                }),
            ),
            true,
        ),
        FeatureCase::new(
            Feature::OneOf,
            "a text expectation source mixing two branches' fields matches neither",
            set(
                &base,
                &format!(
                    "{}/expectedSource",
                    expectation_pointer(&base, "fileTextEquals")
                ),
                json!({ "kind": "quoted", "value": "hello", "name": "greeting" }),
            ),
            false,
        ),
        FeatureCase::new(
            Feature::NestedLocalRef,
            "a logical composition recurses into child expectations",
            base.clone(),
            true,
        ),
        FeatureCase::new(
            Feature::NestedLocalRef,
            "a defect inside a logical composition's child is rejected",
            set(
                &base,
                &format!("{logical}/children/0/expected"),
                json!("zero"),
            ),
            false,
        ),
        FeatureCase::new(
            Feature::NestedLocalRef,
            "a logical composition nested inside another is rejected when its operator is not declared",
            set(
                &base,
                &format!("{logical}/children/0"),
                json!({
                    "kind": "logical",
                    "status": "passed",
                    "operator": "either",
                    "children": []
                }),
            ),
            false,
        ),
    ];

    for (kind, gates_on_observed) in CONDITIONAL_EXPECTATIONS {
        let expectation = expectation_pointer(&base, kind);
        cases.push(FeatureCase::new(
            Feature::Conditional,
            format!("a mismatching {kind} carries its mismatch object"),
            base.clone(),
            true,
        ));
        cases.push(FeatureCase::new(
            Feature::Conditional,
            format!("a mismatching {kind} without a mismatch object is rejected"),
            remove(&base, &format!("{expectation}/mismatch")),
            false,
        ));
        cases.push(FeatureCase::new(
            Feature::Conditional,
            format!("a matching {kind} needs no mismatch object"),
            remove(
                &set(&base, &format!("{expectation}/outcome"), json!("match")),
                &format!("{expectation}/mismatch"),
            ),
            true,
        ));

        if !gates_on_observed {
            continue;
        }
        cases.push(FeatureCase::new(
            Feature::Conditional,
            format!("a compared {kind} without an outcome is rejected"),
            remove(&base, &format!("{expectation}/outcome")),
            false,
        ));
        // The `if` guarding the mismatch requirement has to test that `outcome` is present, not
        // only that it is not `mismatch`: `properties` matches vacuously on an absent member, so an
        // unguarded conditional would require `mismatch` on every observation that never got as far
        // as a comparison. `tests/fixtures/run_result/contents_equals.repor` covers the same branch
        // from the producer side.
        cases.push(FeatureCase::new(
            Feature::Conditional,
            format!("an unread {kind} carries neither an outcome nor a mismatch object"),
            remove(
                &remove(
                    &set(
                        &base,
                        &format!("{expectation}/observed"),
                        json!("actualMissing"),
                    ),
                    &format!("{expectation}/outcome"),
                ),
                &format!("{expectation}/mismatch"),
            ),
            true,
        ));
    }

    cases.extend(closed_object_cases(&base));
    cases.extend(declared_type_cases(&base));
    cases.extend(contract_specific);
    cases
}

/// One case per object in the base document: a member the contract does not define must be
/// rejected there.
///
/// `additionalProperties: false` is stated in more than twenty definitions per contract. A single
/// hand-written case shows the keyword works somewhere, not that any particular definition still
/// carries it — the same one-case-for-many-definitions gap that let the contents-comparison
/// conditional ship broken. Generating a case per object closes it for every definition the base
/// document reaches.
///
/// A definition the contract intentionally leaves open would fail here. That is the right direction
/// to fail in: `shimInvocations` is open on purpose, and its case says so explicitly rather than
/// being absorbed by a rule that would then stop covering everything else.
fn closed_object_cases(base: &Value) -> Vec<FeatureCase> {
    instance_pointers(base)
        .into_iter()
        .filter(|(_, node)| node.is_object())
        .map(|(pointer, _)| {
            FeatureCase::new(
                Feature::AdditionalProperties,
                format!(
                    "an undefined member at {} is rejected",
                    render_pointer(&pointer)
                ),
                set(base, &format!("{pointer}/undefinedMember"), json!(true)),
                false,
            )
        })
        .collect()
}

/// One case per scalar in the base document: a value of the wrong JSON type must be rejected there.
///
/// `type` is the most-stated keyword in both contracts, and the reasoning is the same as for
/// closed objects: covering it once says nothing about the several hundred other places it appears.
/// An object is used as the wrong value because no scalar position in either contract accepts one.
fn declared_type_cases(base: &Value) -> Vec<FeatureCase> {
    instance_pointers(base)
        .into_iter()
        .filter(|(_, node)| !node.is_object() && !node.is_array())
        .map(|(pointer, _)| {
            FeatureCase::new(
                Feature::Type,
                format!(
                    "an object where {} declares a scalar is rejected",
                    render_pointer(&pointer)
                ),
                set(base, &pointer, json!({ "wrongType": true })),
                false,
            )
        })
        .collect()
}

/// Every value in `document` paired with its JSON Pointer, the document root included.
fn instance_pointers(document: &Value) -> Vec<(String, &Value)> {
    fn descend<'a>(value: &'a Value, pointer: String, found: &mut Vec<(String, &'a Value)>) {
        match value {
            Value::Object(members) => {
                for (name, child) in members {
                    descend(child, format!("{pointer}/{name}"), found);
                }
            }
            Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    descend(item, format!("{pointer}/{index}"), found);
                }
            }
            _ => {}
        }
        found.push((pointer, value));
    }

    let mut found = Vec::new();
    descend(document, String::new(), &mut found);
    found
}

fn render_pointer(pointer: &str) -> &str {
    if pointer.is_empty() {
        "the document root"
    } else {
        pointer
    }
}

fn json_report_cases() -> Vec<FeatureCase> {
    let base = json_report_document();
    feature_cases(
        base.clone(),
        vec![
            // `artifactRoot` and `tool.version` are the two locations the internal source schema
            // annotates. A snapshot placeholder is what the normalization harness substitutes
            // there, so it has to stay an ordinary accepted string.
            FeatureCase::new(
                Feature::ExtensionKeyword,
                "both snapshot-annotated locations accept their placeholders",
                set(
                    &set(&base, "/artifactRoot", json!("<ARTIFACT_ROOT>")),
                    "/tool/version",
                    json!("<VERSION>"),
                ),
                true,
            ),
            FeatureCase::new(
                Feature::ExtensionKeyword,
                "an annotated location still enforces its declared type",
                set(&base, "/artifactRoot", json!(7)),
                false,
            ),
            FeatureCase::new(
                Feature::Required,
                "the stdout document's artifactRoot is required",
                remove(&base, "/artifactRoot"),
                false,
            ),
            FeatureCase::new(
                Feature::AdditionalProperties,
                "the artifact-only noop field is not part of the stdout document",
                set(&base, "/noop", json!(false)),
                false,
            ),
            FeatureCase::new(
                Feature::AdditionalProperties,
                "the artifact-only evidence digest is not part of the stdout document",
                set(
                    &base,
                    "/tests/0/actions/0/stdout/sha256",
                    json!(EMPTY_SHA256),
                ),
                false,
            ),
        ],
    )
}

fn run_result_cases() -> Vec<FeatureCase> {
    let base = run_result_document();
    feature_cases(
        base.clone(),
        vec![
            FeatureCase::new(
                Feature::ExtensionKeyword,
                "the snapshot-annotated tool version accepts its placeholder",
                set(&base, "/tool/version", json!("<VERSION>")),
                true,
            ),
            FeatureCase::new(
                Feature::ExtensionKeyword,
                "an annotated location still enforces its declared type",
                set(&base, "/tool/version", json!(7)),
                false,
            ),
            FeatureCase::new(
                Feature::Required,
                "the canonical manifest's noop field is required",
                remove(&base, "/noop"),
                false,
            ),
            FeatureCase::new(
                Feature::Required,
                "an evidence reference without its digest is rejected",
                remove(&base, "/tests/0/actions/0/stdout/sha256"),
                false,
            ),
            FeatureCase::new(
                Feature::Pattern,
                "an evidence digest that is not lowercase hex is rejected",
                set(
                    &base,
                    "/tests/0/actions/0/stdout/sha256",
                    json!(EMPTY_SHA256.to_uppercase()),
                ),
                false,
            ),
            FeatureCase::new(
                Feature::AdditionalProperties,
                "the stdout-only artifactRoot is not part of the canonical manifest",
                set(&base, "/artifactRoot", json!(".reportage/runs/run-1")),
                false,
            ),
        ],
    )
}

fn contract_cases() -> [(&'static Contract, Vec<FeatureCase>); 2] {
    [
        (&JSON_REPORT, json_report_cases()),
        (&RUN_RESULT, run_result_cases()),
    ]
}

#[test]
fn schema_feature_cases_are_accepted_or_rejected_as_declared() {
    for (contract, cases) in contract_cases() {
        for case in &cases {
            assert_case(contract, SchemaVariant::InternalSource, case);
        }
    }
}

/// The generated public schema must accept and reject exactly what the internal source schema
/// does. Issue #115's generation check keeps the two documents identical apart from stripped
/// metadata; this is the instance-level counterpart, covering the possibility that a stripped
/// keyword was load-bearing after all.
#[test]
fn internal_and_public_schemas_agree_on_every_feature_case() {
    for (contract, cases) in contract_cases() {
        for case in &cases {
            let internal = contract.is_valid(SchemaVariant::InternalSource, &case.instance);
            let public = contract.is_valid(SchemaVariant::Public, &case.instance);
            assert_eq!(
                internal,
                public,
                "{} schemas disagree on `{}`: the {} says {}, the {} says {}",
                contract.name(),
                case.description,
                SchemaVariant::InternalSource.label(),
                verdict(internal),
                SchemaVariant::Public.label(),
                verdict(public),
            );
        }
    }
}

/// Coverage is checked against the schemas rather than against a hand-written list of features, so
/// that adding a keyword to a contract without adding cases for it fails here.
#[test]
fn every_keyword_the_schemas_use_maps_to_a_covered_feature() {
    for (contract, cases) in contract_cases() {
        let covered: BTreeSet<Feature> = REQUIRED_FEATURES
            .iter()
            .copied()
            .filter(|feature| {
                let verdicts: Vec<bool> = cases
                    .iter()
                    .filter(|case| case.feature == *feature)
                    .map(|case| case.valid)
                    .collect();
                verdicts.contains(&true) && verdicts.contains(&false)
            })
            .collect();

        for variant in SchemaVariant::ALL {
            for (keyword, pointer) in schema_keywords(contract.document(variant)) {
                let Ok(feature) = keyword_feature(&keyword) else {
                    panic!(
                        "{} uses the keyword `{keyword}` at {pointer}, which this suite does not account for; map it to a Feature and add cases, or record it as structural in keyword_feature",
                        contract.path(variant),
                    );
                };
                let Some(feature) = feature else { continue };
                assert!(
                    covered.contains(&feature),
                    "{} uses `{keyword}` at {pointer}, but the {} contract has no valid and invalid case pair for {feature:?}",
                    contract.path(variant),
                    contract.name(),
                );
            }
        }
    }
}

/// Per-site coverage is generated from the base document, so a definition the base document never
/// builds silently gets none. This is what keeps the two in step.
///
/// A definition is matched to an instance by its `kind` discriminator; definitions without one are
/// reached through the properties that hold them and are covered as long as the shapes above are.
#[test]
fn every_closed_definition_with_a_kind_is_instantiated_by_the_base_document() {
    for (contract, _) in contract_cases() {
        let document = contract.document(SchemaVariant::InternalSource);
        let base = match contract.name() {
            "json-report" => json_report_document(),
            _ => run_result_document(),
        };
        let built: BTreeSet<String> = instance_pointers(&base)
            .into_iter()
            .filter_map(|(_, node)| node.get("kind")?.as_str().map(str::to_owned))
            .collect();

        let definitions = document["$defs"]
            .as_object()
            .expect("a contract schema has $defs");
        for (name, definition) in definitions {
            if definition.get("additionalProperties") != Some(&json!(false)) {
                continue;
            }
            let Some(kind) = definition.pointer("/properties/kind") else {
                continue;
            };
            let kinds: Vec<String> = match (kind.get("const"), kind.get("enum")) {
                (Some(Value::String(value)), _) => vec![value.clone()],
                (_, Some(Value::Array(values))) => values
                    .iter()
                    .filter_map(|value| value.as_str().map(str::to_owned))
                    .collect(),
                _ => continue,
            };
            assert!(
                kinds.iter().any(|kind| built.contains(kind)),
                "{} defines the closed shape `{name}` ({kinds:?}), which the {} base document never builds, so no per-site additionalProperties or type case covers it; add an instance of it to the base document",
                contract.path(SchemaVariant::InternalSource),
                contract.name(),
            );
        }
    }
}

/// Every [`Feature`], so the coverage check above compares against a stable set rather than against
/// whatever the case tables happen to contain.
const REQUIRED_FEATURES: &[Feature] = &[
    Feature::Const,
    Feature::Enum,
    Feature::Pattern,
    Feature::Minimum,
    Feature::Type,
    Feature::Items,
    Feature::Required,
    Feature::AdditionalProperties,
    Feature::OneOf,
    Feature::Conditional,
    Feature::NestedLocalRef,
    Feature::ExtensionKeyword,
];

fn assert_case(contract: &Contract, variant: SchemaVariant, case: &FeatureCase) {
    let violations = contract.violations(variant, &case.instance);
    if case.valid {
        assert!(
            violations.is_empty(),
            "the {} {} must accept `{}` ({:?}), but reported:\n\n{}\n",
            contract.name(),
            variant.label(),
            case.description,
            case.feature,
            numbered(&violations),
        );
    } else {
        assert!(
            !violations.is_empty(),
            "the {} {} must reject `{}` ({:?}), but accepted it:\n{}",
            contract.name(),
            variant.label(),
            case.description,
            case.feature,
            serde_json::to_string_pretty(&case.instance).unwrap(),
        );
    }
}

fn verdict(valid: bool) -> &'static str {
    if valid { "valid" } else { "invalid" }
}

/// JSON Pointer of the first assertion expectation in `document` with the given `kind`.
///
/// Cases address expectations by kind rather than by index so that adding an assertion to a base
/// document cannot silently repoint an existing case at a different shape.
fn expectation_pointer(document: &Value, kind: &str) -> String {
    let assertions = document["tests"][0]["assertions"]
        .as_array()
        .expect("the base document's first test has assertions");
    let index = assertions
        .iter()
        .position(|assertion| assertion["expectation"]["kind"] == kind)
        .unwrap_or_else(|| panic!("the base document has no `{kind}` expectation"));
    format!("/tests/0/assertions/{index}/expectation")
}

// ---------------------------------------------------------------------------
// Base documents
// ---------------------------------------------------------------------------

/// SHA-256 of the empty byte string, used wherever a case needs a well-formed digest whose value
/// is not what the case is about.
const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// A bounded first-difference report, the shape every conditional-bearing comparison requires once
/// its outcome is `mismatch`.
fn mismatch() -> Value {
    json!({
        "firstDiffOffset": 0,
        "firstDiffLine": 1,
        "actualContext": "hello\\n",
        "expectedContext": "HELLO"
    })
}

/// A valid artifact `result.json` instantiating every closed shape the contracts define.
///
/// Completeness matters here beyond the cases written by hand: `closed_object_cases` and
/// `declared_type_cases` generate their coverage from what this document contains, so a definition
/// this document never builds gets no per-site case. That is why it carries every expectation kind,
/// both diagnostic origin variants, and each `TextExpectedSource` branch, rather than only the
/// shapes a hand-written case happens to mutate. `every_closed_definition_is_instantiated` fails if
/// a definition is added to a contract and not built here.
///
/// This is hand-built rather than captured from a fixture run: a case that removes a required
/// field has to start from a document whose every field is deliberately there, and no single run
/// produces every shape.
fn run_result_document() -> Value {
    json!({
        "schemaVersion": 1,
        "tool": { "name": "reportage", "version": "0.0.0" },
        "status": "failed",
        "processExitCode": 1,
        "noop": false,
        "summary": {
            "scripts": 1,
            "actions": 1,
            "assertions": 3,
            "passed": 0,
            "failed": 1,
            "errors": 0
        },
        "diagnostics": [
            {
                "id": "diagnostic-1",
                "category": "parse",
                "severity": "error",
                "message": "unexpected token",
                "origin": { "kind": "source", "source": "feature.repor" },
                "location": { "line": 3, "column": 5 },
                "code": "parse.syntax"
            },
            {
                "id": "diagnostic-2",
                "category": "assertion",
                "severity": "failure",
                "message": "stdout does not contain the expected text",
                // The test-origin branch of Origin, instantiated so the generated cases reach its
                // closed shape; the source branch above is the one the hand-written cases mutate.
                "origin": { "kind": "test", "test": "test-1" },
                "location": null,
                "code": "assertion.stdout.contains.mismatch"
            }
        ],
        "tests": [
            {
                "id": "test-1",
                "name": "feature",
                "path": "feature.repor",
                "status": "failed",
                "actions": [
                    {
                        "id": "action-1",
                        "command": "echo hello",
                        "exitCode": 0,
                        "stdout": {
                            "artifactRef": "test-1/action-1/stdout.bin",
                            "sizeBytes": 6,
                            "sha256": EMPTY_SHA256
                        },
                        "stderr": {
                            "artifactRef": "test-1/action-1/stderr.bin",
                            "sizeBytes": 0,
                            "sha256": EMPTY_SHA256
                        }
                    }
                ],
                "assertions": [
                    {
                        "id": "assertion-1",
                        "status": "passed",
                        "checkpoint": "action-1",
                        "expectation": {
                            "kind": "exit",
                            "status": "passed",
                            "expected": 0,
                            "actual": 0
                        }
                    },
                    {
                        "id": "assertion-2",
                        "status": "failed",
                        "checkpoint": "action-1",
                        "expectation": {
                            "kind": "fileContentsEquals",
                            "status": "failed",
                            "path": "out.txt",
                            "expectedSource": { "kind": "workspace", "path": "expected.txt" },
                            "observed": "compared",
                            "outcome": "mismatch",
                            "actualSizeBytes": 6,
                            "expectedSizeBytes": 5,
                            "mismatch": mismatch()
                        },
                        "diagnosticRef": "diagnostic-1"
                    },
                    {
                        "id": "assertion-3",
                        "status": "passed",
                        "checkpoint": "action-1",
                        "expectation": {
                            "kind": "logical",
                            "status": "passed",
                            "operator": "all",
                            "children": [
                                {
                                    "kind": "exit",
                                    "status": "passed",
                                    "expected": 0,
                                    "actual": 0
                                },
                                {
                                    "kind": "stdoutContains",
                                    "status": "passed",
                                    "expected": "hello",
                                    "expectedSource": { "kind": "quoted", "value": "hello" },
                                    "actualRef": "test-1/action-1/stdout.bin",
                                    "actualSizeBytes": 6
                                }
                            ]
                        }
                    },
                    {
                        "id": "assertion-4",
                        "status": "failed",
                        "checkpoint": "action-1",
                        "expectation": {
                            "kind": "fileTextEquals",
                            "status": "failed",
                            "path": "out.txt",
                            "expectedSource": { "kind": "quoted", "value": "HELLO" },
                            "observed": "compared",
                            "outcome": "mismatch",
                            "actualSizeBytes": 6,
                            "expectedSizeBytes": 5,
                            "mismatch": mismatch()
                        },
                        "diagnosticRef": "diagnostic-1"
                    },
                    {
                        "id": "assertion-5",
                        "status": "failed",
                        "checkpoint": "action-1",
                        "expectation": {
                            "kind": "stdoutContentsEquals",
                            "status": "failed",
                            "expectedSource": { "kind": "workspace", "path": "expected.txt" },
                            "actualRef": "test-1/action-1/stdout.bin",
                            "outcome": "mismatch",
                            "actualSizeBytes": 6,
                            "expectedSizeBytes": 5,
                            "mismatch": mismatch()
                        },
                        "diagnosticRef": "diagnostic-1"
                    },
                    {
                        "id": "assertion-6",
                        "status": "failed",
                        "checkpoint": "action-1",
                        "expectation": {
                            "kind": "stdoutTextEquals",
                            "status": "failed",
                            "expectedSource": { "kind": "quoted", "value": "HELLO" },
                            "actualRef": "test-1/action-1/stdout.bin",
                            "outcome": "mismatch",
                            "actualSizeBytes": 6,
                            "expectedSizeBytes": 5,
                            "mismatch": mismatch()
                        },
                        "diagnosticRef": "diagnostic-1"
                    },
                    {
                        "id": "assertion-7",
                        "status": "passed",
                        "checkpoint": "action-1",
                        "expectation": {
                            "kind": "fileExists",
                            "status": "passed",
                            "path": "out.txt",
                            "observed": "regularFile"
                        }
                    },
                    {
                        "id": "assertion-8",
                        "status": "passed",
                        "checkpoint": "action-1",
                        "expectation": {
                            "kind": "fileContains",
                            "status": "passed",
                            "path": "out.txt",
                            "expected": "hello",
                            // The binding branch of TextExpectedSource. Instantiated here so the
                            // generated cases reach its closed shape.
                            "expectedSource": {
                                "kind": "binding",
                                "name": "greeting",
                                "actionIndex": 0,
                                "stream": "stdout",
                                "captureMode": "exact"
                            },
                            "observed": "found"
                        }
                    },
                    {
                        "id": "assertion-9",
                        "status": "passed",
                        "checkpoint": "action-1",
                        "expectation": {
                            "kind": "stdoutEmpty",
                            "status": "passed",
                            "actualRef": "test-1/action-1/stdout.bin",
                            "actualSizeBytes": 0
                        }
                    },
                    {
                        "id": "assertion-10",
                        "status": "passed",
                        "checkpoint": "action-1",
                        "expectation": {
                            "kind": "dirExists",
                            "status": "passed",
                            "path": "out",
                            "observed": "directory"
                        }
                    },
                    {
                        "id": "assertion-11",
                        "status": "passed",
                        "checkpoint": "action-1",
                        "expectation": {
                            "kind": "dirContains",
                            "status": "passed",
                            "path": "out",
                            "expectedEntry": "data.txt",
                            "observed": "found"
                        }
                    },
                    {
                        "id": "assertion-12",
                        "status": "passed",
                        "checkpoint": "action-1",
                        "expectation": {
                            "kind": "stderrContains",
                            "status": "passed",
                            "expected": "hello world",
                            // The interpolated branch of TextExpectedSource, and the only place a
                            // binding reference item appears.
                            "expectedSource": {
                                "kind": "interpolated",
                                "form": "string",
                                "line": 4,
                                "column": 12,
                                "references": [
                                    {
                                        "name": "greeting",
                                        "line": 4,
                                        "column": 20,
                                        "actionIndex": 0,
                                        "stream": "stdout",
                                        "captureMode": "line"
                                    }
                                ]
                            },
                            "actualRef": "test-1/action-1/stderr.bin",
                            "actualSizeBytes": 11
                        }
                    }
                ]
            }
        ]
    })
}

/// The stdout document for the same run, derived by the projection
/// `spec/artifacts/run-result/README.md` defines: add `artifactRoot`, drop `noop`, drop evidence
/// digests.
///
/// Deriving it keeps the two base documents describing one run. Hand-maintaining a second literal
/// would let them drift into describing different runs, which would quietly weaken every case
/// that compares how the two contracts treat the same shape.
fn json_report_document() -> Value {
    let mut document = run_result_document();
    let object = document
        .as_object_mut()
        .expect("the base document is an object");
    object.remove("noop");
    object.insert(
        "artifactRoot".to_owned(),
        json!(".reportage/runs/schema-feature"),
    );
    for test in document["tests"].as_array_mut().expect("tests is an array") {
        for action in test["actions"].as_array_mut().expect("actions is an array") {
            for stream in ["stdout", "stderr"] {
                action[stream]
                    .as_object_mut()
                    .expect("an evidence reference is an object")
                    .remove("sha256");
            }
        }
    }
    document
}

// ---------------------------------------------------------------------------
// Instance editing helpers
// ---------------------------------------------------------------------------

/// Sets `pointer` to `value`, adding the member when the parent object does not have it yet.
///
/// The parent must resolve. A case built on a pointer whose parent no longer exists would still
/// produce an instance, and an "invalid" case would keep passing for a reason that has nothing to
/// do with the keyword it names.
fn set(document: &Value, pointer: &str, value: Value) -> Value {
    let mut document = document.clone();
    let (parent, key) = split_pointer(pointer);
    let target = document
        .pointer_mut(parent)
        .unwrap_or_else(|| panic!("no value at `{parent}` to set `{key}` on"));

    match target {
        Value::Object(members) => {
            members.insert(key.to_owned(), value);
        }
        Value::Array(items) => {
            let index: usize = key
                .parse()
                .unwrap_or_else(|_| panic!("`{key}` is not an array index in `{pointer}`"));
            assert!(
                index < items.len(),
                "index `{key}` is out of range in `{pointer}`"
            );
            items[index] = value;
        }
        other => panic!("`{parent}` is {other}, which has no member `{key}`"),
    }
    document
}

/// Removes the member or element at `pointer`, which must exist.
fn remove(document: &Value, pointer: &str) -> Value {
    let mut document = document.clone();
    let (parent, key) = split_pointer(pointer);
    let target = document
        .pointer_mut(parent)
        .unwrap_or_else(|| panic!("no value at `{parent}` to remove `{key}` from"));

    let removed = match target {
        Value::Object(members) => members.remove(key).is_some(),
        Value::Array(items) => match key.parse::<usize>() {
            Ok(index) if index < items.len() => {
                items.remove(index);
                true
            }
            _ => false,
        },
        _ => false,
    };
    assert!(removed, "nothing to remove at `{pointer}`");
    document
}

fn split_pointer(pointer: &str) -> (&str, &str) {
    let separator = pointer
        .rfind('/')
        .unwrap_or_else(|| panic!("`{pointer}` is not a JSON Pointer to a member"));
    (&pointer[..separator], &pointer[separator + 1..])
}

// ---------------------------------------------------------------------------
// Schema document traversal
// ---------------------------------------------------------------------------

/// Every keyword occurring in a schema position in `document`, paired with its JSON Pointer.
///
/// Descent is keyword-directed rather than blind: only the values of the keywords below are
/// schemas. A blind walk would read instance data — `$defs` definition names, `properties` member
/// names, the contents of a `const` or an `enum` — as if it were schema vocabulary, and report
/// keywords the contracts never use.
fn schema_keywords(document: &Value) -> Vec<(String, String)> {
    /// Keywords whose value is a single subschema.
    const SUBSCHEMA: &[&str] = &[
        "items",
        "not",
        "if",
        "then",
        "else",
        "additionalProperties",
        "propertyNames",
        "contains",
        "unevaluatedItems",
        "unevaluatedProperties",
    ];
    /// Keywords whose value is an array of subschemas.
    const SUBSCHEMA_ARRAY: &[&str] = &["allOf", "anyOf", "oneOf", "prefixItems"];
    /// Keywords whose value is an object whose members are subschemas.
    const SUBSCHEMA_MAP: &[&str] = &[
        "properties",
        "$defs",
        "patternProperties",
        "dependentSchemas",
    ];

    fn descend(schema: &Value, pointer: &str, found: &mut Vec<(String, String)>) {
        let Some(members) = schema.as_object() else {
            return;
        };
        for (keyword, value) in members {
            let keyword_pointer = format!("{pointer}/{}", escape_pointer_token(keyword));
            found.push((keyword.clone(), keyword_pointer.clone()));

            if SUBSCHEMA.contains(&keyword.as_str()) {
                descend(value, &keyword_pointer, found);
            } else if SUBSCHEMA_ARRAY.contains(&keyword.as_str()) {
                for (index, item) in value.as_array().into_iter().flatten().enumerate() {
                    descend(item, &format!("{keyword_pointer}/{index}"), found);
                }
            } else if SUBSCHEMA_MAP.contains(&keyword.as_str()) {
                for (name, item) in value.as_object().into_iter().flatten() {
                    descend(
                        item,
                        &format!("{keyword_pointer}/{}", escape_pointer_token(name)),
                        found,
                    );
                }
            }
        }
    }

    let mut found = Vec::new();
    descend(document, "", &mut found);
    found
}

/// RFC 6901 reference token escaping, so a pointer in a failure message can be pasted back.
fn escape_pointer_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

fn collect_refs(document: &Value) -> Vec<String> {
    let mut refs = Vec::new();
    walk(document, &mut |value| {
        if let Some(Value::String(reference)) = value.get("$ref") {
            refs.push(reference.clone());
        }
    });
    refs
}

fn document_contains_key(document: &Value, key: &str) -> bool {
    let mut found = false;
    walk(document, &mut |value| {
        found |= value.get(key).is_some();
    });
    found
}

fn walk(value: &Value, visit: &mut impl FnMut(&Value)) {
    match value {
        Value::Object(members) => {
            visit(value);
            for child in members.values() {
                walk(child, visit);
            }
        }
        Value::Array(items) => {
            for item in items {
                walk(item, visit);
            }
        }
        _ => {}
    }
}
