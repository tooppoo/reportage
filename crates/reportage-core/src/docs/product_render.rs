//! The wording every product format uses for a verified condition, and the
//! rendering of the values inside it.
//!
//! Phrasing lives here, not in [`super::product`]: the catalog stays a
//! structured model so that formats can differ, but *what a condition means*
//! must not differ between them — only how a value is decorated. Each format
//! passes its own [`ValueStyle`], and everything else is shared, so plain text
//! and Markdown can never end up describing the same assertion differently.
//! See docs/adr/20260908T131134Z_product-documentation-projection.md and
//! docs/adr/20260914T161520Z_product-document-serialization.md.
//!
//! No wording here names a Reportage construct: a reader of a product's
//! documentation is told what was verified, not which DSL expressed it.

use super::product::{
    CompositionOperator, DocumentedExpectation, DocumentedText, ExpectedValue, FileContentsOrigin,
    ObservedOperation, ObservedSubject, TextSegment,
};

/// How a format delimits the values inside a condition.
///
/// The only thing a format is allowed to vary: plain text quotes a path,
/// Markdown wraps it in a code span, and the sentence around it stays
/// identical. Each implementation owns making its own delimiter unambiguous
/// for the value it is given — quoting a value containing a quote, or spanning
/// one containing backticks — because that hazard is format-specific.
///
/// The value arrives already reduced to one line by [`one_line`], since
/// neither a quoted fragment nor a code span can carry a line break.
pub(super) trait ValueStyle {
    /// Delimits a literal fragment of the documentation: a path, a piece of
    /// expected text, a pattern.
    fn code(&self, value: &str) -> String;
}

/// One line of a rendered condition, with the nesting depth it sits at.
///
/// Depth rather than pre-applied indentation: plain text indents, and Markdown
/// indents *and* adds a list marker, so the amount of nesting is shared while
/// how it is drawn stays each format's own.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct ConditionLine {
    pub(super) depth: usize,
    pub(super) text: String,
}

/// One condition as display lines, in the order they must be shown.
///
/// A composition yields its operator line followed by its children, each one
/// level deeper, so nesting survives into a flat list that any format can lay
/// out.
pub(super) fn condition_lines(
    expectation: &DocumentedExpectation,
    style: &dyn ValueStyle,
) -> Vec<ConditionLine> {
    fn walk(
        expectation: &DocumentedExpectation,
        style: &dyn ValueStyle,
        depth: usize,
        out: &mut Vec<ConditionLine>,
    ) {
        match expectation {
            DocumentedExpectation::Observation { subject, operation } => out.push(ConditionLine {
                depth,
                text: observation_sentence(subject, operation, style),
            }),
            DocumentedExpectation::Composition { operator, children } => {
                out.push(ConditionLine {
                    depth,
                    text: match operator {
                        // `not { A B }` negates the two taken together —
                        // `not(all(A, B))`, never `not(A) and not(B)` (see
                        // docs/reference/semantics.md — Logical composition).
                        // Wording it as "none of the following" would claim
                        // each child fails, which is a different and stronger
                        // condition than the one verified. With a single
                        // child the group is the child, so the plain negation
                        // is both correct and the readable form.
                        CompositionOperator::Not if children.len() == 1 => "not:".to_string(),
                        CompositionOperator::Not => "not all of the following:".to_string(),
                        CompositionOperator::All => "all of the following:".to_string(),
                        CompositionOperator::Any => "at least one of the following:".to_string(),
                    },
                });
                for child in children {
                    walk(child, style, depth + 1, out);
                }
            }
        }
    }

    let mut lines = Vec::new();
    walk(expectation, style, 0, &mut lines);
    lines
}

