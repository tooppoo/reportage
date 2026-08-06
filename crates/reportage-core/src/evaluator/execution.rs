use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::{
    Checkpoint,
    expectation::{evaluate_expectation_with_bindings, validate_expectation_paths},
};
use crate::diagnostic::DiagnosticCode;
use crate::executor::{ExecutionEnvironment, execute_action};
use crate::model::{
    BeforeEach, Binding, BindingSource, BoundValue, CaptureMode, Case, RuntimeEvidenceSource,
    Script, SideEffectingStep, Step, TextValue,
};
use crate::result::{
    ActionResult, AssertionBlockResult, CaseResult, CaseStatus, ExecutionReport, ExpectationResult,
    RuntimeError, ScriptError, StepOrigin, StepPhase,
};
use crate::shim::CommandRegistry;
use crate::text_value::{ResolveTextValue, TextResolutionContext};
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
    evaluate_with(
        script,
        env,
        source_path,
        commands,
        Workspace::new,
        build_case_execution_environment,
    )
}

pub(super) fn evaluate_with(
    script: &Script,
    env: &ExecutionEnvironment,
    source_path: &Path,
    commands: &CommandRegistry,
    create_workspace: fn() -> std::io::Result<Workspace>,
    build_environment: fn(
        &ExecutionEnvironment,
        &CommandRegistry,
        &Path,
    ) -> std::io::Result<ExecutionEnvironment>,
) -> ExecutionReport {
    ExecutionReport {
        cases: script
            .cases
            .iter()
            .map(|c| {
                evaluate_case(
                    c,
                    script.before_each.as_ref(),
                    env,
                    source_path,
                    commands,
                    create_workspace,
                    build_environment,
                )
            })
            .collect(),
        file_errors: vec![],
    }
}

/// The evidence one concrete case has accumulated so far, threaded through
/// every step it executes.
///
/// Held as one value rather than as separate locals because every exit path —
/// pass, assertion failure, script error, runtime error — must report the same
/// accumulated evidence: a step that aborts the case must not drop the actions
/// and assertion blocks that already ran.
struct CaseExecution {
    /// The evidence context the next assertion block verifies: the initial
    /// checkpoint until a `$` action replaces it.
    checkpoint: Checkpoint,
    actions: Vec<ActionResult>,
    bindings: HashMap<String, Binding>,
    assertion_blocks: Vec<AssertionBlockResult>,
    /// Successful `write` (and future side-effecting) step count, independent
    /// of `actions`. See `RunSummary::steps_executed`.
    side_effects_executed: usize,
}

impl CaseExecution {
    fn new(checkpoint: Checkpoint) -> Self {
        Self {
            checkpoint,
            actions: Vec::new(),
            bindings: HashMap::new(),
            assertion_blocks: Vec::new(),
            side_effects_executed: 0,
        }
    }

    /// Ends the case with `status`, reporting the evidence gathered so far.
    fn finish(self, case_name: &str, source_path: &Path, status: CaseStatus) -> CaseResult {
        CaseResult {
            name: case_name.to_string(),
            source_path: Some(source_path.to_path_buf()),
            status,
            actions: self.actions,
            assertion_blocks: self.assertion_blocks,
            side_effects_executed: self.side_effects_executed,
        }
    }
}

/// The case-local inputs that stay fixed for every step of one step sequence.
struct StepContext<'a> {
    /// Which source block the steps being executed come from. Held here rather
    /// than decided at each origin-producing site, so one sequence cannot
    /// attribute its steps to two phases.
    phase: StepPhase,
    /// Used only to build diagnostic messages, which name the failing case.
    case_name: &'a str,
    /// This concrete case's isolated workspace: the root every `$` action runs
    /// in and every `write` step and workspace expectation resolves against.
    workspace: &'a Workspace,
    /// The environment `$` actions run under, with this case's own shim `bin`
    /// directory already prepended. See [`build_case_execution_environment`].
    env: &'a ExecutionEnvironment,
    /// Directory containing the `*.repor` file this case was loaded from.
    /// See `Checkpoint::repor_dir`.
    repor_dir: &'a Path,
}

impl StepContext<'_> {
    /// The prefix a diagnostic message uses to name the block a step is in.
    ///
    /// Empty for a case body: those messages predate `before_each` holding
    /// steps, and are what the existing e2e expectations and snapshots pin.
    fn phase_prefix(&self) -> &'static str {
        match self.phase {
            StepPhase::BeforeEach => "before_each ",
            StepPhase::Case => "",
        }
    }
}

