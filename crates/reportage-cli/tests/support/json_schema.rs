//! JSON Schema contract validation for the two JSON contracts Reportage publishes.
//!
//! The schemas under `spec/` are the authoritative specification of those contracts; this module
//! is how a test checks a document against one. It answers only the question JSON Schema answers
//! — does this instance satisfy the published constraints — and deliberately not the two adjacent
//! questions that have their own tests: whether this repository's typed Rust consumers can
//! deserialize the document, and whether the document satisfies domain invariants the schema does
//! not express. See docs/adr/20260728T092956Z_json-contract-validation-policy.md.
//!
//! Every test target that validates a contract document goes through here rather than compiling a
//! schema itself, so that draft selection, external-resource policy, and validator reuse are
//! decided in one place.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use jsonschema::{Retrieve, Uri, ValidationError, Validator};
use serde_json::{Value, json};

/// Which of a contract's two schema artifacts a document is being checked against.
///
/// Both are validated everywhere a producer document is checked. The public schema is what
/// external consumers read, so it is the one a conformance claim is really about; the internal
/// source schema is the file maintainers edit, so validating it too keeps a defect visible in the
/// artifact that has to be fixed. `just schema-artifacts-check` separately guarantees the two
/// differ only by stripped metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaVariant {
    /// The metadata-bearing `schema.internal.json` maintainers edit.
    InternalSource,
    /// The generated `schema.json` external consumers reference.
    Public,
}

impl SchemaVariant {
    pub const ALL: [SchemaVariant; 2] = [SchemaVariant::InternalSource, SchemaVariant::Public];

    pub fn label(self) -> &'static str {
        match self {
            SchemaVariant::InternalSource => "internal source schema",
            SchemaVariant::Public => "generated public schema",
        }
    }
}

/// Repository root, resolved from this package's compile-time manifest directory so a test
/// addresses the same schema files regardless of the working directory it runs in.
pub fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/reportage-cli sits two levels below the repository root")
        .to_path_buf()
}

/// Rejects every external reference.
///
/// The published schemas use fragment-only local `$ref` exclusively, so nothing legitimate ever
/// reaches this retriever. Installing it makes that a checked property of the harness rather than
/// a consequence of which `jsonschema` cargo features happen to be enabled: contract validation
/// must never depend on the network or on files outside the schema document, and an external
/// reference introduced by mistake must fail loudly instead of resolving on a developer machine.
struct NoExternalResources;

impl Retrieve for NoExternalResources {
    fn retrieve(
        &self,
        uri: &Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Err(format!(
            "external schema reference `{uri}` is not resolvable: Reportage contract schemas must use fragment-only local `$ref` and are validated without network or filesystem resource resolution"
        )
        .into())
    }
}

/// Compiles a schema document as Draft 2020-12 with external resolution disabled.
///
/// The draft is stated rather than detected. The schemas do declare `$schema`, but a contract
/// check must not silently change evaluation semantics if that declaration is edited or if the
/// validator's detection heuristics change between versions.
///
/// `format` is left as an annotation, which is the Draft 2020-12 default. No published schema
/// currently uses `format` as a constraint, and turning it into an assertion would impose contract
/// requirements the schema does not visibly state.
pub fn compile(schema: &Value) -> Result<Validator, String> {
    jsonschema::draft202012::options()
        .with_retriever(NoExternalResources)
        .build(schema)
        .map_err(|error| error.to_string())
}

/// One JSON contract: its two schema artifacts and the validators built from them.
///
/// A validator is compiled once per artifact and reused for every instance, because compilation is
/// the expensive half of validation and these suites check many fixture documents against the same
/// four schemas.
pub struct Contract {
    name: &'static str,
    artifacts: [SchemaArtifact; 2],
}

struct SchemaArtifact {
    variant: SchemaVariant,
    /// Repository-relative path, named in failure messages so a diagnostic identifies the file to
    /// edit.
    path: &'static str,
    document: Value,
    validator: Validator,
}

