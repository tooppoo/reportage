//! Contract tests for JSON Schema artifact generation.
//!
//! Two kinds of test live here. Tests against a synthetic repository root exercise the
//! generator's rules; tests against the real repository root lock the committed schema pair
//! down, so a hand-edited public schema or a moved `x-reportage-snapshot` fails the suite and
//! not only the `just schema-artifacts-check` recipe.

use std::fs;
use std::path::Path;

use serde_json::Value;
use tempfile::TempDir;

use xtask::json::{self, JsonValue};
use xtask::output::{OutputFormat, ReportBody, render};
use xtask::schema_artifacts::{
    CONTRACTS, PreparationError, SNAPSHOT_ANNOTATION, SchemaContract, check, generate,
    public_schema_text, repository_root,
};

/// A json-report internal source schema carrying exactly the allowlisted annotations.
const JSON_REPORT_SOURCE: &str = r#"{
  "$id": "https://example.test/json-report",
  "properties": {
    "artifactRoot": {
      "type": "string",
      "x-reportage-snapshot": {
        "operation": "replace",
        "value": "<ARTIFACT_ROOT>"
      }
    }
  },
  "$defs": {
    "Tool": {
      "properties": {
        "version": {
          "type": "string",
          "x-reportage-snapshot": {
            "operation": "replace",
            "value": "<VERSION>"
          }
        }
      }
    }
  }
}
"#;

/// A run-result internal source schema carrying exactly the allowlisted annotation.
const RUN_RESULT_SOURCE: &str = r#"{
  "$id": "https://example.test/run-result",
  "$defs": {
    "Tool": {
      "properties": {
        "version": {
          "type": "string",
          "x-reportage-snapshot": {
            "operation": "replace",
            "value": "<VERSION>"
          }
        }
      }
    }
  }
}
"#;

fn contract(name: &str) -> &'static SchemaContract {
    CONTRACTS
        .iter()
        .find(|contract| contract.name == name)
        .expect("contract is registered")
}

/// Builds a repository root holding only the two internal source schemas.
fn synthetic_repository(json_report: &str, run_result: &str) -> TempDir {
    let root = TempDir::new().expect("temporary directory");
    write(
        root.path(),
        contract("json-report").internal_path,
        json_report,
    );
    write(
        root.path(),
        contract("run-result").internal_path,
        run_result,
    );
    root
}

fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("relative path has a parent"))
        .expect("create schema directory");
    fs::write(path, contents).expect("write schema file");
}

fn read(root: &Path, relative: &str) -> String {
    fs::read_to_string(root.join(relative)).expect("read schema file")
}

fn failure_text(report: &xtask::output::Report) -> String {
    let rendered = render(report, OutputFormat::Text);
    assert!(
        rendered.stdout.is_empty(),
        "a failing command must keep stdout empty"
    );
    rendered.stderr
}

fn error_code(report: &xtask::output::Report) -> &'static str {
    report.error().expect("report is a failure").code
}

// ---------------------------------------------------------------------------
// Stripping rules
// ---------------------------------------------------------------------------

#[test]
fn metadata_nested_in_objects_and_arrays_is_removed() {
    let source = r#"{
  "oneOf": [
    { "x-reportage-snapshot": { "operation": "replace", "value": "<A>" } },
    { "items": { "deep": { "x-reportage-snapshot": {} } } }
  ]
}"#;

    let generated = public_schema_text(
        source,
        &[
            "/oneOf/0/x-reportage-snapshot",
            "/oneOf/1/items/deep/x-reportage-snapshot",
        ],
    )
    .expect("both locations are allowlisted");

    assert!(!generated.contains(SNAPSHOT_ANNOTATION));
}

#[test]
fn everything_other_than_the_metadata_survives_generation() {
    let source = r##"{
  "$id": "https://example.test/s",
  "description": "kept",
  "enum": [3, 1, 2],
  "const": 1.5,
  "nullable": null,
  "flag": false,
  "$ref": "#/$defs/T",
  "x-reportage-other": { "kept": true },
  "$defs": {
    "T": {
      "type": "string",
      "x-reportage-snapshot": { "operation": "replace", "value": "<T>" }
    }
  }
}"##;

    let generated =
        public_schema_text(source, &["/$defs/T/x-reportage-snapshot"]).expect("allowlisted");

    let expected: Value = serde_json::from_str(
        r##"{
  "$id": "https://example.test/s",
  "description": "kept",
  "enum": [3, 1, 2],
  "const": 1.5,
  "nullable": null,
  "flag": false,
  "$ref": "#/$defs/T",
  "x-reportage-other": { "kept": true },
  "$defs": { "T": { "type": "string" } }
}"##,
    )
    .expect("valid JSON");

    assert_eq!(
        serde_json::from_str::<Value>(&generated).expect("valid JSON"),
        expected
    );
    // Array order is positional, so a value comparison alone would not catch a reordering.
    assert!(generated.contains("[\n    3,\n    1,\n    2\n  ]"));
}

