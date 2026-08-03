//! The compiled outcome of schema preparation.
//!
//! Instance processing applies this plan and must not re-read the schema, so everything an
//! instruction needs — including the schema location it came from, for diagnostics — is captured
//! here at preparation time.
//!
//! Compiling is also where the collected instructions are merged: at most one survives per instance
//! location, so applying the plan never has to decide which of two requests to honour.

use super::annotation::{Operation, SnapshotAnnotation};
use super::error::PreparationError;
use super::location::{InstanceLocation, SchemaLocation};

/// Every normalization instruction one schema document produces, at most one per instance location.
///
/// Instructions are kept in the order the traversal reached them, and instance processing applies
/// them in that order to one document. Distinct targets are not by themselves independent — one can
/// be an ancestor of another — but the order still cannot change whether normalization succeeds: a
/// replacement writes a scalar, so any instruction reaching through a replaced position fails its
/// next step, and an ancestor that could not be replaced was a container, which fails too. What
/// order does decide is which of several failures is the one reported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizationPlan {
    instructions: Vec<NormalizationInstruction>,
}

impl NormalizationPlan {
    /// Merges `instructions` into the plan they describe, or reports the locations they disagree
    /// about.
    ///
    /// Instructions reaching one instance location are the same request written in more than one
    /// place, so identical ones collapse to a single application and differing ones are a defect
    /// rather than a precedence question: the normalization foundation refuses to let schema member
    /// order or traversal order decide which annotation wins. See
    /// docs/adr/20260723T160117Z_json-schema-driven-snapshot-normalization-foundation.md.
    ///
    /// A document with several conflicting locations reports one of them, the first in collection
    /// order, the way the traversal stops at the first defect it reaches. That is a reporting
    /// choice and not a precedence one: which conflict is reported depends on collection order,
    /// while what any conflict says about the annotations that caused it does not.
    ///
    /// The initial traversal subset cannot in fact produce two instructions for one instance
    /// location, because `properties`, `items`, and `$ref` give each location exactly one schema
    /// path. This is where the applicator and dynamic-property keywords that can (issues #163,
    /// #164, #165) will meet a decided policy instead of one improvised to unblock them.
    pub fn compile(
        instructions: Vec<NormalizationInstruction>,
    ) -> Result<NormalizationPlan, PreparationError> {
        let mut merged: Vec<NormalizationInstruction> = Vec::new();
        for instruction in &instructions {
            let already_kept = merged
                .iter()
                .any(|kept| kept.target() == instruction.target());
            if already_kept {
                continue;
            }

            // Compared against every instruction rather than only the later ones: agreement is
            // transitive, so one disagreement anywhere in the group makes the whole group a
            // conflict, and the diagnostic has to name every annotation that asked for this
            // location rather than the pair that happened to be compared first.
            let group: Vec<&NormalizationInstruction> = instructions
                .iter()
                .filter(|other| other.target() == instruction.target())
                .collect();
            if group.iter().any(|other| !instruction.agrees_with(other)) {
                return Err(PreparationError::conflict(
                    instruction.target().clone(),
                    group.iter().map(|other| other.source().clone()).collect(),
                ));
            }
            merged.push(instruction.clone());
        }
        Ok(NormalizationPlan {
            instructions: merged,
        })
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
    pub fn new(
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
    ///
    /// Where several annotations agreed on one instance location, this is the first the traversal
    /// reached. Any of them would be as true, since a surviving instruction is exactly the request
    /// they all made.
    pub fn source(&self) -> &SchemaLocation {
        &self.source
    }

    /// Whether two instructions for the same instance location ask for the same rewrite.
    fn agrees_with(&self, other: &NormalizationInstruction) -> bool {
        self.operation == other.operation && self.value == other.value
    }
}
