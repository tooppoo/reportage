//! JSON snapshot normalization (issues #162, #193, #114).
//!
//! Snapshot normalization stabilizes intentionally volatile values — tool versions, artifact roots
//! — before a JSON document is compared with its snapshot. The policy lives in the schema, as
//! `x-reportage-snapshot` annotations beside the fields they apply to. Schema preparation compiles
//! those annotations into a normalization plan — instructions naming the instance positions to
//! rewrite — and instance processing applies one plan to each document. Moving the existing
//! snapshot suites onto it is the rest of issue #114.
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
//! - [`collector`] walks the schema, tracking instance location and active expansions;
//! - [`plan`] merges what the walk collected, and is what preparation hands to instance processing;
//!   and
//! - [`application`] walks a document with a plan, and is the only part that sees an instance.

// A support module is compiled into each test target that includes it, and every target uses only
// the part of it that target is about. Unused items and unused re-exports are therefore the normal
// state here, not a sign that something is left over.
#![allow(dead_code, unused_imports)]

mod annotation;
mod application;
mod collector;
mod compatibility;
mod error;
mod location;
mod plan;
mod reference;

pub use annotation::{ANNOTATION_KEYWORD, Operation, SnapshotAnnotation};
pub use application::{ApplicationError, ApplicationErrorKind, apply};
pub use collector::prepare;
pub use error::{
    InstructionConflict, PreparationError, PreparationErrorKind, ReferenceCycle, ReferenceStep,
};
pub use location::{
    InstanceLocation, InstancePointer, InstanceSegment, InstanceToken, SchemaLocation,
};
pub use plan::{NormalizationInstruction, NormalizationPlan};
pub use reference::{DEFINITIONS_KEYWORD, REFERENCE_KEYWORD, ResolvedReference, resolve};
