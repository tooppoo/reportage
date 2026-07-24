use std::path::Path;

use super::{Checkpoint, observation};
use crate::diagnostic::DiagnosticCode;
use crate::fixture;
use crate::model::{
    DirExpectation, DirMatcher, Expectation, FileContentsReference, FileExpectation, FileMatcher,
    LogicalOperator, OutputMatcher, TextLiteral,
};
use crate::result::{
    ContentsEqualsComparison, ContentsEqualsExpectedSource, ContentsEqualsObservation,
    ContentsEqualsOutcome, DirContainsObservation, DirExistsObservation, ExpectationKind,
    ExpectationResult, FileContentObservation, FileExistsObservation, TextEqualsExpectedSource,
};
use crate::semantic::{
    SemanticError, validate_dir_entry_name, validate_dir_path, validate_file_path,
};

/// A `contents_equals` expected value could not be resolved to bytes: not a subject-under-test
/// failure but a problem with the test definition itself (a missing / non-regular / unreadable
/// expected `WorkspacePath`, or any `FixtureReference` resolution failure). Surfaces as
/// `CaseStatus::ScriptError` (exit code 2), never as an assertion failure. See
/// docs/adr/20260707T012055Z_contents-equals-evaluation.md.
#[derive(Debug)]
pub struct ExpectedContentsError {
    pub message: String,
    pub diagnostic_code: DiagnosticCode,
}

/// Resolves a `contents_equals` expected [`FileContentsReference`] to its bytes and a
/// display-only [`ContentsEqualsExpectedSource`].
///
/// A `Workspace` reference reads directly from the case's isolated workspace; a `Fixture`
/// reference is resolved against `repor_dir` and materialized into a fresh runner-reserved
/// directory before being read, so evaluation never reads directly from the test-definition
/// source tree. See `fixture::resolve_fixture_source` / `fixture::materialize_fixture`.
fn resolve_expected_contents(
    expected: &FileContentsReference,
    workspace_root: &Path,
    repor_dir: &Path,
) -> Result<(Vec<u8>, ContentsEqualsExpectedSource), ExpectedContentsError> {
    match expected {
        FileContentsReference::Workspace(path) => {
            let resolved = workspace_root.join(path.as_str());
            let meta = std::fs::metadata(&resolved).map_err(|_| ExpectedContentsError {
                message: format!("expected workspace path {:?} does not exist", path.as_str()),
                diagnostic_code: DiagnosticCode::SemanticFileContentsReferenceMissing,
            })?;
            if !meta.is_file() {
                return Err(ExpectedContentsError {
                    message: format!(
                        "expected workspace path {:?} is not a regular file",
                        path.as_str()
                    ),
                    diagnostic_code: DiagnosticCode::SemanticFileContentsReferenceNotARegularFile,
                });
            }
            let bytes = std::fs::read(&resolved).map_err(|e| ExpectedContentsError {
                message: format!(
                    "expected workspace path {:?} could not be read: {e}",
                    path.as_str()
                ),
                diagnostic_code: DiagnosticCode::SemanticFileContentsReferenceReadError,
            })?;
            Ok((
                bytes,
                ContentsEqualsExpectedSource::Workspace(path.as_str().to_string()),
            ))
        }
        FileContentsReference::Fixture(fixture_ref) => {
            let resolved =
                fixture::resolve_fixture_source(repor_dir, fixture_ref).map_err(|e| {
                    ExpectedContentsError {
                        message: e.to_string(),
                        diagnostic_code: e.code(),
                    }
                })?;
            let reserved_dir = tempfile::TempDir::new().map_err(|e| ExpectedContentsError {
                message: format!("failed to create fixture materialization directory: {e}"),
                diagnostic_code: DiagnosticCode::SemanticFixtureReferenceMissing,
            })?;
            let materialized = fixture::materialize_fixture(&resolved, reserved_dir.path())
                .map_err(|e| ExpectedContentsError {
                    message: format!(
                        "fixture reference {:?} could not be materialized: {e}",
                        fixture_ref.as_str()
                    ),
                    diagnostic_code: DiagnosticCode::SemanticFixtureReferenceMissing,
                })?;
            let bytes = std::fs::read(&materialized).map_err(|e| ExpectedContentsError {
                message: format!(
                    "fixture reference {:?} could not be read after materialization: {e}",
                    fixture_ref.as_str()
                ),
                diagnostic_code: DiagnosticCode::SemanticFixtureReferenceMissing,
            })?;
            Ok((
                bytes,
                ContentsEqualsExpectedSource::Fixture(fixture_ref.as_str().to_string()),
            ))
        }
    }
}

