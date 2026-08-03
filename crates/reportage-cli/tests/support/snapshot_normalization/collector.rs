//! Normalization traversal: walks the schema once and collects the instructions it reaches.
//!
//! [`prepare`] is this module's whole interface, and the entry point of schema preparation. The
//! traversal owns everything the resolver deliberately does not know — where in the instance the
//! current schema node applies, which references are currently being expanded, and what has been
//! collected so far.
//!
//! The traversal subset is the root schema, object `properties`, homogeneous array `items`, and
//! supported static local `$ref`. Everything else is left alone; values under an unsupported
//! keyword keep their observed value in the snapshot rather than being normalized, per
//! docs/adr/20260723T160117Z_json-schema-driven-snapshot-normalization-foundation.md.

use serde_json::Value;

use super::annotation;
use super::compatibility::{self, ReachedNode};
use super::error::{PreparationError, ReferenceStep};
use super::location::{InstanceLocation, SchemaLocation};
use super::plan::{NormalizationInstruction, NormalizationPlan};
use super::reference::{self, REFERENCE_KEYWORD};

const PROPERTIES_KEYWORD: &str = "properties";
const ITEMS_KEYWORD: &str = "items";

/// Compiles `schema` into the normalization plan its annotations describe.
///
/// Fails on the first defect found in the part of the schema normalization traversal reaches, or —
/// once the walk has seen every annotation, which is what makes the question answerable — on
/// instructions that disagree about an instance location.
pub fn prepare(schema: &Value) -> Result<NormalizationPlan, PreparationError> {
    let mut collector = Collector {
        root: schema,
        instructions: Vec::new(),
        active: Vec::new(),
    };
    collector.visit(schema, SchemaLocation::root(), InstanceLocation::root())?;
    NormalizationPlan::compile(collector.instructions)
}

/// The state one run of the traversal carries.
///
/// Not an object with a lifecycle: [`prepare`] is the only way to run a traversal, and it builds
/// this, walks once, and drops it. It is a struct because `visit` recurses and every level needs
/// the same document, the collected instructions, and the expansion stack; the alternative is
/// threading three parameters through every recursive call.
struct Collector<'a> {
    root: &'a Value,
    instructions: Vec<NormalizationInstruction>,
    /// The references currently being expanded, outermost first.
    ///
    /// This is a stack rather than a set of everything ever resolved: a definition is a cycle only
    /// when reaching it again would re-enter an expansion that has not finished. Reaching the same
    /// definition from two properties, or twice in sequence down one chain, is ordinary reuse and
    /// must keep working.
    active: Vec<ReferenceStep>,
}

impl<'a> Collector<'a> {
    /// Visits one schema node, which describes the instance positions `instance` selects.
    fn visit(
        &mut self,
        node: &'a Value,
        schema: SchemaLocation,
        instance: InstanceLocation,
    ) -> Result<(), PreparationError> {
        // A boolean schema is terminal: it carries neither annotations nor subschemas, so the
        // instance positions it describes are simply preserved. Whether any instance can satisfy
        // `false` is a validation question, not a normalization one.
        //
        // Anything else that is not an object is not a schema at all. Traversal skips it instead of
        // reporting it, because normalization is not the document's validity check (issue #192) and
        // this position was only reached by descending into a keyword, never by resolving a
        // reference — a reference target is type-checked by the resolver.
        let Some(members) = node.as_object() else {
            return Ok(());
        };

        compatibility::check(&ReachedNode::new(members, &schema))?;

        if let Some(reference) = members.get(REFERENCE_KEYWORD) {
            return self.expand(reference, schema, instance);
        }

        if let Some(found) = annotation::parse(members, &schema)? {
            self.instructions.push(NormalizationInstruction::new(
                instance.clone(),
                &found,
                schema.clone(),
            ));
        }

        if let Some(properties) = members.get(PROPERTIES_KEYWORD).and_then(Value::as_object) {
            let properties_location = schema.child(PROPERTIES_KEYWORD);
            for (name, subschema) in properties {
                self.visit(
                    subschema,
                    properties_location.child(name),
                    instance.property(name),
                )?;
            }
        }

        if let Some(items) = members.get(ITEMS_KEYWORD) {
            self.visit(items, schema.child(ITEMS_KEYWORD), instance.array_element())?;
        }

        Ok(())
    }

    /// Continues the traversal through a reference.
    ///
    /// The instance location is carried across unchanged: a reference says nothing about the
    /// instance, so the referring schema and the referenced schema describe the same positions.
    /// Descending into the target's `properties` or `items` is what extends it, which is why the
    /// same definition reached from two places yields two instructions with different targets.
    fn expand(
        &mut self,
        reference: &Value,
        schema: SchemaLocation,
        instance: InstanceLocation,
    ) -> Result<(), PreparationError> {
        let reference_location = schema.child(REFERENCE_KEYWORD);
        let resolved = reference::resolve(self.root, reference, &reference_location)?;

        if self
            .active
            .iter()
            .any(|step| step.target() == resolved.location())
        {
            return Err(PreparationError::cycle(
                reference_location,
                self.active.clone(),
                resolved.location().clone(),
            ));
        }

        self.active.push(ReferenceStep::new(
            reference_location,
            resolved.location().clone(),
        ));
        let result = self.visit(resolved.target(), resolved.location().clone(), instance);
        self.active.pop();
        result
    }
}
