//! The static local `$ref` resolver.
//!
//! This is a pure component: it maps a `$ref` value to the schema it denotes and knows nothing
//! about instance locations, annotations, normalization instructions, or which references are
//! currently being expanded. Cycle detection therefore lives in the collector, which is the only
//! part that knows the expansion state, and the resolver holds no cache — the same reference must
//! resolve identically no matter which instance location reached it.
//!
//! The supported profile is deliberately far narrower than JSON Schema reference resolution: a
//! literal `#/$defs/<token>` naming one direct member of the document root's `$defs`. See
//! docs/adr/20260729T182026Z_static-local-reference-resolution-for-snapshot-normalization.md.

use serde_json::Value;

use super::error::{PreparationError, PreparationErrorKind};
use super::location::{SchemaLocation, decode_pointer_token};

pub const REFERENCE_KEYWORD: &str = "$ref";
pub const DEFINITIONS_KEYWORD: &str = "$defs";

/// The only reference spelling the initial profile accepts.
const SUPPORTED_PREFIX: &str = "#/$defs/";

/// What a `$ref` denotes: the schema itself and where it sits in the document.
///
/// The location travels with the target because the collector needs it for cycle identity and for
/// the schema locations in diagnostics raised below the reference, neither of which can be
/// recovered from the target value alone.
pub struct ResolvedReference<'a> {
    location: SchemaLocation,
    target: &'a Value,
}

impl<'a> ResolvedReference<'a> {
    pub fn location(&self) -> &SchemaLocation {
        &self.location
    }

    pub fn target(&self) -> &'a Value {
        self.target
    }
}

/// Resolves `reference` against `root`, or reports why it is outside the supported profile.
///
/// `location` is the schema location of the `$ref` keyword itself, used only to place diagnostics.
///
/// The checks run in a fixed order, and each one narrows what the following check may assume:
///
/// 1. the value is a string;
/// 2. it starts with the literal `#/$defs/`;
/// 3. the remainder holds no further `/`, so it is a single reference token;
/// 4. the remainder holds no `%`;
/// 5. every `~` in it starts a valid escape;
/// 6. the token is decoded;
/// 7. the document root has an object `$defs`;
/// 8. the decoded token names a member of it; and
/// 9. that member is an object or boolean schema.
///
/// Step 2 is a literal prefix test rather than URI parsing on purpose. A general URI parser would
/// accept spellings whose resolution depends on `$id` rebasing, percent-decoding, and base-URI
/// rules that this profile does not implement, so the reference form must be recognisable by
/// inspection. Step 9 is what keeps a reference from targeting an arbitrary JSON object — a
/// `properties` map or an annotation object — that is not a schema at all.
pub fn resolve<'a>(
    root: &'a Value,
    reference: &Value,
    location: &SchemaLocation,
) -> Result<ResolvedReference<'a>, PreparationError> {
    let Some(text) = reference.as_str() else {
        return Err(PreparationError::new(
            PreparationErrorKind::NonStringReference,
            location.clone(),
            "`$ref` must be a string",
        )
        .with_value(reference));
    };

    let unsupported = |detail: &str| {
        Err(PreparationError::new(
            PreparationErrorKind::UnsupportedReferenceForm,
            location.clone(),
            detail,
        )
        .with_value(reference))
    };

    let Some(raw_token) = text.strip_prefix(SUPPORTED_PREFIX) else {
        return unsupported(
            "only same-document references spelled `#/$defs/<definition>` are supported",
        );
    };
    if raw_token.contains('/') {
        return unsupported(
            "only a direct member of the document root's `$defs` can be referenced, so the reference must end after one pointer token",
        );
    }
    if raw_token.contains('%') {
        return unsupported("percent-encoded references are not decoded and are not supported");
    }
    let Some(definition) = decode_pointer_token(raw_token) else {
        return unsupported("`~` in a JSON Pointer token must start either `~0` or `~1`");
    };

    let Some(definitions) = root.get(DEFINITIONS_KEYWORD) else {
        return Err(PreparationError::new(
            PreparationErrorKind::UnresolvedReference,
            location.clone(),
            "the document root has no `$defs`",
        )
        .with_value(reference));
    };
    let Some(definitions) = definitions.as_object() else {
        return Err(PreparationError::new(
            PreparationErrorKind::InvalidReferenceContainer,
            location.clone(),
            "the document root's `$defs` is not an object, so it holds no definitions",
        )
        .with_value(reference));
    };
    let Some(target) = definitions.get(&definition) else {
        return Err(PreparationError::new(
            PreparationErrorKind::UnresolvedReference,
            location.clone(),
            format!("the document root's `$defs` has no `{definition}` member"),
        )
        .with_value(reference));
    };
    if !(target.is_object() || target.is_boolean()) {
        return Err(PreparationError::new(
            PreparationErrorKind::InvalidResolvedTarget,
            location.clone(),
            "a reference must resolve to a schema, which is an object or a boolean",
        )
        .with_value(reference));
    }

    Ok(ResolvedReference {
        location: SchemaLocation::root()
            .child(DEFINITIONS_KEYWORD)
            .child(definition),
        target,
    })
}
