//! Plain text renderer for product-facing documentation: serializes a
//! [`ProductDocumentationCatalog`] into the fixed single-document plain text
//! contract.
//!
//! The serialization contract (fixed by generated-document snapshots and the
//! reference documentation, docs/reference/docs-generation.md):
//!
//! - the document starts with the document title from the render options
//! - blocks are separated by exactly one empty line
//! - a block is a label line followed by its value lines, indented two spaces;
//!   file content is indented four, under its path
//! - the block labels are `Group`, `Section`, `Description`, `Example`,
//!   `Preparation`, `Steps`, `File`, `Command`, and `Verified outcome`
//! - `Description` is omitted entirely when absent
//! - `Preparation` and `Steps` are label-only separator blocks, emitted only
//!   when the example has preparation steps; without preparation, the
//!   example's own steps follow its metadata directly
//! - a `File` block holds the path, its `(mode 0o...)` suffix when the source
//!   named one, and the file's content lines under it; an empty file is its
//!   path alone
//! - a `Verified outcome` block holds one condition per line, with nested
//!   logical compositions indented two further spaces
//! - line endings are normalized to LF and the document ends with exactly one
//!   LF; no renderer-generated line adds trailing whitespace, while a file
//!   content line that carries some keeps it, because the block is what a
//!   reader copies
//! - a value's own trailing blank lines are not reproduced: they have no
//!   representation between blocks separated by one empty line
//!
//! Nothing here names a Reportage construct. The source path is deliberately
//! not shown either: it locates the scenario that produced the example, which
//! is the product's own repository detail, not the reader's. See
//! docs/adr/20260914T161520Z_product-document-serialization.md.

use super::product::{
    ExampleStep, ProductDocumentationCatalog, ProductDocumentedFile, ProductExample,
};
use super::product_render::{ValueStyle, condition_lines, content_lines, mode_suffix, one_line};
use super::render::{DocumentRenderer, RenderOptions, lf, logical_lines};

const VALUE_INDENT: usize = 2;
const CONTENT_INDENT: usize = 4;
/// What one nesting level of a logical composition adds to a condition line.
const NESTING_INDENT: usize = 2;

/// The `plain` format for product documentation.
pub struct ProductPlainRenderer;

/// Plain text has no code span, so a value is quoted.
struct PlainStyle;

impl ValueStyle for PlainStyle {
    /// A quote inside the value is escaped, so the closing quote is always the
    /// one that ends the value. The backslash is already escaped by
    /// `one_line`, which is what keeps `\"` unambiguous here.
    fn code(&self, value: &str) -> String {
        format!("\"{}\"", value.replace('"', "\\\""))
    }
}

impl DocumentRenderer<ProductDocumentationCatalog> for ProductPlainRenderer {
    fn render(&self, catalog: &ProductDocumentationCatalog, options: &RenderOptions) -> String {
        let mut blocks: Vec<String> = Vec::new();

        for group in &catalog.groups {
            blocks.push(block("Group", &group.name, VALUE_INDENT));
            for file in &group.files {
                blocks.extend(file_blocks(file));
            }
        }

        // A value whose last logical line is blank — a file whose content ends
        // with an empty line — would otherwise leave its block ending in a
        // newline and put two empty lines before the next block. Plain text
        // has no way to show a trailing blank line inside a block anyway, so
        // the separation contract wins over reproducing it.
        //
        // The title is exempt and prepended after the trim: it is used
        // verbatim except for the document-wide LF normalization, and trimming
        // it here would make `--title` mean something different on this
        // subcommand than on every other format and subcommand.
        let mut document = vec![lf(&options.document_title)];
        document.extend(
            blocks
                .iter()
                .map(|block| block.trim_end_matches('\n').to_string()),
        );
        document.join("\n\n") + "\n"
    }

    fn file_extension(&self) -> &'static str {
        "txt"
    }
}

