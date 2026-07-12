# Shim Scaffold Command

- Status: Accepted
- Created: 2026-07-08T06:21:46Z

## Context

Issue #127 asked for a `reportage shim scaffold` command that writes a coverage-integration shim file (a small wrapper script that sets up coverage instrumentation, then execs an application's real entry point) so that adopting reportage for a new language/coverage-tool combination does not require hand-writing that wrapper from scratch every time.

This is a new CLI subcommand, a new output-writing behavior, and a new place where reportage decides what it will and will not manage on the caller's behalf. It also arrives before any real template exists: #127 only builds the scaffold pipeline itself (CLI parsing, template lookup, rendering, output-path handling), while the first real templates (`typescript-c8-tsx`, `golang`) are follow-up issues. Several decisions here shape both what #127 can commit to and what those follow-up issues inherit, so they belong in an ADR rather than only in the issue/PR discussion:

- whether reportage treats a generated shim as a resource it keeps managing, or as a one-time scaffold;
- how much of the project/toolchain state reportage is allowed to inspect or assume;
- whether the v0 builtin-only template model is allowed to bake in assumptions that later block loading templates from outside the binary;
- the output-path and permission policy for a command that writes an executable file the caller did not necessarily expect to already exist.

## Decision

### A scaffold, not a managed resource

`reportage shim scaffold` renders a template once and writes it to `--out`. That write is the entire lifecycle reportage participates in. It does not detect coverage tools, package managers, or language toolchains; it does not read `package.json`, `go.mod`, or CI configuration; it does not verify `--entry-point` against the filesystem; and it never re-runs, resyncs, or diffs a previously generated file. The generated file becomes an ordinary project-owned file the moment it is written — indistinguishable, from reportage's perspective, from a file the project author wrote by hand.

This mirrors the existing PATH-overlay shim model's separation of concerns (see `docs/shims.md`): reportage decides *that* a command resolves to a particular executable invocation, never *how* that invocation collects coverage. `scaffold` extends that same boundary to the initial authoring step, instead of drawing a new one.

### Builtin-only in v0, without foreclosing external templates

v0's `TemplateRegistry::builtin()` (`reportage_core::shim_scaffold`) returns zero entries — `typescript-c8-tsx` and `golang` are added by follow-up issues, not this one. Every `--template` value is therefore "unknown" until those land, and this is exactly what #127's own tests assert.

Even though every template is currently a plain Rust function embedded in the binary, `TemplateRegistry` and `scaffold()` never look at a template except through the `ShimTemplate` trait (`fn render(&self, ctx: &TemplateContext) -> String`). Template name resolution, the render context, and rendering itself are three separate seams for this reason: a future loader that reads template files from disk only has to produce something implementing `ShimTemplate` and register it under a name — it does not need `scaffold()` or the CLI layer to change. v0 does not design that loader; it only avoids writing code that assumes a builtin Rust function is the only possible shape of a template.

### `--entry-point` is a lexical value, not a filesystem reference

`--entry-point` is copied verbatim into the render context after checking only that it is non-empty and contains no NUL byte, line feed, or carriage return. reportage never stats, canonicalizes, or otherwise resolves it. What it means (a path, a package script name, a build target) is entirely up to the chosen template's documentation, since v0 has no template-independent notion of "entry point" to validate against.

The NUL/LF/CR checks exist because `--entry-point` is embedded into generated file content, most commonly a shell script. A newline or carriage return could inject an unintended second line; a NUL byte could truncate a C-string consumer downstream. A template that embeds `--entry-point` into shell script text must additionally single-quote it (escaping embedded single quotes) so that shell metacharacters in the value cannot break out of the quoting.

A NUL byte can never actually arrive through `--entry-point` in practice: the OS cannot represent one in `argv`. The check exists anyway as defense in depth for any caller that constructs a `TemplateContext` directly rather than through CLI argument parsing, and is covered by a direct unit test rather than a CLI-level one for that reason.

### "missing" and "empty" are the same failure

`--template`, `--entry-point`, and `--out` are `Option<String>` at the clap layer (`required = true` is deliberately not used), and the CLI layer collapses an absent flag and an explicitly empty value (`--template ''`) to the same empty `String`/`PathBuf` before calling into `reportage_core::shim_scaffold::scaffold`. This gives every argument exactly one validation path and one error message for "you didn't give me a usable value," instead of clap producing one message for "flag not given" and application code producing a different one for "flag given as empty."

