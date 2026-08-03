//! The `x-reportage-snapshot` annotation and its typed parse.
//!
//! The annotation is the carrier of snapshot normalization policy: it sits beside the field
//! definition it stabilizes, so that adding a volatile field means annotating it once instead of
//! repeating an instance path in every fixture. Its shape is fixed by
//! docs/adr/20260723T160117Z_json-schema-driven-snapshot-normalization-foundation.md.
//!
//! Parsing is strict — an unknown member is a failure rather than something ignored — because JSON
//! Schema ignores unknown keywords, so a misspelled annotation member would otherwise be accepted
//! by the schema, ignored here, and leave a volatile value in the snapshot with nothing reporting
//! it.

use serde_json::{Map, Value};

use super::error::{PreparationError, PreparationErrorKind};
use super::location::SchemaLocation;

pub const ANNOTATION_KEYWORD: &str = "x-reportage-snapshot";

const OPERATION_MEMBER: &str = "operation";
const VALUE_MEMBER: &str = "value";
const REPLACE_OPERATION: &str = "replace";

/// What normalization does to the annotated instance values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    /// Substitute a fixed placeholder for the observed value.
    Replace,
}

impl Operation {
    pub fn keyword(self) -> &'static str {
        match self {
            Operation::Replace => REPLACE_OPERATION,
        }
    }
}

/// A parsed `x-reportage-snapshot` annotation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotAnnotation {
    operation: Operation,
    value: String,
}

impl SnapshotAnnotation {
    /// The annotation a schema node carrying `operation` and `value` would parse to.
    ///
    /// Schema preparation only ever obtains one through [`parse`]. This exists so that instruction
    /// merge can be exercised over instructions the initial traversal subset cannot produce, which
    /// is the only way to state its policy before the keywords that reach one land (issues #163,
    /// #164, #165).
    pub fn new(operation: Operation, value: impl Into<String>) -> SnapshotAnnotation {
        SnapshotAnnotation {
            operation,
            value: value.into(),
        }
    }

    pub fn operation(&self) -> Operation {
        self.operation
    }

    /// The placeholder written in place of the observed value.
    pub fn value(&self) -> &str {
        &self.value
    }
}

/// Reads the annotation of the schema node whose members are `members`, if it carries one.
///
/// `location` is the schema location of the node, not of the annotation.
pub fn parse(
    members: &Map<String, Value>,
    location: &SchemaLocation,
) -> Result<Option<SnapshotAnnotation>, PreparationError> {
    let Some(annotation) = members.get(ANNOTATION_KEYWORD) else {
        return Ok(None);
    };
    let annotation_location = location.child(ANNOTATION_KEYWORD);
    let invalid = |at: &SchemaLocation, detail: &str, value: &Value| {
        PreparationError::new(
            PreparationErrorKind::InvalidAnnotation,
            at.clone(),
            detail.to_string(),
        )
        .with_value(value)
    };

    let Some(members) = annotation.as_object() else {
        return Err(invalid(
            &annotation_location,
            "`x-reportage-snapshot` must be an object",
            annotation,
        ));
    };
    for member in members.keys() {
        if member != OPERATION_MEMBER && member != VALUE_MEMBER {
            return Err(invalid(
                &annotation_location.child(member),
                "`x-reportage-snapshot` accepts only `operation` and `value`",
                annotation,
            ));
        }
    }

    let operation_location = annotation_location.child(OPERATION_MEMBER);
    let Some(operation) = members.get(OPERATION_MEMBER) else {
        return Err(invalid(
            &annotation_location,
            "`x-reportage-snapshot` requires `operation`",
            annotation,
        ));
    };
    let operation = match operation.as_str() {
        Some(REPLACE_OPERATION) => Operation::Replace,
        Some(_) | None => {
            return Err(invalid(
                &operation_location,
                "`operation` must be the string `replace`, the only operation this profile defines",
                operation,
            ));
        }
    };

    let value_location = annotation_location.child(VALUE_MEMBER);
    let Some(value) = members.get(VALUE_MEMBER) else {
        return Err(invalid(
            &annotation_location,
            "`x-reportage-snapshot` requires `value`",
            annotation,
        ));
    };
    let Some(value) = value.as_str() else {
        return Err(invalid(&value_location, "`value` must be a string", value));
    };

    Ok(Some(SnapshotAnnotation {
        operation,
        value: value.to_string(),
    }))
}
