//! Representative-fixture conformance for the artifact `result.json` manifest (issues #102, #192).
//!
//! Fixtures live in `tests/fixtures/run_result/*.repor`.
//! Each has a companion `<name>.snapshot.json` with the values the schema annotates as volatile normalised out, refreshed via `UPDATE_RUN_RESULT_SNAPSHOTS`, mirroring `json_report_fixtures.rs`'s convention.
//!
//! Which values are normalised out is not decided here: they are the ones `spec/artifacts/run-result/schema.internal.json` annotates with `x-reportage-snapshot`, so adding a volatile field to the contract means annotating it beside its definition rather than editing an instance path into this file.
//! Snapshots are then written by the shared deterministic formatter, so their object key order is a property of the harness rather than of whichever map `serde_json` was built with.
//! Both stages are `support/snapshot_normalization/`, shared with `json_report_fixtures.rs`; that the schema decides what is normalised, and that preparation is separate from application, is docs/adr/20260723T160117Z_json-schema-driven-snapshot-normalization-foundation.md.
//!
//! Each fixture run's `result.json` is checked by four separate suites here, because they establish four different things and can fail independently:
//!
//! 1. **Schema conformance** — the manifest satisfies the authoritative JSON Schema at `spec/artifacts/run-result/schema.json`, checked with the `jsonschema` crate (`support/json_schema.rs`).
//! 2. **Typed Rust consumer compatibility** — this repository's own consumers can deserialize the manifest, checked against a model of the *full* stable contract (`run_result_fixtures/typed_consumer_model.rs`).
//! 3. **Semantic and integrity invariants** — properties JSON Schema cannot state: `diagnosticRef` resolving within the document, summary counts agreeing with the results they count, and a test naming a source file that exists (`support/invariants.rs`), plus evidence files carrying the byte size and SHA-256 digest the manifest records.
//! 4. **Projection parity** — for the same run, the `--format=json` stdout document agrees with `result.json` on the parity items required by issue #102, and is exactly the canonical document minus the defined projection differences.
//!
//! Typed deserialization is deliberately *not* called schema validation: modelling the full contract is a consumer-side requirement, and even a full model enforces neither the schema's `const`, `pattern`, `minimum`, nor its conditional requirements. See docs/adr/20260728T092956Z_json-contract-validation-policy.md.
//!
//! Where a suite above names a module, that module is where the check is implemented and this file uses only what it exposes. Everything else stays here as the harness the four suites are written against: fixture discovery, the CLI run, snapshot comparison, and the assertions made directly over the JSON.
//!
//! `spec/artifacts/run-result/schema.json` is generated: edit `spec/artifacts/run-result/schema.internal.json` and run `just schema-artifacts-gen`.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use assert_cmd::Command;
use assert_fs::TempDir;
use assert_fs::prelude::*;
use reportage_core::run_result::sha256_hex;
use serde_json::Value;

#[path = "support/invariants.rs"]
mod invariants;
#[path = "support/json_schema.rs"]
mod json_schema;
#[path = "support/snapshot_normalization/mod.rs"]
mod snapshot_normalization;
#[path = "run_result_fixtures/typed_consumer_model.rs"]
mod typed_consumer_model;

use json_schema::{RUN_RESULT, SchemaVariant};
use snapshot_normalization::{NormalizationPlan, apply, format_snapshot, prepare};

// ---------------------------------------------------------------------------
// Fixture / CLI helpers
// ---------------------------------------------------------------------------

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture_dir() -> PathBuf {
    repo_root().join("tests/fixtures/run_result")
}