/// How a step sequence ended when no step aborted the case.
enum StepSequenceOutcome {
    /// Every step ran.
    Completed,
    /// An assertion block failed, so the remaining steps were skipped.
    AssertionFailed,
}

/// A failure that ends a case before its remaining steps run.
///
/// Narrower than [`CaseStatus`] on purpose: a step can only ever abort a case
/// with a script error or a runtime error, so `Pass` / `Fail` stay
/// unrepresentable on the abort path rather than relying on every future
/// return site to pick the right variant.
enum StepAbort {
    Script(ScriptError),
    Runtime(RuntimeError),
}

impl From<StepAbort> for CaseStatus {
    fn from(abort: StepAbort) -> Self {
        match abort {
            StepAbort::Script(error) => CaseStatus::ScriptError(error),
            StepAbort::Runtime(error) => CaseStatus::RuntimeError(error),
        }
    }
}

/// A case that ends before any step runs, and so has no evidence to report.
fn abort_before_execution(case_name: &str, source_path: &Path, status: CaseStatus) -> CaseResult {
    CaseResult {
        name: case_name.to_string(),
        source_path: Some(source_path.to_path_buf()),
        status,
        actions: vec![],
        assertion_blocks: vec![],
        side_effects_executed: 0,
    }
}

fn evaluate_case(
    case: &Case,
    before_each: Option<&BeforeEach>,
    env: &ExecutionEnvironment,
    source_path: &Path,
    commands: &CommandRegistry,
    create_workspace: fn() -> std::io::Result<Workspace>,
    build_environment: fn(
        &ExecutionEnvironment,
        &CommandRegistry,
        &Path,
    ) -> std::io::Result<ExecutionEnvironment>,
) -> CaseResult {
    // Every case must contain at least one assertion block.
    let has_assertion_block = case
        .steps
        .iter()
        .any(|s| matches!(s, Step::AssertionBlock(_)));
    if !has_assertion_block {
        return abort_before_execution(
            &case.name,
            source_path,
            CaseStatus::ScriptError(ScriptError {
                message: format!(
                    "case '{}' has no assertion block; every case requires at least one assert {{ ... }} block",
                    case.name
                ),
                diagnostic_code: Some(DiagnosticCode::ParseMissingAssertionBlock),
                origin: None,
            }),
        );
    }

    // Each concrete case gets its own isolated workspace, destroyed when
    // this function returns. See docs/reference/semantics.md — Workspace lifecycle.
    let workspace = match create_workspace() {
        Ok(w) => w,
        Err(e) => {
            return abort_before_execution(
                &case.name,
                source_path,
                CaseStatus::RuntimeError(RuntimeError {
                    message: format!(
                        "case '{}': failed to create isolated case workspace: {e}",
                        case.name
                    ),
                    diagnostic_code: None,
                    origin: None,
                }),
            );
        }
    };

    // When commands are registered, materialize a fresh set of shims into this case's own `bin`
    // directory and prepend it to `env`'s PATH prefixes, so `$` steps resolve registered command
    // names through the shim before falling through to `env`'s own prefixes and the inherited
    // PATH. See docs/reference/semantics.md — Command resolution through PATH shims.
    let case_env = match build_environment(env, commands, workspace.root()) {
        Ok(case_env) => case_env,
        Err(e) => {
            return abort_before_execution(
                &case.name,
                source_path,
                CaseStatus::RuntimeError(RuntimeError {
                    message: format!(
                        "case '{}': failed to set up registered command shims: {e}",
                        case.name
                    ),
                    diagnostic_code: None,
                    origin: None,
                }),
            );
        }
    };

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

    let step_context = |phase| StepContext {
        phase,
        case_name: &case.name,
        workspace: &workspace,
        env: &case_env,
        repor_dir: &repor_dir,
    };
    let mut execution = CaseExecution::new(Checkpoint::initial(
        workspace.root().to_path_buf(),
        repor_dir.clone(),
    ));

    // `before_each` setup replays inside this concrete case's own workspace,
    // before the case body's first step runs, through the same executor and
    // against the same accumulating evidence. Its failures therefore belong to
    // this concrete case, and its steps are told apart from case body steps by
    // their `StepPhase`, not by a separate execution path.
    // See docs/reference/execution-model.md — Execution order and `before_each`.
    if let Some(before_each) = before_each {
        // Matched exhaustively rather than testing only for `Err`: a setup
        // assertion failure has to end the case, and the parser's `assert` ban
        // is the only reason that outcome cannot occur yet. Dropping the `Ok`
        // payload by pattern would let a later unit lift the ban and silently
        // report `Pass` for a case whose setup assertion failed.
        match execute_steps(
            before_each.steps(),
            &mut execution,
            &step_context(StepPhase::BeforeEach),
        ) {
            Ok(StepSequenceOutcome::Completed) => {}
            Ok(StepSequenceOutcome::AssertionFailed) => {
                return execution.finish(&case.name, source_path, CaseStatus::Fail);
            }
            Err(abort) => {
                return execution.finish(&case.name, source_path, abort.into());
            }
        }
    }

    let status = match execute_steps(&case.steps, &mut execution, &step_context(StepPhase::Case)) {
        Ok(StepSequenceOutcome::Completed) => CaseStatus::Pass,
        Ok(StepSequenceOutcome::AssertionFailed) => CaseStatus::Fail,
        Err(abort) => abort.into(),
    };

    execution.finish(&case.name, source_path, status)
}