fn file_blocks(file: &ProductDocumentedFile) -> Vec<String> {
    let mut blocks = vec![block("Section", &file.title, VALUE_INDENT)];
    if let Some(description) = &file.description {
        blocks.push(block("Description", description, VALUE_INDENT));
    }
    for example in &file.examples {
        blocks.extend(example_blocks(example));
    }
    blocks
}

fn example_blocks(example: &ProductExample) -> Vec<String> {
    let mut blocks = vec![block("Example", &example.title, VALUE_INDENT)];
    if let Some(description) = &example.description {
        blocks.push(block("Description", description, VALUE_INDENT));
    }

    // The two separator blocks exist only to close the preparation section, so
    // an example without preparation gets neither: there is nothing to
    // separate, and a lone `Steps` label would suggest a missing section.
    if !example.preparation.is_empty() {
        blocks.push("Preparation".to_string());
        blocks.extend(example.preparation.iter().map(step_block));
        blocks.push("Steps".to_string());
    }
    blocks.extend(example.steps.iter().map(step_block));
    blocks
}

fn step_block(step: &ExampleStep) -> String {
    match step {
        ExampleStep::File(file) => {
            // Through `one_line`, like every value in a code span on the
            // Markdown side: a path carrying a line break would otherwise put
            // its tail at column 0, where nothing distinguishes it from a
            // block label.
            let mut out = format!(
                "File\n{}{}{}",
                " ".repeat(VALUE_INDENT),
                one_line(&file.path),
                mode_suffix(file.mode)
            );
            for line in content_lines(&file.content) {
                out.push('\n');
                push_indented(&mut out, &line, CONTENT_INDENT);
            }
            out
        }
        ExampleStep::Command(command) => block("Command", &command.command, VALUE_INDENT),
        ExampleStep::Verification(verification) => {
            let mut out = String::from("Verified outcome");
            for expectation in &verification.expectations {
                for line in condition_lines(expectation, &PlainStyle) {
                    out.push('\n');
                    push_indented(
                        &mut out,
                        &line.text,
                        VALUE_INDENT + line.depth * NESTING_INDENT,
                    );
                }
            }
            out
        }
    }
}

/// One labeled block: the label line followed by the value's logical lines,
/// each non-empty line indented by `indent` spaces.
///
/// Empty logical lines are emitted without indentation so the document never
/// contains trailing whitespace, and a trailing final newline in the value
/// does not add an empty line: block separation is owned by the join in
/// `render`. Both rules match the Reportage-source plain format, because they
/// are properties of plain text, not of either projection.
fn block(label: &str, value: &str, indent: usize) -> String {
    let mut out = String::from(label);
    for line in logical_lines(value) {
        out.push('\n');
        push_indented(&mut out, &line, indent);
    }
    out
}

