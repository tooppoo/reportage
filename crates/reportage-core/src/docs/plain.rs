//! Plain text renderer: serializes a [`DocumentationCatalog`] into the fixed
//! single-document plain text contract.
//!
//! The serialization contract (fixed by generated-document snapshots and the
//! reference documentation, docs/reference/docs-generation.md):
//!
//! - the document starts with the fixed title `Reportage Documentation`
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

const DOCUMENT_TITLE: &str = "Reportage Documentation";
const VALUE_INDENT: usize = 2;
const SOURCE_INDENT: usize = 4;

/// Renders the whole catalog as one plain text document.
pub fn render_plain(catalog: &DocumentationCatalog) -> String {
    let mut blocks: Vec<String> = vec![DOCUMENT_TITLE.to_string()];

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

/// One labeled block: the label line followed by the value's logical lines,
/// each non-empty line indented by `indent` spaces.
///
/// Empty logical lines are emitted without indentation so the document never
/// contains trailing whitespace. A trailing final newline in the value does
/// not add an extra empty line: block separation is owned by the join in
/// [`render_plain`], keeping "exactly one empty line between blocks"
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
        let output = render_plain(&single_case_catalog("case \"x\" {\n  $ true\n}\n"));

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

        let output = render_plain(&catalog);
        assert!(!output.contains("Description"));
        // A zero-case file has no Case / Reportage source blocks either.
        assert!(!output.contains("Case"));
        assert!(!output.contains("Reportage source"));
        assert!(output.ends_with("Source path\n  minimal.repor\n"));
    }

    #[test]
    fn source_empty_lines_stay_empty_without_trailing_whitespace() {
        let output = render_plain(&single_case_catalog("case \"x\" {\n\n  $ true\n}\n"));

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
        let output = render_plain(&single_case_catalog("case \"x\" {\r\n  $ true\r\n}\r\n"));

        assert!(!output.contains('\r'));
        assert!(output.contains("Reportage source\n    case \"x\" {\n      $ true\n    }\n"));
    }

    #[test]
    fn final_newline_presence_does_not_change_block_separation() {
        let with_newline = render_plain(&single_case_catalog("case \"x\" {\n  $ true\n}\n"));
        let without_newline = render_plain(&single_case_catalog("case \"x\" {\n  $ true\n}"));

        assert_eq!(with_newline, without_newline);
    }

    #[test]
    fn document_ends_with_exactly_one_lf() {
        let output = render_plain(&single_case_catalog("case \"x\" {\n  $ true\n}\n"));
        assert!(output.ends_with('\n'));
        assert!(!output.ends_with("\n\n"));
    }

    #[test]
    fn multi_line_descriptions_indent_each_logical_line() {
        let mut catalog = single_case_catalog("case \"x\" {\n  $ true\n}\n");
        catalog.groups[0].files[0].description = Some("First line.\n\nThird line.\n".to_string());

        let output = render_plain(&catalog);
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

        let output = render_plain(&catalog);
        assert!(
            output.contains("Group\n  A\n\nFile\n  a\n\nSource path\n  a.repor\n\nGroup\n  B\n")
        );
    }
}
