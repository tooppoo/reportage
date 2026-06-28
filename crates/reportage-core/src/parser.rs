use crate::model::{
    ActionStep, AssertionBlock, AssertionBlockError, Case, ExitExpectation, Expectation, Script,
    Step,
};

#[derive(Debug, PartialEq)]
pub enum ParseError {
    UnexpectedToken { line: usize, message: String },
    UnclosedCase { line: usize },
    UnclosedAssertBlock { line: usize },
    InvalidExitCode { line: usize, value: String },
    EmptyAssertionBlock { line: usize },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::UnexpectedToken { line, message } => {
                write!(f, "parse error at line {line}: {message}")
            }
            ParseError::UnclosedCase { line } => {
                write!(
                    f,
                    "parse error: unclosed case block starting at line {line}"
                )
            }
            ParseError::UnclosedAssertBlock { line } => {
                write!(
                    f,
                    "parse error: unclosed assert block starting at line {line}"
                )
            }
            ParseError::InvalidExitCode { line, value } => {
                write!(
                    f,
                    "parse error at line {line}: invalid exit code '{value}', expected integer in 0..=255"
                )
            }
            ParseError::EmptyAssertionBlock { line } => {
                write!(
                    f,
                    "parse error at line {line}: empty assert block; assert {{ }} must contain at least one expectation"
                )
            }
        }
    }
}

impl std::error::Error for ParseError {}

// State is consumed and replaced each line to avoid borrow-checker issues with
// mutable references across state transitions.
enum ParseState {
    TopLevel,
    InCase {
        name: String,
        steps: Vec<Step>,
        start_line: usize,
    },
    InAssertBlock {
        // Saved case context, restored when the block closes.
        case_name: String,
        case_steps: Vec<Step>,
        case_start_line: usize,
        // Block context.
        expectations: Vec<Expectation>,
        block_start_line: usize,
    },
}

pub fn parse(source: &str) -> Result<Script, ParseError> {
    let mut cases: Vec<Case> = Vec::new();
    let mut state = ParseState::TopLevel;

    for (idx, line) in source.lines().enumerate() {
        let line_num = idx + 1;
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        // Consume the state and produce the next state each iteration.
        state = process_line(state, trimmed, line_num, &mut cases)?;
    }

    match state {
        ParseState::InAssertBlock {
            block_start_line, ..
        } => Err(ParseError::UnclosedAssertBlock {
            line: block_start_line,
        }),
        ParseState::InCase { start_line, .. } => Err(ParseError::UnclosedCase { line: start_line }),
        ParseState::TopLevel => Ok(Script { cases }),
    }
}

fn process_line(
    state: ParseState,
    trimmed: &str,
    line_num: usize,
    cases: &mut Vec<Case>,
) -> Result<ParseState, ParseError> {
    match state {
        ParseState::TopLevel => process_top_level(trimmed, line_num),

        ParseState::InCase {
            name,
            steps,
            start_line,
        } => process_in_case(name, steps, start_line, trimmed, line_num, cases),

        ParseState::InAssertBlock {
            case_name,
            case_steps,
            case_start_line,
            expectations,
            block_start_line,
        } => process_in_assert_block(
            case_name,
            case_steps,
            case_start_line,
            expectations,
            block_start_line,
            trimmed,
            line_num,
        ),
    }
}

fn process_top_level(trimmed: &str, line_num: usize) -> Result<ParseState, ParseError> {
    if trimmed.starts_with("case ") && trimmed.ends_with('{') {
        let name = extract_case_name(trimmed, line_num)?;
        Ok(ParseState::InCase {
            name,
            steps: Vec::new(),
            start_line: line_num,
        })
    } else if trimmed.starts_with('$') {
        Err(ParseError::UnexpectedToken {
            line: line_num,
            message: "action '$' is not allowed at top level; actions must be inside a case block"
                .to_string(),
        })
    } else if trimmed.starts_with("assert") {
        Err(ParseError::UnexpectedToken {
            line: line_num,
            message: "assertion 'assert' is not allowed at top level; assertions must be inside a case block"
                .to_string(),
        })
    } else {
        Err(ParseError::UnexpectedToken {
            line: line_num,
            message: format!("unexpected token at top level: '{trimmed}'"),
        })
    }
}

