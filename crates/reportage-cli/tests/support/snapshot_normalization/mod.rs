//! Schema preparation for JSON snapshot normalization (issues #162, #193).
//!
//! Snapshot normalization stabilizes intentionally volatile values — tool versions, artifact roots
//! — before a JSON document is compared with its snapshot. The policy lives in the schema, as
//! `x-reportage-snapshot` annotations beside the fields they apply to, and this module compiles
//! those annotations into a normalization plan: instructions naming the instance positions to
//! rewrite. Applying a plan to a document, and moving the existing snapshot suites onto it, is
//! issue #114.
//!
//! This is a harness-internal facility. It is not a general JSON Schema implementation, it never
//! processes user-supplied schemas, and nothing about `reportage run` depends on it.
//!
//! Preparation is separate from instance processing so a schema defect is found once, against the
//! schema, instead of being rediscovered per fixture and reported against whichever document
//! happened to reach it. The decisions this implements are recorded in
//! docs/adr/20260723T160117Z_json-schema-driven-snapshot-normalization-foundation.md and
//! docs/adr/20260729T182026Z_static-local-reference-resolution-for-snapshot-normalization.md.
//!
//! Responsibilities are split so each part can be changed without the others:
//!
//! - [`reference`] maps a `$ref` to the schema it denotes, and knows nothing else;
//! - [`compatibility`] holds one independent rule per unsupported form;
//! - [`collector`] walks the schema, tracking instance location and active expansions; and
//! - [`plan`] is what preparation hands to instance processing.

// A support module is compiled into each test target that includes it, and every target uses only
// the part of it that target is about. Unused items and unused re-exports are therefore the normal
// state here, not a sign that something is left over.
#![allow(dead_code, unused_imports)]

mod annotation;
mod collector;
mod compatibility;
mod error;
mod location;
mod plan;
mod reference;

pub use annotation::{ANNOTATION_KEYWORD, Operation, SnapshotAnnotation};
pub use collector::prepare;
pub use error::{PreparationError, PreparationErrorKind, ReferenceCycle, ReferenceStep};
pub use location::{InstanceLocation, InstanceSegment, SchemaLocation};
pub use plan::{NormalizationInstruction, NormalizationPlan};
pub use reference::{DEFINITIONS_KEYWORD, REFERENCE_KEYWORD, ResolvedReference, resolve};
