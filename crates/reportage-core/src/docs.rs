//! Documentation generation (`reportage docs`): glob discovery, the source
//! loading boundary, the Documentation Catalog, renderers, and output
//! writing.
//!
//! Generation parses sources but never executes them: `SourceFile::into_script`,
//! the executor, and the evaluator are not reachable from this module, and no
//! execution artifact is produced. See
//! docs/adr/20260723T070556Z_documentation-generation-command.md and
//! docs/reference/docs-generation.md for the user-facing contract.

pub mod catalog;
pub mod discovery;
pub mod layout;
pub mod loader;
pub mod output;
pub mod plain;
pub mod render;

use std::path::{Path, PathBuf};

use discovery::DiscoveryError;
use layout::{DocumentLayoutPlan, PlannedDocument};
use loader::SourceLoadError;
use output::OutputError;
use render::DocumentRenderer;

/// The generated document format, as selected on the CLI. The closed set of
/// v0 values: an unknown `--format` is a clap usage error, never a fallback.
///
/// Formats are implemented behind the uniform [`render::DocumentRenderer`]
/// interface; this enum is only the user-facing selector, resolved to an
/// implementation in [`renderer_for`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DocumentFormat {
    #[default]
    Plain,
}

/// The generated document layout, as selected on the CLI. The closed set of
/// v0 values: an unknown `--layout` is a clap usage error, never a fallback.
///
/// Layouts are implemented behind the uniform [`layout::DocumentLayoutPlan`]
/// interface; this enum is only the user-facing selector, resolved to an
/// implementation in [`layout_for`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DocumentLayout {
    #[default]
    SingleFile,
}

/// Resolves the format selector to its renderer implementation.
///
/// The exhaustive match is deliberate: adding a format extends this single
/// factory, and the compiler points here when the enum grows.
pub fn renderer_for(format: DocumentFormat) -> &'static dyn DocumentRenderer {
    match format {
        DocumentFormat::Plain => &plain::PlainRenderer,
    }
}

/// Resolves the layout selector to its plan implementation.
///
/// The exhaustive match is deliberate, exactly as in [`renderer_for`].
pub fn layout_for(document_layout: DocumentLayout) -> &'static dyn DocumentLayoutPlan {
    match document_layout {
        DocumentLayout::SingleFile => &layout::SingleFileLayout,
    }
}

/// A `reportage docs` invocation, as validated by the CLI layer.
#[derive(Debug)]
pub struct GenerateRequest {
    pub patterns: Vec<String>,
    pub out_dir: PathBuf,
    pub format: DocumentFormat,
    pub layout: DocumentLayout,
}

/// The successful outcome: every written document path, for mutation
/// reporting by the caller.
#[derive(Debug)]
pub struct GenerateReport {
    pub written: Vec<PathBuf>,
}

/// Exit-code-level error classification for the CLI:
/// request/source validation errors versus filesystem/runtime infrastructure
/// errors. See docs/reference/exit-codes.md — `docs` exit codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClass {
    RequestValidation,
    Infrastructure,
}

#[derive(Debug)]
pub enum GenerateError {
    Discovery(DiscoveryError),
    SourceLoad(Vec<SourceLoadError>),
    Output(OutputError),
}

impl GenerateError {
    /// Classifies this error for the exit code contract.
    pub fn class(&self) -> ErrorClass {
        match self {
            GenerateError::Discovery(DiscoveryError::Traversal { .. }) => {
                ErrorClass::Infrastructure
            }
            GenerateError::Discovery(_) => ErrorClass::RequestValidation,
            GenerateError::SourceLoad(_) => ErrorClass::RequestValidation,
            GenerateError::Output(
                OutputError::CreateFailed { .. }
                | OutputError::WriteFailed { .. }
                | OutputError::ReplaceFailed { .. },
            ) => ErrorClass::Infrastructure,
            GenerateError::Output(_) => ErrorClass::RequestValidation,
        }
    }

    /// The error detail lines, in the deterministic order they must be
    /// reported: source load errors are already ordered by display path.
    pub fn messages(&self) -> Vec<String> {
        match self {
            GenerateError::Discovery(e) => vec![e.to_string()],
            GenerateError::SourceLoad(errors) => errors.iter().map(|e| e.to_string()).collect(),
            GenerateError::Output(e) => vec![e.to_string()],
        }
    }
}

impl std::fmt::Display for GenerateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.messages().join("\n"))
    }
}

impl std::error::Error for GenerateError {}