/// Executes `steps` in source order, updating `execution` as each step
/// produces evidence.
///
/// `Err` carries the [`StepAbort`] of a case that cannot continue. The evidence
/// gathered before that point stays in `execution`, so the caller reports it
/// either way.
///
/// Steps are never reordered into phases, and an assertion block failure stops
/// the sequence before the next step.
/// See docs/reference/semantics.md — Assertion block and the checkpoint-based assertion ADR.
fn execute_steps(
    steps: &[Step],
    execution: &mut CaseExecution,
    ctx: &StepContext<'_>,
) -> Result<StepSequenceOutcome, StepAbort> {
    let case_name = ctx.case_name;
    let phase = ctx.phase_prefix();

    for (step_idx, step) in steps.iter().enumerate() {
        match step {
            Step::Action(action) => {
                match execute_action(&action.command, ctx.env, ctx.workspace.root()) {
                    Ok(result) => {
                        execution.checkpoint = Checkpoint::after_action(
                            result.clone(),
                            ctx.workspace.root().to_path_buf(),
                            ctx.repor_dir.to_path_buf(),
                        );
                        execution.actions.push(result);
                    }
                    Err(e) => {
                        return Err(StepAbort::Runtime(RuntimeError {
                            message: e.message,
                            diagnostic_code: None,
                            origin: Some(StepOrigin::new(ctx.phase, step_idx)),
                        }));
                    }
                }
            }

            Step::SideEffect(SideEffectingStep::WriteFile(write_step)) => {
                // The same resolver every text matcher uses: a raw literal, a
                // direct binding reference, and an interpolated literal all
                // reach `write_file` as one resolved `TextValue`.
                let content = match write_step
                    .content
                    .resolve_text_value(&TextResolutionContext::new(&execution.bindings))
                {
                    Ok(resolved) => resolved.into_value(),
                    Err(error) => {
                        return Err(StepAbort::Runtime(RuntimeError {
                            message: format!(
                                "case '{}': {}write step at step {} could not resolve its content: {}",
                                case_name,
                                phase,
                                step_idx + 1,
                                error.message,
                            ),
                            diagnostic_code: Some(error.diagnostic_code),
                            origin: Some(StepOrigin::new(ctx.phase, step_idx)),
                        }));
                    }
                };
                match ctx.workspace.write_file(&write_step.path, content.as_str()) {
                    Ok(()) => execution.side_effects_executed += 1,
                    Err(e) => {
                        return Err(StepAbort::Runtime(RuntimeError {
                            message: format!(
                                "case '{}': {}write step at step {} failed: {e}",
                                case_name,
                                phase,
                                step_idx + 1,
                            ),
                            diagnostic_code: Some(e.code()),
                            origin: Some(StepOrigin::new(ctx.phase, step_idx)),
                        }));
                    }
                }
            }

            Step::Binding(declaration) => {
                // Two separate guarantees, one per phase: `validate_bindings`
                // rejects a case body `let` with no preceding action, and the
                // parser's `let` ban keeps `before_each` from reaching here at
                // all. A unit that lifts that ban must give `before_each` the
                // equivalent validation, or this `expect` — and the
                // `actions.len() - 1` below — become panics.
                let action = execution
                    .checkpoint
                    .last_action
                    .as_ref()
                    .expect("binding capture is validated to follow an action");
                let bytes = match declaration.source.stream() {
                    crate::model::OutputSource::Stdout => &action.stdout,
                    crate::model::OutputSource::Stderr => &action.stderr,
                };
                let captured = match capture_text(bytes, declaration.source) {
                    Ok(value) => value,
                    Err((message, diagnostic_code)) => {
                        return Err(StepAbort::Runtime(RuntimeError {
                            message: format!(
                                "case '{}': {}binding '{}' at step {} failed: {message}",
                                case_name,
                                phase,
                                declaration.name,
                                step_idx + 1,
                            ),
                            diagnostic_code: Some(diagnostic_code),
                            origin: Some(StepOrigin::new(ctx.phase, step_idx)),
                        }));
                    }
                };
                let capture_mode = match declaration.source {
                    RuntimeEvidenceSource::StdoutExact | RuntimeEvidenceSource::StderrExact => {
                        CaptureMode::Exact
                    }
                    RuntimeEvidenceSource::StdoutLine | RuntimeEvidenceSource::StderrLine => {
                        CaptureMode::Line
                    }
                };
                execution.bindings.insert(
                    declaration.name.clone(),
                    Binding {
                        name: declaration.name.clone(),
                        value: BoundValue::Text(captured),
                        declaration_span: declaration.declaration_span,
                        source: BindingSource {
                            action_index: execution.actions.len() - 1,
                            stream: declaration.source.stream(),
                            capture_mode,
                        },
                    },
                );
            }

            Step::AssertionBlock(block) => {
                // Check that all expectations have the evidence they require.
                for expectation in block.expectations() {
                    if expectation.required_evidence().needs_action_result()
                        && execution.checkpoint.last_action.is_none()
                    {
                        return Err(StepAbort::Script(ScriptError {
                            message: format!(
                                "case '{}': {}assertion block at step {} uses a process expectation \
                                 (exit, stdout, stderr) but no '$' action has run yet; \
                                 the initial checkpoint has no last action result",
                                case_name,
                                phase,
                                step_idx + 1,
                            ),
                            diagnostic_code: Some(
                                DiagnosticCode::SemanticExpectationRequiresAction,
                            ),
                            origin: Some(StepOrigin::new(ctx.phase, step_idx)),
                        }));
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
                        return Err(StepAbort::Script(ScriptError {
                            message: format!(
                                "case '{}': {}assertion block at step {} has an invalid \
                                 expectation: {semantic_err}",
                                case_name,
                                phase,
                                step_idx + 1,
                            ),
                            diagnostic_code: Some(semantic_err.code()),
                            origin: Some(StepOrigin::new(ctx.phase, step_idx)),
                        }));
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
                    .map(|exp| {
                        evaluate_expectation_with_bindings(
                            exp,
                            &execution.checkpoint,
                            &execution.bindings,
                        )
                    })
                    .collect()
                {
                    Ok(results) => results,
                    Err(err) => {
                        return Err(StepAbort::Script(ScriptError {
                            message: format!(
                                "case '{}': {}assertion block at step {} has an unresolvable \
                                 contents_equals expected value: {}",
                                case_name,
                                phase,
                                step_idx + 1,
                                err.message,
                            ),
                            diagnostic_code: Some(err.diagnostic_code),
                            origin: Some(StepOrigin::new(ctx.phase, step_idx)),
                        }));
                    }
                };

                let block_result = AssertionBlockResult {
                    origin: StepOrigin::new(ctx.phase, step_idx),
                    expectations: expectation_results,
                    checkpoint_action_index: execution.actions.len().checked_sub(1),
                };
                let failed = block_result.has_failures();
                execution.assertion_blocks.push(block_result);

                if failed {
                    return Ok(StepSequenceOutcome::AssertionFailed);
                }
            }
        }
    }

    Ok(StepSequenceOutcome::Completed)
}

fn capture_text(
    bytes: &[u8],
    source: RuntimeEvidenceSource,
) -> Result<TextValue, (String, DiagnosticCode)> {
    let mut value = std::str::from_utf8(bytes)
        .map_err(|_| {
            (
                "captured output is not valid UTF-8".to_string(),
                DiagnosticCode::StepBindingNonUtf8,
            )
        })?
        .to_string();
    if matches!(
        source,
        RuntimeEvidenceSource::StdoutLine | RuntimeEvidenceSource::StderrLine
    ) {
        if value.ends_with("\r\n") {
            value.truncate(value.len() - 2);
        } else if value.ends_with('\n') {
            value.pop();
        }
        if value.contains(['\r', '\n']) {
            return Err((
                "captured output contains more than one line".to_string(),
                DiagnosticCode::StepBindingNotSingleLine,
            ));
        }
    }
    Ok(TextValue::new(value))
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
pub(super) fn build_case_execution_environment(
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
