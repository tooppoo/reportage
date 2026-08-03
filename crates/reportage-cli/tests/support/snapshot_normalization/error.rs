//! Schema preparation failures and their classification.
//!
//! Every failure here is a defect in the schema document or its normalization metadata, detected
//! before any instance is seen. Failures that depend on a concrete instance belong to instance
//! processing and are not modelled by this type; see
//! docs/adr/20260723T160117Z_json-schema-driven-snapshot-normalization-foundation.md.
//!
//! The classification is part of what a caller may assert on, so that a test can state which defect
//! it expects rather than matching on rendered message text.

use std::fmt;

use serde_json::Value;

use super::location::{InstanceLocation, SchemaLocation};

/// What kind of schema defect was found.
///
/// Each variant is a distinct repair: they are separate so that a diagnostic never has to say only
/// "this reference is bad" when the schema author needs to know whether the target is missing, the
/// spelling is outside the supported profile, or the document simply has no `$defs` object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreparationErrorKind {
    /// `$ref` is present but its value is not a string.
    NonStringReference,
    /// The `$ref` string is well-formed JSON but outside the supported reference profile.
    UnsupportedReferenceForm,
    /// The document root has a `$defs` member that is not an object, so no definition can be
    /// looked up in it.
    InvalidReferenceContainer,
    /// The reference is supported but names a definition the document does not contain.
    UnresolvedReference,
    /// The reference resolved to a value that is not a schema.
    InvalidResolvedTarget,
    /// A schema node other than the document root carries `$id`.
    NestedIdentifier,
    /// A reached schema node carries `$dynamicRef` or `$dynamicAnchor`.
    DynamicReference,
    /// A reference schema object carries members besides `$ref`.
    ReferenceSibling,
    /// A reached schema node describes a tuple-prefixed array, which `items` alone cannot.
    TupleItems,
    /// Following the reference would re-enter a definition that is already being expanded.
    ReferenceCycle,
    /// An `x-reportage-snapshot` annotation does not match the annotation contract.
    InvalidAnnotation,
    /// Instructions reaching the same instance positions disagree about what to write there.
    ConflictingInstructions,
}

impl PreparationErrorKind {
    /// Every classification, so a test can show that each one is reachable rather than that the
    /// ones it happened to think of are.
    ///
    /// Declaration order, checked as such: a new variant must be added to the enum and appended
    /// here at the same position, or the inventory would silently stop being complete.
    pub const ALL: [PreparationErrorKind; 12] = [
        PreparationErrorKind::NonStringReference,
        PreparationErrorKind::UnsupportedReferenceForm,
        PreparationErrorKind::InvalidReferenceContainer,
        PreparationErrorKind::UnresolvedReference,
        PreparationErrorKind::InvalidResolvedTarget,
        PreparationErrorKind::NestedIdentifier,
        PreparationErrorKind::DynamicReference,
        PreparationErrorKind::ReferenceSibling,
        PreparationErrorKind::TupleItems,
        PreparationErrorKind::ReferenceCycle,
        PreparationErrorKind::InvalidAnnotation,
        PreparationErrorKind::ConflictingInstructions,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PreparationErrorKind::NonStringReference => "non-string reference",
            PreparationErrorKind::UnsupportedReferenceForm => "unsupported reference form",
            PreparationErrorKind::InvalidReferenceContainer => "invalid reference container",
            PreparationErrorKind::UnresolvedReference => "unresolved reference",
            PreparationErrorKind::InvalidResolvedTarget => "invalid resolved target",
            PreparationErrorKind::NestedIdentifier => "nested $id",
            PreparationErrorKind::DynamicReference => "dynamic reference",
            PreparationErrorKind::ReferenceSibling => "$ref sibling",
            PreparationErrorKind::TupleItems => "tuple items",
            PreparationErrorKind::ReferenceCycle => "reference cycle",
            PreparationErrorKind::InvalidAnnotation => "invalid annotation",
            PreparationErrorKind::ConflictingInstructions => "conflicting instructions",
        }
    }
}

impl fmt::Display for PreparationErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

/// One `$ref` expansion that was in progress when a failure was detected.
///
/// Both halves are kept because a chain of target locations alone cannot be walked back to the
/// schema text: the same definition is normally referenced from several places, so only the `$ref`
/// keyword location identifies which of them the expansion came through.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceStep {
    reference: SchemaLocation,
    target: SchemaLocation,
}

impl ReferenceStep {
    pub(super) fn new(reference: SchemaLocation, target: SchemaLocation) -> ReferenceStep {
        ReferenceStep { reference, target }
    }

    /// The location of the `$ref` keyword that was expanded.
    pub fn reference(&self) -> &SchemaLocation {
        &self.reference
    }

    /// The location of the schema the `$ref` resolved to.
    pub fn target(&self) -> &SchemaLocation {
        &self.target
    }
}

/// What a reference cycle diagnostic needs beyond the classification and location.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReferenceCycle {
    chain: Vec<ReferenceStep>,
    start: SchemaLocation,
}

impl ReferenceCycle {
    /// The expansions that were active when the cycle closed, outermost first.
    pub fn chain(&self) -> &[ReferenceStep] {
        &self.chain
    }

    /// The already-active target the closing reference pointed back at, which is where the cycle
    /// begins within [`ReferenceCycle::chain`].
    pub fn start(&self) -> &SchemaLocation {
        &self.start
    }
}

