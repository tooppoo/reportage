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

/// A JSON Schema keyword the contracts rely on, named so a missing case is detectable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Feature {
    Const,
    Pattern,
    Minimum,
    Required,
    AdditionalProperties,
    OneOf,
    Conditional,
    NestedLocalRef,
    ExtensionKeyword,
}

/// Every feature a case table must cover. A keyword that appears in a schema with no case here is
/// a constraint nothing proves is enforced.
const REQUIRED_FEATURES: &[Feature] = &[
    Feature::Const,
    Feature::Pattern,
    Feature::Minimum,
    Feature::Required,
    Feature::AdditionalProperties,
    Feature::OneOf,
    Feature::Conditional,
    Feature::NestedLocalRef,
    Feature::ExtensionKeyword,
];

struct FeatureCase {
    feature: Feature,
    /// What the case demonstrates, phrased so a failure message reads as a sentence.
    description: &'static str,
    instance: Value,
    valid: bool,
}

/// Builds every case for one contract from that contract's own valid base document.
///
/// Cases are expressed as edits to a valid document rather than as standalone fragments, so an
/// "invalid" case fails for exactly the reason it names instead of for an unrelated omission.
fn feature_cases(base: Value, contract_specific: Vec<FeatureCase>) -> Vec<FeatureCase> {
    let mut cases = vec![
        FeatureCase {
            feature: Feature::Const,
            description: "the declared schemaVersion is the only accepted value",
            instance: base.clone(),
            valid: true,
        },
        FeatureCase {
            feature: Feature::Const,
            description: "a different schemaVersion is rejected",
            instance: set(&base, "/schemaVersion", json!(2)),
            valid: false,
        },
        FeatureCase {
            feature: Feature::Const,
            description: "an origin kind outside the two declared variants is rejected",
            instance: set(&base, "/diagnostics/0/origin/kind", json!("plugin")),
            valid: false,
        },
        FeatureCase {
            feature: Feature::Pattern,
            description: "a dotted lowercase diagnostic code matches the code pattern",
            instance: set(
                &base,
                "/diagnostics/0/code",
                json!("step.write.target_exists"),
            ),
            valid: true,
        },
        FeatureCase {
            feature: Feature::Pattern,
            description: "an uppercase diagnostic code is rejected",
            instance: set(&base, "/diagnostics/0/code", json!("Parse.Syntax")),
            valid: false,
        },
        FeatureCase {
            feature: Feature::Pattern,
            description: "an undotted diagnostic code is rejected",
            instance: set(&base, "/diagnostics/0/code", json!("parse")),
            valid: false,
        },
        FeatureCase {
            feature: Feature::Minimum,
            description: "a zero summary count is accepted",
            instance: set(&base, "/summary/passed", json!(0)),
            valid: true,
        },
        FeatureCase {
            feature: Feature::Minimum,
            description: "a negative summary count is rejected",
            instance: set(&base, "/summary/passed", json!(-1)),
            valid: false,
        },
        FeatureCase {
            feature: Feature::Minimum,
            description: "a diagnostic location line below one is rejected",
            instance: set(&base, "/diagnostics/0/location/line", json!(0)),
            valid: false,
        },
        FeatureCase {
            feature: Feature::Required,
            description: "an optional diagnostic code may be absent",
            instance: remove(&base, "/diagnostics/0/code"),
            valid: true,
        },
        FeatureCase {
            feature: Feature::Required,
            description: "a missing top-level tests array is rejected",
            instance: remove(&base, "/tests"),
            valid: false,
        },
        FeatureCase {
            feature: Feature::Required,
            description: "a diagnostic without the always-present location key is rejected",
            instance: remove(&base, "/diagnostics/0/location"),
            valid: false,
        },
        FeatureCase {
            feature: Feature::AdditionalProperties,
            description: "an unknown top-level field is rejected",
            instance: set(&base, "/runDuration", json!(12)),
            valid: false,
        },
        FeatureCase {
            feature: Feature::AdditionalProperties,
            description: "an unknown field inside a nested closed object is rejected",
            instance: set(&base, "/tool/commit", json!("abc123")),
            valid: false,
        },
        FeatureCase {
            feature: Feature::AdditionalProperties,
            description: "a shim invocation may carry fields the renderer does not define",
            instance: set(
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
            valid: true,
        },
        FeatureCase {
            feature: Feature::OneOf,
            description: "the test-origin variant of a diagnostic origin is accepted",
            instance: set(
                &base,
                "/diagnostics/0/origin",
                json!({ "kind": "test", "test": "test-1" }),
            ),
            valid: true,
        },
        FeatureCase {
            feature: Feature::OneOf,
            description: "an origin mixing both variants' fields matches neither",
            instance: set(
                &base,
                "/diagnostics/0/origin",
                json!({ "kind": "source", "source": "feature.repor", "test": "test-1" }),
            ),
            valid: false,
        },
        FeatureCase {
            feature: Feature::OneOf,
            description: "an expectation whose kind is not one of the declared kinds is rejected",
            instance: set(
                &base,
                "/tests/0/assertions/0/expectation",
                json!({ "kind": "exitsQuietly", "status": "passed" }),
            ),
            valid: false,
        },
        FeatureCase {
            feature: Feature::Conditional,
            description: "a compared contents comparison carries its comparison fields",
            instance: base.clone(),
            valid: true,
        },
        FeatureCase {
            feature: Feature::Conditional,
            description: "a compared contents comparison without an outcome is rejected",
            instance: remove(&base, "/tests/0/assertions/1/expectation/outcome"),
            valid: false,
        },
        FeatureCase {
            feature: Feature::Conditional,
            description: "a mismatching contents comparison without a mismatch object is rejected",
            instance: remove(&base, "/tests/0/assertions/1/expectation/mismatch"),
            valid: false,
        },
        FeatureCase {
            feature: Feature::Conditional,
            description: "a matching contents comparison needs no mismatch object",
            instance: remove(
                &set(
                    &base,
                    "/tests/0/assertions/1/expectation/outcome",
                    json!("match"),
                ),
                "/tests/0/assertions/1/expectation/mismatch",
            ),
            valid: true,
        },
        FeatureCase {
            feature: Feature::NestedLocalRef,
            description: "a logical composition recurses into child expectations",
            instance: base.clone(),
            valid: true,
        },
        FeatureCase {
            feature: Feature::NestedLocalRef,
            description: "a defect inside a logical composition's child is rejected",
            instance: set(
                &base,
                "/tests/0/assertions/2/expectation/children/0/expected",
                json!("zero"),
            ),
            valid: false,
        },
        FeatureCase {
            feature: Feature::NestedLocalRef,
            description: "a logical composition nested inside another is rejected when its operator is not declared",
            instance: set(
                &base,
                "/tests/0/assertions/2/expectation/children/0",
                json!({
                    "kind": "logical",
                    "status": "passed",
                    "operator": "either",
                    "children": []
                }),
            ),
            valid: false,
        },
    ];

    cases.extend(contract_specific);
    cases
}

fn json_report_cases() -> Vec<FeatureCase> {
    let base = json_report_document();
    feature_cases(
        base.clone(),
        vec![
            FeatureCase {
                feature: Feature::ExtensionKeyword,
                // `artifactRoot` and `tool.version` are the two locations the internal source
                // schema annotates. A snapshot placeholder is what the normalization harness
                // substitutes there, so it has to stay an ordinary accepted string.
                description: "both snapshot-annotated locations accept their placeholders",
                instance: set(
                    &set(&base, "/artifactRoot", json!("<ARTIFACT_ROOT>")),
                    "/tool/version",
                    json!("<VERSION>"),
                ),
                valid: true,
            },
            FeatureCase {
                feature: Feature::ExtensionKeyword,
                description: "an annotated location still enforces its declared type",
                instance: set(&base, "/artifactRoot", json!(7)),
                valid: false,
            },
            FeatureCase {
                feature: Feature::Required,
                description: "the stdout document's artifactRoot is required",
                instance: remove(&base, "/artifactRoot"),
                valid: false,
            },
            FeatureCase {
                feature: Feature::AdditionalProperties,
                description: "the artifact-only noop field is not part of the stdout document",
                instance: set(&base, "/noop", json!(false)),
                valid: false,
            },
            FeatureCase {
                feature: Feature::AdditionalProperties,
                description: "the artifact-only evidence digest is not part of the stdout document",
                instance: set(
                    &base,
                    "/tests/0/actions/0/stdout/sha256",
                    json!(EMPTY_SHA256),
                ),
                valid: false,
            },
        ],
    )
}

fn run_result_cases() -> Vec<FeatureCase> {
    let base = run_result_document();
    feature_cases(
        base.clone(),
        vec![
            FeatureCase {
                feature: Feature::ExtensionKeyword,
                description: "the snapshot-annotated tool version accepts its placeholder",
                instance: set(&base, "/tool/version", json!("<VERSION>")),
                valid: true,
            },
            FeatureCase {
                feature: Feature::ExtensionKeyword,
                description: "an annotated location still enforces its declared type",
                instance: set(&base, "/tool/version", json!(7)),
                valid: false,
            },
            FeatureCase {
                feature: Feature::Required,
                description: "the canonical manifest's noop field is required",
                instance: remove(&base, "/noop"),
                valid: false,
            },
            FeatureCase {
                feature: Feature::Required,
                description: "an evidence reference without its digest is rejected",
                instance: remove(&base, "/tests/0/actions/0/stdout/sha256"),
                valid: false,
            },
            FeatureCase {
                feature: Feature::Pattern,
                description: "an evidence digest that is not lowercase hex is rejected",
                instance: set(
                    &base,
                    "/tests/0/actions/0/stdout/sha256",
                    json!(EMPTY_SHA256.to_uppercase()),
                ),
                valid: false,
            },
            FeatureCase {
                feature: Feature::AdditionalProperties,
                description: "the stdout-only artifactRoot is not part of the canonical manifest",
                instance: set(&base, "/artifactRoot", json!(".reportage/runs/run-1")),
                valid: false,
            },
        ],
    )
}

#[test]
fn schema_feature_cases_are_accepted_or_rejected_as_declared() {
    for (contract, cases) in [
        (&*JSON_REPORT, json_report_cases()),
        (&*RUN_RESULT, run_result_cases()),
    ] {
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
    for (contract, cases) in [
        (&*JSON_REPORT, json_report_cases()),
        (&*RUN_RESULT, run_result_cases()),
    ] {
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

#[test]
fn every_required_schema_feature_has_a_valid_and_an_invalid_case() {
    for (contract, cases) in [
        (&*JSON_REPORT, json_report_cases()),
        (&*RUN_RESULT, run_result_cases()),
    ] {
        for feature in REQUIRED_FEATURES {
            let covered: Vec<bool> = cases
                .iter()
                .filter(|case| case.feature == *feature)
                .map(|case| case.valid)
                .collect();
            assert!(
                covered.contains(&true) && covered.contains(&false),
                "the {} contract needs both a valid and an invalid case for {feature:?}, got {covered:?}",
                contract.name(),
            );
        }
    }
}

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

// ---------------------------------------------------------------------------
// Base documents
// ---------------------------------------------------------------------------

/// SHA-256 of the empty byte string, used wherever a case needs a well-formed digest whose value
/// is not what the case is about.
const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// A valid artifact `result.json`, exercising the shapes the feature cases mutate: a diagnostic
/// with a code and a location, an evidence reference, a conditional contents comparison, and a
/// logical composition whose children recurse through the expectation `$ref`.
///
/// This is hand-built rather than captured from a fixture run: a case that removes a required
/// field has to start from a document whose every field is deliberately there.
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
                            "mismatch": {
                                "firstDiffOffset": 0,
                                "firstDiffLine": 1,
                                "actualContext": "hello\\n",
                                "expectedContext": "HELLO"
                            }
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
