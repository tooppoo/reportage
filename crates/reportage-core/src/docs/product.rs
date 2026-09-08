//! The Product Documentation Catalog: the renderer-ready intermediate model
//! for documenting a product that is *tested* with reportage
//! (`reportage docs`), as opposed to reportage itself.
//!
//! A product's readers need the product, not the DSL. This projection
//! therefore never carries `.repor` source text: it reads the semantic step
//! model and restates each case as what a reader would do and observe —
//! the files they prepare, the commands they run, and the conditions verified
//! afterwards. See
//! docs/adr/20260907T230710Z_reportage-source-documentation-subcommand.md.
//!
//! Three properties are load-bearing and easy to lose:
//!
//! - **Source order is preserved.** Commands and verifications are one step
//!   sequence, not a `commands[]` plus a `results[]`. A reportage assertion
//!   verifies the current checkpoint rather than the action before it, so
//!   flattening the two would invent a causal pairing the language does not
//!   define. See docs/reference/execution-model.md — Checkpoint.
//! - **Expectations stay structured.** An expectation is modeled as an
//!   observed subject and the operation applied to it, never as a rendered
//!   sentence or the source text that produced it, so a renderer can phrase
//!   `dir <"d"> contains "x"` and `file <"f"> contains "x"` differently even
//!   though both spell `contains`.
//! - **Literal text survives interpolation.** A value assembled from captured
//!   output keeps its literal parts and names the binding filling each gap,
//!   because a config file with one interpolated field is otherwise entirely
//!   literal text a reader needs.
//!
//! Like the Reportage-source Catalog, this API exposes only plain values, so
//! renderers never depend on parser or execution-model types. Display
//! fallbacks and ordering come from [`super::metadata`], shared with that
//! Catalog because both read the same `document` blocks.
//!
//! The model's decisions — and what it deliberately does not carry — are
//! recorded in
//! docs/adr/20260908T131134Z_product-documentation-projection.md.

use crate::model::{
    AssertionBlock, BeforeEach, CountOp, DirMatcher, Expectation, FileContentsReference,
    FileMatcher, InterpolatedTextSegment, LogicalOperator, OutputMatcher, OutputSource,
    SideEffectingStep, Step, TextValueExpression, WriteFileStep,
};

use super::loader::LoadedSourceFile;
use super::metadata::{self, DocumentGroup};

#[derive(Debug, PartialEq, Eq)]
pub struct ProductDocumentationCatalog {
    pub groups: Vec<ProductGroup>,
}

/// One group of documented sources, named by `document file.group`.
pub type ProductGroup = DocumentGroup<ProductDocumentedFile>;

/// One source file's examples, under its `document file` metadata.
#[derive(Debug, PartialEq, Eq)]
pub struct ProductDocumentedFile {
    pub title: String,
    pub description: Option<String>,
    /// The normalized display path of the source these examples came from.
    ///
    /// Carried for the same reason the Reportage-source Catalog carries it —
    /// it is the identity files are ordered by — not because a product
    /// document has to show it; that is a renderer's decision.
    pub source_path: String,
    pub examples: Vec<ProductExample>,
}

/// One case, projected as an example a reader can follow.
#[derive(Debug, PartialEq, Eq)]
pub struct ProductExample {
    pub title: String,
    pub description: Option<String>,
    /// The file's `before_each` steps, shared by every example in the file.
    ///
    /// Repeated on each example rather than held once per file: setup is part
    /// of what makes *this* example reproducible, and `before_each` is a
    /// reportage-specific name that must not reach a product's readers.
    /// Empty when the source declares no setup.
    pub preparation: Vec<ExampleStep>,
    pub steps: Vec<ExampleStep>,
}

/// One step of an example, in source order.
#[derive(Debug, PartialEq, Eq)]
pub enum ExampleStep {
    /// A file the reader prepares, from a `write` step.
    File(ExampleFile),
    /// A command the reader runs, from a `$` action.
    ///
    /// Not classified as product operation versus example setup: reportage
    /// states no such distinction, so inferring one would attribute meaning
    /// the source never carried.
    Command(ExampleCommand),
    /// The conditions verified at this point, from an `assert` block.
    Verification(ExampleVerification),
}

/// A file and its content, as an input example.
#[derive(Debug, PartialEq, Eq)]
pub struct ExampleFile {
    pub path: String,
    pub content: DocumentedText,
    /// The POSIX permission bits, only when the source names them explicitly.
    ///
    /// `None` does not mean "unspecified": every `write` applies a mode, and
    /// an unnamed one is reportage's fixed `0o600` workspace default (see
    /// docs/reference/semantics.md — File mode). That default is a property of
    /// the case workspace, not guidance a product's reader should reproduce,
    /// so it is deliberately not surfaced. An explicitly named mode is: an
    /// example whose file has to be executable is wrong without it.
    pub mode: Option<u32>,
}