fn push_indented(out: &mut String, line: &str, indent: usize) {
    if !line.is_empty() {
        out.push_str(&" ".repeat(indent));
        out.push_str(line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docs::product::{
        DocumentedExpectation, DocumentedText, ExampleCommand, ExampleFile, ExampleVerification,
        ExpectedValue, ObservedOperation, ObservedSubject, ProductGroup,
    };

    fn file_step(path: &str, content: &str, mode: Option<u32>) -> ExampleStep {
        ExampleStep::File(ExampleFile {
            path: path.to_string(),
            content: DocumentedText::Literal(content.to_string()),
            mode,
        })
    }

    fn command_step(command: &str) -> ExampleStep {
        ExampleStep::Command(ExampleCommand {
            command: command.to_string(),
        })
    }

    fn verification_step(expectations: Vec<DocumentedExpectation>) -> ExampleStep {
        ExampleStep::Verification(ExampleVerification { expectations })
    }

    fn observation(
        subject: ObservedSubject,
        operation: ObservedOperation,
    ) -> DocumentedExpectation {
        DocumentedExpectation::Observation { subject, operation }
    }

    fn catalog(examples: Vec<ProductExample>) -> ProductDocumentationCatalog {
        ProductDocumentationCatalog {
            groups: vec![ProductGroup {
                name: "Getting started".to_string(),
                files: vec![ProductDocumentedFile {
                    title: "Project setup".to_string(),
                    description: Some("How a project is created.".to_string()),
                    source_path: "examples/setup.repor".to_string(),
                    examples,
                }],
            }],
        }
    }

    fn render(catalog: &ProductDocumentationCatalog) -> String {
        ProductPlainRenderer.render(catalog, &RenderOptions::default())
    }

    /// The shape the projection exists for, end to end: prepare a file, run a
    /// command, read what the run is verified to produce.
    #[test]
    fn a_file_command_and_verification_render_as_one_readable_example() {
        let document = render(&catalog(vec![ProductExample {
            title: "Initializing a project".to_string(),
            description: Some("Creates the config file.".to_string()),
            preparation: Vec::new(),
            steps: vec![
                file_step("config.kdl", "name \"demo\"\nversion 1\n", None),
                command_step("demo init"),
                verification_step(vec![
                    observation(
                        ObservedSubject::ExitCode,
                        ObservedOperation::Is(ExpectedValue::Number(0)),
                    ),
                    observation(
                        ObservedSubject::Stdout,
                        ObservedOperation::Contains(ExpectedValue::Text(DocumentedText::Literal(
                            "created".to_string(),
                        ))),
                    ),
                    observation(
                        ObservedSubject::File {
                            path: "config.kdl".to_string(),
                        },
                        ObservedOperation::Exists,
                    ),
                ]),
            ],
        }]));

        assert_eq!(
            document,
            concat!(
                "Reportage Documentation\n",
                "\n",
                "Group\n",
                "  Getting started\n",
                "\n",
                "Section\n",
                "  Project setup\n",
                "\n",
                "Description\n",
                "  How a project is created.\n",
                "\n",
                "Example\n",
                "  Initializing a project\n",
                "\n",
                "Description\n",
                "  Creates the config file.\n",
                "\n",
                "File\n",
                "  config.kdl\n",
                "    name \"demo\"\n",
                "    version 1\n",
                "\n",
                "Command\n",
                "  demo init\n",
                "\n",
                "Verified outcome\n",
                "  the exit code is 0\n",
                "  standard output contains \"created\"\n",
                "  \"config.kdl\" exists\n",
            )
        );
    }

    /// Preparation is fenced by its two label blocks so a reader always knows
    /// where the shared setup ends and the example's own steps begin.
    #[test]
    fn preparation_is_separated_from_the_examples_own_steps() {
        let document = render(&catalog(vec![ProductExample {
            title: "With setup".to_string(),
            description: None,
            preparation: vec![file_step("seed.txt", "seed\n", None), command_step("setup")],
            steps: vec![
                command_step("run"),
                verification_step(vec![observation(
                    ObservedSubject::ExitCode,
                    ObservedOperation::Is(ExpectedValue::Number(0)),
                )]),
            ],
        }]));

        assert!(document.contains(concat!(
            "Preparation\n",
            "\n",
            "File\n",
            "  seed.txt\n",
            "    seed\n",
            "\n",
            "Command\n",
            "  setup\n",
            "\n",
            "Steps\n",
            "\n",
            "Command\n",
            "  run\n",
        )));
    }

    /// Without preparation there is nothing to separate, so neither label
    /// appears and the steps follow the example metadata directly.
    #[test]
    fn an_example_without_preparation_has_no_separator_labels() {
        let document = render(&catalog(vec![ProductExample {
            title: "No setup".to_string(),
            description: None,
            preparation: Vec::new(),
            steps: vec![
                command_step("run"),
                verification_step(vec![observation(
                    ObservedSubject::ExitCode,
                    ObservedOperation::Is(ExpectedValue::Number(0)),
                )]),
            ],
        }]));

        assert!(!document.contains("Preparation"));
        assert!(!document.contains("Steps"));
        assert!(document.contains("Example\n  No setup\n\nCommand\n  run\n"));
    }

    /// A named mode follows the path, because a reader recreating the file
    /// needs it; an empty file is its path alone, with no blank content line.
    #[test]
    fn a_file_shows_its_mode_and_omits_empty_content() {
        let document = render(&catalog(vec![ProductExample {
            title: "Files".to_string(),
            description: None,
            preparation: Vec::new(),
            steps: vec![
                file_step("bin/run", "#!/bin/sh\n", Some(0o755)),
                file_step("empty.txt", "", None),
                verification_step(vec![observation(
                    ObservedSubject::ExitCode,
                    ObservedOperation::Is(ExpectedValue::Number(0)),
                )]),
            ],
        }]));

        assert!(document.contains("File\n  bin/run (mode 0o755)\n    #!/bin/sh\n"));
        assert!(document.contains("File\n  empty.txt\n\n"));
    }

    /// No line may carry trailing whitespace, and the document ends with
    /// exactly one LF, whatever the content's own line structure is.
    #[test]
    fn document_tail_and_whitespace_contract() {
        let document = render(&catalog(vec![ProductExample {
            title: "Blank lines".to_string(),
            description: Some("Multi\n\nline.\n".to_string()),
            preparation: Vec::new(),
            steps: vec![
                file_step("a.txt", "first\n\nthird\n", None),
                verification_step(vec![observation(
                    ObservedSubject::ExitCode,
                    ObservedOperation::Is(ExpectedValue::Number(0)),
                )]),
            ],
        }]));

        assert!(document.ends_with('\n'));
        assert!(!document.ends_with("\n\n"));
        // Scoped to renderer-generated lines: file content is reproduced
        // verbatim, so content that carries trailing whitespace keeps it (see
        // `content_keeps_trailing_whitespace_but_not_trailing_blank_lines`).
        for line in document.lines() {
            assert_eq!(
                line,
                line.trim_end(),
                "no renderer-generated line may carry trailing whitespace"
            );
        }
    }

    /// CRLF anywhere in the catalog becomes LF, so the document is
    /// platform-independent.
    #[test]
    fn crlf_is_normalized_to_lf() {
        let document = render(&catalog(vec![ProductExample {
            title: "CRLF".to_string(),
            description: Some("Line one.\r\nLine two.\r\n".to_string()),
            preparation: Vec::new(),
            steps: vec![
                file_step("a.txt", "first\r\nsecond\r\n", None),
                verification_step(vec![observation(
                    ObservedSubject::ExitCode,
                    ObservedOperation::Is(ExpectedValue::Number(0)),
                )]),
            ],
        }]));

        assert!(!document.contains('\r'));
        assert!(document.contains("  Line one.\n  Line two.\n"));
        assert!(document.contains("    first\n    second\n"));
    }

    /// A path carrying a line break must not put its tail at column 0, where
    /// it would be indistinguishable from a block label.
    #[test]
    fn a_path_containing_a_line_break_stays_on_one_line() {
        let document = render(&catalog(vec![ProductExample {
            title: "Odd path".to_string(),
            description: None,
            preparation: Vec::new(),
            steps: vec![
                file_step("a\nb.txt", "x\n", None),
                verification_step(vec![observation(
                    ObservedSubject::ExitCode,
                    ObservedOperation::Is(ExpectedValue::Number(0)),
                )]),
            ],
        }]));

        assert!(document.contains("File\n  a\\nb.txt\n    x\n"));
    }

    /// An empty expected value is named rather than delimited: an empty quoted
    /// fragment reads as a typo.
    #[test]
    fn an_empty_expected_value_is_named_rather_than_quoted() {
        let document = render(&catalog(vec![ProductExample {
            title: "Empty".to_string(),
            description: None,
            preparation: Vec::new(),
            steps: vec![verification_step(vec![observation(
                ObservedSubject::Stdout,
                ObservedOperation::Contains(ExpectedValue::Text(DocumentedText::Literal(
                    String::new(),
                ))),
            )])],
        }]));

        assert!(document.contains("  standard output contains the empty string\n"));
        assert!(!document.contains("\"\""));
    }

    /// A value containing the format's own delimiter must not close it: the
    /// quote is escaped, and the backslash `one_line` already escaped keeps
    /// the result unambiguous.
    #[test]
    fn a_quoted_value_escapes_a_quote_inside_it() {
        let document = render(&catalog(vec![ProductExample {
            title: "Quoting".to_string(),
            description: None,
            preparation: Vec::new(),
            steps: vec![
                command_step("run"),
                verification_step(vec![observation(
                    ObservedSubject::Stdout,
                    ObservedOperation::Contains(ExpectedValue::Text(DocumentedText::Literal(
                        "say \"hi\"".to_string(),
                    ))),
                )]),
            ],
        }]));

        assert!(document.contains("standard output contains \"say \\\"hi\\\"\"\n"));
    }

    /// File content is reproduced verbatim, so a content line that carries
    /// trailing whitespace keeps it — trimming would corrupt the example a
    /// reader copies. A value's own trailing blank lines are dropped instead,
    /// because plain text cannot show them between one-empty-line separators.
    #[test]
    fn content_keeps_trailing_whitespace_but_not_trailing_blank_lines() {
        let document = render(&catalog(vec![ProductExample {
            title: "Whitespace".to_string(),
            description: None,
            preparation: Vec::new(),
            steps: vec![
                file_step("a.txt", "trailing   \nlast\n\n", None),
                command_step("run"),
                verification_step(vec![observation(
                    ObservedSubject::ExitCode,
                    ObservedOperation::Is(ExpectedValue::Number(0)),
                )]),
            ],
        }]));

        assert!(document.contains("    trailing   \n"));
        assert!(document.contains("    last\n\nCommand\n"));
        assert!(!document.contains("\n\n\n"));
    }

    /// A nested composition keeps its structure through indentation, so the
    /// grouping the scenario wrote survives into the documentation.
    #[test]
    fn nested_conditions_are_indented_under_their_operator() {
        let document = render(&catalog(vec![ProductExample {
            title: "Logic".to_string(),
            description: None,
            preparation: Vec::new(),
            steps: vec![verification_step(vec![
                DocumentedExpectation::Composition {
                    operator: crate::docs::product::CompositionOperator::Not,
                    children: vec![observation(
                        ObservedSubject::Stderr,
                        ObservedOperation::IsEmpty,
                    )],
                },
            ])],
        }]));

        assert!(document.contains(concat!(
            "Verified outcome\n",
            "  not:\n",
            "    standard error is empty\n",
        )));
    }

    /// An empty catalog still produces a document: the title alone, with the
    /// same tail contract as any other.
    #[test]
    fn an_empty_catalog_renders_the_title_alone() {
        let document = render(&ProductDocumentationCatalog { groups: vec![] });
        assert_eq!(document, "Reportage Documentation\n");
    }

    /// The document exposes no Reportage construct: not the DSL wrappers, not
    /// the `before_each` name, and not the scenario's own path.
    #[test]
    fn the_document_exposes_no_reportage_construct() {
        let document = render(&catalog(vec![ProductExample {
            title: "Anything".to_string(),
            description: None,
            preparation: vec![command_step("setup")],
            steps: vec![
                command_step("run"),
                verification_step(vec![observation(
                    ObservedSubject::ExitCode,
                    ObservedOperation::Is(ExpectedValue::Number(0)),
                )]),
            ],
        }]));

        for construct in [
            "case \"",
            "assert {",
            "before_each",
            "write <",
            "exit 0",
            ".repor",
        ] {
            assert!(
                !document.contains(construct),
                "product output must not contain {construct:?}:\n{document}"
            );
        }
    }
}
