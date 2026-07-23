//! The Documentation Catalog: the renderer-ready intermediate model between
//! the parser's source-level model and the documentation renderers.
//!
//! The Catalog API deliberately exposes only plain `String` values — never
//! `SourceFile` / `SourceCase` / `SourceSpan` / `DocumentationText` — so a
//! renderer can be written and tested without depending on parser types.
//! All display fallbacks (file stem as title, the `Index` default group, the
//! case name as case title) are applied here and only here; the source-level
//! model never materializes them. See
//! docs/adr/20260723T070556Z_documentation-generation-command.md.

use super::loader::LoadedSourceFile;

/// The default group for files whose source declares no `document file` group.
///
/// This value is a user-facing output contract, fixed by Catalog tests and
/// generated-document snapshots.
pub const DEFAULT_GROUP: &str = "Index";

#[derive(Debug, PartialEq, Eq)]
pub struct DocumentationCatalog {
    pub groups: Vec<DocumentationGroup>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct DocumentationGroup {
    pub name: String,
    pub files: Vec<DocumentedFile>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct DocumentedFile {
    pub title: String,
    pub description: Option<String>,
    /// The normalized display path, not a filesystem access path: renderers
    /// print it verbatim.
    pub source_path: String,
    pub cases: Vec<DocumentedCase>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct DocumentedCase {
    pub title: String,
    pub description: Option<String>,
    /// The exact case block text sliced from the source span: indentation,
    /// interior whitespace and comments, line endings (LF or CRLF), and the
    /// presence or absence of a final newline are preserved unchanged.
    /// Presentation transformations (indentation, LF normalization) belong to
    /// renderers, never to the Catalog.
    pub source: String,
}

/// Builds the Catalog from loaded sources, applying display fallbacks and the
/// deterministic ordering contract.
///
/// Ordering: groups by ascending name; within a group, files with a declared
/// `document file` order before files without one, then ascending order value,
/// then ascending `source_path`; cases stay in source order. All string
/// comparisons are locale-independent, case-sensitive `String` ordering.
pub fn build_catalog(sources: &[LoadedSourceFile]) -> DocumentationCatalog {
    struct FileEntry {
        order: Option<u64>,
        file: DocumentedFile,
    }

    let mut groups: std::collections::BTreeMap<String, Vec<FileEntry>> =
        std::collections::BTreeMap::new();

    for loaded in sources {
        let documentation = loaded.source.file_documentation();

        let group = documentation
            .and_then(|d| d.group.clone())
            .unwrap_or_else(|| DEFAULT_GROUP.to_string());
        let title = documentation
            .and_then(|d| d.title.clone())
            .unwrap_or_else(|| file_stem(&loaded.display_path).to_string());
        let description = documentation
            .and_then(|d| d.description.as_ref())
            .map(|text| text.as_str().to_string());
        let order = documentation.and_then(|d| d.order);

        let cases = loaded
            .source
            .cases()
            .iter()
            .map(|source_case| {
                let case_documentation = source_case.documentation();
                DocumentedCase {
                    title: case_documentation
                        .and_then(|d| d.title.clone())
                        .unwrap_or_else(|| source_case.case().name.clone()),
                    description: case_documentation
                        .and_then(|d| d.description.as_ref())
                        .map(|text| text.as_str().to_string()),
                    source: loaded.source.case_source(source_case).to_string(),
                }
            })
            .collect();

        groups.entry(group).or_default().push(FileEntry {
            order,
            file: DocumentedFile {
                title,
                description,
                source_path: loaded.display_path.clone(),
                cases,
            },
        });
    }

    DocumentationCatalog {
        groups: groups
            .into_iter()
            .map(|(name, mut entries)| {
                entries.sort_by(|a, b| {
                    (a.order.is_none(), a.order, &a.file.source_path).cmp(&(
                        b.order.is_none(),
                        b.order,
                        &b.file.source_path,
                    ))
                });
                DocumentationGroup {
                    name,
                    files: entries.into_iter().map(|entry| entry.file).collect(),
                }
            })
            .collect(),
    }
}

/// The last display path segment without its `.repor` extension, used as the
/// file title fallback.
fn file_stem(display_path: &str) -> &str {
    let name = display_path.rsplit('/').next().unwrap_or(display_path);
    name.strip_suffix(".repor").unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;
    use std::path::PathBuf;

    fn loaded(display_path: &str, source: &str) -> LoadedSourceFile {
        LoadedSourceFile {
            load_path: PathBuf::from(display_path),
            display_path: display_path.to_string(),
            source: parser::parse(source).expect("test source must parse"),
        }
    }

    const UNDOCUMENTED: &str = "case \"first case\" {\n  $ true\n  assert {\n    exit 0\n  }\n}\n";

    #[test]
    fn fallbacks_apply_for_undocumented_sources() {
        let catalog = build_catalog(&[loaded("dir/sample-file.repor", UNDOCUMENTED)]);

        assert_eq!(catalog.groups.len(), 1);
        assert_eq!(catalog.groups[0].name, DEFAULT_GROUP);
        let file = &catalog.groups[0].files[0];
        assert_eq!(file.title, "sample-file");
        assert_eq!(file.description, None);
        assert_eq!(file.source_path, "dir/sample-file.repor");
        assert_eq!(file.cases[0].title, "first case");
        assert_eq!(file.cases[0].description, None);
    }

    #[test]
    fn documentation_metadata_reaches_the_catalog_as_plain_strings() {
        let source = "document file {\n  title \"File assertions\"\n  group \"Filesystem\"\n  description \"About files.\"\n}\n\ndocument case {\n  title \"File creation\"\n  description \"Creates a file.\"\n}\n\ncase \"file exists\" {\n  $ true\n  assert {\n    exit 0\n  }\n}\n";
        let catalog = build_catalog(&[loaded("a.repor", source)]);

        assert_eq!(catalog.groups[0].name, "Filesystem");
        let file = &catalog.groups[0].files[0];
        assert_eq!(file.title, "File assertions");
        assert_eq!(file.description.as_deref(), Some("About files."));
        assert_eq!(file.cases[0].title, "File creation");
        assert_eq!(
            file.cases[0].description.as_deref(),
            Some("Creates a file.")
        );
    }

    #[test]
    fn case_source_is_preserved_exactly() {
        // CRLF line endings, interior blank line, an inline comment on the
        // closing brace line, and no final newline after the case block.
        let source =
            "case \"crlf\" {\r\n  $ true\r\n\r\n  assert {\r\n    exit 0\r\n  }\r\n} # done";
        let catalog = build_catalog(&[loaded("crlf.repor", source)]);

        let case = &catalog.groups[0].files[0].cases[0];
        assert_eq!(case.source, source);
    }

    #[test]
    fn zero_case_sources_are_included() {
        let catalog = build_catalog(&[loaded("empty.repor", "")]);

        let file = &catalog.groups[0].files[0];
        assert_eq!(file.title, "empty");
        assert_eq!(file.source_path, "empty.repor");
        assert!(file.cases.is_empty());
    }

    #[test]
    fn cases_stay_in_source_order() {
        let source = "case \"b\" {\n  $ true\n  assert {\n    exit 0\n  }\n}\n\ncase \"a\" {\n  $ true\n  assert {\n    exit 0\n  }\n}\n";
        let catalog = build_catalog(&[loaded("x.repor", source)]);

        let titles: Vec<_> = catalog.groups[0].files[0]
            .cases
            .iter()
            .map(|c| c.title.as_str())
            .collect();
        assert_eq!(titles, vec!["b", "a"]);
    }

    fn with_order(group: &str, order: Option<u64>) -> String {
        let order_field = order.map(|o| format!("  order {o}\n")).unwrap_or_default();
        format!(
            "document file {{\n  group \"{group}\"\n{order_field}}}\n\ncase \"c\" {{\n  $ true\n  assert {{\n    exit 0\n  }}\n}}\n"
        )
    }

    #[test]
    fn files_with_declared_order_come_before_undeclared_and_ties_break_on_path() {
        let catalog = build_catalog(&[
            loaded("a-unordered.repor", &with_order("G", None)),
            loaded("z-first.repor", &with_order("G", Some(1))),
            loaded("m-second.repor", &with_order("G", Some(2))),
            loaded("b-second-too.repor", &with_order("G", Some(2))),
        ]);

        let paths: Vec<_> = catalog.groups[0]
            .files
            .iter()
            .map(|f| f.source_path.as_str())
            .collect();
        assert_eq!(
            paths,
            vec![
                "z-first.repor",
                "b-second-too.repor",
                "m-second.repor",
                "a-unordered.repor"
            ]
        );
    }

    #[test]
    fn groups_sort_case_sensitively_and_locale_independently() {
        let catalog = build_catalog(&[
            loaded("a.repor", &with_order("advanced", None)),
            loaded("b.repor", &with_order("Guides", None)),
            loaded("c.repor", UNDOCUMENTED),
        ]);

        let names: Vec<_> = catalog.groups.iter().map(|g| g.name.as_str()).collect();
        // Uppercase before lowercase: byte-wise String ordering, no locale.
        assert_eq!(names, vec!["Guides", "Index", "advanced"]);
    }
}
