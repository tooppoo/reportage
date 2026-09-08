//! The display metadata and ordering shared by every documentation
//! projection: what a `document file` / `document case` block resolves to, and
//! how the selected sources are grouped and ordered.
//!
//! This is deliberately the only thing the Reportage-source and product-facing
//! projections share above the loading boundary. Both answer the same question
//! here — which group does this source belong to, what is it called, in what
//! order do sources appear — because both read the same `document` blocks; they
//! diverge entirely on what a *case* becomes. Keeping the answer in one place
//! is what stops the two projections from drifting into two ordering contracts
//! for the same metadata. See
//! docs/adr/20260907T230710Z_reportage-source-documentation-subcommand.md for
//! the split, and
//! docs/adr/20260908T131134Z_product-documentation-projection.md for what the
//! two projections do and do not share.
//!
//! Like the catalogs built on top of it, this module exposes only plain values:
//! `SourceFile` / `SourceCase` go in, `String` / `Option<String>` / `u64` come
//! out, so no renderer can grow a dependency on parser types.

use crate::source::SourceCase;

use super::loader::LoadedSourceFile;

/// The default group for files whose source declares no `document file` group.
///
/// This value is a user-facing output contract, fixed by Catalog tests and
/// generated-document snapshots.
pub const DEFAULT_GROUP: &str = "Index";

/// One file's resolved display metadata, with every fallback already applied.
///
/// The source-level model states only what the source states; the fallbacks
/// (file stem as title, [`DEFAULT_GROUP`]) are materialized here and only here,
/// so every projection inherits identical behavior.
#[derive(Debug, PartialEq, Eq)]
pub struct FileMetadata {
    pub title: String,
    pub group: String,
    pub description: Option<String>,
    /// The declared `document file.order`, left unresolved: ordering has to
    /// distinguish "declared" from "not declared", which no fallback value can
    /// express.
    pub order: Option<u64>,
    /// The normalized display path, not a filesystem access path.
    pub source_path: String,
}

/// One case's resolved display metadata, with the case-name title fallback
/// applied.
#[derive(Debug, PartialEq, Eq)]
pub struct CaseMetadata {
    pub title: String,
    pub description: Option<String>,
}

/// A group of per-file payloads, in the order they must be presented.
///
/// Generic over the payload so each projection keeps its own file type:
/// grouping and ordering are metadata concerns, and nothing here needs to know
/// what a documented file contains.
#[derive(Debug, PartialEq, Eq)]
pub struct DocumentGroup<F> {
    pub name: String,
    pub files: Vec<F>,
}

/// Resolves a loaded source's `document file` metadata for display.
pub fn file_metadata(loaded: &LoadedSourceFile) -> FileMetadata {
    let documentation = loaded.source.file_documentation();

    FileMetadata {
        title: documentation
            .and_then(|d| d.title.clone())
            .unwrap_or_else(|| file_stem(&loaded.display_path).to_string()),
        group: documentation
            .and_then(|d| d.group.clone())
            .unwrap_or_else(|| DEFAULT_GROUP.to_string()),
        description: documentation
            .and_then(|d| d.description.as_ref())
            .map(|text| text.as_str().to_string()),
        order: documentation.and_then(|d| d.order),
        source_path: loaded.display_path.clone(),
    }
}

/// Resolves a case's `document case` metadata for display.
pub fn case_metadata(source_case: &SourceCase) -> CaseMetadata {
    let documentation = source_case.documentation();

    CaseMetadata {
        title: documentation
            .and_then(|d| d.title.clone())
            .unwrap_or_else(|| source_case.case().name.clone()),
        description: documentation
            .and_then(|d| d.description.as_ref())
            .map(|text| text.as_str().to_string()),
    }
}

