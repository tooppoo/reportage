//! Semantic spec schema validation.
//!
//! Loads every `spec/language/semantics/*.json` file, deserialises it into
//! typed Rust structs (with `deny_unknown_fields`), and checks fixture
//! consistency invariants. This is the CI-integrated validation gate that
//! ensures semantic spec files conform to the expected schema.

// Serde-populated struct fields are not "used" in the conventional sense;
// their value comes from deserialisation rather than direct assignment.
#![allow(dead_code)]

use base64::Engine as _;
use reportage_core::model::{Expectation, OutputMatcher, Step};
use reportage_core::parser::parse;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Typed representation of a semantic spec file
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticSpec {
    #[serde(rename = "$schema")]
    schema: String,
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
    id: String,
    category: Category,
    syntax: String,
    normative: NormativeFields,
    #[serde(rename = "conformanceCases")]
    conformance_cases: Vec<ConformanceCase>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum Category {
    Assertion,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NormativeFields {
    #[serde(rename = "checkpointField")]
    checkpoint_field: CheckpointField,
    operator: Operator,
    #[serde(rename = "expectedValueType")]
    expected_value_type: ExpectedValueType,
    #[serde(rename = "matchSemantics")]
    match_semantics: MatchSemantics,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum CheckpointField {
    ExitCode,
    Stdout,
    Stderr,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum ExpectedValueType {
    Uint8,
    Utf8String,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum Operator {
    Equals,
    Contains,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MatchSemantics {
    comparison: Comparison,
    #[serde(rename = "caseSensitive")]
    case_sensitive: Option<bool>,
    #[serde(rename = "lineEndingNormalization")]
    line_ending_normalization: Option<bool>,
    #[serde(rename = "emptyExpectedAlwaysMatches")]
    empty_expected_always_matches: Option<bool>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum Comparison {
    Exact,
    ByteSubstring,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConformanceCase {
    description: String,
    #[serde(rename = "assertionSource")]
    assertion_source: String,
    assertion: Assertion,
    checkpoint: Checkpoint,
    #[serde(rename = "expectedResult")]
    expected_result: ExpectedResult,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Assertion {
    subject: AssertionSubject,
    operator: Operator,
    expected: serde_json::Value,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum AssertionSubject {
    Exit,
    Stdout,
    Stderr,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Checkpoint {
    #[serde(rename = "exitCode")]
    exit_code: i32,
    stdout: StreamData,
    stderr: StreamData,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StreamData {
    data: String,
    encoding: Encoding,
    text: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum Encoding {
    Base64,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum ExpectedResult {
    Pass,
    Fail,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn spec_dir() -> PathBuf {
    // Resolve relative to the workspace root, not the crate root.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent() // crates/
        .unwrap()
        .parent() // workspace root
        .unwrap()
        .join("spec/language/semantics")
}

fn load_spec_files() -> Vec<(PathBuf, SemanticSpec)> {
    let dir = spec_dir();
    let mut entries: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", dir.display(), e))
        .filter_map(|entry| {
            let path = entry.expect("dir entry error").path();
            if path.extension().is_some_and(|e| e == "json")
                && path.file_name().is_some_and(|n| n != "schema.json")
            {
                Some(path)
            } else {
                None
            }
        })
        .collect();
    entries.sort();

    entries
        .into_iter()
        .map(|path| {
            let src = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("cannot read {}: {}", path.display(), e));
            let spec: SemanticSpec = serde_json::from_str(&src)
                .unwrap_or_else(|e| panic!("failed to deserialise {}: {}", path.display(), e));
            (path, spec)
        })
        .collect()
}

fn decode_base64_stream(stream: &StreamData) -> Vec<u8> {
    if stream.data.is_empty() {
        return vec![];
    }
    base64::engine::general_purpose::STANDARD
        .decode(&stream.data)
        .unwrap_or_else(|e| panic!("invalid base64 in stream data '{}': {}", stream.data, e))
}

/// Byte-level substring match matching the v0 `contains` semantics: an empty
/// needle always matches; otherwise a raw byte-window search with no
/// normalization.
fn byte_contains(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Evaluate a conformance case's normalized `assertion` against its `checkpoint`
/// using the v0 semantics documented in spec/language/semantics/README.md.
///
/// This reimplements the small v0 rule set (exit equals, byte-level substring
/// contains) directly rather than invoking the production evaluator. It is a
/// fixture-consistency check that the static `expectedResult` agrees with the
/// documented semantics; running conformance cases against the real semantic
/// evaluator is out of scope for #29 and belongs to #30.
fn evaluate_case_semantics(case: &ConformanceCase) -> ExpectedResult {
    let passed = match case.assertion.subject {
        AssertionSubject::Exit => {
            let expected = case
                .assertion
                .expected
                .as_i64()
                .expect("exit expected must be an integer");
            i64::from(case.checkpoint.exit_code) == expected
        }
        AssertionSubject::Stdout | AssertionSubject::Stderr => {
            let stream = match case.assertion.subject {
                AssertionSubject::Stdout => &case.checkpoint.stdout,
                _ => &case.checkpoint.stderr,
            };
            let haystack = decode_base64_stream(stream);
            let needle = case
                .assertion
                .expected
                .as_str()
                .expect("stdout/stderr expected must be a string");
            byte_contains(&haystack, needle.as_bytes())
        }
    };
    if passed {
        ExpectedResult::Pass
    } else {
        ExpectedResult::Fail
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn all_semantic_spec_files_load_successfully() {
    let specs = load_spec_files();
    assert!(
        !specs.is_empty(),
        "no semantic spec files found in {}",
        spec_dir().display()
    );
    // Loading without panic is the assertion; names are printed for context.
    for (path, spec) in &specs {
        println!("loaded: {} (id={})", path.display(), spec.id);
    }
}

#[test]
fn all_specs_reference_local_schema() {
    for (path, spec) in load_spec_files() {
        assert_eq!(
            spec.schema,
            "./schema.json",
            "{}: expected $schema ./schema.json, got {}",
            path.display(),
            spec.schema
        );
    }
}

#[test]
fn all_specs_have_schema_version_1() {
    for (path, spec) in load_spec_files() {
        assert_eq!(
            spec.schema_version,
            1,
            "{}: expected schemaVersion 1, got {}",
            path.display(),
            spec.schema_version
        );
    }
}

#[test]
fn all_specs_have_assertion_category() {
    for (path, spec) in load_spec_files() {
        assert_eq!(
            spec.category,
            Category::Assertion,
            "{}: unexpected category",
            path.display()
        );
    }
}

#[test]
fn all_specs_have_at_least_one_conformance_case() {
    for (path, spec) in load_spec_files() {
        assert!(
            !spec.conformance_cases.is_empty(),
            "{}: conformanceCases must not be empty",
            path.display()
        );
    }
}

#[test]
fn all_spec_ids_are_non_empty_and_dot_separated() {
    for (path, spec) in load_spec_files() {
        assert!(
            !spec.id.is_empty(),
            "{}: id must not be empty",
            path.display()
        );
        let parts: Vec<&str> = spec.id.split('.').collect();
        assert_eq!(
            parts.len(),
            3,
            "{}: id '{}' must have exactly three dot-separated parts",
            path.display(),
            spec.id
        );
        for part in &parts {
            assert!(
                !part.is_empty(),
                "{}: id '{}' must not have empty parts",
                path.display(),
                spec.id
            );
        }
    }
}

#[test]
fn stream_data_text_matches_base64_decoded_data() {
    // If `text` is present, it must equal the UTF-8 decoding of base64-decoded `data`.
    // This is a fixture consistency check, not a semantic rule.
    for (path, spec) in load_spec_files() {
        for (i, case) in spec.conformance_cases.iter().enumerate() {
            for (stream_name, stream) in [
                ("stdout", &case.checkpoint.stdout),
                ("stderr", &case.checkpoint.stderr),
            ] {
                if let Some(text) = &stream.text {
                    let decoded = decode_base64_stream(stream);
                    let decoded_str = std::str::from_utf8(&decoded).unwrap_or_else(|e| {
                        panic!(
                            "{} case[{}] {}: base64 data is not valid UTF-8: {}",
                            path.display(),
                            i,
                            stream_name,
                            e
                        )
                    });
                    assert_eq!(
                        decoded_str,
                        text.as_str(),
                        "{} case[{}] {}: text does not match base64-decoded data",
                        path.display(),
                        i,
                        stream_name
                    );
                }
            }
        }
    }
}

#[test]
fn all_stream_data_bytes_are_valid_base64() {
    // Validates every data field as base64, regardless of whether text is present.
    // text is optional (non-UTF-8 fixtures may omit it), but data is always normative.
    for (path, spec) in load_spec_files() {
        for (i, case) in spec.conformance_cases.iter().enumerate() {
            for (stream_name, stream) in [
                ("stdout", &case.checkpoint.stdout),
                ("stderr", &case.checkpoint.stderr),
            ] {
                if !stream.data.is_empty() {
                    base64::engine::general_purpose::STANDARD
                        .decode(&stream.data)
                        .unwrap_or_else(|e| {
                            panic!(
                                "{} case[{}] {}: data is not valid base64: {}",
                                path.display(),
                                i,
                                stream_name,
                                e
                            )
                        });
                }
            }
        }
    }
}

#[test]
fn exit_assertion_expected_is_integer() {
    for (path, spec) in load_spec_files() {
        for (i, case) in spec.conformance_cases.iter().enumerate() {
            if case.assertion.subject == AssertionSubject::Exit {
                assert!(
                    case.assertion.expected.is_i64() || case.assertion.expected.is_u64(),
                    "{} case[{}]: exit assertion expected must be an integer, got {:?}",
                    path.display(),
                    i,
                    case.assertion.expected
                );
            }
        }
    }
}

#[test]
fn exit_assertion_expected_is_in_uint8_range() {
    for (path, spec) in load_spec_files() {
        for (i, case) in spec.conformance_cases.iter().enumerate() {
            if case.assertion.subject == AssertionSubject::Exit {
                let value = case
                    .assertion
                    .expected
                    .as_u64()
                    .or_else(|| {
                        case.assertion
                            .expected
                            .as_i64()
                            .filter(|&v| v >= 0)
                            .map(|v| v as u64)
                    })
                    .unwrap_or_else(|| {
                        panic!(
                            "{} case[{}]: exit expected {:?} is negative or not a number",
                            path.display(),
                            i,
                            case.assertion.expected
                        )
                    });
                assert!(
                    value <= 255,
                    "{} case[{}]: exit expected {} is out of uint8 range 0-255",
                    path.display(),
                    i,
                    value
                );
            }
        }
    }
}

#[test]
fn stdout_stderr_assertion_expected_is_string() {
    for (path, spec) in load_spec_files() {
        for (i, case) in spec.conformance_cases.iter().enumerate() {
            if matches!(
                case.assertion.subject,
                AssertionSubject::Stdout | AssertionSubject::Stderr
            ) {
                assert!(
                    case.assertion.expected.is_string(),
                    "{} case[{}]: stdout/stderr assertion expected must be a string, got {:?}",
                    path.display(),
                    i,
                    case.assertion.expected
                );
            }
        }
    }
}

#[test]
fn all_spec_ids_match_filenames() {
    for (path, spec) in load_spec_files() {
        let expected_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_else(|| panic!("cannot read file stem from {}", path.display()));
        assert_eq!(
            spec.id,
            expected_id,
            "{}: spec id '{}' must match the filename stem '{}'",
            path.display(),
            spec.id,
            expected_id
        );
    }
}

#[test]
fn conformance_case_subjects_match_normative_checkpoint_field() {
    for (path, spec) in load_spec_files() {
        for (i, case) in spec.conformance_cases.iter().enumerate() {
            let subject_matches = matches!(
                (&case.assertion.subject, &spec.normative.checkpoint_field),
                (AssertionSubject::Exit, CheckpointField::ExitCode)
                    | (AssertionSubject::Stdout, CheckpointField::Stdout)
                    | (AssertionSubject::Stderr, CheckpointField::Stderr)
            );
            assert!(
                subject_matches,
                "{} case[{}]: assertion subject {:?} does not match normative checkpointField {:?}",
                path.display(),
                i,
                case.assertion.subject,
                spec.normative.checkpoint_field
            );
        }
    }
}

#[test]
fn v0_required_spec_ids_are_all_present() {
    // These are the v0 initial semantic rules defined in Issue #29 and listed
    // in spec/language/semantics/README.md. Removing any of these files must
    // fail CI.
    const REQUIRED_IDS: &[&str] = &[
        "assertion.exit.equals",
        "assertion.stdout.contains",
        "assertion.stderr.contains",
    ];

    let loaded_ids: std::collections::BTreeSet<String> = load_spec_files()
        .into_iter()
        .map(|(_, spec)| spec.id)
        .collect();

    for required in REQUIRED_IDS {
        assert!(
            loaded_ids.contains(*required),
            "required v0 semantic spec '{}' is missing from spec/language/semantics/",
            required
        );
    }
}

#[test]
fn conformance_case_operators_match_normative_operator() {
    for (path, spec) in load_spec_files() {
        for (i, case) in spec.conformance_cases.iter().enumerate() {
            assert_eq!(
                case.assertion.operator,
                spec.normative.operator,
                "{} case[{}]: assertion operator does not match normative operator",
                path.display(),
                i
            );
        }
    }
}

/// Parse a conformance case `assertionSource` through the real Reportage parser
/// and reduce the single parsed expectation to a normalised `(subject, operator,
/// expected)` triple comparable to a spec's `assertion` fields.
fn parse_source_to_normalized(source: &str) -> (&'static str, &'static str, serde_json::Value) {
    // Wrap the bare expectation in a minimal script so the grammar accepts it.
    let script_src = format!("case \"c\" {{\n  assert {{\n    {source}\n  }}\n}}\n");
    let script = parse(&script_src)
        .unwrap_or_else(|e| panic!("failed to parse assertionSource '{source}': {e}"));

    let block = match &script.cases[0].steps[0] {
        Step::AssertionBlock(b) => b,
        other => {
            panic!("assertionSource '{source}' did not parse to an assertion block: {other:?}")
        }
    };
    assert_eq!(
        block.expectations().len(),
        1,
        "assertionSource '{source}' must parse to exactly one expectation"
    );

    match &block.expectations()[0] {
        Expectation::Exit(e) => ("exit", "equals", serde_json::Value::from(e.expected)),
        Expectation::Stdout(e) => match &e.matcher {
            OutputMatcher::Contains(s) => {
                ("stdout", "contains", serde_json::Value::from(s.as_str()))
            }
            other => {
                panic!("unsupported stdout matcher in conformance source '{source}': {other:?}")
            }
        },
        Expectation::Stderr(e) => match &e.matcher {
            OutputMatcher::Contains(s) => {
                ("stderr", "contains", serde_json::Value::from(s.as_str()))
            }
            other => {
                panic!("unsupported stderr matcher in conformance source '{source}': {other:?}")
            }
        },
        other => panic!("unsupported expectation in conformance source '{source}': {other:?}"),
    }
}

#[test]
fn conformance_case_assertion_source_matches_normalized_assertion() {
    // Parsing the human-facing `assertionSource` through the real parser must
    // yield the same subject/operator/expected as the normalised `assertion`.
    // This prevents the two representations from drifting apart.
    for (path, spec) in load_spec_files() {
        for (i, case) in spec.conformance_cases.iter().enumerate() {
            let (parsed_subject, parsed_operator, parsed_expected) =
                parse_source_to_normalized(&case.assertion_source);

            let want_subject = match case.assertion.subject {
                AssertionSubject::Exit => "exit",
                AssertionSubject::Stdout => "stdout",
                AssertionSubject::Stderr => "stderr",
            };
            let want_operator = match case.assertion.operator {
                Operator::Equals => "equals",
                Operator::Contains => "contains",
            };

            assert_eq!(
                parsed_subject,
                want_subject,
                "{} case[{}]: assertionSource subject '{}' does not match normalized subject '{}'",
                path.display(),
                i,
                parsed_subject,
                want_subject
            );
            assert_eq!(
                parsed_operator,
                want_operator,
                "{} case[{}]: assertionSource operator '{}' does not match normalized operator '{}'",
                path.display(),
                i,
                parsed_operator,
                want_operator
            );
            assert_eq!(
                parsed_expected,
                case.assertion.expected,
                "{} case[{}]: assertionSource expected {:?} does not match normalized expected {:?}",
                path.display(),
                i,
                parsed_expected,
                case.assertion.expected
            );
        }
    }
}

#[test]
fn initial_v0_rules_have_expected_normative_fields() {
    // Pin the normative fields of the three v0 rules so that a machine-readable
    // source of truth cannot silently change its comparison semantics.
    for (path, spec) in load_spec_files() {
        let n = &spec.normative;
        let ms = &n.match_semantics;
        match spec.id.as_str() {
            "assertion.exit.equals" => {
                assert_eq!(
                    n.checkpoint_field,
                    CheckpointField::ExitCode,
                    "{}",
                    path.display()
                );
                assert_eq!(n.operator, Operator::Equals, "{}", path.display());
                assert_eq!(
                    n.expected_value_type,
                    ExpectedValueType::Uint8,
                    "{}",
                    path.display()
                );
                assert_eq!(ms.comparison, Comparison::Exact, "{}", path.display());
            }
            "assertion.stdout.contains" => {
                assert_eq!(
                    n.checkpoint_field,
                    CheckpointField::Stdout,
                    "{}",
                    path.display()
                );
                assert_eq!(n.operator, Operator::Contains, "{}", path.display());
                assert_eq!(
                    n.expected_value_type,
                    ExpectedValueType::Utf8String,
                    "{}",
                    path.display()
                );
                assert_eq!(
                    ms.comparison,
                    Comparison::ByteSubstring,
                    "{}",
                    path.display()
                );
                assert_eq!(ms.case_sensitive, Some(true), "{}", path.display());
                assert_eq!(
                    ms.line_ending_normalization,
                    Some(false),
                    "{}",
                    path.display()
                );
                assert_eq!(
                    ms.empty_expected_always_matches,
                    Some(true),
                    "{}",
                    path.display()
                );
            }
            "assertion.stderr.contains" => {
                assert_eq!(
                    n.checkpoint_field,
                    CheckpointField::Stderr,
                    "{}",
                    path.display()
                );
                assert_eq!(n.operator, Operator::Contains, "{}", path.display());
                assert_eq!(
                    n.expected_value_type,
                    ExpectedValueType::Utf8String,
                    "{}",
                    path.display()
                );
                assert_eq!(
                    ms.comparison,
                    Comparison::ByteSubstring,
                    "{}",
                    path.display()
                );
                assert_eq!(ms.case_sensitive, Some(true), "{}", path.display());
                assert_eq!(
                    ms.line_ending_normalization,
                    Some(false),
                    "{}",
                    path.display()
                );
                assert_eq!(
                    ms.empty_expected_always_matches,
                    Some(true),
                    "{}",
                    path.display()
                );
            }
            _ => {}
        }
    }
}

#[test]
fn conformance_case_expected_result_matches_v0_semantics() {
    // The static expectedResult must agree with evaluating the normalized
    // assertion against the checkpoint under the documented v0 semantics.
    for (path, spec) in load_spec_files() {
        for (i, case) in spec.conformance_cases.iter().enumerate() {
            let computed = evaluate_case_semantics(case);
            assert_eq!(
                computed,
                case.expected_result,
                "{} case[{}] ({}): computed result {:?} does not match declared expectedResult {:?}",
                path.display(),
                i,
                case.description,
                computed,
                case.expected_result
            );
        }
    }
}