fn fixture_paths() -> Vec<PathBuf> {
    let pattern = fixture_dir()
        .join("*.repor")
        .to_str()
        .expect("fixture glob path must be valid UTF-8")
        .to_string();

    let mut paths = glob::glob(&pattern)
        .expect("run_result fixture glob pattern must be valid")
        .map(|entry| entry.expect("run_result fixture path must be readable"))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn fixture_stem(path: &Path) -> &str {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .expect("run_result fixture file name must be valid UTF-8")
}

fn snapshot_path_for_fixture(path: &Path) -> PathBuf {
    path.with_extension("snapshot.json")
}

fn update_snapshots_enabled() -> bool {
    std::env::var_os("UPDATE_RUN_RESULT_SNAPSHOTS").is_some()
}

/// The fixed run id every fixture run in this suite uses; each run gets its own temp dir, so reuse across runs never collides.
const RUN_ID: &str = "run-result-fixture";

/// Copies `fixture` into a fresh temp dir (under its own file name) and runs `reportage --debug-run-id <RUN_ID> [--format json] <file>` there.
/// Returns the run directory containing `result.json`, the parsed stdout (JSON document when `json_stdout`, otherwise `None`), the process exit code, and the temp dir keeping everything alive.
fn run_fixture(fixture: &Path, json_stdout: bool) -> (PathBuf, Option<Value>, i32, TempDir) {
    let dir = TempDir::new().unwrap();
    let name = fixture.file_name().unwrap().to_str().unwrap();
    let content = std::fs::read_to_string(fixture).unwrap();
    dir.child(name).write_str(&content).unwrap();

    let mut cmd = Command::cargo_bin("reportage").unwrap();
    cmd.current_dir(&dir).arg("--debug-run-id").arg(RUN_ID);
    if json_stdout {
        cmd.arg("--format").arg("json");
    }
    let output = cmd.arg(name).output().unwrap();

    let stdout_doc = if json_stdout {
        let stdout = String::from_utf8(output.stdout).unwrap();
        Some(serde_json::from_str(&stdout).unwrap_or_else(|e| {
            panic!("stdout was not a single valid JSON document: {e}\n{stdout}")
        }))
    } else {
        None
    };

    let run_dir = dir.path().join(".reportage/runs").join(RUN_ID);
    (run_dir, stdout_doc, output.status.code().unwrap(), dir)
}

fn read_result_json(run_dir: &Path) -> Value {
    let path = run_dir.join("result.json");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("{} is not valid JSON: {e}", path.display()))
}

/// The plan compiled from this contract's annotations, prepared once for every fixture.
///
/// Preparation is per schema and application is per document, so a defect in the annotations is reported against the schema once rather than rediscovered against whichever fixture reached it.
/// A failure here is a schema preparation error, distinct from the application error a fixture document can cause below and from a snapshot mismatch.
///
/// The plan holds one instruction where the `--format=json` one holds two: the artifact document has no `artifactRoot` to normalise, because it resolves references against its own directory, and evidence digests and sizes are deterministic for these fixtures.
static SNAPSHOT_PLAN: LazyLock<NormalizationPlan> = LazyLock::new(|| {
    let path = RUN_RESULT.path(SchemaVariant::InternalSource);
    prepare(RUN_RESULT.document(SchemaVariant::InternalSource)).unwrap_or_else(|error| {
        panic!("{path} could not be prepared for snapshot normalization: {error}")
    })
});

// ---------------------------------------------------------------------------
// Completeness
// ---------------------------------------------------------------------------

#[test]
fn all_required_representative_scenarios_are_present() {
    const REQUIRED: &[&str] = &[
        "passed",
        "assertion_failure",
        "parse_error",
        "semantic_error",
        "runtime_error",
        "partial_execution_after_runtime_error",
        "expectation_kinds",
        "contents_equals",
        "text_equals",
        "interpolated_text",
        "noop",
    ];

    let stems: std::collections::BTreeSet<String> = fixture_paths()
        .iter()
        .map(|p| fixture_stem(p).to_string())
        .collect();

    for required in REQUIRED {
        assert!(
            stems.contains(*required),
            "required representative fixture '{required}' is missing from tests/fixtures/run_result/"
        );
    }
    assert_eq!(
        stems.len(),
        REQUIRED.len(),
        "unexpected extra fixture(s) in tests/fixtures/run_result/: {stems:?}"
    );
}

// ---------------------------------------------------------------------------
// JSON Schema conformance
// ---------------------------------------------------------------------------

#[test]
fn every_fixture_result_json_conforms_to_the_run_result_schema() {
    let paths = fixture_paths();
    assert!(
        !paths.is_empty(),
        "expected at least one run_result fixture"
    );

    for path in paths {
        let (run_dir, _stdout, _exit_code, _dir) = run_fixture(&path, false);
        let json = read_result_json(&run_dir);
        RUN_RESULT.assert_conforms(
            &json,
            &format!("the result.json of fixture {}", path.display()),
        );
    }
}