#[test]
fn an_object_left_empty_by_stripping_is_kept() {
    let generated = public_schema_text(
        r#"{ "properties": { "a": { "x-reportage-snapshot": { "operation": "replace", "value": "<A>" } } } }"#,
        &["/properties/a/x-reportage-snapshot"],
    )
    .expect("allowlisted");

    assert_eq!(generated, "{\n  \"properties\": {\n    \"a\": {}\n  }\n}\n");
}

#[test]
fn a_local_ref_is_preserved_rather_than_inlined() {
    let generated = public_schema_text(
        r##"{ "properties": { "tool": { "$ref": "#/$defs/Tool" } }, "$defs": { "Tool": { "type": "object" } } }"##,
        &[],
    )
    .expect("no annotation to check");

    assert!(generated.contains("\"$ref\": \"#/$defs/Tool\""));
}

#[test]
fn generation_is_byte_for_byte_repeatable() {
    let first = public_schema_text(
        JSON_REPORT_SOURCE,
        contract("json-report").annotation_locations,
    )
    .expect("allowlisted");
    let second = public_schema_text(
        JSON_REPORT_SOURCE,
        contract("json-report").annotation_locations,
    )
    .expect("allowlisted");

    assert_eq!(first, second);

    let root = synthetic_repository(JSON_REPORT_SOURCE, RUN_RESULT_SOURCE);
    generate(root.path(), false);
    let after_first_run = read(root.path(), contract("json-report").public_path);
    generate(root.path(), false);

    assert_eq!(
        read(root.path(), contract("json-report").public_path),
        after_first_run
    );
}

// ---------------------------------------------------------------------------
// Allowlist and malformed input
// ---------------------------------------------------------------------------

#[test]
fn an_annotation_outside_the_allowlist_fails_generation_and_check() {
    let source = JSON_REPORT_SOURCE.replace(
        r#""$id": "https://example.test/json-report","#,
        r#""$id": "https://example.test/json-report",
  "title": { "x-reportage-snapshot": { "operation": "replace", "value": "<T>" } },"#,
    );
    let root = synthetic_repository(&source, RUN_RESULT_SOURCE);

    for report in [generate(root.path(), false), check(root.path())] {
        assert_eq!(error_code(&report), "SNAPSHOT_ANNOTATION_LOCATION_INVALID");
        let text = failure_text(&report);
        assert!(text.contains("SNAPSHOT_ANNOTATION_UNSUPPORTED_LOCATION"));
        assert!(text.contains("/title/x-reportage-snapshot"));
    }

    assert!(
        !root
            .path()
            .join(contract("json-report").public_path)
            .exists(),
        "a rejected source must not produce a public schema"
    );
}

#[test]
fn a_missing_allowlisted_annotation_fails_generation() {
    let source = RUN_RESULT_SOURCE.replace(
        r#""x-reportage-snapshot": {
            "operation": "replace",
            "value": "<VERSION>"
          }"#,
        r#""deprecated": false"#,
    );
    let report = generate(
        synthetic_repository(JSON_REPORT_SOURCE, &source).path(),
        false,
    );

    assert_eq!(error_code(&report), "SNAPSHOT_ANNOTATION_LOCATION_INVALID");
    let text = failure_text(&report);
    assert!(text.contains("SNAPSHOT_ANNOTATION_MISSING"));
    assert!(text.contains("/$defs/Tool/properties/version/x-reportage-snapshot"));
}

#[test]
fn a_malformed_internal_source_reports_its_position() {
    let root = synthetic_repository(JSON_REPORT_SOURCE, "{ \"a\": }");
    let report = generate(root.path(), false);

    assert_eq!(error_code(&report), "SOURCE_SCHEMA_MALFORMED");
    let text = failure_text(&report);
    assert!(text.contains("is not valid JSON"));
    assert!(text.contains("line 1, column 8"), "{text}");
    assert!(text.contains("spec/artifacts/run-result/schema.internal.json"));
}