/// Validates every `file` / `dir` subject path (and `dir` `contains` entry name) reachable from
/// `expectation`, recursing into `not` / `all` / `any` children.
///
/// A logical composition combines assertion *outcomes*; it must never let an invalid path or
/// entry name bypass semantic validation just because it is nested inside `not { ... }` — that
/// would let a path escape the case workspace sandbox (e.g. `not { dir <"../../etc"> exists }`)
/// while merely looking like an ordinary assertion failure.
/// See docs/adr/20260704T112155Z_subject-first-file-assertion-syntax.md and
/// docs/adr/20260706T000000Z_subject-first-directory-assertion-syntax.md.
pub(super) fn validate_expectation_paths(expectation: &Expectation) -> Result<(), SemanticError> {
    match expectation {
        Expectation::File(file_exp) => validate_file_path(&file_exp.path),
        Expectation::Dir(dir_exp) => {
            validate_dir_path(&dir_exp.path)?;
            if let DirMatcher::Contains(name) = &dir_exp.matcher {
                validate_dir_entry_name(name)?;
            }
            Ok(())
        }
        Expectation::Logical(l) => {
            for child in l.children() {
                validate_expectation_paths(child)?;
            }
            Ok(())
        }
        Expectation::Exit(_) | Expectation::Stdout(_) | Expectation::Stderr(_) => Ok(()),
        _ => unreachable!("expectation variant not implemented in v0 parser"),
    }
}

/// Evaluate one expectation against the current checkpoint.
///
/// This is the checkpoint-level semantic evaluator used by normal case execution.
/// Semantic conformance tests call the same entry point with static JSON checkpoint fixtures so those specs validate the production evaluator behavior without running an external command.
///
/// Returns `Err` only when a `contents_equals` expected value fails to resolve to bytes — a
/// test-definition problem, not an assertion outcome. See `evaluate_case`'s caller, which turns
/// that into `CaseStatus::ScriptError` and aborts the case, exactly like the path-policy checks
/// that run before this function is ever called.
pub fn evaluate_expectation_at_checkpoint(
    expectation: &Expectation,
    checkpoint: &Checkpoint,
) -> Result<ExpectationResult, ExpectedContentsError> {
    match expectation {
        Expectation::Exit(exp) => {
            let actual = checkpoint
                .last_action
                .as_ref()
                .map(|a| a.exit_code)
                .unwrap_or(-1);
            let passed = actual == exp.expected as i32;
            Ok(ExpectationResult {
                kind: ExpectationKind::Exit {
                    expected: exp.expected,
                    actual,
                },
                passed,
            })
        }
        Expectation::Stdout(exp) => {
            let actual = checkpoint
                .last_action
                .as_ref()
                .map(|a| a.stdout.clone())
                .unwrap_or_default();
            match &exp.matcher {
                OutputMatcher::Contains(expected) => {
                    let passed = bytes_contains(&actual, expected.as_bytes());
                    Ok(ExpectationResult {
                        kind: ExpectationKind::StdoutContains {
                            expected: expected.clone(),
                            actual,
                        },
                        passed,
                    })
                }
                OutputMatcher::Empty => {
                    let passed = actual.is_empty();
                    Ok(ExpectationResult {
                        kind: ExpectationKind::StdoutEmpty { actual },
                        passed,
                    })
                }
                OutputMatcher::ContentsEquals(expected_ref) => {
                    let (expected_bytes, expected_source) = resolve_expected_contents(
                        expected_ref,
                        &checkpoint.workspace.root,
                        &checkpoint.repor_dir,
                    )?;
                    let comparison = ContentsEqualsComparison::compare(actual, expected_bytes);
                    let passed = matches!(comparison.outcome, ContentsEqualsOutcome::Match);
                    Ok(ExpectationResult {
                        kind: ExpectationKind::StdoutContentsEquals {
                            expected_source,
                            comparison,
                        },
                        passed,
                    })
                }
                OutputMatcher::TextEquals(text_literal) => {
                    let (expected_source, comparison) =
                        compare_output_text_equals(text_literal, actual);
                    let passed = matches!(comparison.outcome, ContentsEqualsOutcome::Match);
                    Ok(ExpectationResult {
                        kind: ExpectationKind::StdoutTextEquals {
                            expected_source,
                            comparison,
                        },
                        passed,
                    })
                }
                _ => unreachable!("output matcher variant not implemented in v0 evaluator"),
            }
        }
        Expectation::Stderr(exp) => {
            let actual = checkpoint
                .last_action
                .as_ref()
                .map(|a| a.stderr.clone())
                .unwrap_or_default();
            match &exp.matcher {
                OutputMatcher::Contains(expected) => {
                    let passed = bytes_contains(&actual, expected.as_bytes());
                    Ok(ExpectationResult {
                        kind: ExpectationKind::StderrContains {
                            expected: expected.clone(),
                            actual,
                        },
                        passed,
                    })
                }
                OutputMatcher::Empty => {
                    let passed = actual.is_empty();
                    Ok(ExpectationResult {
                        kind: ExpectationKind::StderrEmpty { actual },
                        passed,
                    })
                }
                OutputMatcher::ContentsEquals(expected_ref) => {
                    let (expected_bytes, expected_source) = resolve_expected_contents(
                        expected_ref,
                        &checkpoint.workspace.root,
                        &checkpoint.repor_dir,
                    )?;
                    let comparison = ContentsEqualsComparison::compare(actual, expected_bytes);
                    let passed = matches!(comparison.outcome, ContentsEqualsOutcome::Match);
                    Ok(ExpectationResult {
                        kind: ExpectationKind::StderrContentsEquals {
                            expected_source,
                            comparison,
                        },
                        passed,
                    })
                }
                OutputMatcher::TextEquals(text_literal) => {
                    let (expected_source, comparison) =
                        compare_output_text_equals(text_literal, actual);
                    let passed = matches!(comparison.outcome, ContentsEqualsOutcome::Match);
                    Ok(ExpectationResult {
                        kind: ExpectationKind::StderrTextEquals {
                            expected_source,
                            comparison,
                        },
                        passed,
                    })
                }
                _ => unreachable!("output matcher variant not implemented in v0 evaluator"),
            }
        }
        Expectation::File(exp) => {
            evaluate_file_expectation(exp, &checkpoint.workspace.root, &checkpoint.repor_dir)
        }
        Expectation::Dir(exp) => Ok(evaluate_dir_expectation(exp, &checkpoint.workspace.root)),
        Expectation::Logical(l) => {
            // Evaluate every child regardless of earlier results, so a failing composition still
            // reports each child's own outcome. A child whose `contents_equals` expected value
            // fails to resolve short-circuits the whole composition as a script error, the same
            // as a bare (non-composed) expectation — a composition combines assertion outcomes,
            // it must not mask a test-definition problem in one of its children.
            // See docs/reference/semantics.md — Logical composition.
            let children: Vec<ExpectationResult> = l
                .children()
                .iter()
                .map(|child| evaluate_expectation_at_checkpoint(child, checkpoint))
                .collect::<Result<Vec<_>, _>>()?;

            // `not` negates the implicit-`all` grouping of its children, not each child individually: `not { A B }` is `not(all(A, B))`, never `not(A) and not(B)`.
            let all_children_passed = children.iter().all(|c| c.passed);
            let passed = match l.operator() {
                LogicalOperator::All => all_children_passed,
                LogicalOperator::Any => children.iter().any(|c| c.passed),
                LogicalOperator::Not => !all_children_passed,
            };

            Ok(ExpectationResult {
                kind: ExpectationKind::Logical {
                    operator: l.operator(),
                    children,
                },
                passed,
            })
        }
        _ => unreachable!("expectation variant not implemented in v0 evaluator"),
    }
}