// ---------------------------------------------------------------------------
// Typed Rust consumer compatibility (over the full stable contract)
// ---------------------------------------------------------------------------

#[test]
fn every_fixture_result_json_deserializes_into_the_typed_consumer_model() {
    let paths = fixture_paths();
    assert!(
        !paths.is_empty(),
        "expected at least one run_result fixture"
    );

    for path in paths {
        let (run_dir, _stdout, _exit_code, _dir) = run_fixture(&path, false);
        typed_consumer_model::assert_deserializes(
            &read_result_json(&run_dir),
            &path.display().to_string(),
        );
    }
}

/// The `result.json` of `fixture`, asserted to be one the typed consumer model accepts.
///
/// Asserted through the entry point rather than through [`typed_consumer_model_rejects`], so that a
/// fixture that stopped deserializing is reported with serde's own error rather than as a bare
/// boolean — and so that a rejection either test below reports is attributable to the mutation it
/// applied rather than to something the fixture never satisfied.
fn accepted_result_json(fixture: &Path) -> Value {
    let (run_dir, _stdout, _exit_code, _dir) = run_fixture(fixture, false);
    let document = read_result_json(&run_dir);
    typed_consumer_model::assert_deserializes(&document, &fixture.display().to_string());
    document
}

/// Whether the typed consumer model rejects `document`.
///
/// `catch_unwind` rather than `#[should_panic]` because every departure below has to be rejected and
/// `#[should_panic]` would be satisfied by the first one that was. No panic hook is installed to
/// quiet the expected panics: the hook is process-wide, and this target's other tests run as threads
/// in that same process under `cargo test`, so silencing it would silence their failures too.
fn typed_consumer_model_rejects(document: &Value) -> bool {
    std::panic::catch_unwind(|| {
        typed_consumer_model::assert_deserializes(document, "a deliberately departing document")
    })
    .is_err()
}

/// Departures from the contract the typed consumer model has to notice, as mutations of a document
/// it accepts.
///
/// One per property the model has because it was written to have it, rather than because a derive
/// gives it away: a field required by being declared non-`Option`, `location` having to be present
/// at all, and its value having to be `null` or a `Location`. The last two are the same field, and
/// deliberately so: `location: Value` keeps "missing key" and "present but null" apart, so the
/// `Option<Location>` that would collapse them is caught by removing the key, while deleting the
/// walk over `diagnostics` is caught only by giving the key a value that is neither.
///
/// Closed shapes are not listed here. They are covered by
/// [`the_typed_consumer_model_notices_any_closed_shape_opening`], which reaches them by sweeping the
/// fixtures' own documents rather than by naming the few a table would.
const REJECTED_DEPARTURES: &[(&str, fn(&mut Value))] = &[
    ("the required `noop` field removed", |document| {
        document.as_object_mut().unwrap().remove("noop").unwrap();
    }),
    ("the always-present `location` key removed", |document| {
        document["diagnostics"][0]
            .as_object_mut()
            .unwrap()
            .remove("location")
            .unwrap();
    }),
    (
        "a diagnostic `location` that is neither null nor a Location",
        |document| {
            document["diagnostics"][0]["location"] = Value::String("somewhere".to_string());
        },
    ),
];

/// Guards the suite above against becoming vacuous.
///
/// It only ever feeds the model real producer output, so making a required field `Option` or
/// deleting the walk over `diagnostics` would leave it green while it checked nothing.
#[test]
fn the_typed_consumer_model_rejects_a_document_that_departs_from_the_contract() {
    // One fixture is enough for these three, and it is chosen for shape rather than for its
    // scenario: a diagnostic for the last two to apply to, and a null `location` on it, so that
    // accepting this document is what establishes "present but null" while the departure below
    // establishes "missing". Keeping those two halves in one document is the whole reason `location`
    // is typed as an open `Value`. This is the smallest fixture that qualifies.
    let accepted = accepted_result_json(&fixture_dir().join("runtime_error.repor"));

    for (departure, apply) in REJECTED_DEPARTURES {
        let mut departing = accepted.clone();
        apply(&mut departing);
        assert_ne!(
            departing, accepted,
            "the mutation for {departure} left the document unchanged, so it cannot be rejected for it"
        );
        assert!(
            typed_consumer_model_rejects(&departing),
            "the typed consumer model accepted a result.json with {departure}; it no longer enforces what this suite reports it does"
        );
    }
}

