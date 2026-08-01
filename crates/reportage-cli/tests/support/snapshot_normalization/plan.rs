//! The compiled outcome of schema preparation.
//!
//! Instance processing applies this plan and must not re-read the schema, so everything an
//! instruction needs — including the schema location it came from, for diagnostics — is captured
//! here at preparation time.

use super::annotation::{Operation, SnapshotAnnotation};
use super::location::{InstanceLocation, SchemaLocation};

/// Every normalization instruction one schema document produces.
///
/// Instructions are kept in traversal order and are not yet deduplicated, and instructions that
/// disagree about the same instance location are not yet rejected as conflicts; both belong to the
/// instance-processing work in issue #114, which is what gives "the same instruction twice" and
/// "two different instructions here" an observable difference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizationPlan {
    instructions: Vec<NormalizationInstruction>,
}

impl NormalizationPlan {
    pub(super) fn new(instructions: Vec<NormalizationInstruction>) -> NormalizationPlan {
        NormalizationPlan { instructions }
    }

    pub fn instructions(&self) -> &[NormalizationInstruction] {
        &self.instructions
    }
}

/// One annotation, resolved to the instance positions it applies to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizationInstruction {
    target: InstanceLocation,
    operation: Operation,
    value: String,
    source: SchemaLocation,
}

impl NormalizationInstruction {
    pub(super) fn new(
        target: InstanceLocation,
        annotation: &SnapshotAnnotation,
        source: SchemaLocation,
    ) -> NormalizationInstruction {
        NormalizationInstruction {
            target,
            operation: annotation.operation(),
            value: annotation.value().to_string(),
            source,
        }
    }

    pub fn target(&self) -> &InstanceLocation {
        &self.target
    }

    pub fn operation(&self) -> Operation {
        self.operation
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    /// The annotated schema node this instruction came from.
    ///
    /// A definition reached through several references produces one instruction per instance
    /// location while sharing this source, so a defect in the annotation is reported against the
    /// one place that has to be edited.
    pub fn source(&self) -> &SchemaLocation {
        &self.source
    }
}