/// Text a source states, possibly assembled from values captured while the
/// example runs.
///
/// The composed form keeps the literal parts rather than collapsing the whole
/// value into "captured": a config file with one interpolated field is almost
/// entirely literal, and a reader needs that literal text.
#[derive(Debug, PartialEq, Eq)]
pub enum DocumentedText {
    /// Text the source states in full.
    Literal(String),
    /// Text assembled from literal parts and captured values, in order.
    Composed(Vec<TextSegment>),
}

/// One piece of a [`DocumentedText::Composed`] value.
#[derive(Debug, PartialEq, Eq)]
pub enum TextSegment {
    Literal(String),
    /// A value captured during the run, named by the binding it came from, so
    /// a renderer can say which value goes here instead of only that one does.
    Captured {
        binding: String,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub struct ExampleCommand {
    /// The command line as written, passed to a POSIX shell unchanged.
    pub command: String,
}

/// One checkpoint's verified conditions, in source order.
#[derive(Debug, PartialEq, Eq)]
pub struct ExampleVerification {
    pub expectations: Vec<DocumentedExpectation>,
}

/// One documented condition.
///
/// Structured, never a sentence: rendering is a renderer's job, and a
/// pre-rendered string could not be re-phrased per format or per subject.
#[derive(Debug, PartialEq, Eq)]
pub enum DocumentedExpectation {
    /// A condition on one observed subject.
    Observation {
        subject: ObservedSubject,
        operation: ObservedOperation,
    },
    /// A logical composition over nested conditions, kept nested so a renderer
    /// can preserve the grouping the source wrote.
    Composition {
        operator: CompositionOperator,
        children: Vec<DocumentedExpectation>,
    },
}

/// What a condition observes.
///
/// The subject half of the subject-and-operation pair: the same operation word
/// means different things on different subjects (`contains` is a substring on
/// `Stdout` and a directory entry name on `Dir`), so a renderer must read both
/// halves together.
#[derive(Debug, PartialEq, Eq)]
pub enum ObservedSubject {
    /// The exit status of the last command run before this checkpoint.
    ExitCode,
    Stdout,
    Stderr,
    File {
        path: String,
    },
    Dir {
        path: String,
    },
    /// The files matching a glob. Reserved by the expectation model; no v0
    /// syntax produces it.
    FileCount {
        glob: String,
    },
}

/// What is asserted about the observed subject.
#[derive(Debug, PartialEq, Eq)]
pub enum ObservedOperation {
    Exists,
    DoesNotExist,
    /// The subject holds nothing at all.
    IsEmpty,
    /// The subject equals the expected value exactly.
    Is(ExpectedValue),
    Contains(ExpectedValue),
    DoesNotContain(ExpectedValue),
    /// The subject matches a regular expression.
    Matches {
        pattern: String,
    },
    /// Reserved by the expectation model alongside
    /// [`ObservedSubject::FileCount`]; no v0 syntax produces it.
    CountIs {
        comparison: CountComparison,
        count: usize,
    },
    /// A structured query over the subject. Reserved by the expectation model;
    /// no v0 syntax produces it.
    StructuredQuery {
        expression: String,
    },
}

/// The value a condition compares its subject against.
#[derive(Debug, PartialEq, Eq)]
pub enum ExpectedValue {
    Text(DocumentedText),
    /// A whole number stated by the source, such as an exit code.
    Number(u64),
    /// The contents of another file, named by its path and by whether that
    /// file is part of the example.
    FileContents {
        path: String,
        origin: FileContentsOrigin,
    },
}

/// Where a compared-against file lives, relative to the example.
///
/// The distinction is a reader's, not a syntactic one: a path they can see in
/// the example is guidance, and a path they cannot is a detail of how the
/// example is checked. Collapsing the two would let a renderer point readers
/// at a file that appears nowhere in the documentation.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FileContentsOrigin {
    /// A file in the example's own working directory: one the example prepares
    /// or a command produces, so it is visible in the example itself.
    Example,
    /// A file kept alongside the `.repor` source, outside the example: a
    /// reader following the example never encounters it.
    External,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CountComparison {
    Exactly,
    AtLeast,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CompositionOperator {
    Not,
    All,
    Any,
}

/// Builds the Product Catalog from loaded sources.
///
/// Grouping, file ordering, and the metadata fallbacks are
/// [`super::metadata`]'s; examples stay in case order and each example's steps
/// stay in source order.
/// A file with a `before_each` but no case produces no example at all, so its
/// setup does not appear: there is nothing for a reader to prepare for. The
/// Reportage-source projection deliberately does show that setup, because it
/// documents the file as written rather than as an example.
pub fn build_product_catalog(sources: &[LoadedSourceFile]) -> ProductDocumentationCatalog {
    ProductDocumentationCatalog {
        groups: metadata::group_files(sources, |loaded, file_metadata| {
            let setup_steps = loaded.source.before_each().map(BeforeEach::steps);

            ProductDocumentedFile {
                title: file_metadata.title.clone(),
                description: file_metadata.description.clone(),
                source_path: file_metadata.source_path.clone(),
                examples: loaded
                    .source
                    .cases()
                    .iter()
                    .map(|source_case| {
                        let case_metadata = metadata::case_metadata(source_case);
                        ProductExample {
                            title: case_metadata.title,
                            description: case_metadata.description,
                            preparation: setup_steps.map(project_steps).unwrap_or_default(),
                            steps: project_steps(&source_case.case().steps),
                        }
                    })
                    .collect(),
            }
        }),
    }
}

/// Projects a step sequence, dropping the steps a product reader cannot
/// observe.
///
/// A `let` binding declares where a captured value comes from; it produces no
/// file, no command, and no verified condition, so it has no product-facing
/// meaning of its own. Its effect stays visible wherever the bound value is
/// used, as a [`TextSegment::Captured`] piece of the text that names it.
fn project_steps(steps: &[Step]) -> Vec<ExampleStep> {
    steps.iter().filter_map(project_step).collect()
}

fn project_step(step: &Step) -> Option<ExampleStep> {
    match step {
        Step::Action(action) => Some(ExampleStep::Command(ExampleCommand {
            command: action.command.clone(),
        })),
        Step::AssertionBlock(block) => Some(ExampleStep::Verification(project_assertions(block))),
        Step::SideEffect(SideEffectingStep::WriteFile(write)) => {
            Some(ExampleStep::File(project_write(write)))
        }
        Step::Binding(_) => None,
    }
}

fn project_write(write: &WriteFileStep) -> ExampleFile {
    ExampleFile {
        path: write.path.as_str().to_string(),
        content: project_text(&write.content),
        mode: write.mode.map(|mode| mode.bits()),
    }
}

/// Projects a text expression, keeping the literal text a source states even
/// when part of the value is captured at run time.
fn project_text(expression: &TextValueExpression) -> DocumentedText {
    match expression {
        TextValueExpression::Raw(literal) => {
            DocumentedText::Literal(literal.to_text_value().as_str().to_string())
        }
        TextValueExpression::Binding(reference) => {
            DocumentedText::Composed(vec![TextSegment::Captured {
                binding: reference.name.clone(),
            }])
        }
        TextValueExpression::Interpolated(text) => {
            let segments: Vec<TextSegment> = text
                .segments()
                .iter()
                .map(|segment| match segment {
                    InterpolatedTextSegment::Literal(literal) => {
                        TextSegment::Literal(literal.as_str().to_string())
                    }
                    InterpolatedTextSegment::Binding(reference) => TextSegment::Captured {
                        binding: reference.name.clone(),
                    },
                })
                .collect();

            // A reference-free interpolated literal is legal and states its
            // text in full, so it documents as literal text: the interpolation
            // syntax is a source-level detail with nothing left to substitute.
            match literal_text(&segments) {
                Some(text) => DocumentedText::Literal(text),
                None => DocumentedText::Composed(segments),
            }
        }
    }
}

/// The concatenated text of `segments` when none of them is captured.
fn literal_text(segments: &[TextSegment]) -> Option<String> {
    segments
        .iter()
        .map(|segment| match segment {
            TextSegment::Literal(text) => Some(text.as_str()),
            TextSegment::Captured { .. } => None,
        })
        .collect()
}

fn project_assertions(block: &AssertionBlock) -> ExampleVerification {
    ExampleVerification {
        expectations: block
            .expectations()
            .iter()
            .map(project_expectation)
            .collect(),
    }
}

fn project_expectation(expectation: &Expectation) -> DocumentedExpectation {
    let observation =
        |subject, operation| DocumentedExpectation::Observation { subject, operation };

    match expectation {
        Expectation::Exit(exit) => observation(
            ObservedSubject::ExitCode,
            ObservedOperation::Is(ExpectedValue::Number(u64::from(exit.expected))),
        ),
        Expectation::Stdout(output) => observation(
            ObservedSubject::Stdout,
            project_output_matcher(&output.matcher),
        ),
        Expectation::Stderr(output) => observation(
            ObservedSubject::Stderr,
            project_output_matcher(&output.matcher),
        ),
        Expectation::File(file) => observation(
            ObservedSubject::File {
                path: file.path.clone(),
            },
            project_file_matcher(&file.matcher),
        ),
        Expectation::Dir(dir) => observation(
            ObservedSubject::Dir {
                path: dir.path.clone(),
            },
            project_dir_matcher(&dir.matcher),
        ),
        Expectation::FileCount(file_count) => observation(
            ObservedSubject::FileCount {
                glob: file_count.glob.clone(),
            },
            ObservedOperation::CountIs {
                comparison: match file_count.op {
                    CountOp::Eq => CountComparison::Exactly,
                    CountOp::Gte => CountComparison::AtLeast,
                },
                count: file_count.count,
            },
        ),
        Expectation::Jq(jq) => observation(
            match jq.source {
                OutputSource::Stdout => ObservedSubject::Stdout,
                OutputSource::Stderr => ObservedSubject::Stderr,
            },
            ObservedOperation::StructuredQuery {
                expression: jq.expression.clone(),
            },
        ),
        Expectation::Logical(logical) => DocumentedExpectation::Composition {
            operator: match logical.operator() {
                LogicalOperator::Not => CompositionOperator::Not,
                LogicalOperator::All => CompositionOperator::All,
                LogicalOperator::Any => CompositionOperator::Any,
            },
            children: logical.children().iter().map(project_expectation).collect(),
        },
    }
}

fn project_output_matcher(matcher: &OutputMatcher) -> ObservedOperation {
    match matcher {
        OutputMatcher::Empty => ObservedOperation::IsEmpty,
        OutputMatcher::Contains(text) => ObservedOperation::Contains(project_expected_text(text)),
        OutputMatcher::NotContains(text) => ObservedOperation::DoesNotContain(ExpectedValue::Text(
            DocumentedText::Literal(text.clone()),
        )),
        OutputMatcher::Matches(pattern) => ObservedOperation::Matches {
            pattern: pattern.clone(),
        },
        OutputMatcher::ContentsEquals(reference) => {
            ObservedOperation::Is(project_file_contents(reference))
        }
        OutputMatcher::TextEquals(text) => ObservedOperation::Is(project_expected_text(text)),
    }
}

fn project_file_matcher(matcher: &FileMatcher) -> ObservedOperation {
    match matcher {
        FileMatcher::Exists => ObservedOperation::Exists,
        FileMatcher::NotExists => ObservedOperation::DoesNotExist,
        FileMatcher::Contains(text) => ObservedOperation::Contains(project_expected_text(text)),
        FileMatcher::Matches(pattern) => ObservedOperation::Matches {
            pattern: pattern.clone(),
        },
        FileMatcher::ContentsEquals(reference) => {
            ObservedOperation::Is(project_file_contents(reference))
        }
        FileMatcher::TextEquals(text) => ObservedOperation::Is(project_expected_text(text)),
    }
}

fn project_dir_matcher(matcher: &DirMatcher) -> ObservedOperation {
    match matcher {
        DirMatcher::Exists => ObservedOperation::Exists,
        DirMatcher::NotExists => ObservedOperation::DoesNotExist,
        // A directory entry name, not a substring: the `Dir` subject is what
        // tells a renderer which of the two `contains` means.
        DirMatcher::Contains(name) => {
            ObservedOperation::Contains(ExpectedValue::Text(DocumentedText::Literal(name.clone())))
        }
    }
}

fn project_expected_text(text: &TextValueExpression) -> ExpectedValue {
    ExpectedValue::Text(project_text(text))
}

/// Projects a compared-against file, keeping whether it belongs to the example.
///
/// A workspace path names a file in the case workspace — the same directory
/// the example's own `write` steps and commands act on. A fixture reference
/// resolves against the directory holding the `.repor` source instead (see
/// docs/reference/semantics.md — Fixture reference value), so it is invisible
/// to a reader following the example, and a renderer needs to know which it has.
fn project_file_contents(reference: &FileContentsReference) -> ExpectedValue {
    match reference {
        FileContentsReference::Workspace(path) => ExpectedValue::FileContents {
            path: path.as_str().to_string(),
            origin: FileContentsOrigin::Example,
        },
        FileContentsReference::Fixture(reference) => ExpectedValue::FileContents {
            path: reference.as_str().to_string(),
            origin: FileContentsOrigin::External,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;
    use rstest::rstest;
    use std::path::PathBuf;

    fn loaded(display_path: &str, source: &str) -> LoadedSourceFile {
        LoadedSourceFile {
            load_path: PathBuf::from(display_path),
            display_path: display_path.to_string(),
            source: parser::parse(source).expect("test source must parse"),
        }
    }

    fn only_example(source: &str) -> ProductExample {
        let mut catalog = build_product_catalog(&[loaded("a.repor", source)]);
        let mut file = catalog.groups.remove(0).files.remove(0);
        file.examples.remove(0)
    }

    fn only_expectations(source: &str) -> Vec<DocumentedExpectation> {
        match only_example(source).steps.pop() {
            Some(ExampleStep::Verification(verification)) => verification.expectations,
            other => panic!("expected a trailing verification step, got {other:?}"),
        }
    }

    fn literal(value: &str) -> DocumentedText {
        DocumentedText::Literal(value.to_string())
    }

    fn text(value: &str) -> ExpectedValue {
        ExpectedValue::Text(literal(value))
    }

    fn observation(
        subject: ObservedSubject,
        operation: ObservedOperation,
    ) -> DocumentedExpectation {
        DocumentedExpectation::Observation { subject, operation }
    }

    fn file_subject(path: &str) -> ObservedSubject {
        ObservedSubject::File {
            path: path.to_string(),
        }
    }

    fn file_contents(path: &str, origin: FileContentsOrigin) -> ExpectedValue {
        ExpectedValue::FileContents {
            path: path.to_string(),
            origin,
        }
    }

    /// The single expectation of a one-line `assert` body, projected.
    fn only_expectation(assert_body: &str) -> DocumentedExpectation {
        let mut expectations = only_expectations(&format!(
            "case \"c\" {{\n  $ run\n  assert {{\n    {assert_body}\n  }}\n}}\n"
        ));
        assert_eq!(expectations.len(), 1);
        expectations.remove(0)
    }

    /// The shape this projection exists for: a reader prepares a file, runs a
    /// command, and is told what the run is verified to have produced.
    #[test]
    fn a_write_command_assertion_case_becomes_file_command_and_verified_outcome() {
        let example = only_example(
            "document case {\n  title \"Initializing a project\"\n  description \"Creates the config.\"\n}\n\ncase \"init\" {\n  write <\"config.kdl\"> \"name \\\"demo\\\"\\n\"\n  $ demo init\n  assert {\n    exit 0\n    stdout contains \"created\"\n    file <\"config.kdl\"> exists\n  }\n}\n",
        );

        assert_eq!(example.title, "Initializing a project");
        assert_eq!(example.description.as_deref(), Some("Creates the config."));
        assert_eq!(
            example.steps,
            vec![
                ExampleStep::File(ExampleFile {
                    path: "config.kdl".to_string(),
                    content: literal("name \"demo\"\n"),
                    mode: None,
                }),
                ExampleStep::Command(ExampleCommand {
                    command: "demo init".to_string(),
                }),
                ExampleStep::Verification(ExampleVerification {
                    expectations: vec![
                        observation(
                            ObservedSubject::ExitCode,
                            ObservedOperation::Is(ExpectedValue::Number(0)),
                        ),
                        observation(
                            ObservedSubject::Stdout,
                            ObservedOperation::Contains(text("created"))
                        ),
                        observation(
                            ObservedSubject::File {
                                path: "config.kdl".to_string(),
                            },
                            ObservedOperation::Exists,
                        ),
                    ],
                }),
            ]
        );
    }

    /// A reportage assertion verifies the current checkpoint, not the action
    /// before it, so the projection must keep the interleaving rather than
    /// collapse it into commands and results: two commands followed by one
    /// assertion stay exactly that, and a second checkpoint stays separate.
    #[test]
    fn multiple_commands_and_checkpoints_keep_their_source_order() {
        let example = only_example(
            "case \"multi\" {\n  $ first\n  $ second\n  assert {\n    exit 0\n  }\n  $ third\n  assert {\n    stderr empty\n  }\n}\n",
        );

        let shape: Vec<&str> = example
            .steps
            .iter()
            .map(|step| match step {
                ExampleStep::File(_) => "file",
                ExampleStep::Command(command) => command.command.as_str(),
                ExampleStep::Verification(_) => "verification",
            })
            .collect();
        assert_eq!(
            shape,
            vec!["first", "second", "verification", "third", "verification"]
        );
    }

    /// `before_each` reaches every example as preparation, and its own step
    /// order is preserved for the same reason a case body's is.
    #[test]
    fn before_each_becomes_each_examples_preparation() {
        let catalog = build_product_catalog(&[loaded(
            "a.repor",
            "before_each {\n  write <\"seed.txt\"> \"seed\"\n  $ setup\n}\n\ncase \"one\" {\n  $ run\n  assert {\n    exit 0\n  }\n}\n\ncase \"two\" {\n  $ run\n  assert {\n    exit 0\n  }\n}\n",
        )]);

        let examples = &catalog.groups[0].files[0].examples;
        assert_eq!(examples.len(), 2);
        for example in examples {
            assert_eq!(
                example.preparation,
                vec![
                    ExampleStep::File(ExampleFile {
                        path: "seed.txt".to_string(),
                        content: literal("seed"),
                        mode: None,
                    }),
                    ExampleStep::Command(ExampleCommand {
                        command: "setup".to_string(),
                    }),
                ]
            );
        }
    }

    /// `before_each` accepts the whole case-body step surface, so preparation
    /// projects through the same rules: a setup `assert` is a verification
    /// checkpoint inside the preparation, and a setup binding is invisible on
    /// its own while still naming the value a later case-body step uses.
    #[test]
    fn setup_projects_the_whole_case_body_step_surface_including_across_phases() {
        let example = only_example(
            "before_each {\n  write <\"seed.txt\"> \"seed\"\n  $ setup\n  assert {\n    exit 0\n  }\n  let id <- stdout_line\n}\n\ncase \"one\" {\n  write <\"out.txt\"> &id\n  $ run\n  assert {\n    exit 0\n  }\n}\n",
        );

        assert_eq!(
            example.preparation,
            vec![
                ExampleStep::File(ExampleFile {
                    path: "seed.txt".to_string(),
                    content: literal("seed"),
                    mode: None,
                }),
                ExampleStep::Command(ExampleCommand {
                    command: "setup".to_string(),
                }),
                ExampleStep::Verification(ExampleVerification {
                    expectations: vec![observation(
                        ObservedSubject::ExitCode,
                        ObservedOperation::Is(ExpectedValue::Number(0)),
                    )],
                }),
            ]
        );
        assert_eq!(
            example.steps[0],
            ExampleStep::File(ExampleFile {
                path: "out.txt".to_string(),
                content: DocumentedText::Composed(vec![TextSegment::Captured {
                    binding: "id".to_string(),
                }]),
                mode: None,
            })
        );
    }

    /// A file whose only content is setup produces no example, so there is
    /// nothing to prepare for and no preparation is emitted. The file itself
    /// still appears, under its metadata.
    #[test]
    fn a_source_with_setup_but_no_case_documents_the_file_without_examples() {
        let catalog = build_product_catalog(&[loaded(
            "setup-only.repor",
            "before_each {\n  write <\"seed.txt\"> \"seed\"\n}\n",
        )]);

        let file = &catalog.groups[0].files[0];
        assert_eq!(file.title, "setup-only");
        assert!(file.examples.is_empty());
    }

    /// A `write` naming a mode keeps it: a reader recreating an executable
    /// fixture without its permission bits would not reproduce the example.
    #[test]
    fn a_write_mode_reaches_the_file_example() {
        let example = only_example(
            "case \"mode\" {\n  write <\"bin/run\"> mode=0o755 \"#!/bin/sh\\n\"\n  $ ./bin/run\n  assert {\n    exit 0\n  }\n}\n",
        );

        assert_eq!(
            example.steps[0],
            ExampleStep::File(ExampleFile {
                path: "bin/run".to_string(),
                content: literal("#!/bin/sh\n"),
                mode: Some(0o755),
            })
        );
    }

    /// A binding declaration produces no step of its own — it is reportage
    /// plumbing, not something a reader does — but the value it captures stays
    /// visible where it is used, named by the binding it came from, instead of
    /// the step being dropped silently.
    #[test]
    fn a_captured_binding_leaves_the_value_named_rather_than_the_step_dropped() {
        let example = only_example(
            "case \"capture\" {\n  $ emit\n  let id <- stdout_line\n  write <\"out.txt\"> &id\n  assert {\n    file <\"out.txt\"> contains &id\n  }\n}\n",
        );

        let captured = || {
            DocumentedText::Composed(vec![TextSegment::Captured {
                binding: "id".to_string(),
            }])
        };
        assert_eq!(
            example.steps,
            vec![
                ExampleStep::Command(ExampleCommand {
                    command: "emit".to_string(),
                }),
                ExampleStep::File(ExampleFile {
                    path: "out.txt".to_string(),
                    content: captured(),
                    mode: None,
                }),
                ExampleStep::Verification(ExampleVerification {
                    expectations: vec![observation(
                        file_subject("out.txt"),
                        ObservedOperation::Contains(ExpectedValue::Text(captured())),
                    )],
                }),
            ]
        );
    }

    /// An interpolated value is mostly literal text in practice, so the
    /// literal parts must survive: collapsing the whole value into "captured"
    /// would delete the input example a reader came for.
    #[test]
    fn interpolated_content_keeps_its_literal_parts_around_the_captured_value() {
        let example = only_example(
            "case \"compose\" {\n  $ emit\n  let id <- stdout_line\n  write <\"config.kdl\"> &\"name \\\"&{id}\\\"\\nversion 1\\n\"\n  assert {\n    file <\"config.kdl\"> exists\n  }\n}\n",
        );

        assert_eq!(
            example.steps[1],
            ExampleStep::File(ExampleFile {
                path: "config.kdl".to_string(),
                content: DocumentedText::Composed(vec![
                    TextSegment::Literal("name \"".to_string()),
                    TextSegment::Captured {
                        binding: "id".to_string(),
                    },
                    TextSegment::Literal("\"\nversion 1\n".to_string()),
                ]),
                mode: None,
            })
        );
    }

    /// An interpolated literal that references nothing states its text in
    /// full, so it documents as a literal: there is no captured value to name.
    #[test]
    fn a_reference_free_interpolated_literal_documents_as_literal_text() {
        let example = only_example(
            "case \"plain\" {\n  write <\"note.txt\"> &\"just text\\n\"\n  $ run\n  assert {\n    exit 0\n  }\n}\n",
        );

        assert_eq!(
            example.steps[0],
            ExampleStep::File(ExampleFile {
                path: "note.txt".to_string(),
                content: literal("just text\n"),
                mode: None,
            })
        );
    }

    /// `contains` means a substring on a file and an entry name on a
    /// directory. The projection keeps both as `Contains` and lets the subject
    /// carry the difference, which is why subject and operation are modeled as
    /// a pair rather than as one flattened phrase.
    #[test]
    fn the_same_operation_on_different_subjects_stays_distinguishable() {
        let expectations = only_expectations(
            "case \"subjects\" {\n  $ run\n  assert {\n    file <\"out.txt\"> contains \"x\"\n    dir <\"out\"> contains \"x\"\n  }\n}\n",
        );

        assert_eq!(
            expectations,
            vec![
                observation(
                    ObservedSubject::File {
                        path: "out.txt".to_string(),
                    },
                    ObservedOperation::Contains(text("x")),
                ),
                observation(
                    ObservedSubject::Dir {
                        path: "out".to_string(),
                    },
                    ObservedOperation::Contains(text("x")),
                ),
            ]
        );
    }

    /// A logical composition keeps its nesting: `not { a b }` negates the two
    /// together, and a projection that flattened it would state a different
    /// condition.
    #[test]
    fn logical_composition_keeps_its_operator_and_nesting() {
        let expectations = only_expectations(
            "case \"logic\" {\n  $ run\n  assert {\n    not {\n      any {\n        exit 1\n        stderr empty\n      }\n    }\n  }\n}\n",
        );

        assert_eq!(
            expectations,
            vec![DocumentedExpectation::Composition {
                operator: CompositionOperator::Not,
                children: vec![DocumentedExpectation::Composition {
                    operator: CompositionOperator::Any,
                    children: vec![
                        observation(
                            ObservedSubject::ExitCode,
                            ObservedOperation::Is(ExpectedValue::Number(1)),
                        ),
                        observation(ObservedSubject::Stderr, ObservedOperation::IsEmpty),
                    ],
                }],
            }]
        );
    }

    /// Every expectation form the v0 grammar accepts, mapped one by one.
    ///
    /// The compiler enforces that each matcher variant is handled; it cannot
    /// tell `Contains` from `DoesNotContain`, and a swapped arm would produce
    /// a silently wrong sentence in generated documentation. Only a per-form
    /// assertion catches that.
    #[rstest]
    #[case::exit(
        "exit 0",
        observation(
            ObservedSubject::ExitCode,
            ObservedOperation::Is(ExpectedValue::Number(0))
        )
    )]
    #[case::stdout_empty(
        "stdout empty",
        observation(ObservedSubject::Stdout, ObservedOperation::IsEmpty)
    )]
    #[case::stdout_contains(
        "stdout contains \"created\"",
        observation(ObservedSubject::Stdout, ObservedOperation::Contains(text("created")))
    )]
    #[case::stdout_text_equals(
        "stdout text_equals \"done\"",
        observation(ObservedSubject::Stdout, ObservedOperation::Is(text("done")))
    )]
    #[case::stdout_contents_equals_workspace_path(
        "stdout contents_equals <\"expected.txt\">",
        observation(
            ObservedSubject::Stdout,
            ObservedOperation::Is(file_contents("expected.txt", FileContentsOrigin::Example))
        )
    )]
    #[case::stderr_empty(
        "stderr empty",
        observation(ObservedSubject::Stderr, ObservedOperation::IsEmpty)
    )]
    #[case::stderr_contains(
        "stderr contains \"warning\"",
        observation(ObservedSubject::Stderr, ObservedOperation::Contains(text("warning")))
    )]
    #[case::file_exists(
        "file <\"a.txt\"> exists",
        observation(file_subject("a.txt"), ObservedOperation::Exists)
    )]
    #[case::file_contains(
        "file <\"a.txt\"> contains \"x\"",
        observation(file_subject("a.txt"), ObservedOperation::Contains(text("x")))
    )]
    #[case::file_text_equals(
        "file <\"a.txt\"> text_equals \"x\"",
        observation(file_subject("a.txt"), ObservedOperation::Is(text("x")))
    )]
    #[case::file_contents_equals_workspace_path(
        "file <\"a.txt\"> contents_equals <\"expected.txt\">",
        observation(
            file_subject("a.txt"),
            ObservedOperation::Is(file_contents("expected.txt", FileContentsOrigin::Example))
        )
    )]
    #[case::file_contents_equals_fixture(
        "file <\"a.txt\"> contents_equals @\"expected.txt\"",
        observation(
            file_subject("a.txt"),
            ObservedOperation::Is(file_contents("expected.txt", FileContentsOrigin::External))
        )
    )]
    #[case::dir_exists(
        "dir <\"out\"> exists",
        observation(ObservedSubject::Dir { path: "out".to_string() }, ObservedOperation::Exists)
    )]
    #[case::dir_contains(
        "dir <\"out\"> contains \"entry\"",
        observation(ObservedSubject::Dir { path: "out".to_string() }, ObservedOperation::Contains(text("entry")))
    )]
    #[case::composition_all(
        "all { exit 0 }",
        DocumentedExpectation::Composition {
            operator: CompositionOperator::All,
            children: vec![observation(
                ObservedSubject::ExitCode,
                ObservedOperation::Is(ExpectedValue::Number(0)),
            )],
        }
    )]
    fn each_v0_expectation_form_projects_to_its_subject_and_operation(
        #[case] assert_body: &str,
        #[case] expected: DocumentedExpectation,
    ) {
        assert_eq!(only_expectation(assert_body), expected);
    }

    /// The expectation model defines matchers the v0 grammar cannot produce.
    /// No `.repor` source reaches them, so no parsing test — and no future
    /// e2e test — can check their mapping; asserting on the projection
    /// functions directly is the only place a swapped arm would be caught.
    #[test]
    fn matchers_without_v0_syntax_project_to_their_operation() {
        use crate::model::{FileCountExpectation, JqExpectation};

        assert_eq!(
            project_output_matcher(&OutputMatcher::NotContains("x".to_string())),
            ObservedOperation::DoesNotContain(text("x"))
        );
        assert_eq!(
            project_output_matcher(&OutputMatcher::Matches("^ok$".to_string())),
            ObservedOperation::Matches {
                pattern: "^ok$".to_string()
            }
        );
        assert_eq!(
            project_file_matcher(&FileMatcher::NotExists),
            ObservedOperation::DoesNotExist
        );
        assert_eq!(
            project_file_matcher(&FileMatcher::Matches("^ok$".to_string())),
            ObservedOperation::Matches {
                pattern: "^ok$".to_string()
            }
        );
        assert_eq!(
            project_dir_matcher(&DirMatcher::NotExists),
            ObservedOperation::DoesNotExist
        );

        assert_eq!(
            project_expectation(&Expectation::FileCount(FileCountExpectation {
                glob: "out/*.txt".to_string(),
                op: CountOp::Gte,
                count: 2,
            })),
            observation(
                ObservedSubject::FileCount {
                    glob: "out/*.txt".to_string()
                },
                ObservedOperation::CountIs {
                    comparison: CountComparison::AtLeast,
                    count: 2,
                },
            )
        );
        assert_eq!(
            project_expectation(&Expectation::FileCount(FileCountExpectation {
                glob: "out/*.txt".to_string(),
                op: CountOp::Eq,
                count: 1,
            })),
            observation(
                ObservedSubject::FileCount {
                    glob: "out/*.txt".to_string()
                },
                ObservedOperation::CountIs {
                    comparison: CountComparison::Exactly,
                    count: 1,
                },
            )
        );
        assert_eq!(
            project_expectation(&Expectation::Jq(JqExpectation {
                source: OutputSource::Stderr,
                expression: ".error".to_string(),
            })),
            observation(
                ObservedSubject::Stderr,
                ObservedOperation::StructuredQuery {
                    expression: ".error".to_string()
                },
            )
        );
    }

    /// Byte-for-byte comparison against another file keeps the file's identity
    /// rather than inlining unknown contents, and keeps whether that file
    /// belongs to the example: a workspace path names a file the example
    /// itself has, while a fixture reference names one kept beside the
    /// `.repor` source that a reader never sees.
    #[test]
    fn contents_equals_documents_the_file_it_compares_against_and_where_it_lives() {
        let expectations = only_expectations(
            "case \"contents\" {\n  $ run\n  assert {\n    file <\"out.txt\"> contents_equals <\"expected.txt\">\n    stdout contents_equals @\"expected-stdout.txt\"\n  }\n}\n",
        );

        assert_eq!(
            expectations,
            vec![
                observation(
                    file_subject("out.txt"),
                    ObservedOperation::Is(file_contents(
                        "expected.txt",
                        FileContentsOrigin::Example
                    )),
                ),
                observation(
                    ObservedSubject::Stdout,
                    ObservedOperation::Is(file_contents(
                        "expected-stdout.txt",
                        FileContentsOrigin::External
                    )),
                ),
            ]
        );
    }

    /// A smoke check over one representative source, not a proof: the real
    /// guarantee is structural — no field of the catalog has a source-text
    /// type — and only reviewing a newly added field can enforce that. Note
    /// that this check reads written file content too, so a source whose
    /// `write` content legitimately spells one of these wrappers would fail it
    /// for a correct catalog.
    #[test]
    fn the_catalog_carries_no_reportage_source_text() {
        let source = "before_each {\n  write <\"seed.txt\"> \"seed\"\n}\n\ncase \"init\" {\n  $ demo init\n  assert {\n    exit 0\n  }\n}\n";
        let catalog = build_product_catalog(&[loaded("a.repor", source)]);

        let debug = format!("{catalog:?}");
        for wrapper in ["case \"", "assert {", "before_each {", "write <"] {
            assert!(
                !debug.contains(wrapper),
                "product catalog must not carry the {wrapper:?} source wrapper"
            );
        }
    }

    /// Examples follow case order, and files follow the shared metadata
    /// ordering contract rather than a second one defined here.
    #[test]
    fn examples_follow_case_order_under_the_shared_file_ordering() {
        let catalog = build_product_catalog(&[
            loaded(
                "second.repor",
                "document file {\n  group \"Guides\"\n  order 2\n}\n\ncase \"only\" {\n  $ run\n  assert {\n    exit 0\n  }\n}\n",
            ),
            loaded(
                "first.repor",
                "document file {\n  group \"Guides\"\n  order 1\n}\n\ncase \"b\" {\n  $ run\n  assert {\n    exit 0\n  }\n}\n\ncase \"a\" {\n  $ run\n  assert {\n    exit 0\n  }\n}\n",
            ),
        ]);

        let paths: Vec<_> = catalog.groups[0]
            .files
            .iter()
            .map(|f| f.source_path.as_str())
            .collect();
        assert_eq!(paths, vec!["first.repor", "second.repor"]);

        let titles: Vec<_> = catalog.groups[0].files[0]
            .examples
            .iter()
            .map(|e| e.title.as_str())
            .collect();
        assert_eq!(titles, vec!["b", "a"]);
    }
}
