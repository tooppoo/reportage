//! Markdown renderer for product-facing documentation: serializes a
//! [`ProductDocumentationCatalog`] into the fixed single-document Markdown
//! contract.
//!
//! Navigation is identical to the Reportage-source Markdown format — a
//! `## Contents` list linking to explicit anchors, one anchored heading per
//! structural entity — because `--format markdown` must mean the same kind of
//! document whichever projection produced it. Only the heading vocabulary and
//! the body of an example differ.
//!
//! The serialization contract (fixed by generated-document snapshots and the
//! reference documentation, docs/reference/docs-generation.md):
//!
//! | Section | Heading |
//! | --- | --- |
//! | document title | `#` |
//! | table of contents (`Contents`) | `##` |
//! | group | `##` |
//! | section (one documented source) | `###` |
//! | example (one case) | `####` |
//!
//! - every group, section, and example heading is immediately preceded by its
//!   explicit `<a id="...">` anchor on its own line
//! - anchor IDs are `group-{g}`, `section-{g}-{s}`, and
//!   `example-{g}-{s}-{e}`, each with the shared readability slug
//! - descriptions follow their heading and are omitted entirely when absent
//! - an example's steps are `**Preparation**` and `**Steps**` labelled
//!   paragraphs, emitted only when the example has preparation; the labels are
//!   not headings, so they add no anchor and no table-of-contents entry
//! - a file step is its path as a code span, with its `(mode 0o...)` suffix
//!   when the source named one, followed by a fenced block holding the content
//! - a command step is a `sh` fenced block holding the command line alone,
//!   with no prompt
//! - a verification step is a `**Verified outcome**` label followed by a
//!   bulleted list, one condition per item, nested compositions as nested
//!   items
//! - renderer-generated blocks are separated by exactly one empty line, line
//!   endings are normalized to LF, and the document ends with exactly one LF
//!
//! Metadata is inserted verbatim, with the same consequences and the same two
//! renderer-owned exceptions as the Reportage-source Markdown format: anchor
//! IDs and code fences. See
//! docs/adr/20260914T161520Z_product-document-serialization.md.

use super::markdown_parts::{
    anchor_id, anchored_heading, code_span, description_block, fenced, numbered, toc_entry,
};
use super::product::{ExampleStep, ProductDocumentationCatalog};
use super::product_render::{ValueStyle, condition_lines, content_lines, mode_suffix, one_line};
use super::render::{DocumentRenderer, RenderOptions, lf};

/// The `markdown` format for product documentation.
pub struct ProductMarkdownRenderer;

/// Markdown has code spans, so a value is wrapped in one rather than quoted.
struct MarkdownStyle;

impl ValueStyle for MarkdownStyle {
    /// Through `code_span`, so a value containing backticks — a CLI message
    /// that quotes a file name, a config format that uses them — cannot close
    /// its own span.
    fn code(&self, value: &str) -> String {
        code_span(value)
    }
}

impl DocumentRenderer<ProductDocumentationCatalog> for ProductMarkdownRenderer {
    fn render(&self, catalog: &ProductDocumentationCatalog, options: &RenderOptions) -> String {
        // The table of contents and the section anchors are built in the same
        // pass so an anchor can never diverge from the entry linking to it.
        let mut toc_lines: Vec<String> = Vec::new();
        let mut section_blocks: Vec<String> = Vec::new();

        for (group_number, group) in numbered(&catalog.groups) {
            let group_anchor = anchor_id(&format!("group-{group_number}"), &group.name);
            toc_lines.push(toc_entry(0, &group.name, &group_anchor));
            section_blocks.push(anchored_heading("##", &group_anchor, &group.name));

            for (file_number, file) in numbered(&group.files) {
                let file_anchor = anchor_id(
                    &format!("section-{group_number}-{file_number}"),
                    &file.title,
                );
                toc_lines.push(toc_entry(1, &file.title, &file_anchor));
                section_blocks.push(anchored_heading("###", &file_anchor, &file.title));
                if let Some(description) = &file.description {
                    section_blocks.push(description_block(description));
                }

                for (example_number, example) in numbered(&file.examples) {
                    let example_anchor = anchor_id(
                        &format!("example-{group_number}-{file_number}-{example_number}"),
                        &example.title,
                    );
                    toc_lines.push(toc_entry(2, &example.title, &example_anchor));
                    section_blocks.push(anchored_heading("####", &example_anchor, &example.title));
                    if let Some(description) = &example.description {
                        section_blocks.push(description_block(description));
                    }

                    // The two labels exist only to close the preparation
                    // section, so an example without preparation gets neither.
                    // They are bold paragraphs, not headings: preparation is
                    // not a navigation entity, so the heading hierarchy and
                    // the example anchor numbering stay unchanged by it.
                    if !example.preparation.is_empty() {
                        section_blocks.push("**Preparation**".to_string());
                        section_blocks.extend(example.preparation.iter().flat_map(step_blocks));
                        section_blocks.push("**Steps**".to_string());
                    }
                    section_blocks.extend(example.steps.iter().flat_map(step_blocks));
                }
            }
        }

        let mut blocks = vec![
            format!("# {}", lf(&options.document_title)),
            "## Contents".to_string(),
        ];
        if !toc_lines.is_empty() {
            blocks.push(toc_lines.join("\n"));
        }
        blocks.extend(section_blocks);
        blocks.join("\n\n") + "\n"
    }

