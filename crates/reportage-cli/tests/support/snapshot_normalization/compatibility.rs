//! Compatibility rules applied to each schema node normalization traversal reaches.
//!
//! These reject forms the normalizer cannot interpret without guessing. They are not statements
//! about JSON Schema validity: a rejected document may be a perfectly valid schema that this
//! profile refuses to normalize against. Document validity is checked separately (issue #192).
//!
//! Each rule is one function in [`RULES`], so adding support for a form later is removing its rule
//! and adding a collector for it, without touching the others. That removability is a requirement
//! of the normalization foundation, not an incidental shape:
//! docs/adr/20260723T160117Z_json-schema-driven-snapshot-normalization-foundation.md.
//!
//! Rules only ever see nodes traversal actually reached. A subtree the normalizer does not enter —
//! an unsupported applicator keyword, or a `$defs` entry no supported reference reaches — is never
//! checked, because refusing to normalize a document over a form the normalizer never has to
//! interpret would make normalization support a constraint on unrelated parts of the contract.

use serde_json::{Map, Value};

use super::error::{PreparationError, PreparationErrorKind};
use super::location::SchemaLocation;
use super::reference::REFERENCE_KEYWORD;

const IDENTIFIER_KEYWORD: &str = "$id";
const DYNAMIC_KEYWORDS: [&str; 2] = ["$dynamicRef", "$dynamicAnchor"];
const PREFIX_ITEMS_KEYWORD: &str = "prefixItems";

/// A schema object normalization traversal has reached.
pub struct ReachedNode<'a> {
    members: &'a Map<String, Value>,
    location: &'a SchemaLocation,
}

impl<'a> ReachedNode<'a> {
    pub fn new(members: &'a Map<String, Value>, location: &'a SchemaLocation) -> ReachedNode<'a> {
        ReachedNode { members, location }
    }
}

type Rule = fn(&ReachedNode<'_>) -> Result<(), PreparationError>;

/// The rules, in the order they run.
///
/// `reference_sibling` runs first because every later rule reads the node's other members as
/// schema keywords, and in a reference object those members are exactly what is unsupported: a
/// `$ref` sibling has no agreed meaning here, so reporting it as a nested `$id`, a dynamic
/// reference, or a tuple array would name the wrong defect. `nested_identifier` runs before
/// `dynamic_reference` because `$id` changes the base URI every other reference-shaped keyword
/// below it resolves against, so it invalidates the frame the dynamic keywords would be
/// interpreted in. `tuple_items` is independent of the other two and runs last.
const RULES: [Rule; 4] = [
    reference_sibling,
    nested_identifier,
    dynamic_reference,
    tuple_items,
];

pub fn check(node: &ReachedNode<'_>) -> Result<(), PreparationError> {
    RULES.iter().try_for_each(|rule| rule(node))
}

/// A reference schema object must hold `$ref` and nothing else.
///
/// Sibling members are rejected rather than ignored or merged. In Draft 2020-12 siblings are
/// evaluated alongside the referenced schema, so ignoring them would silently drop constraints and
/// annotations, and merging them would require the applicator semantics this profile does not
/// implement. Annotations therefore belong in the referenced schema, never beside the `$ref`.
///
/// The document root may carry `$id`, but not while it is also a reference object: `$id` is a
/// sibling like any other here.
fn reference_sibling(node: &ReachedNode<'_>) -> Result<(), PreparationError> {
    let Some(reference) = node.members.get(REFERENCE_KEYWORD) else {
        return Ok(());
    };
    let siblings: Vec<&str> = node
        .members
        .keys()
        .filter(|member| member.as_str() != REFERENCE_KEYWORD)
        .map(String::as_str)
        .collect();
    if siblings.is_empty() {
        return Ok(());
    }
    Err(PreparationError::new(
        PreparationErrorKind::ReferenceSibling,
        node.location.child(REFERENCE_KEYWORD),
        format!(
            "a reference schema object must hold `$ref` alone, but it also holds {}",
            siblings.join(", "),
        ),
    )
    .with_value(reference))
}

/// Only the document root may carry `$id`.
///
/// A nested `$id` starts a new resource whose base URI every reference below it resolves against.
/// This profile resolves references against the document root only, so it would resolve such a
/// subtree's references against the wrong base and quietly reach the wrong schema.
fn nested_identifier(node: &ReachedNode<'_>) -> Result<(), PreparationError> {
    if node.location.is_document_root() {
        return Ok(());
    }
    let Some(identifier) = node.members.get(IDENTIFIER_KEYWORD) else {
        return Ok(());
    };
    Err(PreparationError::new(
        PreparationErrorKind::NestedIdentifier,
        node.location.child(IDENTIFIER_KEYWORD),
        "only the document root may carry `$id`, because references are resolved against the document root",
    )
    .with_value(identifier))
}

/// `$dynamicRef` and `$dynamicAnchor` are rejected where traversal reaches them.
///
/// Their target depends on the dynamic scope of an evaluation, which schema preparation does not
/// have: it walks the schema once, not once per instance. There is no static answer to record in a
/// normalization plan.
fn dynamic_reference(node: &ReachedNode<'_>) -> Result<(), PreparationError> {
    for keyword in DYNAMIC_KEYWORDS {
        let Some(value) = node.members.get(keyword) else {
            continue;
        };
        return Err(PreparationError::new(
            PreparationErrorKind::DynamicReference,
            node.location.child(keyword),
            format!(
                "`{keyword}` resolves against a dynamic scope that schema preparation does not have"
            ),
        )
        .with_value(value));
    }
    Ok(())
}

/// `prefixItems` is rejected where traversal reaches it.
///
/// The traversal treats `items` as describing every element of the array. With `prefixItems`
/// present, Draft 2020-12 applies `items` only to elements after the tuple prefix, so an annotation
/// below `items` would be collected for positions it does not describe. Required by
/// docs/adr/20260723T160117Z_json-schema-driven-snapshot-normalization-foundation.md, which also
/// asks that supporting tuples later be this rule's removal plus a dedicated collector.
fn tuple_items(node: &ReachedNode<'_>) -> Result<(), PreparationError> {
    let Some(prefix_items) = node.members.get(PREFIX_ITEMS_KEYWORD) else {
        return Ok(());
    };
    Err(PreparationError::new(
        PreparationErrorKind::TupleItems,
        node.location.child(PREFIX_ITEMS_KEYWORD),
        "`prefixItems` makes `items` apply only after the tuple prefix, so this array is not homogeneous",
    )
    .with_value(prefix_items))
}
