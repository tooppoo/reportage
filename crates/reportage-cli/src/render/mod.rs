//! Output rendering.
//!
//! A renderer turns a format-agnostic `ExecutionReport` into one concrete output
//! format. The CLI layer owns choosing which renderer runs for a given invocation;
//! the runner (`evaluator`, `executor`) never depends on this module and knows
//! nothing about output formats.
//!
//! Today there are two renderers, [`human::HumanRenderer`] and [`json::JsonRenderer`].
//! Future formats (`ndjson`, `junit`, `tap`, github annotations, ...) are added as new
//! `OutputRenderer` implementations here, without changing how the report is produced.

pub mod human;
pub mod json;

use reportage_core::result::ExecutionReport;

/// Produces one concrete output format from an `ExecutionReport`.
pub trait OutputRenderer {
    fn render(&self, report: &ExecutionReport);
}