#[test]
fn a_missing_internal_source_is_a_filesystem_failure() {
    let root = TempDir::new().expect("temporary directory");
    let report = check(root.path());

    assert_eq!(error_code(&report), "SOURCE_SCHEMA_UNREADABLE");
    assert_eq!(report.exit_code(), 4);
}

#[test]
fn preparation_errors_are_reported_before_any_comparison() {
    let error = public_schema_text("[]", &["/x-reportage-snapshot"])
        .expect_err("the allowlisted location is absent");

    assert!(matches!(error, PreparationError::Annotations(_)));
}

// ---------------------------------------------------------------------------
// Command behaviour
// ---------------------------------------------------------------------------

#[test]
fn generation_creates_a_missing_public_schema_and_check_then_passes() {
    let root = synthetic_repository(JSON_REPORT_SOURCE, RUN_RESULT_SOURCE);

    let generated = generate(root.path(), false);

    assert_eq!(generated.exit_code(), 0);
    assert_eq!(generated.file_changes.len(), 2);
    assert!(
        generated
            .file_changes
            .iter()
            .all(|change| change.action == xtask::output::FileAction::Create
                && change.state == xtask::output::FileState::Completed)
    );
    assert_eq!(check(root.path()).exit_code(), 0);
}

#[test]
fn a_dry_run_reports_planned_changes_without_writing() {
    let root = synthetic_repository(JSON_REPORT_SOURCE, RUN_RESULT_SOURCE);

    let report = generate(root.path(), true);

    assert!(report.dry_run);
    assert_eq!(report.file_changes.len(), 2);
    assert!(
        report
            .file_changes
            .iter()
            .all(|change| change.state == xtask::output::FileState::Planned)
    );
    assert!(
        !root
            .path()
            .join(contract("json-report").public_path)
            .exists()
    );
}

#[test]
fn regenerating_an_unchanged_public_schema_reports_no_file_change() {
    let root = synthetic_repository(JSON_REPORT_SOURCE, RUN_RESULT_SOURCE);
    generate(root.path(), false);

    let report = generate(root.path(), false);

    assert!(report.file_changes.is_empty());
    match &report.body {
        ReportBody::Success { result, .. } => {
            assert_eq!(result["contracts"][0]["state"], "unchanged");
        }
        other => panic!("expected success, got {other:?}"),
    }
}

#[test]
fn check_detects_a_missing_public_schema() {
    let root = synthetic_repository(JSON_REPORT_SOURCE, RUN_RESULT_SOURCE);
    generate(root.path(), false);
    fs::remove_file(root.path().join(contract("run-result").public_path))
        .expect("remove the generated public schema");

    let report = check(root.path());

    assert_eq!(error_code(&report), "PUBLIC_SCHEMA_OUT_OF_DATE");
    assert!(failure_text(&report).contains("PUBLIC_SCHEMA_MISSING"));
}

#[test]
fn check_detects_a_stale_public_schema_and_leaves_it_alone() {
    let root = synthetic_repository(JSON_REPORT_SOURCE, RUN_RESULT_SOURCE);
    generate(root.path(), false);
    let hand_edited = "{}\n";
    write(
        root.path(),
        contract("json-report").public_path,
        hand_edited,
    );

    let report = check(root.path());

    assert_eq!(report.exit_code(), 5);
    assert!(
        report.file_changes.is_empty(),
        "check must never report a mutation"
    );
    assert_eq!(
        read(root.path(), contract("json-report").public_path),
        hand_edited,
        "check must not rewrite the working tree"
    );
}

#[test]
fn a_check_failure_names_both_paths_the_classification_and_the_regeneration_command() {
    let root = synthetic_repository(JSON_REPORT_SOURCE, RUN_RESULT_SOURCE);
    generate(root.path(), false);
    write(root.path(), contract("json-report").public_path, "{}\n");

    let text = failure_text(&check(root.path()));

    assert!(text.contains("PUBLIC_SCHEMA_STALE"), "{text}");
    assert!(
        text.contains("internal source schema: spec/output/json-report/schema.internal.json"),
        "{text}"
    );
    assert!(
        text.contains("public schema:          spec/output/json-report/schema.json"),
        "{text}"
    );
    assert!(text.contains("just schema-artifacts-gen"), "{text}");
}

