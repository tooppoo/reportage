//! The uniform layout interface: how a Catalog is distributed across output
//! files.
//!
//! A layout decides how many documents are generated and their relative
//! structure under the output root; the format (a [`DocumentRenderer`])
//! decides serialization and the file extension. Keeping the two behind
//! independent interfaces lets any layout compose with any format. See
//! docs/adr/20260723T070556Z_documentation-generation-command.md.

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

/// One document layout: maps a [`DocumentationCatalog`] to the set of
/// documents to write, delegating serialization to the given renderer.
pub trait DocumentLayoutPlan {
    /// Renders the catalog into the documents this layout prescribes,
    /// forwarding the document-level render options to every `render` call.
    fn plan(
        &self,
        catalog: &DocumentationCatalog,
        renderer: &dyn DocumentRenderer,
        options: &RenderOptions,
    ) -> Vec<PlannedDocument>;
}

/// The `single-file` layout: the whole Catalog becomes exactly one document
/// at the fixed relative path `index.<extension>`.
pub struct SingleFileLayout;

impl DocumentLayoutPlan for SingleFileLayout {
    fn plan(
        &self,
        catalog: &DocumentationCatalog,
        renderer: &dyn DocumentRenderer,
        options: &RenderOptions,
    ) -> Vec<PlannedDocument> {
        vec![PlannedDocument {
            relative_path: format!("index.{}", renderer.file_extension()),
            contents: renderer.render(catalog, options),
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
    fn single_file_layout_produces_one_index_document() {
        let catalog = DocumentationCatalog { groups: vec![] };

        let planned = SingleFileLayout.plan(&catalog, &FakeRenderer, &RenderOptions::default());
        assert_eq!(
            planned,
            vec![PlannedDocument {
                relative_path: "index.fake".to_string(),
                contents: "Reportage Documentation: 0 groups\n".to_string(),
            }]
        );
    }
}
