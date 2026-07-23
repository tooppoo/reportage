//! Plain text renderer: serializes a [`DocumentationCatalog`] into the fixed
//! single-document plain text contract.
//!
//! The serialization contract (fixed by generated-document snapshots and the
//! reference documentation, docs/reference/docs-generation.md):
//!
//! - the document starts with the document title from the render options
//!   (`Reportage Documentation` unless `--title` overrides it)
//! - blocks are separated by exactly one empty line
//! - `Group` / `File` / `Source path` / `Case` / `Description` values are
//!   indented by two spaces per logical line
//! - a `Description` block is omitted entirely when the value is absent
//! - `Reportage source` lines are indented by four spaces, except empty lines,
//!   which stay empty so no trailing whitespace is produced
//! - line endings are normalized to LF for the whole document, including
//!   inside source blocks
//! - the document ends with exactly one LF
//!
//! Beyond presentation indentation, LF normalization, and the block wrappers,
//! case source content is never dropped or replaced.

use super::catalog::DocumentationCatalog;
use super::render::{DocumentRenderer, RenderOptions};

const VALUE_INDENT: usize = 2;
const SOURCE_INDENT: usize = 4;

/// The `plain` format: renders a catalog into one plain text document.
pub struct PlainRenderer;

impl DocumentRenderer for PlainRenderer {
    fn render(&self, catalog: &DocumentationCatalog, options: &RenderOptions) -> String {
        // The title is inserted verbatim except for the document-wide LF
        // normalization; logical_lines would also drop a trailing-newline
        // tail, which the raw title mapping policy forbids.
        let mut blocks: Vec<String> = vec![options.document_title.replace("\r\n", "\n")];

        for group in &catalog.groups {
            blocks.push(block("Group", &group.name, VALUE_INDENT));
            for file in &group.files {
                blocks.push(block("File", &file.title, VALUE_INDENT));
                blocks.push(block("Source path", &file.source_path, VALUE_INDENT));
                if let Some(description) = &file.description {
                    blocks.push(block("Description", description, VALUE_INDENT));
                }
                for case in &file.cases {
                    blocks.push(block("Case", &case.title, VALUE_INDENT));
                    if let Some(description) = &case.description {
                        blocks.push(block("Description", description, VALUE_INDENT));
                    }
                    blocks.push(block("Reportage source", &case.source, SOURCE_INDENT));
                }
            }
        }

        blocks.join("\n\n") + "\n"
    }

    fn file_extension(&self) -> &'static str {
        "txt"
    }
}

/// One labeled block: the label line followed by the value's logical lines,
/// each non-empty line indented by `indent` spaces.
///
/// Empty logical lines are emitted without indentation so the document never
/// contains trailing whitespace. A trailing final newline in the value does
/// not add an extra empty line: block separation is owned by the join in
/// [`PlainRenderer::render`], keeping "exactly one empty line between blocks"
/// independent of whether the source ends with a newline.
fn block(label: &str, value: &str, indent: usize) -> String {
    let pad = " ".repeat(indent);
    let mut out = String::from(label);
    for line in logical_lines(value) {
        out.push('\n');
        if !line.is_empty() {
            out.push_str(&pad);
            out.push_str(&line);
        }
    }
    out
}