/// Builds each source's payload with `build_file`, then applies the
/// deterministic grouping and ordering contract.
///
/// Ordering: groups by ascending name; within a group, files with a declared
/// `document file.order` before files without one, then ascending order value,
/// then ascending `source_path`. All string comparisons are locale-independent,
/// case-sensitive `String` ordering.
pub fn group_files<F>(
    sources: &[LoadedSourceFile],
    mut build_file: impl FnMut(&LoadedSourceFile, &FileMetadata) -> F,
) -> Vec<DocumentGroup<F>> {
    struct FileEntry<F> {
        order: Option<u64>,
        source_path: String,
        file: F,
    }

    let mut groups: std::collections::BTreeMap<String, Vec<FileEntry<F>>> =
        std::collections::BTreeMap::new();

    for loaded in sources {
        let metadata = file_metadata(loaded);
        let file = build_file(loaded, &metadata);
        groups.entry(metadata.group).or_default().push(FileEntry {
            order: metadata.order,
            source_path: metadata.source_path,
            file,
        });
    }

    groups
        .into_iter()
        .map(|(name, mut entries)| {
            entries.sort_by(|a, b| {
                (a.order.is_none(), a.order, &a.source_path).cmp(&(
                    b.order.is_none(),
                    b.order,
                    &b.source_path,
                ))
            });
            DocumentGroup {
                name,
                files: entries.into_iter().map(|entry| entry.file).collect(),
            }
        })
        .collect()
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
    fn undocumented_sources_fall_back_to_the_stem_and_default_group() {
        let metadata = file_metadata(&loaded("dir/sample-file.repor", UNDOCUMENTED));

        assert_eq!(
            metadata,
            FileMetadata {
                title: "sample-file".to_string(),
                group: DEFAULT_GROUP.to_string(),
                description: None,
                order: None,
                source_path: "dir/sample-file.repor".to_string(),
            }
        );
    }

    #[test]
    fn declared_document_file_metadata_is_used_verbatim() {
        let source = "document file {\n  title \"File assertions\"\n  group \"Filesystem\"\n  order 3\n  description \"About files.\"\n}\n";
        let metadata = file_metadata(&loaded("a.repor", source));

        assert_eq!(
            metadata,
            FileMetadata {
                title: "File assertions".to_string(),
                group: "Filesystem".to_string(),
                description: Some("About files.".to_string()),
                order: Some(3),
                source_path: "a.repor".to_string(),
            }
        );
    }

    #[test]
    fn case_metadata_falls_back_to_the_case_name() {
        let file = loaded("a.repor", UNDOCUMENTED);

        assert_eq!(
            case_metadata(&file.source.cases()[0]),
            CaseMetadata {
                title: "first case".to_string(),
                description: None,
            }
        );
    }

    #[test]
    fn declared_document_case_metadata_is_used_verbatim() {
        let source = "document case {\n  title \"File creation\"\n  description \"Creates a file.\"\n}\n\ncase \"file exists\" {\n  $ true\n  assert {\n    exit 0\n  }\n}\n";
        let file = loaded("a.repor", source);

        assert_eq!(
            case_metadata(&file.source.cases()[0]),
            CaseMetadata {
                title: "File creation".to_string(),
                description: Some("Creates a file.".to_string()),
            }
        );
    }

    fn with_order(group: &str, order: Option<u64>) -> String {
        let order_field = order.map(|o| format!("  order {o}\n")).unwrap_or_default();
        format!(
            "document file {{\n  group \"{group}\"\n{order_field}}}\n\ncase \"c\" {{\n  $ true\n  assert {{\n    exit 0\n  }}\n}}\n"
        )
    }

    #[test]
    fn files_with_declared_order_come_before_undeclared_and_ties_break_on_path() {
        let groups = group_files(
            &[
                loaded("a-unordered.repor", &with_order("G", None)),
                loaded("z-first.repor", &with_order("G", Some(1))),
                loaded("m-second.repor", &with_order("G", Some(2))),
                loaded("b-second-too.repor", &with_order("G", Some(2))),
            ],
            |_, metadata| metadata.source_path.clone(),
        );

        assert_eq!(
            groups[0].files,
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
        let groups = group_files(
            &[
                loaded("a.repor", &with_order("advanced", None)),
                loaded("b.repor", &with_order("Guides", None)),
                loaded("c.repor", UNDOCUMENTED),
            ],
            |_, _| (),
        );

        let names: Vec<_> = groups.iter().map(|g| g.name.as_str()).collect();
        // Uppercase before lowercase: byte-wise String ordering, no locale.
        assert_eq!(names, vec!["Guides", "Index", "advanced"]);
    }
}