/// Guards every shape the model declares closed, by opening each one in turn.
///
/// `deny_unknown_fields` is stated at fifteen sites in the model, four of them internally tagged
/// enums whose serde code path differs from a struct's. Naming a couple in [`REJECTED_DEPARTURES`]
/// would show the attribute still works somewhere, not that any particular shape still carries it —
/// the same one-case-for-many-definitions gap `json_contract_schemas.rs` covers by generation on the
/// schema side.
///
/// The sweep runs over every fixture rather than over one, because a shape no fixture instantiates
/// is a shape this test does not reach: `Location`, `ExpectedSource`, `ContentsMismatch`, and
/// `InterpolatedBindingReference` appear in no single fixture's document together with the rest.
/// The requirement is therefore on the fixture set as a whole, which
/// `all_required_representative_scenarios_are_present` is what pins.
///
/// Every object the fixtures' documents contain maps to a shape the model declares closed, so this
/// needs no exemption list. The model's one open field, `shimInvocations`, is a `Vec<Value>` that no
/// fixture currently populates; a contract shape that is genuinely open would fail here, which is
/// the right way to find out that it has to be accounted for.
#[test]
fn the_typed_consumer_model_notices_any_closed_shape_opening() {
    let mut opened = 0usize;
    for path in fixture_paths() {
        let accepted = accepted_result_json(&path);
        let context = path.display();

        for pointer in object_pointers(&accepted) {
            let mut document = accepted.clone();
            document
                .pointer_mut(&pointer)
                .expect("a pointer collected from this document must resolve in its clone")
                .as_object_mut()
                .expect("a pointer collected at an object must still address an object")
                .insert("unexpectedField".to_string(), Value::Bool(true));
            assert!(
                typed_consumer_model_rejects(&document),
                "fixture {context}: the typed consumer model accepted an undefined member at {}; the shape there is no longer closed",
                render_pointer(&pointer)
            );
            opened += 1;
        }
    }

    assert!(
        opened > 0,
        "no object was opened, so this test checked nothing"
    );
}

/// JSON pointers of every object in `document`, the document itself included as the empty pointer.
///
/// Keys are not escaped because every member name this contract defines is a camelCase identifier.
/// `json_contract_schemas.rs` carries the schema-side twin of this walk and of [`render_pointer`];
/// they are separate because that one visits every value rather than only objects, so a convention
/// changed in one belongs in the other.
fn object_pointers(document: &Value) -> Vec<String> {
    fn collect(value: &Value, pointer: String, found: &mut Vec<String>) {
        match value {
            Value::Object(members) => {
                found.push(pointer.clone());
                for (key, member) in members {
                    collect(member, format!("{pointer}/{key}"), found);
                }
            }
            Value::Array(elements) => {
                for (index, element) in elements.iter().enumerate() {
                    collect(element, format!("{pointer}/{index}"), found);
                }
            }
            _ => {}
        }
    }

    let mut found = Vec::new();
    collect(document, String::new(), &mut found);
    found
}

fn render_pointer(pointer: &str) -> &str {
    if pointer.is_empty() {
        "the document root"
    } else {
        pointer
    }
}

// ---------------------------------------------------------------------------
// Semantic and integrity invariants
//
// Properties of a valid manifest that JSON Schema cannot state, kept as their own tests so a
// domain violation never reads as a contract violation or the other way around.
// ---------------------------------------------------------------------------

#[test]
fn every_diagnostic_ref_resolves_to_a_diagnostic_in_the_same_document() {
    for path in fixture_paths() {
        let (run_dir, _stdout, _exit_code, _dir) = run_fixture(&path, false);
        invariants::assert_diagnostic_refs_resolve(
            &read_result_json(&run_dir),
            &path.display().to_string(),
        );
    }
}

#[test]
fn no_logical_composition_child_carries_its_own_diagnostic_ref() {
    let mut inspected = 0;
    for path in fixture_paths() {
        let (run_dir, _stdout, _exit_code, _dir) = run_fixture(&path, false);
        inspected += invariants::assert_logical_children_have_no_diagnostic_ref(
            &read_result_json(&run_dir),
            &path.display().to_string(),
        );
    }
    invariants::assert_failed_logical_compositions_were_inspected(inspected);
}

