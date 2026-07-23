//! The uniform layout interface: how a Catalog is distributed across output
//! files.
//!
//! A layout decides how many documents are generated and their relative
//! structure under the output root; the format (a [`DocumentRenderer`])
//! decides serialization and the file extension. Keeping the two behind
//! independent interfaces lets any layout compose with any format. See
//! docs/adr/20260723T070556Z_documentation-generation-command.md.

use std::path::{Component, Path};

use super::catalog::DocumentationCatalog;
use super::render::{DocumentRenderer, RenderOptions};

/// A rendered document and its output path relative to the output root.
///
/// The relative path must contain only normal components;
/// [`super::output::OutputDirectory::write_document`] enforces that contract.
#[derive(Debug, PartialEq, Eq)]
pub struct PlannedDocument {
    pub relative_path: String,
    pub contents: String,
}

/// The default base name of the index document, used when `--index-file-name`
/// is not given. The renderer's file extension is appended to it (for example
/// `index.txt`), so an omitted name keeps the extension tracking `--format`.
pub const DEFAULT_INDEX_BASE_NAME: &str = "index";

/// Layout-level options from the invocation: values that decide how a layout
/// names its output files, as opposed to [`RenderOptions`], which decide a
/// single document's body.
///
/// A separate struct from [`RenderOptions`] on purpose: file naming is a
/// layout concern and serialization is a format concern. A future multi-file
/// layout resolves the same index/entry name from here without touching any
/// renderer. See
/// docs/adr/20260723T070556Z_documentation-generation-command.md.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LayoutOptions {
    /// The index document's name. `None` selects [`DEFAULT_INDEX_BASE_NAME`]
    /// joined with the renderer's extension; `Some` is used verbatim,
    /// extension included. A `Some` value must pass
    /// [`validate_index_file_name`] before planning.
    pub index_file_name: Option<String>,
}

/// A rejected `--index-file-name` value: it is not a single in-root file name
/// (it is empty, holds a path separator, or holds a `.`/`..`/root component),
/// so it could name a file outside the output root or inside a missing
/// subdirectory.
#[derive(Debug, PartialEq, Eq)]
pub struct IndexFileNameError {
    pub value: String,
}

impl std::fmt::Display for IndexFileNameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The message states the rule rather than naming one offending
        // character: the rejected shapes include values with no path separator
        // at all (an empty value, `.`, `..`), so a separator-specific message
        // would misattribute the cause for those.
        write!(
            f,
            "index file name '{}' must be a single file name directly under the output directory, without a path separator, '.'/'..', or absolute path",
            self.value
        )
    }
}

impl std::error::Error for IndexFileNameError {}

/// Accepts an `--index-file-name` value only when it is exactly one normal
/// path component, so the resolved output path always stays directly inside
/// the output root.
///
/// The value is otherwise used verbatim (extension included, per the
/// `--index-file-name` contract), so this rejects only the containment- and
/// existence-breaking shapes — an empty value, `.`/`..`, an absolute path, or
/// any value carrying a `/` separator — never the choice of extension.
pub fn validate_index_file_name(name: &str) -> Result<(), IndexFileNameError> {
    let is_single_normal_component = matches!(
        Path::new(name).components().collect::<Vec<_>>().as_slice(),
        [Component::Normal(_)]
    );
    if is_single_normal_component {
        Ok(())
    } else {
        Err(IndexFileNameError {
            value: name.to_string(),
        })
    }
}

/// One document layout: maps a [`DocumentationCatalog`] to the set of
/// documents to write, delegating serialization to the given renderer.
pub trait DocumentLayoutPlan {
    /// Renders the catalog into the documents this layout prescribes,
    /// forwarding the document-level render options to every `render` call and
    /// consulting `layout_options` for file naming.
    fn plan(
        &self,
        catalog: &DocumentationCatalog,
        renderer: &dyn DocumentRenderer,
        render_options: &RenderOptions,
        layout_options: &LayoutOptions,
    ) -> Vec<PlannedDocument>;
}

/// The `single-file` layout: the whole Catalog becomes exactly one document.
/// Its name is [`LayoutOptions::index_file_name`] when given, otherwise
/// [`DEFAULT_INDEX_BASE_NAME`] joined with the renderer's extension (for
/// example `index.txt`).
pub struct SingleFileLayout;

impl DocumentLayoutPlan for SingleFileLayout {
    fn plan(
        &self,
        catalog: &DocumentationCatalog,
        renderer: &dyn DocumentRenderer,
        render_options: &RenderOptions,
        layout_options: &LayoutOptions,
    ) -> Vec<PlannedDocument> {
        let relative_path = match &layout_options.index_file_name {
            Some(name) => name.clone(),
            None => format!("{DEFAULT_INDEX_BASE_NAME}.{}", renderer.file_extension()),
        };
        vec![PlannedDocument {
            relative_path,
            contents: renderer.render(catalog, render_options),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal fake format: the layout contract (one file, `index.<ext>`,
    /// body delegated to the renderer) must hold for any format, not just
    /// `plain`.
    struct FakeRenderer;

    impl DocumentRenderer for FakeRenderer {
        fn render(&self, catalog: &DocumentationCatalog, options: &RenderOptions) -> String {
            format!(
                "{}: {} groups\n",
                options.document_title,
                catalog.groups.len()
            )
        }

        fn file_extension(&self) -> &'static str {
            "fake"
        }
    }

    #[test]
    fn single_file_layout_defaults_to_index_with_the_renderer_extension() {
        let catalog = DocumentationCatalog { groups: vec![] };

        let planned = SingleFileLayout.plan(
            &catalog,
            &FakeRenderer,
            &RenderOptions::default(),
            &LayoutOptions::default(),
        );
        assert_eq!(
            planned,
            vec![PlannedDocument {
                relative_path: "index.fake".to_string(),
                contents: "Reportage Documentation: 0 groups\n".to_string(),
            }]
        );
    }

    #[test]
    fn a_given_index_file_name_replaces_the_default_verbatim() {
        let catalog = DocumentationCatalog { groups: vec![] };

        // The name is used exactly as given, extension included: it need not
        // match the renderer's extension (`fake` here).
        let planned = SingleFileLayout.plan(
            &catalog,
            &FakeRenderer,
            &RenderOptions::default(),
            &LayoutOptions {
                index_file_name: Some("readme.markdown".to_string()),
            },
        );
        assert_eq!(planned[0].relative_path, "readme.markdown");
    }

    #[test]
    fn index_file_name_validation_accepts_a_bare_name_and_rejects_paths() {
        // A single file name — with or without an extension — is accepted.
        for accepted in ["index", "readme.md", "docs.html"] {
            assert!(validate_index_file_name(accepted).is_ok(), "{accepted:?}");
        }
        // Anything that could leave the root or name a missing subdirectory is
        // rejected, and the offending value is echoed for the diagnostic.
        for rejected in ["", ".", "..", "a/b.md", "/abs.md", "sub/index"] {
            let err = validate_index_file_name(rejected).unwrap_err();
            assert_eq!(err.value, rejected);
        }
    }
}
