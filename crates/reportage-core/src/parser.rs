use crate::model::{Action, AssertStep, Case, Script, Step};

#[derive(Debug, PartialEq)]
pub enum ParseError {
    UnexpectedToken { line: usize, message: String },
    UnclosedCase { line: usize },
    InvalidExitCode { line: usize, value: String },
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
            ParseError::InvalidExitCode { line, value } => {
                write!(
                    f,
                    "parse error at line {line}: invalid exit code '{value}', expected integer in 0..=255"
                )
            }
        }
    }
}

impl std::error::Error for ParseError {}

pub fn parse(source: &str) -> Result<Script, ParseError> {
    let mut cases: Vec<Case> = Vec::new();
    // (name, steps, start_line)
    let mut current_case: Option<(String, Vec<Step>, usize)> = None;

    for (idx, line) in source.lines().enumerate() {
        let line_num = idx + 1;
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        if let Some((_, ref mut steps, _)) = current_case {
            if trimmed == "}" {
                let (name, steps, _) = current_case.take().unwrap();
                cases.push(Case { name, steps });
            } else if let Some(rest) = trimmed.strip_prefix('$') {
                let command = rest.trim().to_string();
                steps.push(Step::Action(Action { command }));
            } else if let Some(assert_body) = trimmed.strip_prefix("assert ") {
                let assert_body = assert_body.trim();
                let step = parse_assert(line_num, assert_body)?;
                steps.push(step);
            } else {
                return Err(ParseError::UnexpectedToken {
                    line: line_num,
                    message: format!(
                        "unexpected token in case block: '{trimmed}'; only '$' actions, 'assert' assertions, and '}}' are valid"
                    ),
                });
            }
        } else if trimmed.starts_with("case ") && trimmed.ends_with('{') {
            let after_keyword = trimmed["case ".len()..].trim_end();
            // strip trailing '{'
            let name_section = after_keyword[..after_keyword.len() - 1].trim();
            if name_section.starts_with('"')
                && name_section.ends_with('"')
                && name_section.len() >= 2
            {
                let name = name_section[1..name_section.len() - 1].to_string();
                current_case = Some((name, Vec::new(), line_num));
            } else {
                return Err(ParseError::UnexpectedToken {
                    line: line_num,
                    message: format!(
                        "invalid case declaration; expected `case \"<name>\" {{`, found: '{trimmed}'"
                    ),
                });
            }
        } else if trimmed.starts_with('$') {
            return Err(ParseError::UnexpectedToken {
                line: line_num,
                message:
                    "action '$' is not allowed at top level; actions must be inside a case block"
                        .to_string(),
            });
        } else if trimmed.starts_with("assert") {
            return Err(ParseError::UnexpectedToken {
                line: line_num,
                message: "assertion 'assert' is not allowed at top level; assertions must be inside a case block".to_string(),
            });
        } else {
            return Err(ParseError::UnexpectedToken {
                line: line_num,
                message: format!("unexpected token at top level: '{trimmed}'"),
            });
        }
    }

    if let Some((_, _, start_line)) = current_case {
        return Err(ParseError::UnclosedCase { line: start_line });
    }

    Ok(Script { cases })
}

fn parse_assert(line_num: usize, assert_body: &str) -> Result<Step, ParseError> {
    if let Some(rest) = assert_body.strip_prefix("exit") {
        let code_str = rest.trim();
        if code_str.is_empty() {
            return Err(ParseError::UnexpectedToken {
                line: line_num,
                message: "assert exit requires an exit code, e.g., 'assert exit 0'".to_string(),
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
        Ok(Step::Assert(AssertStep::Exit {
            expected: code as u8,
        }))
    } else {
        Err(ParseError::UnexpectedToken {
            line: line_num,
            message: format!("unsupported assertion: 'assert {assert_body}'"),
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
  assert exit 0
}
"#;
        let script = parse(src).unwrap();
        assert_eq!(script.cases.len(), 1);
        assert_eq!(script.cases[0].name, "first pass");
        assert_eq!(script.cases[0].steps.len(), 2);
    }

    #[test]
    fn parse_two_cases() {
        let src = r#"
case "first" {
  $ true
  assert exit 0
}

case "second" {
  $ false
  assert exit 1
}
"#;
        let script = parse(src).unwrap();
        assert_eq!(script.cases.len(), 2);
        assert_eq!(script.cases[0].name, "first");
        assert_eq!(script.cases[1].name, "second");
    }

    #[test]
    fn parse_multiple_assertions_for_one_action() {
        let src = r#"
case "multi" {
  $ true
  assert exit 0
  assert exit 0
}
"#;
        let script = parse(src).unwrap();
        assert_eq!(script.cases[0].steps.len(), 3);
    }

    #[test]
    fn top_level_action_is_error() {
        let src = "$ true\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::UnexpectedToken { .. }));
    }

    #[test]
    fn top_level_assert_is_error() {
        let src = "assert exit 0\n";
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::UnexpectedToken { .. }));
    }

    #[test]
    fn exit_code_999_is_error() {
        let src = r#"
case "x" {
  $ true
  assert exit 999
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
  assert exit 255
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
    fn assert_exit_without_code_is_error() {
        let src = r#"
case "x" {
  $ true
  assert exit
}
"#;
        let err = parse(src).unwrap_err();
        assert!(matches!(err, ParseError::UnexpectedToken { .. }));
    }

    #[test]
    fn unsupported_assertion_is_error() {
        let src = r#"
case "x" {
  $ true
  assert stdout empty
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
  assert exit 0
}
"#;
        let script = parse(src).unwrap();
        if let Step::Action(a) = &script.cases[0].steps[0] {
            assert_eq!(a.command, "echo hello");
        } else {
            panic!("expected Action step");
        }
    }
}
