use std::path::{Path, PathBuf};

use crate::diagnostic::DiagnosticCode;
use crate::executor::{ExecutionEnvironment, execute_action};
use crate::fixture;
use crate::model::{
    Case, DirExpectation, DirMatcher, Expectation, FileContentsReference, FileExpectation,
    FileMatcher, LogicalOperator, OutputMatcher, Script, SideEffectingStep, Step, TextLiteral,
};
use crate::result::{
    ActionResult, AssertionBlockResult, CaseResult, CaseStatus, ContentsEqualsComparison,
    ContentsEqualsExpectedSource, ContentsEqualsObservation, ContentsEqualsOutcome,
    DirContainsObservation, DirExistsObservation, ExecutionReport, ExpectationKind,
    ExpectationResult, FileContentObservation, FileExistsObservation, RuntimeError, ScriptError,
    TextEqualsExpectedSource,
};
use crate::semantic::{
    SemanticError, validate_dir_entry_name, validate_dir_path, validate_file_path,
};
use crate::shim::CommandRegistry;
use crate::workspace::Workspace;

/// Observable evidence available at a point in case execution.
///
/// A checkpoint is an evidence context, not a full filesystem snapshot.
/// The initial checkpoint has workspace state but no last action result.
///
/// See docs/semantics.md — Checkpoint.
pub struct Checkpoint {
    pub workspace: WorkspaceState,
    pub last_action: Option<ActionResult>,
    /// Directory containing the `*.repor` file this case was loaded from, used to resolve a
    /// `contents_equals` expected `FixtureReference` (`@"<path>"`) relative to it. See
    /// `fixture::resolve_fixture_source`.
    pub repor_dir: PathBuf,
}

impl Checkpoint {
    /// The initial checkpoint: workspace state present, no last action result.
    pub fn initial(workspace_root: PathBuf, repor_dir: PathBuf) -> Self {
        Self {
            workspace: WorkspaceState {
                root: workspace_root,
            },
            last_action: None,
            repor_dir,
        }
    }

    /// An action-updated checkpoint after `$ ...` completes.
    pub fn after_action(action: ActionResult, workspace_root: PathBuf, repor_dir: PathBuf) -> Self {
        Self {
            workspace: WorkspaceState {
                root: workspace_root,
            },
            last_action: Some(action),
            repor_dir,
        }
    }
}

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

/// Observable workspace state: the concrete case's isolated workspace root.
///
/// File and directory expectations, and `write` steps, resolve paths
/// relative to `root`. See docs/semantics.md — Workspace lifecycle.
pub struct WorkspaceState {
    pub root: PathBuf,
}

/// Evaluates every case in `script`, loaded from the file at `source_path`.
///
/// `source_path` is recorded on every `CaseResult` and its parent directory is used to resolve
/// a `contents_equals` expected `FixtureReference` (`@"<path>"`) relative to it — see
/// `Checkpoint::repor_dir`.
pub fn evaluate(
    script: &Script,
    env: &ExecutionEnvironment,
    source_path: &Path,
    commands: &CommandRegistry,
) -> ExecutionReport {
    ExecutionReport {
        cases: script
            .cases
            .iter()
            .map(|c| evaluate_case(c, env, source_path, commands))
            .collect(),
        file_errors: vec![],
    }
}

