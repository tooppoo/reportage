//! Normalization traversal: walks the schema once and collects the instructions it reaches.
//!
//! The collector owns everything the resolver deliberately does not know — where in the instance
//! the current schema node applies, which references are currently being expanded, and what has
//! been collected so far.
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

pub struct Collector<'a> {
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
    pub fn new(root: &'a Value) -> Collector<'a> {
        Collector {
            root,
            instructions: Vec::new(),
            active: Vec::new(),
        }
    }

    pub fn collect(mut self) -> Result<NormalizationPlan, PreparationError> {
        let root = self.root;
        self.visit(root, SchemaLocation::root(), InstanceLocation::root())?;
        Ok(NormalizationPlan::new(self.instructions))
    }

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