    fn file_extension(&self) -> &'static str {
        "md"
    }
}

/// One step as the blocks it contributes, in order.
fn step_blocks(step: &ExampleStep) -> Vec<String> {
    match step {
        ExampleStep::File(file) => {
            let mut blocks = vec![format!(
                "{}{}",
                code_span(&one_line(&file.path)),
                mode_suffix(file.mode)
            )];
            let lines = content_lines(&file.content);
            if !lines.is_empty() {
                // No info string: the content is the product's own file
                // format, which reportage does not know and must not guess.
                blocks.push(fenced("", &(lines.join("\n") + "\n")));
            }
            blocks
        }
        // `sh`, and no `$` prompt: the block is meant to be copied and run, so
        // a prompt character would have to be stripped by the reader.
        ExampleStep::Command(command) => {
            vec![fenced("sh", &(lf(&command.command) + "\n"))]
        }
        ExampleStep::Verification(verification) => {
            let items: Vec<String> = verification
                .expectations
                .iter()
                .flat_map(|expectation| condition_lines(expectation, &MarkdownStyle))
                .map(|line| format!("{}- {}", "  ".repeat(line.depth), line.text))
                .collect();
            vec!["**Verified outcome**".to_string(), items.join("\n")]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docs::product::{
        CompositionOperator, DocumentedExpectation, DocumentedText, ExampleCommand, ExampleFile,
        ExampleVerification, ExpectedValue, ObservedOperation, ObservedSubject,
        ProductDocumentedFile, ProductExample, ProductGroup,
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

    fn exit_zero() -> DocumentedExpectation {
        DocumentedExpectation::Observation {
            subject: ObservedSubject::ExitCode,
            operation: ObservedOperation::Is(ExpectedValue::Number(0)),
        }
    }

    fn verification_step(expectations: Vec<DocumentedExpectation>) -> ExampleStep {
        ExampleStep::Verification(ExampleVerification { expectations })
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
        ProductMarkdownRenderer.render(catalog, &RenderOptions::default())
    }

    fn one_example(steps: Vec<ExampleStep>, preparation: Vec<ExampleStep>) -> String {
        render(&catalog(vec![ProductExample {
            title: "Initializing a project".to_string(),
            description: None,
            preparation,
            steps,
        }]))
    }

    /// The whole document for the shape the projection exists for, fixing the
    /// heading hierarchy, the table of contents, the anchors, and every step
    /// block at once.
    #[test]
    fn a_file_command_and_verification_render_as_one_navigable_document() {
        let document = one_example(
            vec![
                file_step("config.kdl", "name \"demo\"\nversion 1\n", None),
                command_step("demo init"),
                verification_step(vec![
                    exit_zero(),
                    DocumentedExpectation::Observation {
                        subject: ObservedSubject::Stdout,
                        operation: ObservedOperation::Contains(ExpectedValue::Text(
                            DocumentedText::Literal("created".to_string()),
                        )),
                    },
                    DocumentedExpectation::Observation {
                        subject: ObservedSubject::File {
                            path: "config.kdl".to_string(),
                        },
                        operation: ObservedOperation::Exists,
                    },
                ]),
            ],
            Vec::new(),
        );

        assert_eq!(
            document,
            concat!(
                "# Reportage Documentation\n",
                "\n",
                "## Contents\n",
                "\n",
                "- [Getting started](#group-1-getting-started)\n",
                "  - [Project setup](#section-1-1-project-setup)\n",
                "    - [Initializing a project](#example-1-1-1-initializing-a-project)\n",
                "\n",
                "<a id=\"group-1-getting-started\"></a>\n",
                "## Getting started\n",
                "\n",
                "<a id=\"section-1-1-project-setup\"></a>\n",
                "### Project setup\n",
                "\n",
                "How a project is created.\n",
                "\n",
                "<a id=\"example-1-1-1-initializing-a-project\"></a>\n",
                "#### Initializing a project\n",
                "\n",
                "`config.kdl`\n",
                "\n",
                "```\n",
                "name \"demo\"\n",
                "version 1\n",
                "```\n",
                "\n",
                "```sh\n",
                "demo init\n",
                "```\n",
                "\n",
                "**Verified outcome**\n",
                "\n",
                "- the exit code is 0\n",
                "- standard output contains `created`\n",
                "- `config.kdl` exists\n",
            )
        );
    }

    /// Preparation is fenced by its two labels, which are paragraphs rather
    /// than headings: the anchor namespace and heading hierarchy must be the
    /// same as for an example without preparation.
    #[test]
    fn preparation_is_labelled_without_becoming_a_navigation_entity() {
        let document = one_example(
            vec![command_step("run"), verification_step(vec![exit_zero()])],
            vec![command_step("setup")],
        );

        assert!(document.contains(concat!(
            "**Preparation**\n",
            "\n",
            "```sh\n",
            "setup\n",
            "```\n",
            "\n",
            "**Steps**\n",
            "\n",
            "```sh\n",
            "run\n",
            "```\n",
        )));
        assert_eq!(document.matches("<a id=").count(), 3);
        assert_eq!(document.matches("\n#").count(), 4);
    }

    /// Without preparation neither label appears, so no empty section is
    /// generated.
    #[test]
    fn an_example_without_preparation_has_no_labels() {
        let document = one_example(
            vec![command_step("run"), verification_step(vec![exit_zero()])],
            Vec::new(),
        );

        assert!(!document.contains("**Preparation**"));
        assert!(!document.contains("**Steps**"));
    }

    /// A named mode follows the path; an empty file is its path alone, with no
    /// empty fenced block.
    #[test]
    fn a_file_shows_its_mode_and_omits_an_empty_content_block() {
        let document = one_example(
            vec![
                file_step("bin/run", "#!/bin/sh\n", Some(0o755)),
                file_step("empty.txt", "", None),
                verification_step(vec![exit_zero()]),
            ],
            Vec::new(),
        );

        assert!(document.contains("`bin/run` (mode 0o755)\n\n```\n#!/bin/sh\n```\n"));
        assert!(document.contains("`empty.txt`\n\n**Verified outcome**"));
    }

    /// Content containing a fence gets a longer fence, so a product's own
    /// Markdown or fenced config cannot terminate the block early.
    #[test]
    fn file_content_containing_a_fence_gets_a_longer_fence() {
        let document = one_example(
            vec![
                file_step("README.md", "text\n```sh\nrun\n```\n", None),
                verification_step(vec![exit_zero()]),
            ],
            Vec::new(),
        );

        assert!(document.contains("````\ntext\n```sh\nrun\n```\n````"));
    }

    /// An empty expected value is named, never wrapped: `` `` `` is not a code
    /// span in Markdown, so delimiting it would emit two literal backticks.
    #[test]
    fn an_empty_expected_value_is_named_rather_than_spanned() {
        let document = one_example(
            vec![verification_step(vec![
                DocumentedExpectation::Observation {
                    subject: ObservedSubject::Stdout,
                    operation: ObservedOperation::Contains(ExpectedValue::Text(
                        DocumentedText::Literal(String::new()),
                    )),
                },
            ])],
            Vec::new(),
        );

        assert!(document.contains("- standard output contains the empty string\n"));
        assert!(!document.contains("``\n"));
    }

    /// A nested composition becomes a nested list, so the grouping the
    /// scenario wrote survives into the rendered document.
    #[test]
    fn nested_conditions_become_nested_list_items() {
        let document = one_example(
            vec![verification_step(vec![
                DocumentedExpectation::Composition {
                    operator: CompositionOperator::Not,
                    children: vec![DocumentedExpectation::Composition {
                        operator: CompositionOperator::Any,
                        children: vec![
                            exit_zero(),
                            DocumentedExpectation::Observation {
                                subject: ObservedSubject::Stderr,
                                operation: ObservedOperation::IsEmpty,
                            },
                        ],
                    }],
                },
            ])],
            Vec::new(),
        );

        assert!(document.contains(concat!(
            "**Verified outcome**\n",
            "\n",
            "- not:\n",
            "  - at least one of the following:\n",
            "    - the exit code is 0\n",
            "    - standard error is empty\n",
        )));
    }

    /// A value containing backticks — a CLI that quotes a file name, a config
    /// format that uses them — must not close its own code span.
    #[test]
    fn a_code_span_survives_backticks_in_the_value() {
        let document = one_example(
            vec![
                file_step("a`b.txt", "x\n", None),
                verification_step(vec![DocumentedExpectation::Observation {
                    subject: ObservedSubject::Stdout,
                    operation: ObservedOperation::Contains(ExpectedValue::Text(
                        DocumentedText::Literal("created `demo.kdl`".to_string()),
                    )),
                }]),
            ],
            Vec::new(),
        );

        assert!(document.contains("``a`b.txt``"));
        assert!(document.contains("- standard output contains `` created `demo.kdl` ``"));
    }

    /// Metadata is inserted verbatim — the raw metadata policy the
    /// Reportage-source Markdown format already states — while the anchor ID
    /// stays renderer-generated ASCII.
    #[test]
    fn metadata_is_not_escaped_and_anchors_stay_generated_ascii() {
        let document = render(&ProductDocumentationCatalog {
            groups: vec![ProductGroup {
                name: "## Not a heading".to_string(),
                files: vec![ProductDocumentedFile {
                    title: "**bold** <script>x</script>".to_string(),
                    description: None,
                    source_path: "a.repor".to_string(),
                    examples: vec![ProductExample {
                        title: "日本語".to_string(),
                        description: None,
                        preparation: Vec::new(),
                        steps: vec![verification_step(vec![exit_zero()])],
                    }],
                }],
            }],
        });

        assert!(document.contains("## ## Not a heading"));
        assert!(document.contains("### **bold** <script>x</script>"));
        assert!(document.contains("<a id=\"section-1-1-bold-script-x-script\"></a>"));
        // A title with no ASCII slug keeps the index-only anchor.
        assert!(document.contains("<a id=\"example-1-1-1\"></a>"));
    }

    /// CRLF anywhere in the catalog becomes LF, and the document ends with
    /// exactly one LF.
    #[test]
    fn crlf_is_normalized_and_the_document_ends_with_one_newline() {
        let document = render(&catalog(vec![ProductExample {
            title: "CRLF".to_string(),
            description: Some("Line one.\r\nLine two.\r\n".to_string()),
            preparation: Vec::new(),
            steps: vec![
                file_step("a.txt", "first\r\nsecond\r\n", None),
                verification_step(vec![exit_zero()]),
            ],
        }]));

        assert!(!document.contains('\r'));
        assert!(document.contains("Line one.\nLine two.\n"));
        assert!(document.contains("```\nfirst\nsecond\n```"));
        assert!(document.ends_with("\n"));
        assert!(!document.ends_with("\n\n"));
    }

    /// An empty catalog still produces a navigable document skeleton.
    #[test]
    fn an_empty_catalog_renders_the_title_and_contents_heading() {
        let document = render(&ProductDocumentationCatalog { groups: vec![] });
        assert_eq!(document, "# Reportage Documentation\n\n## Contents\n");
    }

    /// The document exposes no Reportage construct: not the DSL wrappers, not
    /// the `before_each` name, not the `reportage` fence language, and not the
    /// scenario's own path.
    #[test]
    fn the_document_exposes_no_reportage_construct() {
        let document = one_example(
            vec![command_step("run"), verification_step(vec![exit_zero()])],
            vec![command_step("setup")],
        );

        for construct in [
            "case \"",
            "assert {",
            "before_each",
            "write <",
            "```reportage",
            ".repor",
        ] {
            assert!(
                !document.contains(construct),
                "product output must not contain {construct:?}:\n{document}"
            );
        }
    }
}