/// Byte-level substring search, per docs/reference/semantics.md's raw byte semantics for `stdout contains` /
/// `stderr contains`: `expected` is UTF-8 bytes of a Reportage string literal, matched against
/// `haystack` (raw process output bytes) without any decoding on either side.
///
/// An empty `needle` always matches, including against an empty `haystack`
/// (`emptyExpectedAlwaysMatches`, pinned by `spec/language/semantics/assertion.stdout.contains.json`
/// and `assertion.stderr.contains.json`).
fn bytes_contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty() || haystack.windows(needle.len()).any(|w| w == needle)
}

/// Compares captured stream bytes against a `text_equals` expected `TextLiteral`.
///
/// The expected side is resolved to its `TextValue`'s UTF-8 bytes and compared byte-for-byte,
/// exactly like `FileMatcher::TextEquals` (see `evaluate_file_expectation`). Unlike a `file`
/// subject there is no actual-side observation to classify: a captured stream always exists
/// (possibly empty), so the comparison outcome is the whole result.
/// See docs/adr — output text_equals evaluation.
fn compare_output_text_equals(
    text_literal: &TextLiteral,
    actual: Vec<u8>,
) -> (TextEqualsExpectedSource, ContentsEqualsComparison) {
    let expected_source = match text_literal {
        TextLiteral::Quoted(value) => TextEqualsExpectedSource::Quoted(value.clone()),
        TextLiteral::Heredoc(value) => TextEqualsExpectedSource::Heredoc(value.clone()),
    };
    let expected_bytes = text_literal.to_text_value().as_str().as_bytes().to_vec();
    let comparison = ContentsEqualsComparison::compare(actual, expected_bytes);
    (expected_source, comparison)
}

