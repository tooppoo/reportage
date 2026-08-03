//! Instance processing: applying a compiled normalization plan to one document (issues #162, #114).
//!
//! This is the second of the two phases the normalization foundation separates. It reads the plan
//! and the document and never the schema, so a schema defect cannot be discovered here — it was
//! already found once, against the schema, by preparation. What is left is what only a concrete
//! document can answer: whether the positions the plan names are there, and whether what is there
//! can be replaced.
//!
//! The document must have passed its contract validation first (issue #192). Normalization rewrites
//! volatile values to make a document comparable; it is not a repair step, and nothing here treats
//! a document that failed its contract as something to fix. See
//! docs/adr/20260723T160117Z_json-schema-driven-snapshot-normalization-foundation.md.

use std::fmt;

use serde_json::Value;

use super::location::{InstanceLocation, InstancePointer, InstanceSegment, SchemaLocation};
use super::plan::{NormalizationInstruction, NormalizationPlan};

/// Normalizes `document` with `plan`.
///
/// `plan` is borrowed and unchanged, which is what lets one prepared plan serve every fixture of a
/// suite: preparation happens per schema, not per document.
///
/// A missing property is not a failure. An annotation says what a value must become if the document
/// has it, and whether the document must have it is the schema's own `required`, checked by
/// contract validation before normalization runs. A second requiredness rule here would state the
/// same thing in a place nothing keeps current.
///
/// Fails on the first instruction that cannot be applied, so a document with several such positions
/// reports one of them, and which one depends on plan order. Whether it fails at all does not: see
/// [`NormalizationPlan`] for why applying instructions in place to one document is nonetheless
/// order-insensitive in outcome.
pub fn apply(plan: &NormalizationPlan, document: Value) -> Result<Value, ApplicationError> {
    let mut document = document;
    for instruction in plan.instructions() {
        descend(
            &mut document,
            instruction.target().segments(),
            InstancePointer::root(),
            instruction,
        )?;
    }
    Ok(document)
}

/// Walks the remaining `segments` of `instruction` from `value`, which sits at `at`.
fn descend(
    value: &mut Value,
    segments: &[InstanceSegment],
    at: InstancePointer,
    instruction: &NormalizationInstruction,
) -> Result<(), ApplicationError> {
    let Some((segment, rest)) = segments.split_first() else {
        return replace(value, at, instruction);
    };

    match segment {
        InstanceSegment::Property(name) => {
            let Some(members) = value.as_object_mut() else {
                return Err(ApplicationError::new(
                    ApplicationErrorKind::NonObjectContainer,
                    at,
                    instruction,
                    format!(
                        "expected an object to read `{name}` from, found {}",
                        kind(value)
                    ),
                ));
            };
            // The one shape difference that is not a defect: the plan describes what an optional
            // property must become, and this document does not have it. Whether it has to is the
            // schema's `required` to govern, not normalization's.
            //
            // A container present as `null` is not this case but the non-object rejection guarding
            // this branch, because instance processing cannot read `type` and so cannot tell a
            // contract-legal `null` container from an illegal one. Skipping it instead would leave
            // a volatile value in the snapshot with nothing saying why.
            let Some(member) = members.get_mut(name) else {
                return Ok(());
            };
            descend(member, rest, at.property(name), instruction)
        }
        InstanceSegment::ArrayElement => {
            let Some(elements) = value.as_array_mut() else {
                return Err(ApplicationError::new(
                    ApplicationErrorKind::NonArrayContainer,
                    at,
                    instruction,
                    format!(
                        "expected an array to apply to every element of, found {}",
                        kind(value)
                    ),
                ));
            };
            // An empty array is every element of it, so there is nothing to do and nothing wrong.
            for (index, element) in elements.iter_mut().enumerate() {
                descend(element, rest, at.index(index), instruction)?;
            }
            Ok(())
        }
    }
}

/// Writes the instruction's replacement value at a reached target.
///
/// Only a scalar may be replaced. A placeholder string in place of an object or an array would
/// erase a whole contract or evidence structure from the snapshot — the shape snapshots exist to
/// protect — so it is reported rather than written.
fn replace(
    value: &mut Value,
    at: InstancePointer,
    instruction: &NormalizationInstruction,
) -> Result<(), ApplicationError> {
    if value.is_object() || value.is_array() {
        return Err(ApplicationError::new(
            ApplicationErrorKind::ContainerTarget,
            at,
            instruction,
            format!(
                "only a scalar may be replaced, and this position holds {}",
                kind(value)
            ),
        ));
    }
    // Written as the literal string the annotation carries: no interpolation, no token resolution,
    // no escape expansion. `<VERSION>` and the like are placeholders by convention only, and
    // nothing here reads them as anything.
    *value = Value::String(instruction.value().to_string());
    Ok(())
}

fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// What kind of instance failure was found.
///
/// None of these is a schema defect: a plan that reaches a position of the wrong shape describes a
/// document that this document is not, which is a question about the document rather than about the
/// annotation that named the position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationErrorKind {
    /// A property step reached a position that is not an object.
    NonObjectContainer,
    /// An every-element step reached a position that is not an array.
    NonArrayContainer,
    /// The position to replace holds an object or an array.
    ContainerTarget,
}

impl ApplicationErrorKind {
    /// Every classification, so a test can show that each one is reachable rather than that the
    /// ones it happened to think of are.
    ///
    /// Declaration order, checked as such: a new variant must be added to the enum and appended
    /// here at the same position. Mirrors [`super::error::PreparationErrorKind::ALL`].
    pub const ALL: [ApplicationErrorKind; 3] = [
        ApplicationErrorKind::NonObjectContainer,
        ApplicationErrorKind::NonArrayContainer,
        ApplicationErrorKind::ContainerTarget,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ApplicationErrorKind::NonObjectContainer => "non-object container",
            ApplicationErrorKind::NonArrayContainer => "non-array container",
            ApplicationErrorKind::ContainerTarget => "container target",
        }
    }
}

impl fmt::Display for ApplicationErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

/// A failure to apply a normalization plan to one document.
///
/// Both locations are carried because either can be the thing to fix: the instance pointer says
/// which position of which document disagreed with the plan, and the source schema location says
/// which annotation asked for it. With only one of them, a reader cannot tell whether the document
/// or the annotation is what is wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationError {
    kind: ApplicationErrorKind,
    instance: InstancePointer,
    target: InstanceLocation,
    source: SchemaLocation,
    detail: String,
}

impl ApplicationError {
    fn new(
        kind: ApplicationErrorKind,
        instance: InstancePointer,
        instruction: &NormalizationInstruction,
        detail: impl Into<String>,
    ) -> ApplicationError {
        ApplicationError {
            kind,
            instance,
            target: instruction.target().clone(),
            source: instruction.source().clone(),
            detail: detail.into(),
        }
    }

    pub fn kind(&self) -> ApplicationErrorKind {
        self.kind
    }

    /// Where in the document the instruction stopped.
    pub fn instance(&self) -> &InstancePointer {
        &self.instance
    }

    /// The positions the instruction was for, which is not where it stopped: an instruction that
    /// reached one bad element of an array was not wrong about the array.
    pub fn target(&self) -> &InstanceLocation {
        &self.target
    }

    /// The annotated schema node the instruction came from.
    pub fn source(&self) -> &SchemaLocation {
        &self.source
    }
}

impl fmt::Display for ApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {} (at {})\n  normalizing: {}\n  requested by: {}",
            self.kind, self.detail, self.instance, self.target, self.source
        )
    }
}