// ---------------------------------------------------------------------------
// The committed schema pair
// ---------------------------------------------------------------------------

#[test]
fn committed_public_schemas_are_up_to_date() {
    let report = check(&repository_root());

    assert_eq!(
        report.exit_code(),
        0,
        "{}",
        render(&report, OutputFormat::Text).stderr
    );
}

#[test]
fn internal_source_schemas_define_the_documented_replacement_policies() {
    let expected_values = [
        (
            "/properties/artifactRoot/x-reportage-snapshot",
            "<ARTIFACT_ROOT>",
        ),
        (
            "/$defs/Tool/properties/version/x-reportage-snapshot",
            "<VERSION>",
        ),
    ];

    for contract in CONTRACTS {
        let document = json::parse(&read(&repository_root(), contract.internal_path))
            .expect("internal source schema is valid JSON");

        for location in contract.annotation_locations {
            let annotation = resolve(&document, location)
                .unwrap_or_else(|| panic!("{location} exists in {}", contract.internal_path));
            let expected = expected_values
                .iter()
                .find(|(pointer, _)| pointer == location)
                .expect("location has a documented replacement value")
                .1;

            assert_eq!(
                annotation.get("operation"),
                Some(&JsonValue::String("replace".to_owned()))
            );
            assert_eq!(
                annotation.get("value"),
                Some(&JsonValue::String(expected.to_owned())),
                "{location} in {}",
                contract.internal_path
            );
        }
    }
}

#[test]
fn each_public_schema_is_its_internal_source_minus_the_metadata() {
    for contract in CONTRACTS {
        let mut internal: Value =
            serde_json::from_str(&read(&repository_root(), contract.internal_path))
                .expect("internal source schema is valid JSON");
        let public: Value = serde_json::from_str(&read(&repository_root(), contract.public_path))
            .expect("public schema is valid JSON");

        remove_annotations(&mut internal);

        assert_eq!(internal, public, "{}", contract.public_path);
        assert!(
            public["$id"].is_string(),
            "{} keeps its $id",
            contract.public_path
        );
    }
}

#[test]
fn each_public_schema_keeps_its_source_member_order_at_every_level() {
    for contract in CONTRACTS {
        let internal = json::parse(&read(&repository_root(), contract.internal_path))
            .expect("internal source schema is valid JSON");
        let public = json::parse(&read(&repository_root(), contract.public_path))
            .expect("public schema is valid JSON");

        assert_eq!(
            member_order(&internal),
            member_order(&public),
            "{}",
            contract.public_path
        );
    }
}

/// Follows a JSON Pointer of plain object keys. Enough for the annotation locations, which
/// never index arrays and never contain escaped tokens.
fn resolve<'a>(document: &'a JsonValue, pointer: &str) -> Option<&'a JsonValue> {
    pointer
        .split('/')
        .skip(1)
        .try_fold(document, |value, token| value.get(token))
}

/// Independent oracle for the stripping rule, so a defect in the generator cannot make the
/// comparison agree with itself.
fn remove_annotations(value: &mut Value) {
    match value {
        Value::Object(members) => {
            members.remove(SNAPSHOT_ANNOTATION);
            for member in members.values_mut() {
                remove_annotations(member);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(remove_annotations),
        _ => {}
    }
}

/// Every object's JSON Pointer paired with its member keys in source order, skipping annotation
/// subtrees so an internal source schema and its public schema are directly comparable.
fn member_order(value: &JsonValue) -> Vec<(String, Vec<String>)> {
    fn walk(value: &JsonValue, prefix: &str, out: &mut Vec<(String, Vec<String>)>) {
        match value {
            JsonValue::Object(members) => {
                out.push((
                    prefix.to_owned(),
                    members
                        .iter()
                        .map(|(key, _)| key.clone())
                        .filter(|key| key != SNAPSHOT_ANNOTATION)
                        .collect(),
                ));
                for (key, child) in members {
                    if key != SNAPSHOT_ANNOTATION {
                        walk(child, &format!("{prefix}/{key}"), out);
                    }
                }
            }
            JsonValue::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    walk(item, &format!("{prefix}/{index}"), out);
                }
            }
            _ => {}
        }
    }

    let mut out = Vec::new();
    walk(value, "", &mut out);
    out
}