#[test]
fn the_summary_agrees_with_the_concrete_results_it_counts() {
    for path in fixture_paths() {
        let (run_dir, _stdout, _exit_code, _dir) = run_fixture(&path, false);
        invariants::assert_summary_agrees_with_results(
            &read_result_json(&run_dir),
            &path.display().to_string(),
        );
    }
}

#[test]
fn every_test_entry_names_a_source_file_that_exists() {
    for path in fixture_paths() {
        let (run_dir, _stdout, _exit_code, dir) = run_fixture(&path, false);
        invariants::assert_test_paths_exist(
            &read_result_json(&run_dir),
            dir.path(),
            &path.display().to_string(),
        );
    }
}

// ---------------------------------------------------------------------------
// Evidence integrity
// ---------------------------------------------------------------------------

#[test]
fn evidence_files_match_their_manifest_references() {
    for path in fixture_paths() {
        let (run_dir, _stdout, _exit_code, _dir) = run_fixture(&path, false);
        let json = read_result_json(&run_dir);

        let mut references = 0usize;
        for test in json["tests"].as_array().unwrap() {
            for action in test["actions"].as_array().unwrap() {
                for stream in ["stdout", "stderr"] {
                    let reference = &action[stream];
                    let artifact_ref = reference["artifactRef"].as_str().unwrap();
                    let evidence_path = run_dir.join(artifact_ref);
                    let bytes = std::fs::read(&evidence_path).unwrap_or_else(|e| {
                        panic!(
                            "fixture {}: evidence file {} referenced by result.json is missing: {e}",
                            path.display(),
                            evidence_path.display()
                        )
                    });
                    assert_eq!(
                        bytes.len() as u64,
                        reference["sizeBytes"].as_u64().unwrap(),
                        "fixture {}: sizeBytes must match the evidence file {}",
                        path.display(),
                        artifact_ref
                    );
                    assert_eq!(
                        sha256_hex(&bytes),
                        reference["sha256"].as_str().unwrap(),
                        "fixture {}: sha256 must match the evidence file {}",
                        path.display(),
                        artifact_ref
                    );
                    references += 1;
                }
            }
        }
        // Only fixtures whose run executed at least one action produce references; for those, an accidentally empty loop must not vacuously pass.
        if !json["tests"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["actions"].as_array().unwrap().is_empty())
        {
            assert!(references > 0);
        }
    }
}

// ---------------------------------------------------------------------------
// Snapshot validation
// ---------------------------------------------------------------------------

#[test]
fn normalization_replaces_the_volatile_value_the_schema_annotates() {
    // Asserted on a normalized document and not only through the snapshots, because a snapshot refreshed while normalization was doing nothing would record the observed value and keep passing against itself for as long as the tool version did not change.
    let path = fixture_dir().join("passed.repor");
    let (run_dir, _stdout, _exit_code, _dir) = run_fixture(&path, false);
    let json = read_result_json(&run_dir);
    let observed_version = json["tool"]["version"].clone();
    let normalized = apply(&SNAPSHOT_PLAN, json).expect("the passed fixture must normalize");

    // The placeholder has to be something the run did not already emit, or the equality below would hold whether or not normalization did anything.
    assert_ne!(observed_version, "<VERSION>");

    assert_eq!(normalized["tool"]["version"], "<VERSION>");
    assert_eq!(
        normalized["tool"]["name"], "reportage",
        "a value no annotation names keeps what was observed"
    );
}

