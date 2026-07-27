//! Generates each JSON contract's public `schema.json` from its metadata-bearing
//! `schema.internal.json`, and checks that the committed public schema is not stale.
//!
//! The internal source schema is the only file a maintainer edits. It carries
//! `x-reportage-snapshot` normalization metadata for the snapshot harness (issue #114); the
//! public schema is the same document with that metadata removed, and is the only path
//! external consumers should reference. See
//! docs/adr/20260727T151234Z_json-schema-artifact-generation.md.
//!
//! This is deliberately not a JSON Schema compiler: it neither validates nor interprets schema
//! semantics, and it never inlines `$ref`. Stripping is defined as a structural transformation
//! over the parsed document, which is why `x-reportage-snapshot` is a reserved object member
//! name across the whole internal source schema and why every occurrence of it must sit at an
//! allowlisted location.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::json;

use crate::json::{self, JsonValue};
use crate::output::{
    Category, Cause, CommandError, FileAction, FileChange, FileState, Report, ReportBody,
};

/// Reserved object member name carrying snapshot normalization metadata.
pub const SNAPSHOT_ANNOTATION: &str = "x-reportage-snapshot";

/// Recovery guidance shared by every failure a regeneration can fix.
const REGENERATE_HINT: &str =
    "Run `just schema-artifacts-gen` and commit the regenerated public schema.";

pub const GEN_COMMAND: &str = "schema-artifacts.gen";
pub const CHECK_COMMAND: &str = "schema-artifacts.check";

/// One JSON contract's schema pair.
pub struct SchemaContract {
    /// Stable identifier used in diagnostics and in the JSON envelope's `result`.
    pub name: &'static str,
    /// Repository-relative path of the metadata-bearing schema maintainers edit.
    pub internal_path: &'static str,
    /// Repository-relative path of the generated schema external consumers reference.
    pub public_path: &'static str,
    /// Every JSON Pointer at which [`SNAPSHOT_ANNOTATION`] may appear in the internal source
    /// schema, in document order.
    ///
    /// This is an exact set, not a lower bound: an occurrence anywhere else and a missing
    /// occurrence are both errors. Adding or moving an annotation therefore has to update the
    /// normalization policy and this list in the same change.
    pub annotation_locations: &'static [&'static str],
}

pub const CONTRACTS: &[SchemaContract] = &[
    SchemaContract {
        name: "json-report",
        internal_path: "spec/output/json-report/schema.internal.json",
        public_path: "spec/output/json-report/schema.json",
        annotation_locations: &[
            "/properties/artifactRoot/x-reportage-snapshot",
            "/$defs/Tool/properties/version/x-reportage-snapshot",
        ],
    },
    SchemaContract {
        name: "run-result",
        internal_path: "spec/artifacts/run-result/schema.internal.json",
        public_path: "spec/artifacts/run-result/schema.json",
        annotation_locations: &["/$defs/Tool/properties/version/x-reportage-snapshot"],
    },
];

/// Repository root, resolved from this crate's compile-time manifest directory rather than the
/// process working directory, so the tool addresses the same schema files no matter where it is
/// invoked from.
pub fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/xtask sits two levels below the repository root")
        .to_path_buf()
}

