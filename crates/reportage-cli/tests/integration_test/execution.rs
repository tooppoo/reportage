use super::*;

// Representative `all`/`any`/`not` scenarios live in e2e/composition/logical-composition.repor.
// The tests here cover the CLI-externalized evaluation result and execution stopping behavior.

#[test]
fn nested_logical_composition_is_evaluated_and_recorded_in_artifact() {
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "nested composition" {
  $ false
  assert {
    all {
      not {
        exit 0
      }
      any {
        exit 1
        exit 2
      }
    }
  }
}
"#,
    );
    reportage(&dir).arg(script).assert().code(0);

    let (json, _) = read_single_result_json(&dir);
    let expectation = &json["tests"][0]["assertions"][0]["expectation"];
    assert_eq!(expectation["kind"], "logical");
    assert_eq!(expectation["operator"], "all");
    assert_eq!(expectation["status"], "passed");
    assert_eq!(expectation["children"][0]["operator"], "not");
    assert_eq!(expectation["children"][1]["operator"], "any");
}

#[test]
fn assertion_block_failure_stops_subsequent_action() {
    // assert { exit 1 } fails because true exits 0.
    // Source order execution must not run the second action after the block failure.
    // This is verified by checking that only one action appears in result.json.
    let dir = TempDir::new().unwrap();
    let script = write_script(
        &dir,
        "test.repor",
        r#"
case "source order" {
  $ true
  assert {
    exit 1
  }
  $ false
  assert {
    exit 0
  }
}
"#,
    );
    reportage(&dir).arg(&script).assert().code(1);

    let runs_dir = dir.child(".reportage").child("runs");
    let entries: Vec<_> = std::fs::read_dir(runs_dir.path())
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let content = std::fs::read_to_string(entries[0].path().join("result.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();

    let actions = json["tests"][0]["actions"].as_array().unwrap();
    assert_eq!(
        actions.len(),
        1,
        "only the first action should have run; source order execution stops on assertion block failure"
    );
}