/// What an instruction conflict diagnostic needs beyond the classification and location.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstructionConflict {
    target: InstanceLocation,
    sources: Vec<SchemaLocation>,
}

impl InstructionConflict {
    /// The instance positions every conflicting instruction applies to.
    pub fn target(&self) -> &InstanceLocation {
        &self.target
    }

    /// Every annotated schema node that reached [`InstructionConflict::target`], once each and in
    /// pointer order.
    ///
    /// All of them are kept, and none is singled out as the offender, because the repair is a
    /// choice between annotations that are each defensible alone: naming one would send the reader
    /// to a schema location that is not, by itself, wrong. Both the order and the deduplication are
    /// about what the reader has to act on: this is the set of places the conflict can be resolved,
    /// so one schema node reached twice is one edit site, and which of them the collection order met
    /// first is not part of the answer.
    pub fn sources(&self) -> &[SchemaLocation] {
        &self.sources
    }
}

/// A schema defect found during schema preparation.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparationError {
    kind: PreparationErrorKind,
    location: SchemaLocation,
    value: Option<Value>,
    detail: String,
    /// The two details that belong to one classification each. Both are boxed because every
    /// `Result` in schema preparation is as large as this error, which the other ten
    /// classifications would otherwise pay for.
    cycle: Option<Box<ReferenceCycle>>,
    conflict: Option<Box<InstructionConflict>>,
}

impl PreparationError {
    pub(super) fn new(
        kind: PreparationErrorKind,
        location: SchemaLocation,
        detail: impl Into<String>,
    ) -> PreparationError {
        PreparationError {
            kind,
            location,
            value: None,
            detail: detail.into(),
            cycle: None,
            conflict: None,
        }
    }

    /// Attaches the offending literal, cloned so that a diagnostic can outlive the schema borrow.
    pub(super) fn with_value(mut self, value: &Value) -> PreparationError {
        self.value = Some(value.clone());
        self
    }

    pub(super) fn cycle(
        closing_reference: SchemaLocation,
        chain: Vec<ReferenceStep>,
        start: SchemaLocation,
    ) -> PreparationError {
        PreparationError {
            kind: PreparationErrorKind::ReferenceCycle,
            location: closing_reference,
            value: None,
            detail: String::from(
                "following this reference would re-enter a definition that is already being expanded",
            ),
            cycle: Some(Box::new(ReferenceCycle { chain, start })),
            conflict: None,
        }
    }

    /// Reports that the instructions reaching `target` do not all ask for the same rewrite.
    ///
    /// `sources` are put in order and deduplicated here rather than by the caller, so that the
    /// guarantee [`InstructionConflict::sources`] states is kept by the one place that can keep it,
    /// and so that the location this error reports — which has to be a single node — is a function
    /// of the contributing annotations rather than of the order they were collected in.
    pub(super) fn conflict(
        target: InstanceLocation,
        mut sources: Vec<SchemaLocation>,
    ) -> PreparationError {
        sources.sort_by_key(SchemaLocation::as_pointer);
        sources.dedup();
        let first = sources
            .first()
            .cloned()
            .expect("a conflict is between instructions, each of which has a source");
        PreparationError {
            kind: PreparationErrorKind::ConflictingInstructions,
            location: first,
            value: None,
            detail: String::from(
                "annotations reaching the same instance positions must agree on the operation and the value",
            ),
            cycle: None,
            conflict: Some(Box::new(InstructionConflict { target, sources })),
        }
    }

    pub fn kind(&self) -> PreparationErrorKind {
        self.kind
    }

    /// The schema location of the offending keyword. For a cycle this is the `$ref` that closed it.
    /// For a conflict, where no contributing annotation is the offender on its own, it is the one
    /// with the smallest pointer, so that the reported location does not depend on collection
    /// order; [`PreparationError::conflict_detail`] carries the rest.
    pub fn location(&self) -> &SchemaLocation {
        &self.location
    }

    /// The offending literal, when the classification has one.
    pub fn value(&self) -> Option<&Value> {
        self.value.as_ref()
    }

    /// The cycle detail, present exactly for [`PreparationErrorKind::ReferenceCycle`].
    pub fn cycle_detail(&self) -> Option<&ReferenceCycle> {
        self.cycle.as_deref()
    }

    /// The conflict detail, present exactly for
    /// [`PreparationErrorKind::ConflictingInstructions`].
    pub fn conflict_detail(&self) -> Option<&InstructionConflict> {
        self.conflict.as_deref()
    }
}

impl fmt::Display for PreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {} (at {})",
            self.kind, self.detail, self.location
        )?;
        if let Some(value) = &self.value {
            write!(formatter, "\n  value: {value}")?;
        }
        if let Some(cycle) = &self.cycle {
            write!(formatter, "\n  cycle starts at: {}", cycle.start)?;
            for step in &cycle.chain {
                write!(
                    formatter,
                    "\n  expanded: {} -> {}",
                    step.reference(),
                    step.target()
                )?;
            }
        }
        if let Some(conflict) = &self.conflict {
            write!(formatter, "\n  normalizing: {}", conflict.target())?;
            for source in conflict.sources() {
                write!(formatter, "\n  requested by: {source}")?;
            }
        }
        Ok(())
    }
}