/// The `reportage run --format=json` stdout document contract.
pub static JSON_REPORT: LazyLock<Contract> = LazyLock::new(|| {
    Contract::load(
        "json-report",
        "spec/output/json-report/schema.internal.json",
        "spec/output/json-report/schema.json",
    )
});

/// The artifact `result.json` canonical manifest contract.
pub static RUN_RESULT: LazyLock<Contract> = LazyLock::new(|| {
    Contract::load(
        "run-result",
        "spec/artifacts/run-result/schema.internal.json",
        "spec/artifacts/run-result/schema.json",
    )
});

impl Contract {
    fn load(
        name: &'static str,
        internal_path: &'static str,
        public_path: &'static str,
    ) -> Contract {
        Contract {
            name,
            artifacts: [
                SchemaArtifact::load(SchemaVariant::InternalSource, internal_path),
                SchemaArtifact::load(SchemaVariant::Public, public_path),
            ],
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn path(&self, variant: SchemaVariant) -> &'static str {
        self.artifact(variant).path
    }

    /// The parsed schema document, for tests that inspect the schema itself.
    pub fn document(&self, variant: SchemaVariant) -> &Value {
        &self.artifact(variant).document
    }

    fn artifact(&self, variant: SchemaVariant) -> &SchemaArtifact {
        self.artifacts
            .iter()
            .find(|artifact| artifact.variant == variant)
            .expect("every contract carries both schema variants")
    }

    /// Whether `instance` satisfies one of the contract's schemas.
    pub fn is_valid(&self, variant: SchemaVariant, instance: &Value) -> bool {
        self.artifact(variant).validator.is_valid(instance)
    }

    /// Every violation of one of the contract's schemas, as rendered failure-message lines.
    ///
    /// All errors are collected rather than only the first, so one failing run shows the whole
    /// extent of a producer/schema divergence instead of one violation per fix-and-rerun cycle.
    /// The order `iter_errors` yields is not part of the validator's contract, so lines are sorted
    /// to keep a failure message stable across runs.
    pub fn violations(&self, variant: SchemaVariant, instance: &Value) -> Vec<String> {
        let artifact = self.artifact(variant);
        let mut violations: Vec<String> = artifact
            .validator
            .iter_errors(instance)
            .map(|error| {
                let mut rendered = format!(
                    "{error}\n     instance path:   {}\n     schema path:     {}\n     evaluation path: {}",
                    render_location(&error.instance_path().to_string()),
                    render_location(&error.schema_path().to_string()),
                    render_location(&error.evaluation_path().to_string()),
                );
                for line in artifact.discriminated_branch_detail(&error) {
                    rendered.push_str("\n     ");
                    rendered.push_str(&line);
                }
                rendered
            })
            .collect();
        violations.sort();
        violations
    }

    /// Asserts `instance` satisfies both of the contract's schemas.
    ///
    /// `document` names the instance being checked — a fixture path, or the artifact file it was
    /// read from — so a failure identifies which producer output diverged, not only which schema
    /// rejected it.
    pub fn assert_conforms(&self, instance: &Value, document: &str) {
        for variant in SchemaVariant::ALL {
            let violations = self.violations(variant, instance);
            assert!(
                violations.is_empty(),
                "{document} does not conform to the {} {} ({}):\n\n{}\n",
                self.name,
                variant.label(),
                self.path(variant),
                numbered(&violations),
            );
        }
    }
}

impl SchemaArtifact {
    fn load(variant: SchemaVariant, path: &'static str) -> SchemaArtifact {
        let absolute = repository_root().join(path);
        let text = std::fs::read_to_string(&absolute)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", absolute.display()));
        let document: Value = serde_json::from_str(&text)
            .unwrap_or_else(|error| panic!("{path} is not valid JSON: {error}"));

        // A document that is not a valid Draft 2020-12 schema makes no meaningful statement about
        // any instance, so reporting instance violations against it would be misleading.
        // `json_contract_schemas.rs` reports this as a first-class failure of its own; failing here
        // stops every other suite from proceeding past it into instance validation.
        if let Err(error) = jsonschema::draft202012::meta::validate(&document) {
            panic!("{path} is not a valid JSON Schema Draft 2020-12 document: {error}");
        }

        let validator = compile(&document)
            .unwrap_or_else(|error| panic!("{path} could not be compiled as a schema: {error}"));

        SchemaArtifact {
            variant,
            path,
            document,
            validator,
        }
    }