/// One observation as a sentence.
///
/// Built from the subject and the operation together, never from the operation
/// alone: `contains` is a substring of a file's text and an entry name in a
/// directory, and only the pair says which.
fn observation_sentence(
    subject: &ObservedSubject,
    operation: &ObservedOperation,
    style: &dyn ValueStyle,
) -> String {
    let subject_phrase = match subject {
        ObservedSubject::ExitCode => "the exit code".to_string(),
        ObservedSubject::Stdout => "standard output".to_string(),
        ObservedSubject::Stderr => "standard error".to_string(),
        ObservedSubject::File { path } => style.code(&one_line(path)),
        ObservedSubject::Dir { path } => format!("the directory {}", style.code(&one_line(path))),
        ObservedSubject::FileCount { glob } => {
            format!(
                "the number of files matching {}",
                style.code(&one_line(glob))
            )
        }
    };

    let predicate = match operation {
        ObservedOperation::Exists => "exists".to_string(),
        ObservedOperation::DoesNotExist => "does not exist".to_string(),
        ObservedOperation::IsEmpty => "is empty".to_string(),
        ObservedOperation::Is(value) => format!("is {}", expected_value(value, style)),
        // A directory's entries are named, not searched: the same operation
        // reads as containment everywhere else.
        ObservedOperation::Contains(value) => match subject {
            ObservedSubject::Dir { .. } => {
                format!("has an entry named {}", expected_value(value, style))
            }
            _ => format!("contains {}", expected_value(value, style)),
        },
        ObservedOperation::DoesNotContain(value) => match subject {
            ObservedSubject::Dir { .. } => {
                format!("has no entry named {}", expected_value(value, style))
            }
            _ => format!("does not contain {}", expected_value(value, style)),
        },
        ObservedOperation::Matches { pattern } => {
            format!("matches {}", style.code(&one_line(pattern)))
        }
        ObservedOperation::CountIs { comparison, count } => match comparison {
            super::product::CountComparison::Exactly => format!("is {count}"),
            super::product::CountComparison::AtLeast => format!("is at least {count}"),
        },
        ObservedOperation::StructuredQuery { expression } => {
            format!("satisfies {}", style.code(&one_line(expression)))
        }
    };

    format!("{subject_phrase} {predicate}")
}

/// One expected value as the object of a condition sentence.
fn expected_value(value: &ExpectedValue, style: &dyn ValueStyle) -> String {
    match value {
        // An empty value is named rather than delimited: an empty quoted
        // fragment reads as a typo, and an empty code span is not a code span
        // at all in Markdown.
        ExpectedValue::Text(text) => match inline_text(text) {
            rendered if rendered.is_empty() => "the empty string".to_string(),
            rendered => style.code(&rendered),
        },
        ExpectedValue::Number(number) => number.to_string(),
        ExpectedValue::FileContents {
            path,
            origin: FileContentsOrigin::Example,
        } => format!("exactly the contents of {}", style.code(&one_line(path))),
        // The path is deliberately not shown: it names a file kept beside the
        // scenario source, which a reader following this example has no way to
        // find and no reason to look for.
        ExpectedValue::FileContents {
            path: _,
            origin: FileContentsOrigin::External,
        } => "exactly the expected contents recorded for this example".to_string(),
    }
}

/// Documented text on one line, so a condition stays scannable.
///
/// A captured value appears as its name in angle brackets, which is the only
/// thing known about it without running the example; literal parts are reduced
/// by [`one_line`].
pub(super) fn inline_text(text: &DocumentedText) -> String {
    match text {
        DocumentedText::Literal(literal) => one_line(literal),
        DocumentedText::Composed(segments) => segments
            .iter()
            .map(|segment| match segment {
                TextSegment::Literal(literal) => one_line(literal),
                TextSegment::Captured { binding } => format!("<{binding}>"),
            })
            .collect(),
    }
}

