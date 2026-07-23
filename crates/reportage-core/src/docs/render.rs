//! The uniform document renderer interface every format implements.
//!
//! A format owns exactly two things — serialization and the file extension —
//! so the interface exposes exactly those. Layouts stay format-agnostic by
//! programming against this trait: a single-file layout passes the whole
//! Catalog, and a future multi-file layout can partition the Catalog into
//! sub-catalogs and pass each one to the same `render`, so adding a format
//! never requires touching layout code (and vice versa). See
//! docs/adr/20260723T070556Z_documentation-generation-command.md.
//!
//! All implementations live in this crate; the trait may grow methods (e.g.
//! index/TOC rendering for multi-file layouts) when a consumer for them
//! exists, rather than speculating on their shape now.

use super::catalog::DocumentationCatalog;

/// One document format: serializes a [`DocumentationCatalog`] and names the
/// file extension of the produced document(s).
///
/// `render` must be a pure function of the catalog: renderers own
/// presentation (indentation, line ending normalization, wrappers) but must
/// never drop or replace case source content beyond that — the exact
/// contract is documented per implementation.
pub trait DocumentRenderer {
    /// Serializes `catalog` into one document body.
    fn render(&self, catalog: &DocumentationCatalog) -> String;

    /// The file extension for documents this format produces, without the
    /// leading dot (e.g. `"txt"`).
    fn file_extension(&self) -> &'static str;
}