### All argument-shape violations are collected, not reported one at a time

`--template`, `--entry-point`, and `--out` are independent of each other: none of their emptiness or (for `--entry-point`) lexical-safety checks depends on another argument's value. `scaffold` collects every violation among these three into a single `ScaffoldError::InvalidRequest(Vec<RequestViolation>)` and reports all of them together, rather than returning on the first one found.

This was added after review feedback pointed out that fail-fast validation forces a caller who invoked `scaffold` with several missing arguments to fix them one at a time, rediscovering each remaining problem only by rerunning the command. Since these checks are pure string/path inspections with no filesystem access and no dependency on each other, there is no ordering reason to stop at the first failure. The output-path policy and template resolution remain fail-fast (see below): those checks do depend on state (the filesystem, the template registry) and on each other in ways the argument-shape checks do not, so collecting *all* possible failures across the whole command would require doing filesystem I/O and template lookups speculatively even when the argument shape alone is already invalid.

Every `--out`-conflict message (existing file, existing directory, existing symlink) was also reworded during the same pass to explicitly name `--out` and include the concrete rejected path, rather than an ambiguous "it" referring back to a quoted path. `docs/shim-scaffold.md`'s "Failure diagnostics" section and the `e2e/shims/*.repor` fixtures reflect the current wording.

### Output-path policy: read-only checks before template resolution

`--out`'s existing-file/directory/symlink policy is checked before template name resolution, and that check performs no filesystem mutation (no directory creation, no write). This ordering was chosen over checking the template first for two reasons:

- An unknown `--template` should not mask an `--out` conflict the caller also needs to fix. Reporting whichever problem is cheaper to detect, first, gets the caller to a fully-valid invocation in fewer round trips.
- Since the check is read-only, ordering it first costs nothing when the template turns out to be unknown anyway — which, in v0, it always does.

Within that policy: a plain existing regular file is the only conflict `--force` can override. A directory or a symlink at `--out` is rejected unconditionally, `--force` or not. `symlink_metadata` (not `metadata`) is used specifically so a symlink is caught as a symlink even when it points at a regular file, at a directory, or at nothing (a dangling symlink) — the file `scaffold` would actually write through is not the path the caller named, and v0 does not attempt to infer whether that's what the caller intended. Directory creation for `--out`'s parent, rendering, and the write itself all happen only after template resolution succeeds, so a request that fails on an unknown template performs no filesystem mutation at all.

### Permission policy: owner-execute only

A generated file has the owner-execute bit added after it is written; no group or other execute bit is added by `scaffold`. Whatever group/other bits ordinary file creation produced (subject to the process umask) are left untouched. This is narrower than the existing PATH-overlay `CommandShim::materialize`, which sets a fixed `0o755` — that shim is reportage-managed and short-lived (materialized fresh per case, inside a directory reportage itself controls), whereas a scaffolded file is handed to the project permanently. Granting only the owner execute bit avoids reportage deciding a group/world-executable policy for a file it will never touch again.

### Exit codes reuse the run command's numbering, not its meaning

`shim scaffold` exits `0` on success, `2` for any request-validation failure (empty/missing arguments, an unknown template, or an `--out` conflict), and `3` for an OS-level I/O failure while creating the parent directory, writing the file, or setting permissions. These reuse the same numbers `docs/exit-codes.md` assigns to "script/config validation error" and "runtime/infrastructure error" for the default run command, even though `shim scaffold` has no script or config file and produces no `result.json`. The numbers were kept aligned because both meanings generalize cleanly ("the requested operation could not be treated as valid input" / "an OS-level failure occurred while doing required I/O"), and a caller scripting around `reportage` gets one consistent rule ("2 means fix your input, 3 means something broke") across both command shapes instead of two unrelated numbering schemes to remember.

### A subcommand namespace, accepting the positional-collision tradeoff

`Cli` gains `#[command(subcommand)] command: Option<Commands>` alongside the pre-existing `scripts: Vec<PathBuf>` positional. clap resolves the first non-flag token against defined subcommand names before falling back to positional parsing, so `reportage shim ...` always dispatches to the `shim` subcommand, never to explicit-script mode. The accepted consequence: a `.repor` script literally named `shim` (in the working directory, invoked as a bare filename) cannot be run positionally; `./shim` or a path with a directory component still works, since only the bare first token is subcommand-matched.

## Alternatives Considered