/// A value reduced to one line, with its line structure escaped.
///
/// A list of verified conditions is read by scanning line starts, so one
/// multi-line value would break that for every condition after it. The
/// backslash is escaped alongside the line escapes, so a literal `\n` in the
/// source stays distinguishable from a real newline. Quoting and code spans
/// are not handled here: those are a format's own delimiter problem, solved by
/// its [`ValueStyle`].
pub(super) fn one_line(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

/// Documented text as the display lines of a file's content.
///
/// The opposite choice from [`inline_text`], for the opposite reason: file
/// content is shown as a block a reader copies, so its line structure is what
/// matters and nothing is escaped. Captured values still appear as their name
/// in angle brackets, because no text for them exists in the source.
pub(super) fn content_lines(text: &DocumentedText) -> Vec<String> {
    let joined: String = match text {
        DocumentedText::Literal(literal) => literal.clone(),
        DocumentedText::Composed(segments) => segments
            .iter()
            .map(|segment| match segment {
                TextSegment::Literal(literal) => literal.clone(),
                TextSegment::Captured { binding } => format!("<{binding}>"),
            })
            .collect(),
    };
    // No lines at all, rather than one empty line: an empty file is shown by
    // its path alone, so the content block never becomes a stray blank line.
    if joined.is_empty() {
        return Vec::new();
    }
    super::render::logical_lines(&joined)
}

/// A file's `mode`, as the parenthetical that follows its path.
///
/// Empty when the source named no mode: the workspace default is reportage's,
/// not something a reader reproduces (see
/// docs/adr/20260908T131134Z_product-documentation-projection.md).
pub(super) fn mode_suffix(mode: Option<u32>) -> String {
    match mode {
        Some(bits) => format!(" (mode 0o{bits:03o})"),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docs::product::CountComparison;

    /// The plain format's style, used to fix wording independently of any
    /// format's decoration.
    struct Quoted;

    impl ValueStyle for Quoted {
        fn code(&self, value: &str) -> String {
            format!("\"{value}\"")
        }
    }

    /// The rendered text of a single observation, which never nests.
    fn observation(subject: ObservedSubject, operation: ObservedOperation) -> Vec<String> {
        condition_lines(
            &DocumentedExpectation::Observation { subject, operation },
            &Quoted,
        )
        .into_iter()
        .map(|line| {
            assert_eq!(line.depth, 0, "an observation never nests");
            line.text
        })
        .collect()
    }

    fn text(value: &str) -> ExpectedValue {
        ExpectedValue::Text(DocumentedText::Literal(value.to_string()))
    }

    #[test]
    fn exit_code_and_output_conditions_read_as_sentences() {
        assert_eq!(
            observation(
                ObservedSubject::ExitCode,
                ObservedOperation::Is(ExpectedValue::Number(0))
            ),
            vec!["the exit code is 0"]
        );
        assert_eq!(
            observation(ObservedSubject::Stdout, ObservedOperation::IsEmpty),
            vec!["standard output is empty"]
        );
        assert_eq!(
            observation(
                ObservedSubject::Stdout,
                ObservedOperation::Contains(text("created"))
            ),
            vec!["standard output contains \"created\""]
        );
        assert_eq!(
            observation(
                ObservedSubject::Stderr,
                ObservedOperation::DoesNotContain(text("error"))
            ),
            vec!["standard error does not contain \"error\""]
        );
    }

    /// The pair, not the operation alone, decides the wording: `contains` on a
    /// directory is an entry name, and on a file a substring.
    #[test]
    fn the_same_operation_reads_differently_per_subject() {
        assert_eq!(
            observation(
                ObservedSubject::File {
                    path: "out.txt".to_string()
                },
                ObservedOperation::Contains(text("x"))
            ),
            vec!["\"out.txt\" contains \"x\""]
        );
        assert_eq!(
            observation(
                ObservedSubject::Dir {
                    path: "out".to_string()
                },
                ObservedOperation::Contains(text("x"))
            ),
            vec!["the directory \"out\" has an entry named \"x\""]
        );
    }

    #[test]
    fn file_and_directory_existence_read_as_sentences() {
        assert_eq!(
            observation(
                ObservedSubject::File {
                    path: "config.kdl".to_string()
                },
                ObservedOperation::Exists
            ),
            vec!["\"config.kdl\" exists"]
        );
        assert_eq!(
            observation(
                ObservedSubject::Dir {
                    path: "out".to_string()
                },
                ObservedOperation::DoesNotExist
            ),
            vec!["the directory \"out\" does not exist"]
        );
    }

    /// A file the example itself has is named; one kept beside the scenario
    /// source is not, because a reader cannot find it.
    #[test]
    fn a_compared_file_is_named_only_when_the_example_contains_it() {
        assert_eq!(
            observation(
                ObservedSubject::Stdout,
                ObservedOperation::Is(ExpectedValue::FileContents {
                    path: "expected.txt".to_string(),
                    origin: FileContentsOrigin::Example,
                })
            ),
            vec!["standard output is exactly the contents of \"expected.txt\""]
        );
        assert_eq!(
            observation(
                ObservedSubject::Stdout,
                ObservedOperation::Is(ExpectedValue::FileContents {
                    path: "fixtures/expected.txt".to_string(),
                    origin: FileContentsOrigin::External,
                })
            ),
            vec!["standard output is exactly the expected contents recorded for this example"]
        );
    }

    #[test]
    fn compositions_report_the_depth_of_every_nested_condition() {
        let lines = condition_lines(
            &DocumentedExpectation::Composition {
                operator: CompositionOperator::Not,
                children: vec![DocumentedExpectation::Composition {
                    operator: CompositionOperator::Any,
                    children: vec![
                        DocumentedExpectation::Observation {
                            subject: ObservedSubject::ExitCode,
                            operation: ObservedOperation::Is(ExpectedValue::Number(1)),
                        },
                        DocumentedExpectation::Observation {
                            subject: ObservedSubject::Stderr,
                            operation: ObservedOperation::IsEmpty,
                        },
                    ],
                }],
            },
            &Quoted,
        );

        assert_eq!(
            depths_and_text(&lines),
            vec![
                (0, "not:"),
                (1, "at least one of the following:"),
                (2, "the exit code is 1"),
                (2, "standard error is empty"),
            ]
        );
    }

    fn depths_and_text(lines: &[ConditionLine]) -> Vec<(usize, &str)> {
        lines
            .iter()
            .map(|line| (line.depth, line.text.as_str()))
            .collect()
    }

    /// `not { A B }` is `not(all(A, B))`, so it must not read as "none of the
    /// following", which would claim both children fail. With one child the
    /// group is the child, so the negation is stated plainly.
    #[test]
    fn a_not_over_several_conditions_negates_the_group_not_each_child() {
        let one = condition_lines(
            &DocumentedExpectation::Composition {
                operator: CompositionOperator::Not,
                children: vec![DocumentedExpectation::Observation {
                    subject: ObservedSubject::Stderr,
                    operation: ObservedOperation::IsEmpty,
                }],
            },
            &Quoted,
        );
        assert_eq!(
            depths_and_text(&one),
            vec![(0, "not:"), (1, "standard error is empty"),]
        );

        let several = condition_lines(
            &DocumentedExpectation::Composition {
                operator: CompositionOperator::Not,
                children: vec![
                    DocumentedExpectation::Observation {
                        subject: ObservedSubject::ExitCode,
                        operation: ObservedOperation::Is(ExpectedValue::Number(0)),
                    },
                    DocumentedExpectation::Observation {
                        subject: ObservedSubject::Stdout,
                        operation: ObservedOperation::Contains(text("ok")),
                    },
                ],
            },
            &Quoted,
        );
        assert_eq!(
            depths_and_text(&several),
            vec![
                (0, "not all of the following:"),
                (1, "the exit code is 0"),
                (1, "standard output contains \"ok\""),
            ]
        );
    }

    /// Conditions stay one line each, so a multi-line expected value is
    /// reduced rather than wrapped. The backslash is escaped alongside, so a
    /// literal `\n` in the source stays distinct from a real newline; the
    /// quote is not, because delimiting is the format's job.
    #[test]
    fn inline_text_escapes_line_structure_but_not_delimiters() {
        assert_eq!(
            inline_text(&DocumentedText::Literal("a\nb\t\"c\"\\d".to_string())),
            "a\\nb\\t\"c\"\\\\d"
        );
    }

    /// File content is the opposite: it is a block a reader copies, so its
    /// lines survive and nothing is escaped.
    #[test]
    fn content_keeps_its_lines_unescaped() {
        assert_eq!(
            content_lines(&DocumentedText::Literal(
                "name \"demo\"\r\nversion 1\n".to_string()
            )),
            vec!["name \"demo\"", "version 1"]
        );
    }

    /// An empty file has no content lines at all, so a format never renders a
    /// stray blank line for it.
    #[test]
    fn empty_content_has_no_lines() {
        assert!(content_lines(&DocumentedText::Literal(String::new())).is_empty());
    }

    /// A captured value has no text in the source, so both renderings name it
    /// instead of inventing one.
    #[test]
    fn captured_values_appear_as_their_name() {
        let composed = DocumentedText::Composed(vec![
            TextSegment::Literal("name \"".to_string()),
            TextSegment::Captured {
                binding: "id".to_string(),
            },
            TextSegment::Literal("\"\n".to_string()),
        ]);

        assert_eq!(inline_text(&composed), "name \"<id>\"\\n");
        assert_eq!(content_lines(&composed), vec!["name \"<id>\""]);
    }

    #[test]
    fn a_named_mode_follows_the_path_and_an_unnamed_one_shows_nothing() {
        assert_eq!(mode_suffix(Some(0o755)), " (mode 0o755)");
        assert_eq!(mode_suffix(Some(0o600)), " (mode 0o600)");
        assert_eq!(mode_suffix(None), "");
    }

    #[test]
    fn file_count_conditions_read_as_sentences() {
        assert_eq!(
            observation(
                ObservedSubject::FileCount {
                    glob: "out/*.txt".to_string()
                },
                ObservedOperation::CountIs {
                    comparison: CountComparison::AtLeast,
                    count: 2,
                }
            ),
            vec!["the number of files matching \"out/*.txt\" is at least 2"]
        );
    }
}