fn process_in_case(
    name: String,
    mut steps: Vec<Step>,
    start_line: usize,
    trimmed: &str,
    line_num: usize,
    cases: &mut Vec<Case>,
) -> Result<ParseState, ParseError> {
    if trimmed == "}" {
        cases.push(Case { name, steps });
        Ok(ParseState::TopLevel)
    } else if let Some(rest) = trimmed.strip_prefix('$') {
        let command = rest.trim().to_string();
        steps.push(Step::Action(ActionStep { command }));
        Ok(ParseState::InCase {
            name,
            steps,
            start_line,
        })
    } else if trimmed == "assert {" {
        // Start a multi-line assertion block.
        Ok(ParseState::InAssertBlock {
            case_name: name,
            case_steps: steps,
            case_start_line: start_line,
            expectations: Vec::new(),
            block_start_line: line_num,
        })
    } else if trimmed.starts_with("assert {") && trimmed.ends_with('}') {
        // Single-line assertion block: assert { exit 0 }
        let inner_start = "assert {".len();
        let inner_end = trimmed.len() - 1;
        let inner = trimmed[inner_start..inner_end].trim();
        if inner.is_empty() {
            return Err(ParseError::EmptyAssertionBlock { line: line_num });
        }
        let exp = parse_expectation(line_num, inner)?;
        let block = AssertionBlock::new(vec![exp])
            .map_err(|_| ParseError::EmptyAssertionBlock { line: line_num })?;
        steps.push(Step::AssertionBlock(block));
        Ok(ParseState::InCase {
            name,
            steps,
            start_line,
        })
    } else if trimmed.starts_with("assert") {
        let rest = trimmed.strip_prefix("assert").unwrap_or("").trim();
        Err(ParseError::UnexpectedToken {
            line: line_num,
            message: format!(
                "expected 'assert {{ ... }}' block; \
                 'assert {rest}' is not valid in v0 — use 'assert {{ {rest} }}' instead"
            ),
        })
    } else {
        Err(ParseError::UnexpectedToken {
            line: line_num,
            message: format!(
                "unexpected token in case block: '{trimmed}'; \
                 only '$' actions, 'assert {{ ... }}' blocks, and '}}' are valid"
            ),
        })
    }
}

fn process_in_assert_block(
    case_name: String,
    mut case_steps: Vec<Step>,
    case_start_line: usize,
    mut expectations: Vec<Expectation>,
    block_start_line: usize,
    trimmed: &str,
    line_num: usize,
) -> Result<ParseState, ParseError> {
    if trimmed == "}" {
        // Close the assertion block and return to InCase.
        let block = AssertionBlock::new(expectations).map_err(|AssertionBlockError::Empty| {
            ParseError::EmptyAssertionBlock {
                line: block_start_line,
            }
        })?;
        case_steps.push(Step::AssertionBlock(block));
        Ok(ParseState::InCase {
            name: case_name,
            steps: case_steps,
            start_line: case_start_line,
        })
    } else if trimmed.starts_with('$') {
        Err(ParseError::UnexpectedToken {
            line: line_num,
            message: "'$' actions are not allowed inside an assert block".to_string(),
        })
    } else if trimmed.starts_with("assert") {
        Err(ParseError::UnexpectedToken {
            line: line_num,
            message: "nested 'assert' is not allowed inside an assert block".to_string(),
        })
    } else {
        let exp = parse_expectation(line_num, trimmed)?;
        expectations.push(exp);
        Ok(ParseState::InAssertBlock {
            case_name,
            case_steps,
            case_start_line,
            expectations,
            block_start_line,
        })
    }
}

fn extract_case_name(trimmed: &str, line_num: usize) -> Result<String, ParseError> {
    // trimmed is `case "<name>" {` (already checked starts_with "case " and ends_with '{')
    let after_keyword = trimmed["case ".len()..].trim_end();
    let name_section = after_keyword[..after_keyword.len() - 1].trim();
    if name_section.starts_with('"') && name_section.ends_with('"') && name_section.len() >= 2 {
        Ok(name_section[1..name_section.len() - 1].to_string())
    } else {
        Err(ParseError::UnexpectedToken {
            line: line_num,
            message: format!(
                "invalid case declaration; expected `case \"<name>\" {{`, found: '{trimmed}'"
            ),
        })
    }
}