### Detect environment/toolchain state during scaffold

Considered having `scaffold` inspect `package.json`, `go.mod`, or an existing coverage tool installation to pick sensible defaults or validate `--entry-point`.

Rejected for v0: environment detection is exactly the kind of implicit, hard-to-predict behavior reportage avoids elsewhere (see `docs/philosophy.md` / `docs/design-principle.md`). A static, fully-argument-driven scaffold is predictable and easy to explain, and keeps `scaffold` usable in contexts (CI generation scripts, templated project skeletons) where the project state it would need to inspect may not exist yet.

### Keep reportage managing the generated shim after creation

Considered recording generated shims in some reportage-owned manifest, so a later command could detect drift or regenerate them in bulk.

Rejected: this would turn a one-shot scaffold into an ongoing managed resource, which contradicts the "your project owns this file" framing and adds a manifest format, migration story, and drift-detection behavior with no immediate requirement driving it. If a real need for bulk regeneration emerges later, it can be layered on without revisiting this decision, since nothing here prevents a future command from reading existing generated files and deciding what to do with them.

### `--out` conflict policy: allow `--force` to bypass the symlink/directory checks

Considered letting `--force` also permit overwriting a directory or writing through a symlink.

Rejected: overwriting a directory has no sensible single-file semantics to fall back to (recursive delete? refuse regardless?), and writing through a symlink means the actual write target is a path the caller never named, which `--force`'s "I know there's something here, replace it" intent does not obviously cover. Restricting `--force` to the one case with an unambiguous meaning (replace this regular file's content) keeps the flag's behavior easy to state precisely.

### Give `shim scaffold` its own, disjoint exit code range

Considered a exit code table for `shim scaffold` that shares no numbers with the run command's table, to avoid implying the two commands' `2`/`3` mean identical things.

Rejected: the categories genuinely are the same shape ("invalid input" vs. "runtime I/O failure") even though the concrete causes differ, and giving every subcommand its own private exit-code numbering would make `reportage`'s exit code harder to reason about as a whole for a caller that just wants to know "did something I need to fix cause this, or did the environment break." `docs/exit-codes.md` now says explicitly that `shim scaffold` has its own table, so the reuse is documented rather than assumed.

## Consequences

### Positive Consequences

- The scaffold pipeline (validation, template lookup, rendering, output-path policy, permissions) is fully exercised — by unit tests through a test-fixture template, and by e2e tests through every failure path reachable without a real template — before any real template exists, so #128/#129 only need to add template content, not pipeline behavior.
- The `ShimTemplate` trait seam means adding `typescript-c8-tsx` and `golang` is "register a render function," and a later external-template loader is an additive change, not a rework.
- The project retains full control over generated files immediately: no reportage-owned state to keep in sync, no surprising re-generation.

### Negative Consequences

- Until #128/#129 land, `shim scaffold` cannot successfully generate anything — every invocation fails with "unknown template." This is intentional scope-splitting per the issue, but it does mean v0 alone ships a command with no successful end-to-end outcome.
- A `.repor` script named exactly `shim` cannot be invoked positionally by its bare filename (see "accepting the positional-collision tradeoff" above).
- Reusing exit codes `2`/`3` across two commands with different failure vocabularies means a caller must still consult `docs/exit-codes.md` to know which table applies; the number alone does not fully disambiguate cause without knowing which command produced it.
- The `--out` existing-file/directory/symlink check (`std::fs::symlink_metadata`) and the eventual `std::fs::write` are two separate filesystem operations, not one atomic one. Something else could replace `--out` with a symlink in the narrow window between them, and the write would then follow it. Closing this fully (e.g. opening the destination with a no-follow flag) needs a platform-specific dependency this v0 foundation does not take on, since `shim scaffold` is a single-user local CLI operation, not a service exposed to another, potentially adversarial, party racing the same path.

### Neutral Consequences

- `TemplateContext` in v0 carries only `entry_point`. Later templates that need more context fields (e.g. a package manager name for `typescript-c8-tsx`) will extend `TemplateContext`, which is an additive change to a `pub` struct's fields — not addressed further here.

## Non-Goals

- Designing the concrete external-template-file loading mechanism referenced above; only the internal seam that would let one exist is in scope here.
- Adding `typescript-c8-tsx`, `golang`, or any other real template; those are follow-up issues.
- Any change to the PATH-overlay shim model (`docs/shims.md`) or the shim invocation event protocol.
