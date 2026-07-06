# Separate ExecutionReport from Output Rendering

- Status: Accepted
- Created: 2026-07-06T00:10:18Z

## Context

`reportage run` is expected to gain machine-readable output (issue #75), and eventually more than one output format (`json`, `ndjson`, `junit`, `tap`, GitHub annotations, ...). Before #75, the CLI's `run_scripts` / `run_with_config` returned a `RunResult` that was already format-agnostic in structure (cases, file errors, structured diagnostic codes), but the only consumer of that structure was a `print_results` function embedded directly in `reportage-cli/src/main.rs`, interleaved with argument parsing, mode selection, and artifact writing.

Adding a second output format directly inside `main.rs` would have made the CLI's entry point responsible for both selecting *and* implementing every output format, and would have made it easy for a JSON renderer to drift from the human-readable renderer by reading different fields or re-deriving information from formatted strings instead of the shared structured model.

## Decision

The result type returned by the runner (evaluator + CLI orchestration) is renamed from `RunResult` to `ExecutionReport`, to name it for what it actually is: a display-format-agnostic record of a run, not a step in producing one particular output. `CaseResult`, `ActionResult`, `AssertionBlockResult`, and the rest of `reportage-core::result` remain as-is, as internal substructures of `ExecutionReport`.

Output rendering is extracted into a `reportage-cli::render` module:

- `render::OutputRenderer` is a trait with one method, `render(&self, report: &ExecutionReport)`.
- `render::human::HumanRenderer` implements it, and contains exactly the printing logic that previously lived in `main.rs`'s `print_results` / `print_failed_expectation` / `print_expectation_detail`, unchanged in behavior.
- `main.rs` no longer contains any `println!`/`eprintln!` calls for run results. It builds an `ExecutionReport`, writes artifacts, then hands the report to a renderer it selects (`HumanRenderer` today) and exits with `report.exit_code()`.

This keeps the runner (`evaluator`, `executor`) fully unaware that rendering exists at all — it only ever produces an `ExecutionReport`. Adding a future `json` (or other) renderer means adding a new `OutputRenderer` implementation and a CLI-layer branch that picks it; it does not require touching the evaluator, executor, or `ExecutionReport` itself. `--format` selection, when it is introduced in #75, is this same CLI-layer branch and is out of scope here.

## Alternatives Considered

### Keep `RunResult` as the public name

Rejected: the name described the mechanics of "a run" rather than its role as the shared input to every renderer. Renaming it costs a mechanical, low-risk change now, versus leaving a confusing name in place once multiple renderers exist and `RunResult` is clearly not "the CLI's result" but "the thing renderers render".

### Put renderers in `reportage-core` instead of `reportage-cli`

Rejected for now: choosing an output format is a CLI-facing concern (`--format`, stdout/stderr shape), not a core execution concern. Keeping renderers in `reportage-cli` matches the principle that the CLI layer resolves format and selects a renderer, while `reportage-core` stays focused on parsing, evaluation, and the artifact JSON (which is a separate, already-existing machine-readable output distinct from `--format`).

### Have renderers return a `String` instead of printing directly

Rejected: the existing human-readable output intentionally splits between stdout (pass/fail labels, meant to be greppable) and stderr (diagnostic detail). Collapsing that into a single returned string would need a second abstraction to represent "which stream" per line, without changing behavior. Writing directly, as `print_results` did, was preserved as the simplest option that doesn't regress this issue's "no behavior changes" goal.

## Consequences

### Positive Consequences

- `ExecutionReport` is the single, named input every current and future renderer consumes; no renderer reads `main.rs`-local strings or re-derives categorization from message text.
- The evaluator and executor have zero references to output formatting, and cannot regress into producing renderer-specific data.
- Adding a JSON (or other) renderer in #75 is additive: a new `render::json` module and one new match arm in the CLI, with no runner changes.

### Negative Consequences

- One more module boundary (`reportage-cli::render`) for a project that, until #75 lands, still only has one renderer.

### Neutral Consequences

- No user-visible output changed. `HumanRenderer`'s output is byte-for-byte the same as the prior `print_results`.
- The existing artifact JSON (`result.json`, written by `reportage-core::artifact::ArtifactWriter`) is a separate, pre-existing machine-readable output and is not affected by this decision.