/// Splits a value into logical lines with CRLF normalized to LF, dropping the
/// empty tail produced by a final newline.
fn logical_lines(value: &str) -> Vec<String> {
    let normalized = value.replace("\r\n", "\n");
    let mut lines: Vec<String> = normalized.split('\n').map(str::to_string).collect();
    if lines.len() > 1 && lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docs::catalog::{
        DocumentationCatalog, DocumentationGroup, DocumentedCase, DocumentedFile,
    };

    fn single_case_catalog(case_source: &str) -> DocumentationCatalog {
        DocumentationCatalog {
            groups: vec![DocumentationGroup {
                name: "Filesystem".to_string(),
                files: vec![DocumentedFile {
                    title: "File assertions".to_string(),
                    description: Some("About files.".to_string()),
                    source_path: "examples/file-assertions.repor".to_string(),
                    cases: vec![DocumentedCase {
                        title: "File creation".to_string(),
                        description: None,
                        source: case_source.to_string(),
                    }],
                }],
            }],
        }
    }

    #[test]
    fn renders_the_fixed_block_layout() {
        let output = PlainRenderer.render(
            &single_case_catalog("case \"x\" {\n  $ true\n}\n"),
            &RenderOptions::default(),
        );

        assert_eq!(
            output,
            "Reportage Documentation\n\
             \n\
             Group\n  Filesystem\n\
             \n\
             File\n  File assertions\n\
             \n\
             Source path\n  examples/file-assertions.repor\n\
             \n\
             Description\n  About files.\n\
             \n\
             Case\n  File creation\n\
             \n\
             Reportage source\n    case \"x\" {\n      $ true\n    }\n"
        );
    }

    /// `--title` reaches the plain format through the render options: the
    /// value replaces the default title verbatim, without escaping.
    #[test]
    fn the_document_title_option_replaces_the_default_title() {
        let options = RenderOptions {
            document_title: "**Custom** <em>title</em>".to_string(),
        };

        let output = PlainRenderer.render(
            &single_case_catalog("case \"x\" {\n  $ true\n}\n"),
            &options,
        );
        assert!(output.starts_with("**Custom** <em>title</em>\n\nGroup\n"));
        assert!(!output.contains("Reportage Documentation"));
    }

    /// The documented empty-title promise: an empty `--title` is neither
    /// rejected nor replaced by the default; the document starts with an
    /// empty title line.
    #[test]
    fn an_empty_document_title_is_rendered_verbatim() {
        let options = RenderOptions {
            document_title: String::new(),
        };

        let output = PlainRenderer.render(
            &single_case_catalog("case \"x\" {\n  $ true\n}\n"),
            &options,
        );
        assert!(output.starts_with("\n\nGroup\n"));
        assert!(!output.contains("Reportage Documentation"));
    }

    #[test]
    fn description_blocks_are_omitted_when_absent() {
        let catalog = DocumentationCatalog {
            groups: vec![DocumentationGroup {
                name: "Index".to_string(),
                files: vec![DocumentedFile {
                    title: "minimal".to_string(),
                    description: None,
                    source_path: "minimal.repor".to_string(),
                    cases: vec![],
                }],
            }],
        };

        let output = PlainRenderer.render(&catalog, &RenderOptions::default());
        assert!(!output.contains("Description"));
        // A zero-case file has no Case / Reportage source blocks either.
        assert!(!output.contains("Case"));
        assert!(!output.contains("Reportage source"));
        assert!(output.ends_with("Source path\n  minimal.repor\n"));
    }

    #[test]
    fn source_empty_lines_stay_empty_without_trailing_whitespace() {
        let output = PlainRenderer.render(
            &single_case_catalog("case \"x\" {\n\n  $ true\n}\n"),
            &RenderOptions::default(),
        );

        assert!(output.contains("Reportage source\n    case \"x\" {\n\n      $ true\n    }\n"));
        for line in output.lines() {
            assert_eq!(
                line,
                line.trim_end(),
                "no line may carry trailing whitespace"
            );
        }
    }

    #[test]
    fn crlf_sources_are_normalized_to_lf() {
        let output = PlainRenderer.render(
            &single_case_catalog("case \"x\" {\r\n  $ true\r\n}\r\n"),
            &RenderOptions::default(),
        );

        assert!(!output.contains('\r'));
        assert!(output.contains("Reportage source\n    case \"x\" {\n      $ true\n    }\n"));
    }

    #[test]
    fn final_newline_presence_does_not_change_block_separation() {
        let with_newline = PlainRenderer.render(
            &single_case_catalog("case \"x\" {\n  $ true\n}\n"),
            &RenderOptions::default(),
        );
        let without_newline = PlainRenderer.render(
            &single_case_catalog("case \"x\" {\n  $ true\n}"),
            &RenderOptions::default(),
        );

        assert_eq!(with_newline, without_newline);
    }

    #[test]
    fn document_ends_with_exactly_one_lf() {
        let output = PlainRenderer.render(
            &single_case_catalog("case \"x\" {\n  $ true\n}\n"),
            &RenderOptions::default(),
        );
        assert!(output.ends_with('\n'));
        assert!(!output.ends_with("\n\n"));
    }

    #[test]
    fn multi_line_descriptions_indent_each_logical_line() {
        let mut catalog = single_case_catalog("case \"x\" {\n  $ true\n}\n");
        catalog.groups[0].files[0].description = Some("First line.\n\nThird line.\n".to_string());

        let output = PlainRenderer.render(&catalog, &RenderOptions::default());
        assert!(output.contains("Description\n  First line.\n\n  Third line.\n"));
    }

    #[test]
    fn groups_files_and_cases_repeat_the_same_shape() {
        let catalog = DocumentationCatalog {
            groups: vec![
                DocumentationGroup {
                    name: "A".to_string(),
                    files: vec![DocumentedFile {
                        title: "a".to_string(),
                        description: None,
                        source_path: "a.repor".to_string(),
                        cases: vec![],
                    }],
                },
                DocumentationGroup {
                    name: "B".to_string(),
                    files: vec![DocumentedFile {
                        title: "b".to_string(),
                        description: None,
                        source_path: "b.repor".to_string(),
                        cases: vec![],
                    }],
                },
            ],
        };

        let output = PlainRenderer.render(&catalog, &RenderOptions::default());
        assert!(
            output.contains("Group\n  A\n\nFile\n  a\n\nSource path\n  a.repor\n\nGroup\n  B\n")
        );
    }
}