fn evaluate_case(
    case: &Case,
    env: &ExecutionEnvironment,
    source_path: &Path,
    commands: &CommandRegistry,
) -> CaseResult {
    // Every case must contain at least one assertion block.
    let has_assertion_block = case
        .steps
        .iter()
        .any(|s| matches!(s, Step::AssertionBlock(_)));
    if !has_assertion_block {
        return CaseResult {
            name: case.name.clone(),
            source_path: Some(source_path.to_path_buf()),
            status: CaseStatus::ScriptError(ScriptError {
                message: format!(
                    "case '{}' has no assertion block; every case requires at least one assert {{ ... }} block",
                    case.name
                ),
                diagnostic_code: Some(DiagnosticCode::ParseMissingAssertionBlock),
                step_index: None,
            }),
            actions: vec![],
            assertion_blocks: vec![],
            side_effects_executed: 0,
        };
    }

    // Each concrete case gets its own isolated workspace, destroyed when
    // this function returns. See docs/semantics.md — Workspace lifecycle.
    let workspace = match Workspace::new() {
        Ok(w) => w,
        Err(e) => {
            return CaseResult {
                name: case.name.clone(),
                source_path: Some(source_path.to_path_buf()),
                status: CaseStatus::RuntimeError(RuntimeError {
                    message: format!(
                        "case '{}': failed to create isolated case workspace: {e}",
                        case.name
                    ),
                    diagnostic_code: None,
                    step_index: None,
                }),
                actions: vec![],
                assertion_blocks: vec![],
                side_effects_executed: 0,
            };
        }
    };

    // When commands are registered, materialize a fresh set of shims into this case's own `bin`
    // directory and prepend it to `env`'s PATH prefixes, so `$` steps resolve registered command
    // names through the shim before falling through to `env`'s own prefixes and the inherited
    // PATH. See docs/semantics.md — Command resolution through PATH shims.
    let case_env = match build_case_execution_environment(env, commands, workspace.root()) {
        Ok(case_env) => case_env,
        Err(e) => {
            return CaseResult {
                name: case.name.clone(),
                source_path: Some(source_path.to_path_buf()),
                status: CaseStatus::RuntimeError(RuntimeError {
                    message: format!(
                        "case '{}': failed to set up registered command shims: {e}",
                        case.name
                    ),
                    diagnostic_code: None,
                    step_index: None,
                }),
                actions: vec![],
                assertion_blocks: vec![],
                side_effects_executed: 0,
            };
        }
    };

    let mut action_results: Vec<ActionResult> = Vec::new();
    let mut assertion_block_results: Vec<AssertionBlockResult> = Vec::new();
    // Successful `write` (and future side-effecting) step count, independent
    // of `action_results`. See `RunSummary::steps_executed`.
    let mut side_effects_executed: usize = 0;
    // The directory containing the referencing `*.repor` file, used to resolve a
    // `contents_equals` expected `FixtureReference` relative to it.
    //
    // `Path::parent()` returns `Some("")` — not `None` — for a bare relative filename with no
    // directory component (e.g. `reportage script.repor` run from the script's own directory),
    // since "" and "." are lexically distinct even though both mean "here". Treat that empty
    // parent the same as a missing one, or `fixture::resolve_fixture_source` fails to
    // canonicalize an empty path (`No such file or directory`) even when the fixture exists.
    let repor_dir = match source_path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };
    // Steps are processed in source order.
    // Assertion block failure stops execution before the next action.
    // See docs/semantics.md — Assertion block and the checkpoint-based assertion ADR.
    let mut checkpoint = Checkpoint::initial(workspace.root().to_path_buf(), repor_dir.clone());
    let mut case_failed = false;

    for (step_idx, step) in case.steps.iter().enumerate() {
        match step {
            Step::Action(action) => {
                if case_failed {
                    // Do not proceed to next action after a block failure.
                    break;
                }
                match execute_action(&action.command, &case_env, workspace.root()) {
                    Ok(result) => {
                        checkpoint = Checkpoint::after_action(
                            result.clone(),
                            workspace.root().to_path_buf(),
                            repor_dir.clone(),
                        );
                        action_results.push(result);
                    }
                    Err(e) => {
                        return CaseResult {
                            name: case.name.clone(),
                            source_path: Some(source_path.to_path_buf()),
                            status: CaseStatus::RuntimeError(RuntimeError {
                                message: e.message,
                                diagnostic_code: None,
                                step_index: Some(step_idx),
                            }),
                            actions: action_results,
                            assertion_blocks: assertion_block_results,
                            side_effects_executed,
                        };
                    }
                }
            }

            Step::SideEffect(SideEffectingStep::WriteFile(write_step)) => {
                if case_failed {
                    break;
                }
                let content = write_step.content.to_text_value();
                match workspace.write_file(&write_step.path, content.as_str()) {
                    Ok(()) => side_effects_executed += 1,
                    Err(e) => {
                        return CaseResult {
                            name: case.name.clone(),
                            source_path: Some(source_path.to_path_buf()),
                            status: CaseStatus::RuntimeError(RuntimeError {
                                message: format!(
                                    "case '{}': write step at step {} failed: {e}",
                                    case.name,
                                    step_idx + 1,
                                ),
                                diagnostic_code: Some(e.code()),
                                step_index: Some(step_idx),
                            }),
                            actions: action_results,
                            assertion_blocks: assertion_block_results,
                            side_effects_executed,
                        };
                    }
                }
            }

            Step::AssertionBlock(block) => {
                if case_failed {
                    break;
                }

                // Check that all expectations have the evidence they require.
                for expectation in block.expectations() {
                    if expectation.required_evidence().needs_action_result()
                        && checkpoint.last_action.is_none()
                    {
                        return CaseResult {
                            name: case.name.clone(),
                            source_path: Some(source_path.to_path_buf()),
                            status: CaseStatus::ScriptError(ScriptError {
                                message: format!(
                                    "case '{}': assertion block at step {} uses a process expectation \
                                     (exit, stdout, stderr) but no '$' action has run yet; \
                                     the initial checkpoint has no last action result",
                                    case.name,
                                    step_idx + 1,
                                ),
                                diagnostic_code: Some(
                                    DiagnosticCode::SemanticExpectationRequiresAction,
                                ),
                                step_index: Some(step_idx),
                            }),
                            actions: action_results,
                            assertion_blocks: assertion_block_results,
                            side_effects_executed,
                        };
                    }

                    // A file assertion path, a dir assertion subject path, and (for `dir`
                    // `contains`) its entry name, must all satisfy reportage's path / entry-name
                    // policy before evidence comparison begins. This is a semantic error, not an
                    // assertion failure. Recurses into `not` / `all` / `any` children so a
                    // `file`/`dir` assertion nested inside a logical composition is validated the
                    // same as a bare one — a composition combines assertion outcomes, it must
                    // never let an unvalidated path reach the filesystem.
                    // See docs/semantic-diagnostics.md,
                    // docs/adr/20260704T112155Z_subject-first-file-assertion-syntax.md, and
                    // docs/adr/20260706T000000Z_subject-first-directory-assertion-syntax.md.
                    if let Err(semantic_err) = validate_expectation_paths(expectation) {
                        return CaseResult {
                            name: case.name.clone(),
                            source_path: Some(source_path.to_path_buf()),
                            status: CaseStatus::ScriptError(ScriptError {
                                message: format!(
                                    "case '{}': assertion block at step {} has an invalid \
                                     expectation: {semantic_err}",
                                    case.name,
                                    step_idx + 1,
                                ),
                                diagnostic_code: Some(semantic_err.code()),
                                step_index: Some(step_idx),
                            }),
                            actions: action_results,
                            assertion_blocks: assertion_block_results,
                            side_effects_executed,
                        };
                    }
                }

                // Evaluate all expectations in the block independently. A `contents_equals`
                // expected value that fails to resolve (a missing/non-regular/unreadable
                // expected `WorkspacePath`, or a fixture reference error) is a test-definition
                // problem, not an assertion outcome: it aborts the case immediately as a
                // `ScriptError`, exactly like the path-policy check above.
                // See docs/adr/20260707T012055Z_contents-equals-evaluation.md.
                let expectation_results: Vec<ExpectationResult> = match block
                    .expectations()
                    .iter()
                    .map(|exp| evaluate_expectation_at_checkpoint(exp, &checkpoint))
                    .collect()
                {
                    Ok(results) => results,
                    Err(err) => {
                        return CaseResult {
                            name: case.name.clone(),
                            source_path: Some(source_path.to_path_buf()),
                            status: CaseStatus::ScriptError(ScriptError {
                                message: format!(
                                    "case '{}': assertion block at step {} has an unresolvable \
                                     contents_equals expected value: {}",
                                    case.name,
                                    step_idx + 1,
                                    err.message,
                                ),
                                diagnostic_code: Some(err.diagnostic_code),
                                step_index: Some(step_idx),
                            }),
                            actions: action_results,
                            assertion_blocks: assertion_block_results,
                            side_effects_executed,
                        };
                    }
                };

                let block_result = AssertionBlockResult {
                    step_index: step_idx,
                    expectations: expectation_results,
                    checkpoint_action_index: action_results.len().checked_sub(1),
                };

                if block_result.has_failures() {
                    case_failed = true;
                }

                assertion_block_results.push(block_result);
            }
        }
    }

    CaseResult {
        name: case.name.clone(),
        source_path: Some(source_path.to_path_buf()),
        status: if case_failed {
            CaseStatus::Fail
        } else {
            CaseStatus::Pass
        },
        actions: action_results,
        assertion_blocks: assertion_block_results,
        side_effects_executed,
    }
}