fn parse_expectation(line_num: usize, content: &str) -> Result<Expectation, ParseError> {
    if let Some(rest) = content.strip_prefix("exit") {
        let code_str = rest.trim();
        if code_str.is_empty() {
            return Err(ParseError::UnexpectedToken {
                line: line_num,
                message: "exit expectation requires an exit code, e.g., 'exit 0'".to_string(),
            });
        }
        let code: u16 = code_str.parse().map_err(|_| ParseError::InvalidExitCode {
            line: line_num,
            value: code_str.to_string(),
        })?;
        if code > 255 {
            return Err(ParseError::InvalidExitCode {
                line: line_num,
                value: code_str.to_string(),
            });
        }
        Ok(Expectation::Exit(ExitExpectation {
            expected: code as u8,
        }))
    } else {
        Err(ParseError::UnexpectedToken {
            line: line_num,
            message: format!("unsupported expectation: '{content}'; v0 supports 'exit <code>'"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_passing_case() {
        let src = r#"
case "first pass" {
  $ true
  assert {
    exit 0
  }
}
"#;
        let script = parse(src).unwrap();
        assert_eq!(script.cases.len(), 1);
        assert_eq!(script.cases[0].name, "first pass");
        // Steps: Action + AssertionBlock
        assert_eq!(script.cases[0].steps.len(), 2);
    }

    #[test]
    fn parse_two_cases() {
        let src = r#"
case "first" {
  $ true
  assert {
    exit 0
  }
}

case "second" {
  $ false
  assert {
    exit 1
  }
}
"#;
        let script = parse(src).unwrap();
        assert_eq!(script.cases.len(), 2);
        assert_eq!(script.cases[0].name, "first");
        assert_eq!(script.cases[1].name, "second");
    }

    #[test]
    fn parse_single_line_assert_block() {
        let src = r#"
case "inline" {
  $ true
  assert { exit 0 }
}
"#;
        let script = parse(src).unwrap();
        assert_eq!(script.cases[0].steps.len(), 2);
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert_eq!(block.expectations().len(), 1);
    }

    #[test]
    fn parse_multiple_expectations_in_one_block() {
        let src = r#"
case "multi" {
  $ true
  assert {
    exit 0
    exit 0
  }
}
"#;
        let script = parse(src).unwrap();
        // Steps: Action + one AssertionBlock
        assert_eq!(script.cases[0].steps.len(), 2);
        let Step::AssertionBlock(block) = &script.cases[0].steps[1] else {
            panic!("expected AssertionBlock");
        };
        assert_eq!(block.expectations().len(), 2);
    }

    #[test]
    fn top_level_action_is_error() {
        let src = "$ true\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::UnexpectedToken { .. }));
    }

    #[test]
    fn top_level_assert_is_error() {
        let src = "assert { exit 0 }\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::UnexpectedToken { .. }));
    }

    #[test]
    fn bare_assert_without_block_is_error() {
        let src = r#"
case "x" {
  $ true
  assert exit 0
}
"#;
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::UnexpectedToken { .. }));
    }

    #[test]
    fn exit_code_999_is_error() {
        let src = r#"
case "x" {
  $ true
  assert {
    exit 999
  }
}
"#;
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::InvalidExitCode { .. }));
    }

    #[test]
    fn exit_code_255_is_valid() {
        let src = r#"
case "x" {
  $ true
  assert {
    exit 255
  }
}
"#;
        assert!(parse(src).is_ok());
    }

    #[test]
    fn unclosed_case_is_error() {
        let src = "case \"x\" {\n  $ true\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::UnclosedCase { .. }));
    }

    #[test]
    fn unclosed_assert_block_is_error() {
        let src = "case \"x\" {\n  $ true\n  assert {\n    exit 0\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::UnclosedAssertBlock { .. }));
    }

    #[test]
    fn empty_assert_block_multi_line_is_error() {
        let src = r#"
case "x" {
  $ true
  assert {
  }
}
"#;
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::EmptyAssertionBlock { .. }));
    }

    #[test]
    fn empty_assert_block_single_line_is_error() {
        // "assert { }" — inner content is empty after trimming
        // Note: this is "assert {" + " }" which ends with '}' and starts with "assert {"
        // so it falls into the single-line branch and finds empty inner content.
        // Actually "assert { }" trims to "assert { }" — ends_with('}') is true,
        // starts_with("assert {") is true. inner = " " which trims to "".
        let src = "case \"x\" {\n  $ true\n  assert { }\n}\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::EmptyAssertionBlock { .. }));
    }

    #[test]
    fn exit_without_code_is_error() {
        let src = r#"
case "x" {
  $ true
  assert {
    exit
  }
}
"#;
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::UnexpectedToken { .. }));
    }

    #[test]
    fn unsupported_expectation_is_error() {
        let src = r#"
case "x" {
  $ true
  assert {
    unknown_assertion
  }
}
"#;
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::UnexpectedToken { .. }));
    }

    #[test]
    fn action_command_is_stripped_of_dollar_and_trimmed() {
        let src = r#"
case "x" {
  $   echo hello
  assert { exit 0 }
}
"#;
        let script = parse(src).unwrap();
        if let Step::Action(a) = &script.cases[0].steps[0] {
            assert_eq!(a.command, "echo hello");
        } else {
            panic!("expected Action step");
        }
    }

    #[test]
    fn action_inside_assert_block_is_error() {
        let src = r#"
case "x" {
  $ true
  assert {
    $ false
  }
}
"#;
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::UnexpectedToken { .. }));
    }

    #[test]
    fn two_assertion_blocks_in_one_case_parses_ok() {
        // Parsing succeeds. ScriptError happens at evaluation time for the
        // first block (process expectation at initial checkpoint).
        let src = r#"
case "two blocks" {
  assert {
    exit 0
  }
  $ true
  assert {
    exit 0
  }
}
"#;
        let script = parse(src).unwrap();
        assert_eq!(script.cases[0].steps.len(), 3); // AssertionBlock, Action, AssertionBlock
    }
}
