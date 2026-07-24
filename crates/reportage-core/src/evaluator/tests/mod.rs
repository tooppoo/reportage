use std::path::{Path, PathBuf};

use super::{Checkpoint, evaluate, evaluate_expectation_at_checkpoint};
use crate::diagnostic::DiagnosticCode;
use crate::executor::ExecutionEnvironment;
use crate::model::*;
use crate::result::*;
use crate::shim::CommandRegistry;

mod contents_equals;
mod execution;
mod logical;
mod output;
mod text_equals;

fn default_env() -> ExecutionEnvironment {
    ExecutionEnvironment::default()
}

fn default_commands() -> CommandRegistry {
    CommandRegistry::default()
}

fn make_script(cases: Vec<Case>) -> Script {
    Script {
        before_each: None,
        cases,
    }
}

fn action(cmd: &str) -> Step {
    Step::Action(ActionStep {
        command: cmd.to_string(),
    })
}

fn assert_exit(code: u8) -> Step {
    let expectations = vec![Expectation::Exit(ExitExpectation { expected: code })];
    Step::AssertionBlock(AssertionBlock::new(expectations).unwrap())
}

fn assert_exits(codes: &[u8]) -> Step {
    let expectations = codes
        .iter()
        .map(|&c| Expectation::Exit(ExitExpectation { expected: c }))
        .collect();
    Step::AssertionBlock(AssertionBlock::new(expectations).unwrap())
}

fn checkpoint_after_output(stdout: Vec<u8>, stderr: Vec<u8>) -> Checkpoint {
    Checkpoint::after_action(
        ActionResult {
            command: "test".to_string(),
            exit_code: 0,
            stdout,
            stderr,
            shim_invocations: vec![],
            shim_event_parse_warnings: vec![],
        },
        PathBuf::from("."),
        PathBuf::from("."),
    )
}

fn stdout_empty_expectation() -> Expectation {
    Expectation::Stdout(crate::model::OutputExpectation {
        matcher: OutputMatcher::Empty,
    })
}

fn stderr_empty_expectation() -> Expectation {
    Expectation::Stderr(crate::model::OutputExpectation {
        matcher: OutputMatcher::Empty,
    })
}

fn exit_exp(code: u8) -> Expectation {
    Expectation::Exit(ExitExpectation { expected: code })
}

fn logical(operator: LogicalOperator, children: Vec<Expectation>) -> Expectation {
    Expectation::Logical(LogicalExpectation::new(operator, children).unwrap())
}

fn checkpoint_after_exit(code: i32) -> Checkpoint {
    Checkpoint::after_action(
        ActionResult {
            command: "test".to_string(),
            exit_code: code,
            stdout: Vec::new(),
            stderr: Vec::new(),
            shim_invocations: vec![],
            shim_event_parse_warnings: vec![],
        },
        PathBuf::from("."),
        PathBuf::from("."),
    )
}

fn write_step(path: &str, content: &str) -> Step {
    Step::SideEffect(SideEffectingStep::WriteFile(WriteFileStep {
        path: WorkspacePath::parse(path).unwrap(),
        content: TextLiteral::Quoted(content.to_string()),
    }))
}

fn assert_file_contents_equals_workspace(actual_path: &str, expected_path: &str) -> Step {
    let expectations = vec![Expectation::File(FileExpectation {
        path: actual_path.to_string(),
        matcher: FileMatcher::ContentsEquals(FileContentsReference::Workspace(
            WorkspacePath::parse(expected_path).unwrap(),
        )),
    })];
    Step::AssertionBlock(AssertionBlock::new(expectations).unwrap())
}

fn assert_stdout_contents_equals_workspace(expected_path: &str) -> Step {
    let expectations = vec![Expectation::Stdout(crate::model::OutputExpectation {
        matcher: OutputMatcher::ContentsEquals(FileContentsReference::Workspace(
            WorkspacePath::parse(expected_path).unwrap(),
        )),
    })];
    Step::AssertionBlock(AssertionBlock::new(expectations).unwrap())
}

fn assert_stderr_contents_equals_workspace(expected_path: &str) -> Step {
    let expectations = vec![Expectation::Stderr(crate::model::OutputExpectation {
        matcher: OutputMatcher::ContentsEquals(FileContentsReference::Workspace(
            WorkspacePath::parse(expected_path).unwrap(),
        )),
    })];
    Step::AssertionBlock(AssertionBlock::new(expectations).unwrap())
}

fn single_case(steps: Vec<Step>) -> Script {
    make_script(vec![Case {
        name: "contents_equals".to_string(),
        steps,
    }])
}

fn assert_file_text_equals(actual_path: &str, expected_text: &str) -> Step {
    let expectations = vec![Expectation::File(FileExpectation {
        path: actual_path.to_string(),
        matcher: FileMatcher::TextEquals(TextLiteral::Quoted(expected_text.to_string())),
    })];
    Step::AssertionBlock(AssertionBlock::new(expectations).unwrap())
}

fn assert_file_text_equals_heredoc(actual_path: &str, expected_text: &str) -> Step {
    let expectations = vec![Expectation::File(FileExpectation {
        path: actual_path.to_string(),
        matcher: FileMatcher::TextEquals(TextLiteral::Heredoc(expected_text.to_string())),
    })];
    Step::AssertionBlock(AssertionBlock::new(expectations).unwrap())
}

fn before_each_writing(path: &str, content: &str) -> BeforeEach {
    BeforeEach::new(vec![SideEffectingStep::WriteFile(WriteFileStep {
        path: WorkspacePath::parse(path).unwrap(),
        content: TextLiteral::Quoted(content.to_string()),
    })])
    .unwrap()
}

fn assert_file_exists_step(path: &str) -> Step {
    let expectations = vec![Expectation::File(crate::model::FileExpectation {
        path: path.to_string(),
        matcher: FileMatcher::Exists,
    })];
    Step::AssertionBlock(AssertionBlock::new(expectations).unwrap())
}