/// Builds the case-local execution environment used for every `$` step in one concrete case.
///
/// When `commands` is empty this is equivalent to `env` (no case-local `bin` directory is
/// created, matching pre-config-command behavior exactly). When `commands` is non-empty, a fresh
/// `bin` directory is created under `workspace_root`, every registered command is materialized
/// into it as a shim, and that directory is prepended to `env`'s own PATH prefixes — so a
/// registered command shadows both `env`'s prefixes and the inherited `PATH`.
///
/// Shims are materialized per case, not once at config-parse time, because each concrete case has
/// its own isolated workspace and `bin` directory. See docs/semantics.md — Execution order and
/// Command resolution through PATH shims.
fn build_case_execution_environment(
    env: &ExecutionEnvironment,
    commands: &CommandRegistry,
    workspace_root: &Path,
) -> std::io::Result<ExecutionEnvironment> {
    if commands.is_empty() {
        return Ok(ExecutionEnvironment::with_path_prefixes(
            env.path_prefixes.clone(),
        ));
    }

    let bin_dir = workspace_root.join("bin");
    std::fs::create_dir_all(&bin_dir)?;
    commands
        .materialize(&bin_dir)
        .map_err(|e| std::io::Error::other(e.to_string()))?;

    let mut path_prefixes = vec![bin_dir];
    path_prefixes.extend(env.path_prefixes.iter().cloned());
    Ok(ExecutionEnvironment::with_path_prefixes(path_prefixes))
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
fn validate_expectation_paths(expectation: &Expectation) -> Result<(), SemanticError> {
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
            // See docs/semantics.md — Logical composition.
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

/// Byte-level substring search, per docs/semantics.md's raw byte semantics for `stdout contains` /
/// `stderr contains`: `expected` is UTF-8 bytes of a Reportage string literal, matched against
/// `haystack` (raw process output bytes) without any decoding on either side.
///
/// An empty `needle` always matches, including against an empty `haystack`
/// (`emptyExpectedAlwaysMatches`, pinned by `spec/language/semantics/assertion.stdout.contains.json`
/// and `assertion.stderr.contains.json`).
fn bytes_contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty() || haystack.windows(needle.len()).any(|w| w == needle)
}

/// Evaluates a `file <"path"> ...` expectation against the real filesystem.
///
/// The path policy (relative, no `.`/`..` segments) is checked earlier, in `evaluate_case`, before this function runs.
/// By the time this function is called, `exp.path` is known to be policy-valid.
///
/// `exp.path` is resolved relative to `workspace_root`, the current concrete case's isolated workspace.
/// Actions never change that directory for the process (each `$` step runs in a fresh child shell), so file assertion paths are always resolved relative to the case workspace root, never affected by a `cd` performed inside an action.
/// See docs/semantics.md.
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
            let observation = observe_file_exists(workspace_root, &exp.path);
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
            let observation =
                observe_file_contains(workspace_root, &exp.path, expected_value.as_str());
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
            let observation =
                observe_file_contents_equals(workspace_root, &exp.path, expected_bytes);
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
            let observation =
                observe_file_contents_equals(workspace_root, &exp.path, expected_bytes);
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

/// Observes whether `path`, resolved against `workspace_root`, is a regular file, following symlinks.
///
/// A directory (or any other non-regular-file type) is observed as `NotRegularFile`, not `Missing`: the path does exist, but it does not satisfy the `file` subject's regular-file requirement.
fn observe_file_exists(workspace_root: &Path, path: &str) -> FileExistsObservation {
    match std::fs::metadata(workspace_root.join(path)) {
        Ok(meta) if meta.is_file() => FileExistsObservation::RegularFile,
        Ok(_) => FileExistsObservation::NotRegularFile,
        Err(_) => FileExistsObservation::Missing,
    }
}

/// Observes the actual side of a `file <"path"> contents_equals <expected>` expectation and, if
/// `path` is a readable regular file, compares its bytes against `expected` byte-for-byte.
///
/// `expected` has already been resolved successfully by the time this runs (see
/// `resolve_expected_contents`): a missing / non-regular / unreadable *actual* `path` is the
/// subject under test failing to produce the expected output, so — unlike an unresolvable
/// expected value — it is always an assertion failure, never a test-definition error.
fn observe_file_contents_equals(
    workspace_root: &Path,
    path: &str,
    expected: Vec<u8>,
) -> ContentsEqualsObservation {
    let resolved = workspace_root.join(path);
    let meta = match std::fs::metadata(&resolved) {
        Ok(meta) => meta,
        Err(_) => return ContentsEqualsObservation::ActualMissing,
    };
    if !meta.is_file() {
        return ContentsEqualsObservation::ActualNotRegularFile;
    }
    let actual = match std::fs::read(&resolved) {
        Ok(bytes) => bytes,
        Err(_) => return ContentsEqualsObservation::ActualUnreadable,
    };
    ContentsEqualsObservation::Compared(ContentsEqualsComparison::compare(actual, expected))
}

/// Observes whether `path`, resolved against `workspace_root`, is a readable UTF-8 regular file containing `expected` as a plain substring.
///
/// Per docs/semantic-diagnostics.md, missing / non-regular-file / unreadable / non-UTF-8 content are all "the `contains` precondition is unmet" — a single failure category distinct from "the file exists and is readable, but does not contain the expected substring".
fn observe_file_contains(
    workspace_root: &Path,
    path: &str,
    expected: &str,
) -> FileContentObservation {
    let resolved = workspace_root.join(path);
    let meta = match std::fs::metadata(&resolved) {
        Ok(meta) => meta,
        Err(_) => return FileContentObservation::Missing,
    };
    if !meta.is_file() {
        return FileContentObservation::NotRegularFile;
    }
    let bytes = match std::fs::read(&resolved) {
        Ok(bytes) => bytes,
        Err(_) => return FileContentObservation::Unreadable,
    };
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => return FileContentObservation::NotUtf8,
    };
    if text.contains(expected) {
        FileContentObservation::Found
    } else {
        FileContentObservation::NotFound
    }
}