/// An `x-reportage-snapshot` occurrence the allowlist does not account for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationViolation {
    pub kind: AnnotationViolationKind,
    /// JSON Pointer of the annotation member itself, not of the schema object holding it.
    pub pointer: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationViolationKind {
    /// An allowlisted location the internal source schema no longer defines. Snapshot
    /// normalization silently loses a placeholder when this happens, so it is an error rather
    /// than a permitted subset.
    Missing,
    /// An occurrence outside the allowlist.
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparationError {
    /// The internal source schema is not valid JSON. Carries the one-based position serde
    /// reported, so the diagnostic can point at the defect.
    Malformed {
        message: String,
        position: (usize, usize),
    },
    Annotations(Vec<AnnotationViolation>),
}

/// Turns one internal source schema's text into its public schema text.
///
/// This is the whole generation contract: parse, verify annotation locations, strip every
/// `x-reportage-snapshot` member, and re-render. Nothing else about the document is touched,
/// so member order, array order, `$defs`, local `$ref`, `$id`, descriptions, and objects left
/// empty by stripping all survive unchanged.
pub fn public_schema_text(
    source: &str,
    annotation_locations: &[&str],
) -> Result<String, PreparationError> {
    let mut document = json::parse(source).map_err(|error| PreparationError::Malformed {
        message: error.to_string(),
        position: (error.line(), error.column()),
    })?;

    let mut removed = Vec::new();
    strip_snapshot_annotations(&mut document, "", &mut removed);

    let violations = annotation_violations(annotation_locations, &removed);
    if !violations.is_empty() {
        return Err(PreparationError::Annotations(violations));
    }

    Ok(json::render(&document))
}

/// Removes every member named [`SNAPSHOT_ANNOTATION`], recording each removal's JSON Pointer in
/// document order.
///
/// Removed subtrees are not descended into: an annotation nested inside another annotation is
/// already gone with its parent and must not be reported as a second occurrence.
fn strip_snapshot_annotations(value: &mut JsonValue, prefix: &str, removed: &mut Vec<String>) {
    match value {
        JsonValue::Object(members) => {
            let mut kept = Vec::with_capacity(members.len());
            for (key, mut child) in std::mem::take(members) {
                let pointer = format!("{prefix}/{}", escape_pointer_token(&key));
                if key == SNAPSHOT_ANNOTATION {
                    removed.push(pointer);
                    continue;
                }
                strip_snapshot_annotations(&mut child, &pointer, removed);
                kept.push((key, child));
            }
            *members = kept;
        }
        JsonValue::Array(items) => {
            for (index, item) in items.iter_mut().enumerate() {
                strip_snapshot_annotations(item, &format!("{prefix}/{index}"), removed);
            }
        }
        _ => {}
    }
}

/// RFC 6901 reference token escaping.
fn escape_pointer_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

fn annotation_violations(allowed: &[&str], removed: &[String]) -> Vec<AnnotationViolation> {
    let missing = allowed
        .iter()
        .filter(|pointer| !removed.iter().any(|found| found == *pointer))
        .map(|pointer| AnnotationViolation {
            kind: AnnotationViolationKind::Missing,
            pointer: (*pointer).to_owned(),
        });

    let unsupported = removed
        .iter()
        .filter(|pointer| !allowed.contains(&pointer.as_str()))
        .map(|pointer| AnnotationViolation {
            kind: AnnotationViolationKind::Unsupported,
            pointer: pointer.clone(),
        });

    missing.chain(unsupported).collect()
}

/// An internal source schema that failed before its public text could be produced.
enum ContractFailure {
    Unreadable(Cause),
    NotUtf8(Cause),
    Malformed(Cause),
    Annotations(Vec<Cause>),
}

fn prepare(root: &Path, contract: &'static SchemaContract) -> Result<String, Box<ContractFailure>> {
    let bytes = fs::read(root.join(contract.internal_path)).map_err(|error| {
        Box::new(ContractFailure::Unreadable(internal_cause(
            contract,
            "SOURCE_SCHEMA_UNREADABLE",
            format!("internal source schema could not be read: {error}"),
        )))
    })?;

    // Decoded explicitly rather than through `read_to_string`, which reports a non-UTF-8 file as
    // an I/O error and would classify an encoding defect as a filesystem failure.
    let source = String::from_utf8(bytes).map_err(|error| {
        Box::new(ContractFailure::NotUtf8(internal_cause(
            contract,
            "SOURCE_SCHEMA_NOT_UTF8",
            format!(
                "internal source schema is not valid UTF-8: first invalid byte at offset {}.",
                error.utf8_error().valid_up_to()
            ),
        )))
    })?;

    match public_schema_text(&source, contract.annotation_locations) {
        Ok(text) => Ok(text),
        Err(PreparationError::Malformed { message, position }) => {
            Err(Box::new(ContractFailure::Malformed(
                internal_cause(
                    contract,
                    "SOURCE_SCHEMA_MALFORMED",
                    format!("internal source schema is not valid JSON: {message}"),
                )
                .with_position(position.0, position.1),
            )))
        }
        Err(PreparationError::Annotations(violations)) => {
            Err(Box::new(ContractFailure::Annotations(
                violations
                    .into_iter()
                    .map(|violation| annotation_cause(contract, violation))
                    .collect(),
            )))
        }
    }
}

/// Prepares every contract, grouping failures so that one error code always describes one kind
/// of defect.
///
/// Failure kinds are reported in severity order rather than merged: a schema that cannot be
/// read or parsed makes its annotation locations unknowable, so mixing those causes into one
/// diagnostic would suggest problems the maintainer cannot yet see.
fn prepare_all(root: &Path) -> Result<Vec<(&'static SchemaContract, String)>, CommandError> {
    let mut prepared = Vec::new();
    let mut unreadable = Vec::new();
    let mut not_utf8 = Vec::new();
    let mut malformed = Vec::new();
    let mut annotations = Vec::new();

    for contract in CONTRACTS {
        match prepare(root, contract) {
            Ok(text) => prepared.push((contract, text)),
            Err(failure) => match *failure {
                ContractFailure::Unreadable(cause) => unreadable.push(cause),
                ContractFailure::NotUtf8(cause) => not_utf8.push(cause),
                ContractFailure::Malformed(cause) => malformed.push(cause),
                ContractFailure::Annotations(causes) => annotations.extend(causes),
            },
        }
    }

    if !unreadable.is_empty() {
        return Err(CommandError {
            code: "SOURCE_SCHEMA_UNREADABLE",
            category: Category::Filesystem,
            message: format!(
                "{} could not be read.",
                pluralize(unreadable.len(), "internal source schema")
            ),
            recovery: Some(
                "Restore the internal source schema from version control and check its file permissions."
                    .to_owned(),
            ),
            causes: unreadable,
        });
    }

    if !not_utf8.is_empty() {
        return Err(CommandError {
            code: "SOURCE_SCHEMA_NOT_UTF8",
            category: Category::Input,
            message: format!(
                "{} {} not valid UTF-8.",
                pluralize(not_utf8.len(), "internal source schema"),
                is_are(not_utf8.len())
            ),
            recovery: Some(
                "Re-save the internal source schema as UTF-8, then rerun the command.".to_owned(),
            ),
            causes: not_utf8,
        });
    }

    if !malformed.is_empty() {
        return Err(CommandError {
            code: "SOURCE_SCHEMA_MALFORMED",
            category: Category::Input,
            message: format!(
                "{} {} not valid JSON.",
                pluralize(malformed.len(), "internal source schema"),
                is_are(malformed.len())
            ),
            recovery: Some(
                "Fix the JSON syntax at the reported position in the internal source schema, then rerun the command."
                    .to_owned(),
            ),
            causes: malformed,
        });
    }

    if !annotations.is_empty() {
        return Err(CommandError {
            code: "SNAPSHOT_ANNOTATION_LOCATION_INVALID",
            category: Category::Input,
            message: format!(
                "{} {} not match the allowlist.",
                pluralize(
                    annotations.len(),
                    &format!("`{SNAPSHOT_ANNOTATION}` location")
                ),
                if annotations.len() == 1 { "does" } else { "do" }
            ),
            recovery: Some(format!(
                "Move each `{SNAPSHOT_ANNOTATION}` back to an allowlisted location, or update `annotation_locations` in crates/xtask/src/schema_artifacts.rs together with the snapshot normalization policy."
            )),
            causes: annotations,
        });
    }

    Ok(prepared)
}

/// Generates every public schema. With `dry_run`, the same comparison runs but no file is
/// written and every reported change is `planned`.
pub fn generate(root: &Path, dry_run: bool) -> Report {
    let prepared = match prepare_all(root) {
        Ok(prepared) => prepared,
        Err(error) => return failure(GEN_COMMAND, dry_run, Vec::new(), error),
    };

    let mut file_changes = Vec::new();
    let mut contracts = Vec::new();

    for (contract, text) in prepared {
        let public = root.join(contract.public_path);
        let committed = match read_optional(&public) {
            Ok(committed) => committed,
            Err(error) => {
                return failure(
                    GEN_COMMAND,
                    dry_run,
                    file_changes,
                    read_failure(contract, error),
                );
            }
        };

        let action = match &committed {
            None => Some(FileAction::Create),
            Some(bytes) if bytes.as_slice() != text.as_bytes() => Some(FileAction::Modify),
            Some(_) => None,
        };

        let state = match action {
            None => "unchanged",
            Some(action) => {
                if !dry_run && let Err(error) = write_atomically(&public, &text) {
                    return failure(
                        GEN_COMMAND,
                        dry_run,
                        file_changes,
                        write_failure(
                            contract,
                            format!("public schema could not be written: {error}"),
                        ),
                    );
                }
                file_changes.push(FileChange {
                    action,
                    path: contract.public_path.to_owned(),
                    state: if dry_run {
                        FileState::Planned
                    } else {
                        FileState::Completed
                    },
                });
                match (action, dry_run) {
                    (FileAction::Create, false) => "created",
                    (FileAction::Modify, false) => "updated",
                    (FileAction::Create, true) => "wouldCreate",
                    (FileAction::Modify, true) => "wouldUpdate",
                }
            }
        };

        contracts.push(json!({
            "name": contract.name,
            "internalSchemaPath": contract.internal_path,
            "publicSchemaPath": contract.public_path,
            "state": state,
        }));
    }

    let changed = pluralize(file_changes.len(), "public schema");
    let outcome = if dry_run {
        format!("{changed} would change.")
    } else {
        format!("{changed} changed.")
    };

    Report {
        command: GEN_COMMAND,
        dry_run,
        file_changes,
        body: ReportBody::Success {
            result: json!({ "contracts": contracts }),
            summary: vec![format!(
                "{} processed. {outcome}",
                pluralize(CONTRACTS.len(), "internal source schema")
            )],
        },
    }
}

/// Regenerates every public schema in memory and compares it byte-for-byte with the committed
/// file. Never writes.
pub fn check(root: &Path) -> Report {
    let prepared = match prepare_all(root) {
        Ok(prepared) => prepared,
        Err(error) => return failure(CHECK_COMMAND, false, Vec::new(), error),
    };

    let mut causes = Vec::new();

    for (contract, text) in prepared {
        match read_optional(&root.join(contract.public_path)) {
            // A public schema that exists but cannot be read is a filesystem fault, not a stale
            // artifact: regenerating it would fail for the same reason, so it must not be folded
            // into the out-of-date diagnostic whose recovery is "regenerate and commit".
            Err(error) => {
                return failure(
                    CHECK_COMMAND,
                    false,
                    Vec::new(),
                    read_failure(contract, error),
                );
            }
            Ok(None) => causes.push(public_cause(
                contract,
                "PUBLIC_SCHEMA_MISSING",
                "public schema does not exist.".to_owned(),
            )),
            Ok(Some(bytes)) if bytes.as_slice() != text.as_bytes() => causes.push(public_cause(
                contract,
                "PUBLIC_SCHEMA_STALE",
                "public schema differs from the bytes its internal source schema generates."
                    .to_owned(),
            )),
            Ok(Some(_)) => {}
        }
    }

    if causes.is_empty() {
        return Report {
            command: CHECK_COMMAND,
            dry_run: false,
            file_changes: Vec::new(),
            body: ReportBody::Success {
                result: json!({
                    "contracts": CONTRACTS
                        .iter()
                        .map(|contract| json!({
                            "name": contract.name,
                            "internalSchemaPath": contract.internal_path,
                            "publicSchemaPath": contract.public_path,
                            "state": "upToDate",
                        }))
                        .collect::<Vec<_>>(),
                }),
                summary: vec![format!(
                    "{} {} up to date.",
                    pluralize(CONTRACTS.len(), "public schema"),
                    is_are(CONTRACTS.len())
                )],
            },
        };
    }

    failure(
        CHECK_COMMAND,
        false,
        Vec::new(),
        CommandError {
            code: "PUBLIC_SCHEMA_OUT_OF_DATE",
            category: Category::Conflict,
            message: format!(
                "{} {} out of date.",
                pluralize(causes.len(), "public schema"),
                is_are(causes.len())
            ),
            recovery: Some(REGENERATE_HINT.to_owned()),
            causes,
        },
    )
}

fn read_optional(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Writes through a sibling temporary file so a failure mid-write cannot leave a truncated
/// public schema behind, which a later `check` would report as ordinary staleness.
///
/// The temporary must be a sibling for `rename` to stay within one filesystem. It is
/// dot-prefixed and matched by the `.gitignore` rule for generator debris, so a temporary left
/// behind by a killed process cannot be committed by a broad `git add`.
fn write_atomically(path: &Path, text: &str) -> io::Result<()> {
    let temporary = temporary_path(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    match fs::write(&temporary, text).and_then(|()| fs::rename(&temporary, path)) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            Err(error)
        }
    }
}

/// Sibling temporary path [`write_atomically`] stages a public schema through. Exposed so tests
/// can assert that no debris survives a run.
pub fn temporary_path(public_schema: &Path) -> PathBuf {
    let name = public_schema
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    public_schema.with_file_name(format!(".{name}.tmp"))
}

fn internal_cause(contract: &'static SchemaContract, code: &'static str, message: String) -> Cause {
    Cause::new(
        code,
        message,
        contract.name,
        contract.internal_path.to_owned(),
        contract.internal_path.to_owned(),
        contract.public_path.to_owned(),
    )
}

fn public_cause(contract: &'static SchemaContract, code: &'static str, message: String) -> Cause {
    Cause::new(
        code,
        message,
        contract.name,
        contract.public_path.to_owned(),
        contract.internal_path.to_owned(),
        contract.public_path.to_owned(),
    )
}

fn annotation_cause(contract: &'static SchemaContract, violation: AnnotationViolation) -> Cause {
    let (code, message) = match violation.kind {
        AnnotationViolationKind::Missing => (
            "SNAPSHOT_ANNOTATION_MISSING",
            format!("allowlisted `{SNAPSHOT_ANNOTATION}` location is absent."),
        ),
        AnnotationViolationKind::Unsupported => (
            "SNAPSHOT_ANNOTATION_UNSUPPORTED_LOCATION",
            format!("`{SNAPSHOT_ANNOTATION}` appears at a location the allowlist does not permit."),
        ),
    };

    internal_cause(contract, code, message).with_pointer(violation.pointer)
}

/// A public schema that exists but cannot be read. Distinct from [`write_failure`], because
/// nothing was written and no reported change is at risk.
fn read_failure(contract: &'static SchemaContract, error: io::Error) -> CommandError {
    CommandError {
        code: "PUBLIC_SCHEMA_UNREADABLE",
        category: Category::Filesystem,
        message: "A public schema could not be read.".to_owned(),
        recovery: Some(
            "Check that the public schema is a readable regular file, then rerun the command."
                .to_owned(),
        ),
        causes: vec![public_cause(
            contract,
            "PUBLIC_SCHEMA_UNREADABLE",
            format!("public schema could not be read: {error}"),
        )],
    }
}

fn write_failure(contract: &'static SchemaContract, message: String) -> CommandError {
    CommandError {
        code: "PUBLIC_SCHEMA_WRITE_FAILED",
        category: Category::Filesystem,
        message: "A public schema could not be updated.".to_owned(),
        recovery: Some(
            "Check the file permissions on the public schema and its directory, inspect any change already reported, then rerun the command."
                .to_owned(),
        ),
        causes: vec![public_cause(contract, "PUBLIC_SCHEMA_WRITE_FAILED", message)],
    }
}

fn failure(
    command: &'static str,
    dry_run: bool,
    file_changes: Vec<FileChange>,
    error: CommandError,
) -> Report {
    Report {
        command,
        dry_run,
        file_changes,
        body: ReportBody::Failure(error),
    }
}

/// Pluralizes a noun against a count, e.g. `1 public schema` / `2 public schemas`.
fn pluralize(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("{count} {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

fn is_are(count: usize) -> &'static str {
    if count == 1 { "is" } else { "are" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_only_the_reserved_member_name() {
        let source = r#"{
  "properties": {
    "a": {
      "type": "string",
      "x-reportage-snapshot": { "operation": "replace", "value": "<A>" }
    }
  },
  "x-reportage-other": { "kept": true }
}"#;

        let generated = public_schema_text(source, &["/properties/a/x-reportage-snapshot"])
            .expect("annotation is allowlisted");

        assert!(!generated.contains(SNAPSHOT_ANNOTATION));
        assert!(generated.contains("\"x-reportage-other\""));
        assert!(generated.contains("\"type\": \"string\""));
    }

    #[test]
    fn reports_missing_and_unsupported_locations_together() {
        let source = r#"{ "a": { "x-reportage-snapshot": {} } }"#;

        let error = public_schema_text(source, &["/b/x-reportage-snapshot"])
            .expect_err("allowlist does not match");

        assert_eq!(
            error,
            PreparationError::Annotations(vec![
                AnnotationViolation {
                    kind: AnnotationViolationKind::Missing,
                    pointer: "/b/x-reportage-snapshot".to_owned(),
                },
                AnnotationViolation {
                    kind: AnnotationViolationKind::Unsupported,
                    pointer: "/a/x-reportage-snapshot".to_owned(),
                },
            ])
        );
    }

    #[test]
    fn escapes_pointer_tokens() {
        let source = r#"{ "a/b": { "c~d": { "x-reportage-snapshot": {} } } }"#;

        let error =
            public_schema_text(source, &[]).expect_err("no annotation location is allowlisted");

        assert_eq!(
            error,
            PreparationError::Annotations(vec![AnnotationViolation {
                kind: AnnotationViolationKind::Unsupported,
                pointer: "/a~1b/c~0d/x-reportage-snapshot".to_owned(),
            }])
        );
    }

    #[test]
    fn does_not_descend_into_a_removed_annotation() {
        let source = r#"{ "a": { "x-reportage-snapshot": { "x-reportage-snapshot": {} } } }"#;

        let generated = public_schema_text(source, &["/a/x-reportage-snapshot"])
            .expect("the outer annotation is allowlisted");

        assert_eq!(generated, "{\n  \"a\": {}\n}\n");
    }

    #[test]
    fn malformed_source_reports_its_position() {
        let error = public_schema_text("{\n  \"a\": ,\n}", &[]).expect_err("invalid JSON");

        match error {
            PreparationError::Malformed { position, .. } => assert_eq!(position, (2, 8)),
            other => panic!("expected a malformed-source error, got {other:?}"),
        }
    }
}
