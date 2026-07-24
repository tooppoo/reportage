use super::*;
use crate::model::Script;

/// Most tests here assert against the execution model, so they project
/// the parse result immediately; span-focused tests call `parse` directly.
fn parse_script(src: &str) -> Result<Script, ParseError> {
    parse(src).map(SourceFile::into_script)
}

fn write_file_step(script: &Script) -> &WriteFileStep {
    let Step::SideEffect(SideEffectingStep::WriteFile(step)) = &script.cases[0].steps[0] else {
        panic!("expected first step to be a write step");
    };
    step
}

const PASSING_CASE: &str = "case \"x\" {\n  $ true\n  assert { exit 0 }\n}\n";

