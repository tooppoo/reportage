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
//! Two properties are load-bearing and easy to lose:
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
//!
//! Like the Reportage-source Catalog, this API exposes only plain values, so
//! renderers never depend on parser or execution-model types. Display
//! fallbacks and ordering come from [`super::metadata`], shared with that
//! Catalog because both read the same `document` blocks.

use crate::model::{
    AssertionBlock, BeforeEach, CountOp, DirMatcher, Expectation, FileContentsReference,
    FileMatcher, LogicalOperator, OutputMatcher, OutputSource, SideEffectingStep, Step,
    TextValueExpression, WriteFileStep,
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
    pub content: ExampleContent,
    /// The POSIX permission bits the file ends up with, when the source names
    /// them. Kept because a reader who recreates the file needs them — an
    /// example whose file must be executable is wrong without them.
    pub mode: Option<u32>,
}

/// Content stated by the source, or produced only while the example runs.
#[derive(Debug, PartialEq, Eq)]
pub enum ExampleContent {
    /// Text the source states in full.
    Text(String),
    /// Content assembled from values captured during the run, so the source
    /// holds no complete text for it. Represented rather than dropped: the
    /// file is still part of the example.
    Captured,
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
    Text(String),
    /// A whole number stated by the source, such as an exit code.
    Number(u64),
    /// The contents of another file, named by its path.
    FileContents {
        path: String,
    },
    /// A value captured during the run, so the source holds no literal for it.
    Captured,
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
/// used, as [`ExampleContent::Captured`] or [`ExpectedValue::Captured`].
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
        content: project_content(&write.content),
        mode: write.mode.map(|mode| mode.bits()),
    }
}

fn project_content(content: &TextValueExpression) -> ExampleContent {
    match content.binding_free_text_value() {
        Some(text) => ExampleContent::Text(text.as_str().to_string()),
        None => ExampleContent::Captured,
    }
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
        OutputMatcher::NotContains(text) => {
            ObservedOperation::DoesNotContain(ExpectedValue::Text(text.clone()))
        }
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
            ObservedOperation::Contains(ExpectedValue::Text(name.clone()))
        }
    }
}

fn project_expected_text(text: &TextValueExpression) -> ExpectedValue {
    match text.binding_free_text_value() {
        Some(value) => ExpectedValue::Text(value.as_str().to_string()),
        None => ExpectedValue::Captured,
    }
}

fn project_file_contents(reference: &FileContentsReference) -> ExpectedValue {
    match reference {
        FileContentsReference::Workspace(path) => ExpectedValue::FileContents {
            path: path.as_str().to_string(),
        },
        FileContentsReference::Fixture(reference) => ExpectedValue::FileContents {
            path: reference.as_str().to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;
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

    fn text(value: &str) -> ExpectedValue {
        ExpectedValue::Text(value.to_string())
    }

    fn observation(
        subject: ObservedSubject,
        operation: ObservedOperation,
    ) -> DocumentedExpectation {
        DocumentedExpectation::Observation { subject, operation }
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
                    content: ExampleContent::Text("name \"demo\"\n".to_string()),
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
                        content: ExampleContent::Text("seed".to_string()),
                        mode: None,
                    }),
                    ExampleStep::Command(ExampleCommand {
                        command: "setup".to_string(),
                    }),
                ]
            );
        }
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
                content: ExampleContent::Text("#!/bin/sh\n".to_string()),
                mode: Some(0o755),
            })
        );
    }

    /// A binding declaration produces no step of its own — it is reportage
    /// plumbing, not something a reader does — but the value it captures stays
    /// visible where it is used, instead of the step being dropped silently.
    #[test]
    fn a_captured_binding_leaves_the_value_marked_rather_than_the_step_dropped() {
        let example = only_example(
            "case \"capture\" {\n  $ emit\n  let id <- stdout_line\n  write <\"out.txt\"> &id\n  assert {\n    file <\"out.txt\"> contains &id\n  }\n}\n",
        );

        assert_eq!(
            example.steps,
            vec![
                ExampleStep::Command(ExampleCommand {
                    command: "emit".to_string(),
                }),
                ExampleStep::File(ExampleFile {
                    path: "out.txt".to_string(),
                    content: ExampleContent::Captured,
                    mode: None,
                }),
                ExampleStep::Verification(ExampleVerification {
                    expectations: vec![observation(
                        ObservedSubject::File {
                            path: "out.txt".to_string(),
                        },
                        ObservedOperation::Contains(ExpectedValue::Captured),
                    )],
                }),
            ]
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

    /// Byte-for-byte comparison against another file keeps the file's identity
    /// rather than inlining unknown contents, for a workspace path and a
    /// fixture reference alike.
    #[test]
    fn contents_equals_documents_the_file_it_compares_against() {
        let expectations = only_expectations(
            "case \"contents\" {\n  $ run\n  assert {\n    file <\"out.txt\"> contents_equals <\"expected.txt\">\n    stdout contents_equals @\"expected-stdout.txt\"\n  }\n}\n",
        );

        assert_eq!(
            expectations,
            vec![
                observation(
                    ObservedSubject::File {
                        path: "out.txt".to_string(),
                    },
                    ObservedOperation::Is(ExpectedValue::FileContents {
                        path: "expected.txt".to_string(),
                    }),
                ),
                observation(
                    ObservedSubject::Stdout,
                    ObservedOperation::Is(ExpectedValue::FileContents {
                        path: "expected-stdout.txt".to_string(),
                    }),
                ),
            ]
        );
    }

    /// The projection carries no `.repor` source text at all: there is no
    /// field a renderer could accidentally print DSL from. Asserting it on the
    /// built model — rather than on rendered output — is what keeps a later
    /// renderer from having the option.
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