/// Evaluates a `file <"path"> ...` expectation against the real filesystem.
///
/// The path policy (relative, no `.`/`..` segments) is checked earlier, in `evaluate_case`, before this function runs.
/// By the time this function is called, `exp.path` is known to be policy-valid.
///
/// `exp.path` is resolved relative to `workspace_root`, the current concrete case's isolated workspace.
/// Actions never change that directory for the process (each `$` step runs in a fresh child shell), so file assertion paths are always resolved relative to the case workspace root, never affected by a `cd` performed inside an action.
/// See docs/reference/semantics.md.
///
/// `repor_dir` is only consulted for `ContentsEquals`, to resolve an expected
/// `FixtureReference` relative to the referencing `*.repor` file's directory.
fn evaluate_file_expectation(
    exp: &FileExpectation,
    workspace_root: &Path,
    repor_dir: &Path,
) -> Result<ExpectationResult, ExpectedContentsError> {
    match &exp.matcher {
        FileMatcher::Exists => {
            let observation = observation::observe_file_exists(workspace_root, &exp.path);
            let passed = matches!(observation, FileExistsObservation::RegularFile);
            Ok(ExpectationResult {
                kind: ExpectationKind::FileExists {
                    path: exp.path.clone(),
                    observation,
                },
                passed,
            })
        }
        FileMatcher::Contains(expected) => {
            let expected_value = expected.to_text_value();
            let observation = observation::observe_file_contains(
                workspace_root,
                &exp.path,
                expected_value.as_str(),
            );
            let passed = matches!(observation, FileContentObservation::Found);
            Ok(ExpectationResult {
                kind: ExpectationKind::FileContains {
                    path: exp.path.clone(),
                    expected: expected_value.as_str().to_string(),
                    observation,
                },
                passed,
            })
        }
        FileMatcher::ContentsEquals(expected_ref) => {
            let (expected_bytes, expected_source) =
                resolve_expected_contents(expected_ref, workspace_root, repor_dir)?;
            let observation = observation::observe_file_contents_equals(
                workspace_root,
                &exp.path,
                expected_bytes,
            );
            let passed = matches!(
                observation,
                ContentsEqualsObservation::Compared(ContentsEqualsComparison {
                    outcome: ContentsEqualsOutcome::Match,
                    ..
                })
            );
            Ok(ExpectationResult {
                kind: ExpectationKind::FileContentsEquals {
                    path: exp.path.clone(),
                    expected_source,
                    observation,
                },
                passed,
            })
        }
        FileMatcher::TextEquals(text_literal) => {
            let expected_source = match text_literal {
                TextLiteral::Quoted(value) => TextEqualsExpectedSource::Quoted(value.clone()),
                TextLiteral::Heredoc(value) => TextEqualsExpectedSource::Heredoc(value.clone()),
            };
            let expected_bytes = text_literal.to_text_value().as_str().as_bytes().to_vec();
            let observation = observation::observe_file_contents_equals(
                workspace_root,
                &exp.path,
                expected_bytes,
            );
            let passed = matches!(
                observation,
                ContentsEqualsObservation::Compared(ContentsEqualsComparison {
                    outcome: ContentsEqualsOutcome::Match,
                    ..
                })
            );
            Ok(ExpectationResult {
                kind: ExpectationKind::FileTextEquals {
                    path: exp.path.clone(),
                    expected_source,
                    observation,
                },
                passed,
            })
        }
        FileMatcher::NotExists | FileMatcher::Matches(_) => {
            unreachable!("file matcher variant not implemented in v0 parser or evaluator")
        }
    }
}

/// Evaluates a `dir <"path"> ...` expectation against the real filesystem.
///
/// The subject path policy (relative, no `.`/`..` segments, non-empty), and for `contains` the entry name policy, are checked earlier, in `evaluate_case`, before this function runs.
///
/// `exp.path` is resolved relative to `workspace_root`, the current concrete case's isolated workspace, exactly like `file` assertion paths. See docs/reference/semantics.md.
fn evaluate_dir_expectation(exp: &DirExpectation, workspace_root: &Path) -> ExpectationResult {
    match &exp.matcher {
        DirMatcher::Exists => {
            let observation = observation::observe_dir_exists(workspace_root, &exp.path);
            let passed = matches!(observation, DirExistsObservation::Directory);
            ExpectationResult {
                kind: ExpectationKind::DirExists {
                    path: exp.path.clone(),
                    observation,
                },
                passed,
            }
        }
        DirMatcher::Contains(entry_name) => {
            let observation =
                observation::observe_dir_contains(workspace_root, &exp.path, entry_name);
            let passed = matches!(observation, DirContainsObservation::Found);
            ExpectationResult {
                kind: ExpectationKind::DirContains {
                    path: exp.path.clone(),
                    expected_entry: entry_name.clone(),
                    observation,
                },
                passed,
            }
        }
        DirMatcher::NotExists => {
            unreachable!("dir matcher variant not implemented in v0 parser or evaluator")
        }
    }
}