/// Evaluates a `dir <"path"> ...` expectation against the real filesystem.
///
/// The subject path policy (relative, no `.`/`..` segments, non-empty), and for `contains` the
/// entry name policy, are checked earlier, in `evaluate_case`, before this function runs.
///
/// `exp.path` is resolved relative to `workspace_root`, the current concrete case's isolated
/// workspace, exactly like `file` assertion paths. See docs/semantics.md.
fn evaluate_dir_expectation(exp: &DirExpectation, workspace_root: &Path) -> ExpectationResult {
    match &exp.matcher {
        DirMatcher::Exists => {
            let observation = observe_dir_exists(workspace_root, &exp.path);
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
            let observation = observe_dir_contains(workspace_root, &exp.path, entry_name);
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

/// Observes whether `path`, resolved against `workspace_root`, is a directory, following symlinks.
///
/// A regular file (or any other non-directory type) is observed as `NotADirectory`, not
/// `Missing`: the path does exist, but it does not satisfy the `dir` subject's directory
/// requirement.
fn observe_dir_exists(workspace_root: &Path, path: &str) -> DirExistsObservation {
    match std::fs::metadata(workspace_root.join(path)) {
        Ok(meta) if meta.is_dir() => DirExistsObservation::Directory,
        Ok(_) => DirExistsObservation::NotADirectory,
        Err(_) => DirExistsObservation::Missing,
    }
}

/// Observes whether `path`, resolved against `workspace_root`, is a directory containing an
/// entry named `entry_name` directly under it.
///
/// Never recurses, never glob-matches, and never inspects file content: `entry_name` is compared
/// against each direct child's raw entry name for an exact match, regardless of that entry's file
/// type. See docs/semantics.md.
fn observe_dir_contains(
    workspace_root: &Path,
    path: &str,
    entry_name: &str,
) -> DirContainsObservation {
    let resolved = workspace_root.join(path);
    let meta = match std::fs::metadata(&resolved) {
        Ok(meta) => meta,
        Err(_) => return DirContainsObservation::SubjectMissing,
    };
    if !meta.is_dir() {
        return DirContainsObservation::SubjectNotADirectory;
    }
    let entries = match std::fs::read_dir(&resolved) {
        Ok(entries) => entries,
        Err(_) => return DirContainsObservation::SubjectUnreadable,
    };
    let found = entries
        .filter_map(Result::ok)
        .any(|entry| entry.file_name() == std::ffi::OsStr::new(entry_name));
    if found {
        DirContainsObservation::Found
    } else {
        DirContainsObservation::EntryMissing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::ExecutionEnvironment;
    use crate::model::{
        ActionStep, AssertionBlock, Case, ExitExpectation, Expectation, Script, TextLiteral,
        WorkspacePath, WriteFileStep,
    };

    fn default_env() -> ExecutionEnvironment {
        ExecutionEnvironment::default()
    }

    fn default_commands() -> CommandRegistry {
        CommandRegistry::default()
    }

    fn make_script(cases: Vec<Case>) -> Script {
        Script { cases }
    }

    fn action(cmd: &str) -> Step {
        Step::Action(ActionStep {
            command: cmd.to_string(),
        })
    }

    fn assert_exit(code: u8) -> Step {
        let expectations = vec![Expectation::Exit(ExitExpectation { expected: code })];
        Step::AssertionBlock(AssertionBlock::new(expectations).unwrap())
    }

    fn assert_exits(codes: &[u8]) -> Step {
        let expectations = codes
            .iter()
            .map(|&c| Expectation::Exit(ExitExpectation { expected: c }))
            .collect();
        Step::AssertionBlock(AssertionBlock::new(expectations).unwrap())
    }

    #[test]
    fn passing_case_exits_zero() {
        let script = make_script(vec![Case {
            name: "pass".to_string(),
            steps: vec![action("true"), assert_exit(0)],
        }]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert_eq!(result.exit_code(), 0);
        assert!(matches!(result.cases[0].status, CaseStatus::Pass));
    }

    #[test]
    fn failing_expectation_sets_fail_status() {
        let script = make_script(vec![Case {
            name: "fail".to_string(),
            steps: vec![action("false"), assert_exit(0)],
        }]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert_eq!(result.exit_code(), 1);
        assert!(matches!(result.cases[0].status, CaseStatus::Fail));
    }

    #[test]
    fn false_with_assert_exit_one_passes() {
        let script = make_script(vec![Case {
            name: "nonzero pass".to_string(),
            steps: vec![action("false"), assert_exit(1)],
        }]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert_eq!(result.exit_code(), 0);
        assert!(matches!(result.cases[0].status, CaseStatus::Pass));
    }

    #[test]
    fn missing_assertion_block_is_script_error() {
        let script = make_script(vec![Case {
            name: "no assert".to_string(),
            steps: vec![action("true")],
        }]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert_eq!(result.exit_code(), 2);
        assert!(matches!(result.cases[0].status, CaseStatus::ScriptError(_)));
    }

    #[test]
    fn process_expectation_at_initial_checkpoint_is_script_error() {
        let script = make_script(vec![Case {
            name: "assert first".to_string(),
            steps: vec![assert_exit(0)],
        }]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert_eq!(result.exit_code(), 2);
        assert!(matches!(result.cases[0].status, CaseStatus::ScriptError(_)));
    }

    #[test]
    fn multiple_expectations_in_one_block_all_evaluated() {
        let script = make_script(vec![Case {
            name: "multi expect".to_string(),
            steps: vec![action("true"), assert_exits(&[0, 0])],
        }]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert_eq!(result.exit_code(), 0);
        assert_eq!(result.cases[0].assertion_blocks.len(), 1);
        assert_eq!(result.cases[0].assertion_blocks[0].expectations.len(), 2);
    }

    #[test]
    fn both_expectations_in_block_reported_when_both_fail() {
        let script = make_script(vec![Case {
            name: "two fails".to_string(),
            steps: vec![action("true"), assert_exits(&[1, 1])],
        }]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert!(matches!(result.cases[0].status, CaseStatus::Fail));
        let block = &result.cases[0].assertion_blocks[0];
        assert_eq!(block.expectations.len(), 2);
        assert!(!block.expectations[0].passed);
        assert!(!block.expectations[1].passed);
    }

    #[test]
    fn exit_code_is_max_across_cases() {
        let script = make_script(vec![
            Case {
                name: "fail".to_string(),
                steps: vec![action("false"), assert_exit(0)],
            },
            Case {
                name: "no assert".to_string(),
                steps: vec![action("true")], // no assertion block -> script error
            },
        ]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert_eq!(result.exit_code(), 2); // script error beats assertion failure
    }

    #[test]
    fn assertion_block_failure_stops_subsequent_action() {
        // assert_exit(1) fails because true exits 0.
        // Block failure must not run the second action.
        let script = make_script(vec![Case {
            name: "source order stop".to_string(),
            steps: vec![
                action("true"),
                assert_exit(1),  // fails: true exits 0
                action("false"), // must not run
            ],
        }]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert!(matches!(result.cases[0].status, CaseStatus::Fail));
        // Only the first action should have executed.
        assert_eq!(result.cases[0].actions.len(), 1);
        assert_eq!(result.cases[0].assertion_blocks.len(), 1);
    }

    // ─── stdout/stderr raw byte semantics (#62) ────────────────────────────

    fn checkpoint_after_output(stdout: Vec<u8>, stderr: Vec<u8>) -> Checkpoint {
        Checkpoint::after_action(
            ActionResult {
                command: "test".to_string(),
                exit_code: 0,
                stdout,
                stderr,
                shim_invocations: vec![],
                shim_event_parse_warnings: vec![],
            },
            PathBuf::from("."),
            PathBuf::from("."),
        )
    }

    fn stdout_empty_expectation() -> Expectation {
        Expectation::Stdout(crate::model::OutputExpectation {
            matcher: OutputMatcher::Empty,
        })
    }

    fn stderr_empty_expectation() -> Expectation {
        Expectation::Stderr(crate::model::OutputExpectation {
            matcher: OutputMatcher::Empty,
        })
    }

    #[test]
    fn stdout_empty_passes_on_zero_bytes() {
        let checkpoint = checkpoint_after_output(vec![], vec![]);
        let result =
            evaluate_expectation_at_checkpoint(&stdout_empty_expectation(), &checkpoint).unwrap();
        assert!(result.passed);
    }

    // Whitespace-only output is still output: `empty` must observe zero bytes, not
    // "nothing but whitespace". Regression coverage for the `.trim().is_empty()` bug.
    #[test]
    fn stdout_empty_fails_on_single_space() {
        let checkpoint = checkpoint_after_output(b" ".to_vec(), vec![]);
        let result =
            evaluate_expectation_at_checkpoint(&stdout_empty_expectation(), &checkpoint).unwrap();
        assert!(!result.passed);
    }

    #[test]
    fn stdout_empty_fails_on_tab() {
        let checkpoint = checkpoint_after_output(b"\t".to_vec(), vec![]);
        let result =
            evaluate_expectation_at_checkpoint(&stdout_empty_expectation(), &checkpoint).unwrap();
        assert!(!result.passed);
    }

    #[test]
    fn stdout_empty_fails_on_lf() {
        let checkpoint = checkpoint_after_output(b"\n".to_vec(), vec![]);
        let result =
            evaluate_expectation_at_checkpoint(&stdout_empty_expectation(), &checkpoint).unwrap();
        assert!(!result.passed);
    }

    #[test]
    fn stdout_empty_fails_on_crlf() {
        let checkpoint = checkpoint_after_output(b"\r\n".to_vec(), vec![]);
        let result =
            evaluate_expectation_at_checkpoint(&stdout_empty_expectation(), &checkpoint).unwrap();
        assert!(!result.passed);
    }

    #[test]
    fn stdout_empty_fails_on_bare_cr() {
        let checkpoint = checkpoint_after_output(b"\r".to_vec(), vec![]);
        let result =
            evaluate_expectation_at_checkpoint(&stdout_empty_expectation(), &checkpoint).unwrap();
        assert!(!result.passed);
    }

    #[test]
    fn stderr_empty_fails_on_whitespace_only() {
        let checkpoint = checkpoint_after_output(vec![], b" \t\r\n".to_vec());
        let result =
            evaluate_expectation_at_checkpoint(&stderr_empty_expectation(), &checkpoint).unwrap();
        assert!(!result.passed);
    }

    #[test]
    fn stdout_contains_matches_substring_in_non_utf8_output() {
        // 0xff is invalid UTF-8 in any position. A lossy decode at capture time would have
        // replaced it with U+FFFD before this match ever ran; raw byte matching must not do that.
        let mut stdout = b"ok".to_vec();
        stdout.push(0xff);
        let checkpoint = checkpoint_after_output(stdout, vec![]);
        let expectation = Expectation::Stdout(crate::model::OutputExpectation {
            matcher: OutputMatcher::Contains("ok".to_string()),
        });
        let result = evaluate_expectation_at_checkpoint(&expectation, &checkpoint).unwrap();
        assert!(result.passed);
    }

    // ─── Logical composition (#25) ────────────────────────────────────────

    use crate::model::{LogicalExpectation, LogicalOperator};

    fn exit_exp(code: u8) -> Expectation {
        Expectation::Exit(ExitExpectation { expected: code })
    }

    fn logical(operator: LogicalOperator, children: Vec<Expectation>) -> Expectation {
        Expectation::Logical(LogicalExpectation::new(operator, children).unwrap())
    }

    fn checkpoint_after_exit(code: i32) -> Checkpoint {
        Checkpoint::after_action(
            ActionResult {
                command: "test".to_string(),
                exit_code: code,
                stdout: Vec::new(),
                stderr: Vec::new(),
                shim_invocations: vec![],
                shim_event_parse_warnings: vec![],
            },
            PathBuf::from("."),
            PathBuf::from("."),
        )
    }

    #[test]
    fn all_passes_when_every_child_passes() {
        let expectation = logical(LogicalOperator::All, vec![exit_exp(0), exit_exp(0)]);
        let result =
            evaluate_expectation_at_checkpoint(&expectation, &checkpoint_after_exit(0)).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn all_fails_when_one_child_fails() {
        let expectation = logical(LogicalOperator::All, vec![exit_exp(0), exit_exp(1)]);
        let result =
            evaluate_expectation_at_checkpoint(&expectation, &checkpoint_after_exit(0)).unwrap();
        assert!(!result.passed);
    }

    #[test]
    fn any_passes_when_one_child_passes() {
        let expectation = logical(LogicalOperator::Any, vec![exit_exp(1), exit_exp(0)]);
        let result =
            evaluate_expectation_at_checkpoint(&expectation, &checkpoint_after_exit(0)).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn any_fails_when_every_child_fails() {
        let expectation = logical(LogicalOperator::Any, vec![exit_exp(1), exit_exp(2)]);
        let result =
            evaluate_expectation_at_checkpoint(&expectation, &checkpoint_after_exit(0)).unwrap();
        assert!(!result.passed);
    }

    #[test]
    fn any_passes_when_all_children_pass() {
        let expectation = logical(LogicalOperator::Any, vec![exit_exp(0), exit_exp(0)]);
        let result =
            evaluate_expectation_at_checkpoint(&expectation, &checkpoint_after_exit(0)).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn not_passes_when_single_child_fails() {
        let expectation = logical(LogicalOperator::Not, vec![exit_exp(1)]);
        let result =
            evaluate_expectation_at_checkpoint(&expectation, &checkpoint_after_exit(0)).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn not_fails_when_single_child_passes() {
        let expectation = logical(LogicalOperator::Not, vec![exit_exp(0)]);
        let result =
            evaluate_expectation_at_checkpoint(&expectation, &checkpoint_after_exit(0)).unwrap();
        assert!(!result.passed);
    }

    #[test]
    fn not_with_multiple_children_negates_implicit_all_not_each_child() {
        // not { A B } is not(all(A, B)), not not(A) and not(B).
        // Here A passes (exit 0) and B fails (exit 1): all(A, B) is false, so not(all(A, B)) is true — the block passes as a whole, even though per-child negation (not(A) and not(B)) would fail on A.
        let expectation = logical(LogicalOperator::Not, vec![exit_exp(0), exit_exp(1)]);
        let result =
            evaluate_expectation_at_checkpoint(&expectation, &checkpoint_after_exit(0)).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn not_with_all_children_passing_fails() {
        // all(A, B) is true here, so not(all(A, B)) must fail.
        let expectation = logical(LogicalOperator::Not, vec![exit_exp(0), exit_exp(0)]);
        let result =
            evaluate_expectation_at_checkpoint(&expectation, &checkpoint_after_exit(0)).unwrap();
        assert!(!result.passed);
    }

    #[test]
    fn not_with_all_children_failing_passes() {
        // all(A, B) is false here, so not(all(A, B)) must pass.
        let expectation = logical(LogicalOperator::Not, vec![exit_exp(1), exit_exp(1)]);
        let result =
            evaluate_expectation_at_checkpoint(&expectation, &checkpoint_after_exit(0)).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn nested_logical_composition_evaluates_recursively() {
        // all { not { exit 1 } any { exit 0 exit 2 } }
        let expectation = logical(
            LogicalOperator::All,
            vec![
                logical(LogicalOperator::Not, vec![exit_exp(1)]),
                logical(LogicalOperator::Any, vec![exit_exp(0), exit_exp(2)]),
            ],
        );
        let result =
            evaluate_expectation_at_checkpoint(&expectation, &checkpoint_after_exit(0)).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn logical_result_retains_each_child_outcome() {
        // Nothing is lost: an `any` whose candidates all fail must retain each candidate's own failure reason.
        let expectation = logical(LogicalOperator::Any, vec![exit_exp(1), exit_exp(2)]);
        let result =
            evaluate_expectation_at_checkpoint(&expectation, &checkpoint_after_exit(0)).unwrap();
        let ExpectationKind::Logical { operator, children } = &result.kind else {
            panic!("expected ExpectationKind::Logical");
        };
        assert!(matches!(operator, LogicalOperator::Any));
        assert_eq!(children.len(), 2);
        assert!(!children[0].passed);
        assert!(!children[1].passed);
        assert!(matches!(
            children[0].kind,
            ExpectationKind::Exit {
                expected: 1,
                actual: 0
            }
        ));
        assert!(matches!(
            children[1].kind,
            ExpectationKind::Exit {
                expected: 2,
                actual: 0
            }
        ));
    }

    #[test]
    fn logical_composition_at_top_level_case_evaluates_via_evaluate_case() {
        let script = make_script(vec![Case {
            name: "any exit".to_string(),
            steps: vec![
                action("false"), // exit 1
                Step::AssertionBlock(
                    AssertionBlock::new(vec![logical(
                        LogicalOperator::Any,
                        vec![exit_exp(0), exit_exp(1)],
                    )])
                    .unwrap(),
                ),
            ],
        }]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert!(matches!(result.cases[0].status, CaseStatus::Pass));
    }

    #[test]
    fn logical_composition_wrapping_process_expectation_at_initial_checkpoint_is_script_error() {
        // A composition wrapping exit/stdout/stderr still requires a preceding action, exactly like a bare process expectation.
        let script = make_script(vec![Case {
            name: "no action yet".to_string(),
            steps: vec![Step::AssertionBlock(
                AssertionBlock::new(vec![logical(LogicalOperator::All, vec![exit_exp(0)])])
                    .unwrap(),
            )],
        }]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert!(matches!(result.cases[0].status, CaseStatus::ScriptError(_)));
    }

    // ─── `contents_equals` comparison evaluation (#87) ─────────────────────

    fn write_step(path: &str, content: &str) -> Step {
        Step::SideEffect(SideEffectingStep::WriteFile(WriteFileStep {
            path: WorkspacePath::parse(path).unwrap(),
            content: TextLiteral::Quoted(content.to_string()),
        }))
    }

    fn assert_file_contents_equals_workspace(actual_path: &str, expected_path: &str) -> Step {
        let expectations = vec![Expectation::File(FileExpectation {
            path: actual_path.to_string(),
            matcher: FileMatcher::ContentsEquals(FileContentsReference::Workspace(
                WorkspacePath::parse(expected_path).unwrap(),
            )),
        })];
        Step::AssertionBlock(AssertionBlock::new(expectations).unwrap())
    }

    fn assert_stdout_contents_equals_workspace(expected_path: &str) -> Step {
        let expectations = vec![Expectation::Stdout(crate::model::OutputExpectation {
            matcher: OutputMatcher::ContentsEquals(FileContentsReference::Workspace(
                WorkspacePath::parse(expected_path).unwrap(),
            )),
        })];
        Step::AssertionBlock(AssertionBlock::new(expectations).unwrap())
    }

    fn assert_stderr_contents_equals_workspace(expected_path: &str) -> Step {
        let expectations = vec![Expectation::Stderr(crate::model::OutputExpectation {
            matcher: OutputMatcher::ContentsEquals(FileContentsReference::Workspace(
                WorkspacePath::parse(expected_path).unwrap(),
            )),
        })];
        Step::AssertionBlock(AssertionBlock::new(expectations).unwrap())
    }

    fn single_case(steps: Vec<Step>) -> Script {
        make_script(vec![Case {
            name: "contents_equals".to_string(),
            steps,
        }])
    }

    #[test]
    fn file_contents_equals_passes_when_actual_and_expected_workspace_bytes_match() {
        let script = single_case(vec![
            write_step("actual.txt", "hello"),
            write_step("expected.txt", "hello"),
            assert_file_contents_equals_workspace("actual.txt", "expected.txt"),
        ]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert!(matches!(result.cases[0].status, CaseStatus::Pass));
    }

    #[test]
    fn file_contents_equals_passes_when_both_workspace_files_are_empty() {
        let script = single_case(vec![
            write_step("actual.txt", ""),
            write_step("expected.txt", ""),
            assert_file_contents_equals_workspace("actual.txt", "expected.txt"),
        ]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert!(matches!(result.cases[0].status, CaseStatus::Pass));
    }

    #[test]
    fn file_contents_equals_fails_on_single_byte_mismatch() {
        let script = single_case(vec![
            write_step("actual.txt", "hello"),
            write_step("expected.txt", "hellp"),
            assert_file_contents_equals_workspace("actual.txt", "expected.txt"),
        ]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert!(matches!(result.cases[0].status, CaseStatus::Fail));
        let expectation = &result.cases[0].assertion_blocks[0].expectations[0];
        assert!(!expectation.passed);
        let ExpectationKind::FileContentsEquals { observation, .. } = &expectation.kind else {
            panic!("expected ExpectationKind::FileContentsEquals");
        };
        let ContentsEqualsObservation::Compared(comparison) = observation else {
            panic!("expected ContentsEqualsObservation::Compared");
        };
        assert_eq!(
            comparison.outcome,
            ContentsEqualsOutcome::Mismatch(crate::result::ContentsMismatch {
                actual_len: 5,
                expected_len: 5,
                first_diff_offset: 4,
            })
        );
    }

    #[test]
    fn file_contents_equals_detects_missing_trailing_newline_as_mismatch() {
        let script = single_case(vec![
            write_step("actual.txt", "hello"),
            write_step("expected.txt", "hello\n"),
            assert_file_contents_equals_workspace("actual.txt", "expected.txt"),
        ]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert!(matches!(result.cases[0].status, CaseStatus::Fail));
    }

    #[test]
    fn file_contents_equals_detects_crlf_vs_lf_as_mismatch() {
        let script = single_case(vec![
            write_step("actual.txt", "hello\n"),
            write_step("expected.txt", "hello\r\n"),
            assert_file_contents_equals_workspace("actual.txt", "expected.txt"),
        ]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert!(matches!(result.cases[0].status, CaseStatus::Fail));
    }

    #[test]
    fn file_contents_equals_missing_actual_is_assertion_failure_not_script_error() {
        let script = single_case(vec![
            write_step("expected.txt", "hello"),
            assert_file_contents_equals_workspace("does-not-exist.txt", "expected.txt"),
        ]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert!(matches!(result.cases[0].status, CaseStatus::Fail));
        let expectation = &result.cases[0].assertion_blocks[0].expectations[0];
        let ExpectationKind::FileContentsEquals { observation, .. } = &expectation.kind else {
            panic!("expected ExpectationKind::FileContentsEquals");
        };
        assert_eq!(*observation, ContentsEqualsObservation::ActualMissing);
    }

    #[test]
    fn file_contents_equals_actual_directory_is_assertion_failure_not_script_error() {
        let script = single_case(vec![
            action("mkdir a-dir"),
            write_step("expected.txt", "hello"),
            assert_file_contents_equals_workspace("a-dir", "expected.txt"),
        ]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert!(matches!(result.cases[0].status, CaseStatus::Fail));
        let expectation = &result.cases[0].assertion_blocks[0].expectations[0];
        let ExpectationKind::FileContentsEquals { observation, .. } = &expectation.kind else {
            panic!("expected ExpectationKind::FileContentsEquals");
        };
        assert_eq!(
            *observation,
            ContentsEqualsObservation::ActualNotRegularFile
        );
    }

    #[test]
    fn file_contents_equals_missing_expected_workspace_path_is_script_error_exit_two() {
        let script = single_case(vec![
            write_step("actual.txt", "hello"),
            assert_file_contents_equals_workspace("actual.txt", "does-not-exist.txt"),
        ]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert_eq!(result.exit_code(), 2);
        let CaseStatus::ScriptError(err) = &result.cases[0].status else {
            panic!("expected CaseStatus::ScriptError");
        };
        assert_eq!(
            err.diagnostic_code,
            Some(DiagnosticCode::SemanticFileContentsReferenceMissing)
        );
    }

    #[test]
    fn file_contents_equals_expected_workspace_path_is_directory_is_script_error() {
        let script = single_case(vec![
            action("mkdir expected-dir"),
            write_step("actual.txt", "hello"),
            assert_file_contents_equals_workspace("actual.txt", "expected-dir"),
        ]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert_eq!(result.exit_code(), 2);
        let CaseStatus::ScriptError(err) = &result.cases[0].status else {
            panic!("expected CaseStatus::ScriptError");
        };
        assert_eq!(
            err.diagnostic_code,
            Some(DiagnosticCode::SemanticFileContentsReferenceNotARegularFile)
        );
    }

    #[test]
    fn stdout_contents_equals_passes_on_matching_bytes() {
        let script = single_case(vec![
            write_step("expected.txt", "hello"),
            action("printf hello"),
            assert_stdout_contents_equals_workspace("expected.txt"),
        ]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert!(matches!(result.cases[0].status, CaseStatus::Pass));
    }

    #[test]
    fn stdout_contents_equals_fails_on_mismatched_bytes() {
        let script = single_case(vec![
            write_step("expected.txt", "world"),
            action("printf hello"),
            assert_stdout_contents_equals_workspace("expected.txt"),
        ]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert!(matches!(result.cases[0].status, CaseStatus::Fail));
    }

    #[test]
    fn stderr_contents_equals_passes_on_matching_bytes() {
        let script = single_case(vec![
            write_step("expected.txt", "oops"),
            action("printf oops 1>&2"),
            assert_stderr_contents_equals_workspace("expected.txt"),
        ]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert!(matches!(result.cases[0].status, CaseStatus::Pass));
    }

    #[test]
    fn stdout_contents_equals_missing_expected_workspace_path_is_script_error_exit_two() {
        let script = single_case(vec![
            action("printf hello"),
            assert_stdout_contents_equals_workspace("does-not-exist.txt"),
        ]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert_eq!(result.exit_code(), 2);
        assert!(matches!(result.cases[0].status, CaseStatus::ScriptError(_)));
    }

    #[test]
    fn contents_equals_script_error_inside_logical_composition_aborts_the_case() {
        // A `contents_equals` expected-value error nested inside `all { ... }` must abort the
        // whole case as a script error, not be swallowed as an ordinary failing child.
        let script = single_case(vec![
            action("true"),
            write_step("actual.txt", "hello"),
            Step::AssertionBlock(
                AssertionBlock::new(vec![logical(
                    LogicalOperator::All,
                    vec![
                        exit_exp(0),
                        Expectation::File(FileExpectation {
                            path: "actual.txt".to_string(),
                            matcher: FileMatcher::ContentsEquals(FileContentsReference::Workspace(
                                WorkspacePath::parse("does-not-exist.txt").unwrap(),
                            )),
                        }),
                    ],
                )])
                .unwrap(),
            ),
        ]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert_eq!(result.exit_code(), 2);
        assert!(matches!(result.cases[0].status, CaseStatus::ScriptError(_)));
    }

    // ─── `text_equals` comparison evaluation (#88) ──────────────────────────

    fn assert_file_text_equals(actual_path: &str, expected_text: &str) -> Step {
        let expectations = vec![Expectation::File(FileExpectation {
            path: actual_path.to_string(),
            matcher: FileMatcher::TextEquals(TextLiteral::Quoted(expected_text.to_string())),
        })];
        Step::AssertionBlock(AssertionBlock::new(expectations).unwrap())
    }

    fn assert_file_text_equals_heredoc(actual_path: &str, expected_text: &str) -> Step {
        let expectations = vec![Expectation::File(FileExpectation {
            path: actual_path.to_string(),
            matcher: FileMatcher::TextEquals(TextLiteral::Heredoc(expected_text.to_string())),
        })];
        Step::AssertionBlock(AssertionBlock::new(expectations).unwrap())
    }

    #[test]
    fn file_text_equals_passes_when_actual_bytes_match_quoted_expected() {
        let script = single_case(vec![
            write_step("actual.txt", "hello"),
            assert_file_text_equals("actual.txt", "hello"),
        ]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert!(matches!(result.cases[0].status, CaseStatus::Pass));
    }

    #[test]
    fn file_text_equals_passes_when_actual_bytes_match_heredoc_expected() {
        // Runtime semantics are transparent to literal form: a heredoc expected value compares
        // identically to the same text written as a quoted string literal.
        let script = single_case(vec![
            write_step("actual.txt", "hello\nworld\n"),
            assert_file_text_equals_heredoc("actual.txt", "hello\nworld\n"),
        ]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert!(matches!(result.cases[0].status, CaseStatus::Pass));
    }

    #[test]
    fn file_text_equals_passes_when_both_sides_are_empty() {
        let script = single_case(vec![
            write_step("actual.txt", ""),
            assert_file_text_equals("actual.txt", ""),
        ]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert!(matches!(result.cases[0].status, CaseStatus::Pass));
    }

    #[test]
    fn file_text_equals_fails_on_single_byte_mismatch() {
        let script = single_case(vec![
            write_step("actual.txt", "hello"),
            assert_file_text_equals("actual.txt", "hellp"),
        ]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert!(matches!(result.cases[0].status, CaseStatus::Fail));
        let expectation = &result.cases[0].assertion_blocks[0].expectations[0];
        assert!(!expectation.passed);
        let ExpectationKind::FileTextEquals { observation, .. } = &expectation.kind else {
            panic!("expected ExpectationKind::FileTextEquals");
        };
        let ContentsEqualsObservation::Compared(comparison) = observation else {
            panic!("expected ContentsEqualsObservation::Compared");
        };
        assert_eq!(
            comparison.outcome,
            ContentsEqualsOutcome::Mismatch(crate::result::ContentsMismatch {
                actual_len: 5,
                expected_len: 5,
                first_diff_offset: 4,
            })
        );
    }

    #[test]
    fn file_text_equals_fails_on_heredoc_mismatch() {
        // Mirrors `file_text_equals_fails_on_single_byte_mismatch`, but with a heredoc expected
        // value: failure classification and `expected_source` must both reflect the heredoc
        // literal form, not just the quoted-string form the sibling test covers.
        let script = single_case(vec![
            write_step("actual.txt", "hello\nworld\n"),
            assert_file_text_equals_heredoc("actual.txt", "hello\nWORLD\n"),
        ]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert!(matches!(result.cases[0].status, CaseStatus::Fail));
        let expectation = &result.cases[0].assertion_blocks[0].expectations[0];
        assert!(!expectation.passed);
        let ExpectationKind::FileTextEquals {
            expected_source,
            observation,
            ..
        } = &expectation.kind
        else {
            panic!("expected ExpectationKind::FileTextEquals");
        };
        assert_eq!(
            *expected_source,
            TextEqualsExpectedSource::Heredoc("hello\nWORLD\n".to_string())
        );
        let ContentsEqualsObservation::Compared(comparison) = observation else {
            panic!("expected ContentsEqualsObservation::Compared");
        };
        assert!(matches!(
            comparison.outcome,
            ContentsEqualsOutcome::Mismatch(_)
        ));
    }

    #[test]
    fn file_text_equals_detects_missing_trailing_newline_as_mismatch() {
        let script = single_case(vec![
            write_step("actual.txt", "hello"),
            assert_file_text_equals("actual.txt", "hello\n"),
        ]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert!(matches!(result.cases[0].status, CaseStatus::Fail));
    }

    #[test]
    fn file_text_equals_detects_crlf_vs_lf_as_mismatch() {
        let script = single_case(vec![
            write_step("actual.txt", "hello\n"),
            assert_file_text_equals("actual.txt", "hello\r\n"),
        ]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert!(matches!(result.cases[0].status, CaseStatus::Fail));
    }

    #[test]
    fn file_text_equals_detects_unicode_normalization_difference_as_mismatch() {
        // NFC "é" (U+00E9) vs. NFD "e" + combining acute (U+0065 U+0301): visually identical,
        // distinct UTF-8 bytes. text_equals performs no normalization of any kind.
        let nfc = "caf\u{00e9}";
        let nfd = "cafe\u{0301}";
        let script = single_case(vec![
            write_step("actual.txt", nfc),
            assert_file_text_equals("actual.txt", nfd),
        ]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert!(matches!(result.cases[0].status, CaseStatus::Fail));
    }

    #[test]
    fn file_text_equals_missing_actual_is_assertion_failure_not_script_error() {
        let script = single_case(vec![assert_file_text_equals("does-not-exist.txt", "hello")]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert!(matches!(result.cases[0].status, CaseStatus::Fail));
        let expectation = &result.cases[0].assertion_blocks[0].expectations[0];
        let ExpectationKind::FileTextEquals { observation, .. } = &expectation.kind else {
            panic!("expected ExpectationKind::FileTextEquals");
        };
        assert_eq!(*observation, ContentsEqualsObservation::ActualMissing);
    }

    #[test]
    fn file_text_equals_actual_directory_is_assertion_failure_not_script_error() {
        let script = single_case(vec![
            action("mkdir a-dir"),
            assert_file_text_equals("a-dir", "hello"),
        ]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        assert!(matches!(result.cases[0].status, CaseStatus::Fail));
        let expectation = &result.cases[0].assertion_blocks[0].expectations[0];
        let ExpectationKind::FileTextEquals { observation, .. } = &expectation.kind else {
            panic!("expected ExpectationKind::FileTextEquals");
        };
        assert_eq!(
            *observation,
            ContentsEqualsObservation::ActualNotRegularFile
        );
    }

    #[test]
    fn file_text_equals_expected_source_reflects_literal_kind() {
        let script = single_case(vec![
            write_step("actual.txt", "hello"),
            assert_file_text_equals("actual.txt", "hello"),
        ]);
        let result = evaluate(
            &script,
            &default_env(),
            Path::new("test.repor"),
            &default_commands(),
        );
        let expectation = &result.cases[0].assertion_blocks[0].expectations[0];
        let ExpectationKind::FileTextEquals {
            expected_source, ..
        } = &expectation.kind
        else {
            panic!("expected ExpectationKind::FileTextEquals");
        };
        assert_eq!(
            *expected_source,
            TextEqualsExpectedSource::Quoted("hello".to_string())
        );
    }
}
