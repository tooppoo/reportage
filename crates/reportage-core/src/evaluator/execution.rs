use std::path::{Path, PathBuf};

use super::{
    Checkpoint, evaluate_expectation_at_checkpoint, expectation::validate_expectation_paths,
};
use crate::diagnostic::DiagnosticCode;
use crate::executor::{ExecutionEnvironment, execute_action};
use crate::model::{BeforeEach, Case, Script, SideEffectingStep, Step};
use crate::result::{
    ActionResult, AssertionBlockResult, CaseResult, CaseStatus, ExecutionReport, ExpectationResult,
    RuntimeError, ScriptError,
};
use crate::shim::CommandRegistry;
use crate::workspace::Workspace;

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
            .map(|c| evaluate_case(c, script.before_each.as_ref(), env, source_path, commands))
            .collect(),
        file_errors: vec![],
    }
}

fn evaluate_case(
    case: &Case,
    before_each: Option<&BeforeEach>,
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
    // this function returns. See docs/reference/semantics.md — Workspace lifecycle.
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
    // PATH. See docs/reference/semantics.md — Command resolution through PATH shims.
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
    // `before_each` setup replays inside this concrete case's own workspace,
    // before the initial checkpoint below is established — so the checkpoint's
    // workspace evidence already contains every file it wrote, and the case
    // body's first assertion block can observe them. A failure is a runtime
    // step error attributed to the module-level block, not to any case body
    // step, hence `step_index: None`; the 1-based position inside
    // `before_each` is carried in the message instead.
    // See docs/reference/execution-model.md — Execution order and `before_each`.
    if let Some(before_each) = before_each {
        for (setup_idx, step) in before_each.steps().iter().enumerate() {
            let SideEffectingStep::WriteFile(write_step) = step;
            let content = write_step.content.to_text_value();
            match workspace.write_file(&write_step.path, content.as_str()) {
                Ok(()) => side_effects_executed += 1,
                Err(e) => {
                    return CaseResult {
                        name: case.name.clone(),
                        source_path: Some(source_path.to_path_buf()),
                        status: CaseStatus::RuntimeError(RuntimeError {
                            message: format!(
                                "case '{}': before_each write step {} failed: {e}",
                                case.name,
                                setup_idx + 1,
                            ),
                            diagnostic_code: Some(e.code()),
                            step_index: None,
                        }),
                        actions: action_results,
                        assertion_blocks: assertion_block_results,
                        side_effects_executed,
                    };
                }
            }
        }
    }

    // Steps are processed in source order.
    // Assertion block failure stops execution before the next action.
    // See docs/reference/semantics.md — Assertion block and the checkpoint-based assertion ADR.
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
                    // See docs/reference/semantic-diagnostics.md,
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
/// its own isolated workspace and `bin` directory. See docs/reference/semantics.md — Execution order and
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