/// Generates documentation end to end: resolve patterns, load sources, build
/// the Catalog, render, then — only after everything succeeded — validate the
/// output directory and write the document(s).
///
/// Patterns and the display path contract are resolved against `base_dir`;
/// the CLI passes the current working directory. `request.out_dir` is used as
/// given.
pub fn generate(
    base_dir: &Path,
    request: &GenerateRequest,
) -> Result<GenerateReport, GenerateError> {
    let discovered = discovery::resolve_patterns(base_dir, &request.patterns)
        .map_err(GenerateError::Discovery)?;
    let loaded = loader::load_sources(discovered).map_err(GenerateError::SourceLoad)?;
    let catalog = catalog::build_catalog(&loaded);
    let planned: Vec<PlannedDocument> =
        layout_for(request.layout).plan(&catalog, renderer_for(request.format));

    let output_directory =
        output::OutputDirectory::prepare(&request.out_dir).map_err(GenerateError::Output)?;

    let mut written = Vec::with_capacity(planned.len());
    for document in &planned {
        written.push(
            output_directory
                .write_document(&document.relative_path, &document.contents)
                .map_err(GenerateError::Output)?,
        );
    }

    Ok(GenerateReport { written })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }

    fn request(dir: &Path, patterns: &[&str]) -> GenerateRequest {
        GenerateRequest {
            patterns: patterns.iter().map(|p| p.to_string()).collect(),
            out_dir: dir.join("generated"),
            format: DocumentFormat::Plain,
            layout: DocumentLayout::SingleFile,
        }
    }

    const VALID_CASE: &str = "case \"ok\" {\n  $ true\n  assert {\n    exit 0\n  }\n}\n";

    #[test]
    fn generates_a_single_index_document() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("src/a.repor"), VALID_CASE);

        let report = generate(dir.path(), &request(dir.path(), &["src/*.repor"])).unwrap();
        assert_eq!(report.written, vec![dir.path().join("generated/index.txt")]);

        let document = std::fs::read_to_string(&report.written[0]).unwrap();
        assert!(document.starts_with("Reportage Documentation\n"));
        assert!(document.contains("Source path\n  src/a.repor"));
    }

    #[test]
    fn source_errors_prevent_output_directory_creation() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("src/bad.repor"), "case \"broken\" {\n");

        let err = generate(dir.path(), &request(dir.path(), &["src/*.repor"])).unwrap_err();
        assert_eq!(err.class(), ErrorClass::RequestValidation);
        assert!(!dir.path().join("generated").exists());
    }

    #[test]
    fn load_error_messages_are_ordered_by_display_path() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("src/b.repor"), "case \"broken\" {\n");
        write(&dir.path().join("src/a.repor"), "also broken");

        let err = generate(dir.path(), &request(dir.path(), &["src/*.repor"])).unwrap_err();
        let messages = err.messages();
        assert_eq!(messages.len(), 2);
        assert!(messages[0].starts_with("src/a.repor: "));
        assert!(messages[1].starts_with("src/b.repor: "));
    }

    #[test]
    fn error_classes_map_request_and_infrastructure_causes() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("src/a.repor"), VALID_CASE);

        // Unmatched pattern: request validation.
        let err = generate(dir.path(), &request(dir.path(), &["missing/*.repor"])).unwrap_err();
        assert_eq!(err.class(), ErrorClass::RequestValidation);

        // Output directory creation blocked by a regular file: infrastructure.
        write(&dir.path().join("blocker"), "");
        let mut req = request(dir.path(), &["src/*.repor"]);
        req.out_dir = dir.path().join("blocker/out");
        let err = generate(dir.path(), &req).unwrap_err();
        assert_eq!(err.class(), ErrorClass::Infrastructure);

        // Existing out-dir that is a regular file: request validation.
        let mut req = request(dir.path(), &["src/*.repor"]);
        req.out_dir = dir.path().join("blocker");
        let err = generate(dir.path(), &req).unwrap_err();
        assert_eq!(err.class(), ErrorClass::RequestValidation);
    }

    #[test]
    fn generation_is_deterministic_and_replaces_existing_output() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("src/a.repor"), VALID_CASE);
        write(&dir.path().join("src/b.repor"), VALID_CASE);

        let req = request(dir.path(), &["src/*.repor"]);
        generate(dir.path(), &req).unwrap();
        let first = std::fs::read_to_string(dir.path().join("generated/index.txt")).unwrap();
        generate(dir.path(), &req).unwrap();
        let second = std::fs::read_to_string(dir.path().join("generated/index.txt")).unwrap();
        assert_eq!(first, second);
    }

    /// Every output error variant maps to the documented exit code class:
    /// wrong filesystem types are request validation, OS-level failures are
    /// infrastructure.
    #[test]
    fn output_error_classification_is_exhaustive() {
        use crate::docs::output::OutputError;

        let request = [
            OutputError::NotADirectory(PathBuf::from("x")),
            OutputError::SymlinkOutputDirectory(PathBuf::from("x")),
            OutputError::ExistingOutputNotReplaceable(PathBuf::from("x")),
            OutputError::InvalidRelativePath("../x".to_string()),
        ];
        for error in request {
            assert_eq!(
                GenerateError::Output(error).class(),
                ErrorClass::RequestValidation
            );
        }

        let infrastructure = [
            OutputError::CreateFailed {
                path: PathBuf::from("x"),
                message: "denied".to_string(),
            },
            OutputError::WriteFailed {
                path: PathBuf::from("x"),
                message: "denied".to_string(),
            },
            OutputError::ReplaceFailed {
                path: PathBuf::from("x"),
                message: "denied".to_string(),
            },
        ];
        for error in infrastructure {
            assert_eq!(
                GenerateError::Output(error).class(),
                ErrorClass::Infrastructure
            );
        }
    }

    #[test]
    fn zero_case_sources_still_produce_a_document() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("src/empty.repor"), "");

        let report = generate(dir.path(), &request(dir.path(), &["src/*.repor"])).unwrap();
        let document = std::fs::read_to_string(&report.written[0]).unwrap();
        assert!(document.contains("File\n  empty"));
        assert!(!document.contains("Reportage source"));
    }
}