#[test]
fn snapshots_for_run_result_fixtures_are_current() {
    let paths = fixture_paths();
    assert!(
        !paths.is_empty(),
        "expected at least one run_result fixture"
    );

    let update_snapshots = update_snapshots_enabled();
    for path in paths {
        let (run_dir, _stdout, _exit_code, _dir) = run_fixture(&path, false);
        let json = read_result_json(&run_dir);

        // Contract validation precedes normalization. Normalization rewrites volatile fields to
        // make a document comparable; it is not a repair step, and a snapshot recorded from a
        // document that never satisfied the schema would pin a contract violation as expected
        // output.
        RUN_RESULT.assert_conforms(
            &json,
            &format!("the result.json of fixture {}", path.display()),
        );

        let normalized = apply(&SNAPSHOT_PLAN, json).unwrap_or_else(|error| {
            panic!(
                "the result.json of fixture {} could not be normalized: {error}",
                path.display()
            )
        });
        let actual = format_snapshot(&normalized);

        let snapshot_path = snapshot_path_for_fixture(&path);
        if update_snapshots {
            std::fs::write(&snapshot_path, actual).unwrap_or_else(|e| {
                panic!("failed to update snapshot {}: {e}", snapshot_path.display())
            });
            continue;
        }

        let expected = std::fs::read_to_string(&snapshot_path).unwrap_or_else(|e| {
            panic!(
                "failed to read snapshot {}: {e}\n\
                 run `UPDATE_RUN_RESULT_SNAPSHOTS=1 cargo test -p reportage-cli --test run_result_fixtures snapshots_for_run_result_fixtures_are_current` to create or refresh snapshots",
                snapshot_path.display()
            )
        });

        assert_eq!(
            expected,
            actual,
            "snapshot for {} is stale; run \
             `UPDATE_RUN_RESULT_SNAPSHOTS=1 cargo test -p reportage-cli --test run_result_fixtures snapshots_for_run_result_fixtures_are_current` \
             and review the JSON diff",
            path.display()
        );
    }
}

// ---------------------------------------------------------------------------
// Projection parity with --format=json
//
// Issue #102's minimum parity items, checked field-by-field, plus a strict structural check that the stdout document is exactly the canonical document minus the defined projection differences (artifactRoot added; noop and evidence sha256 dropped).
// ---------------------------------------------------------------------------

#[test]
fn stdout_projection_agrees_with_the_artifact_result_from_the_same_run() {
    for path in fixture_paths() {
        let (run_dir, stdout_doc, exit_code, dir) = run_fixture(&path, true);
        let stdout_doc = stdout_doc.unwrap();
        let artifact_doc = read_result_json(&run_dir);
        let context = path.display();

        // The stdout document's artifactRoot must name the run directory this result.json and its evidence files were written to.
        assert_eq!(
            dir.path()
                .join(stdout_doc["artifactRoot"].as_str().unwrap()),
            run_dir,
            "{context}: artifactRoot must point at the run directory"
        );

        // Top-level status / processExitCode.
        assert_eq!(stdout_doc["status"], artifact_doc["status"], "{context}");
        assert_eq!(
            stdout_doc["processExitCode"], artifact_doc["processExitCode"],
            "{context}"
        );
        assert_eq!(
            stdout_doc["processExitCode"],
            serde_json::json!(exit_code),
            "{context}: the observed reportage process exit code must match both documents"
        );

        // Summary.
        assert_eq!(stdout_doc["summary"], artifact_doc["summary"], "{context}");

        // Diagnostics code / category / severity (and ids, which both documents share).
        let stdout_diagnostics = stdout_doc["diagnostics"].as_array().unwrap();
        let artifact_diagnostics = artifact_doc["diagnostics"].as_array().unwrap();
        assert_eq!(
            stdout_diagnostics.len(),
            artifact_diagnostics.len(),
            "{context}"
        );
        for (s, a) in stdout_diagnostics.iter().zip(artifact_diagnostics) {
            for field in ["id", "code", "category", "severity"] {
                assert_eq!(
                    s.get(field),
                    a.get(field),
                    "{context}: diagnostics[].{field}"
                );
            }
        }

        // Test / action / assertion ids, action exitCode, expectation kind/status, and
        // captured stdout/stderr artifactRef / sizeBytes.
        let stdout_tests = stdout_doc["tests"].as_array().unwrap();
        let artifact_tests = artifact_doc["tests"].as_array().unwrap();
        assert_eq!(stdout_tests.len(), artifact_tests.len(), "{context}");
        for (s_test, a_test) in stdout_tests.iter().zip(artifact_tests) {
            assert_eq!(s_test["id"], a_test["id"], "{context}");
            assert_eq!(s_test["status"], a_test["status"], "{context}");

            let s_actions = s_test["actions"].as_array().unwrap();
            let a_actions = a_test["actions"].as_array().unwrap();
            assert_eq!(s_actions.len(), a_actions.len(), "{context}");
            for (s_action, a_action) in s_actions.iter().zip(a_actions) {
                assert_eq!(s_action["id"], a_action["id"], "{context}");
                assert_eq!(s_action["exitCode"], a_action["exitCode"], "{context}");
                for stream in ["stdout", "stderr"] {
                    for field in ["artifactRef", "sizeBytes"] {
                        assert_eq!(
                            s_action[stream][field], a_action[stream][field],
                            "{context}: actions[].{stream}.{field}"
                        );
                    }
                }
            }

            let s_assertions = s_test["assertions"].as_array().unwrap();
            let a_assertions = a_test["assertions"].as_array().unwrap();
            assert_eq!(s_assertions.len(), a_assertions.len(), "{context}");
            for (s_assertion, a_assertion) in s_assertions.iter().zip(a_assertions) {
                assert_eq!(s_assertion["id"], a_assertion["id"], "{context}");
                assert_eq!(s_assertion["status"], a_assertion["status"], "{context}");
                assert_eq!(
                    s_assertion["expectation"]["kind"], a_assertion["expectation"]["kind"],
                    "{context}"
                );
                assert_eq!(
                    s_assertion["expectation"]["status"], a_assertion["expectation"]["status"],
                    "{context}"
                );
            }
        }
    }
}