    /// Re-validates a failed `oneOf` instance against the single branch its `kind` selects, so the
    /// message names the constraint that actually went unsatisfied.
    ///
    /// A `oneOf` failure reports only that no branch matched. Both contracts dispatch expectations,
    /// diagnostic origins, and text expectation sources through `oneOf` over `kind`-discriminated
    /// branches, so without this a defect anywhere inside an expectation collapses to "is not valid
    /// under any of the schemas listed in the 'oneOf' keyword" — true, and useless for locating the
    /// defect.
    ///
    /// Returns nothing when the error is not a `oneOf` failure, when the instance carries no `kind`
    /// discriminator, or when no branch or more than one branch claims that `kind`; in those cases
    /// there is no single branch whose errors are the ones worth showing. This runs only while
    /// building a failure message, so re-compiling the branch is not on any passing path.
    fn discriminated_branch_detail(&self, error: &ValidationError<'_>) -> Vec<String> {
        let schema_path = error.schema_path().to_string();
        let Some(parent) = schema_path.strip_suffix("/oneOf") else {
            return Vec::new();
        };
        let Some(branches) = self
            .document
            .pointer(&format!("{parent}/oneOf"))
            .and_then(Value::as_array)
        else {
            return Vec::new();
        };
        let instance = error.instance();
        let Some(kind) = instance.get("kind").and_then(Value::as_str) else {
            return Vec::new();
        };

        let mut selected = branches
            .iter()
            .filter(|branch| self.branch_claims_kind(branch, kind));
        let (Some(branch), None) = (selected.next(), selected.next()) else {
            return Vec::new();
        };

        // Wrapped rather than compiled directly: a branch is normally `{"$ref": "#/$defs/..."}` and
        // its own nested `$ref`s are document-relative, so it only resolves alongside the whole
        // document's `$defs`.
        let wrapper = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$defs": self.document.get("$defs").cloned().unwrap_or(Value::Null),
            "allOf": [branch],
        });
        let Ok(validator) = compile(&wrapper) else {
            return Vec::new();
        };

        let mut lines = vec![format!("branch selected by kind `{kind}`:")];
        lines.extend(validator.iter_errors(instance).map(|branch_error| {
            format!(
                "  {branch_error} (instance path {}, schema path {})",
                render_location(&branch_error.instance_path().to_string()),
                render_location(&branch_error.schema_path().to_string()),
            )
        }));
        lines
    }

    /// Whether a `oneOf` branch pins `kind` to `value`, following one level of local `$ref`.
    fn branch_claims_kind(&self, branch: &Value, value: &str) -> bool {
        let resolved = match branch.get("$ref").and_then(Value::as_str) {
            Some(reference) => match reference
                .strip_prefix('#')
                .and_then(|pointer| self.document.pointer(pointer))
            {
                Some(resolved) => resolved,
                None => return false,
            },
            None => branch,
        };

        let Some(kind) = resolved.pointer("/properties/kind") else {
            return false;
        };
        if kind.get("const").and_then(Value::as_str) == Some(value) {
            return true;
        }
        kind.get("enum")
            .and_then(Value::as_array)
            .is_some_and(|values| values.iter().any(|candidate| candidate == value))
    }
}

/// Renders violation lines as a numbered list.
pub fn numbered(violations: &[String]) -> String {
    violations
        .iter()
        .enumerate()
        .map(|(index, violation)| format!("  {}. {violation}", index + 1))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Renders an empty JSON Pointer as a visible marker rather than as nothing at all, so a message
/// about the whole document does not read as a message with a field missing.
fn render_location(location: &str) -> &str {
    if location.is_empty() {
        "<document root>"
    } else {
        location
    }
}