#[test]
fn stdout_projection_is_the_artifact_result_minus_the_defined_differences() {
    for path in fixture_paths() {
        let (run_dir, stdout_doc, _exit_code, _dir) = run_fixture(&path, true);
        let stdout_doc = stdout_doc.unwrap();
        let mut artifact_doc = read_result_json(&run_dir);

        // Apply the projection differences documented in spec/artifacts/run-result/README.md to the canonical document; the outcome must be the stdout document exactly.
        let object = artifact_doc.as_object_mut().unwrap();
        object.remove("noop").expect("result.json must carry noop");
        object.insert(
            "artifactRoot".to_string(),
            stdout_doc["artifactRoot"].clone(),
        );
        for test in artifact_doc["tests"].as_array_mut().unwrap() {
            for action in test["actions"].as_array_mut().unwrap() {
                for stream in ["stdout", "stderr"] {
                    action[stream]
                        .as_object_mut()
                        .unwrap()
                        .remove("sha256")
                        .expect("result.json evidence references must carry sha256");
                }
            }
        }

        assert_eq!(
            artifact_doc,
            stdout_doc,
            "fixture {}: the stdout document must be derivable from result.json by the defined projection",
            path.display()
        );
    }
}

// ---------------------------------------------------------------------------
// Docs drift check
//
// docs/reference/artifacts.md marks each fixture-derived example with a `<!-- checked-against: `<repo-relative snapshot path>` -->` comment directly above a ```json fence.
// Those examples are the "checked" sections of the generated / checked / handwritten boundary defined in docs/reference/artifacts.md: this test fails when an example drifts from the snapshot it claims to mirror.
// ---------------------------------------------------------------------------

#[test]
fn docs_artifacts_examples_match_their_fixture_snapshots() {
    let docs_path = repo_root().join("docs/reference/artifacts.md");
    let docs = std::fs::read_to_string(&docs_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", docs_path.display()));

    let mut checked = 0usize;
    let mut lines = docs.lines().peekable();
    while let Some(line) = lines.next() {
        let Some(marker) = line.trim().strip_prefix("<!-- checked-against:") else {
            continue;
        };
        let snapshot_rel = marker
            .trim()
            .trim_end_matches("-->")
            .trim()
            .trim_matches('`');

        assert_eq!(
            lines.next().map(str::trim),
            Some("```json"),
            "docs/reference/artifacts.md: a checked-against marker must be immediately followed by a ```json fence ({snapshot_rel})"
        );
        let mut example = String::new();
        for fence_line in lines.by_ref() {
            if fence_line.trim() == "```" {
                break;
            }
            example.push_str(fence_line);
            example.push('\n');
        }

        let snapshot_path = repo_root().join(snapshot_rel);
        let snapshot = std::fs::read_to_string(&snapshot_path).unwrap_or_else(|e| {
            panic!(
                "docs/reference/artifacts.md references snapshot {} which cannot be read: {e}",
                snapshot_path.display()
            )
        });
        assert_eq!(
            example, snapshot,
            "docs/reference/artifacts.md: the example marked checked-against {snapshot_rel} has drifted from the snapshot; update the docs example to match"
        );
        checked += 1;
    }

    assert!(
        checked > 0,
        "docs/reference/artifacts.md must contain at least one checked-against example"
    );
}
