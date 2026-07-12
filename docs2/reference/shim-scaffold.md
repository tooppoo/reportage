# Shim Scaffold

This document describes `reportage shim scaffold`: a command that generates a coverage-integration shim file from a static builtin template.

For PATH-overlay command shims (the mechanism `.repor` actions resolve through at runtime), see [Shims](shims.md).

## Command

```sh
reportage shim scaffold --template <template> --entry-point <path> --out <path> [--force]
```

## Purpose

Wiring a coverage tool into an application's test command usually means writing a small wrapper script: set an instrumentation environment variable, or invoke the language runtime with a coverage flag, then exec the real entry point. That wrapper is boilerplate specific to a language and coverage tool, not to any particular project.

`reportage shim scaffold` writes that boilerplate for you, once, by rendering a builtin template with the values you pass on the command line.

## Scaffold, not a managed resource

The generated file is a one-time scaffold, not something reportage continues to own.

- reportage renders the template and writes the file. That is the entire lifecycle it participates in.
- After generation, the file belongs to the project. Edit it however the coverage tool, the package manager, or the language toolchain actually requires.
- reportage does not re-run scaffold generation, detect drift, or resync a previously generated file. If the template evolves, regenerate manually (with `--force`) or edit the existing file directly.

See [ADR: Shim Scaffold Command](../../docs/adr/20260708T062146Z_shim-scaffold-command.md) for the reasoning behind treating this as a scaffold rather than a managed integration.

## Non-goals

`scaffold` only substitutes CLI arguments into a static template. It deliberately does not:

- detect which coverage tool, package manager, or language toolchain the project uses;
- read `package.json`, `go.mod`, CI configuration, or any other project file;
- verify that `--entry-point` exists, is executable, or means what the template assumes it means;
- parse or validate coverage results, or judge a coverage threshold;
- modify `PATH` or any other environment state;
- generate CI configuration;
- keep a previously generated shim file in sync with anything.

## Template model

- `typescript-c8-tsx`, `golang`, and `rust` (all documented below) are the only builtin templates today. Any other `--template` name currently fails as unknown.
- Templates are resolved from a name through a registry built into the `reportage` binary. There is no support for loading a template file from disk in v0.
- The template resolution, template context, and rendering steps are kept as separate seams internally (see `reportage_core::shim_scaffold::ShimTemplate`), specifically so that a future external-template loader has somewhere to plug in without reworking the scaffold pipeline. v0 does not commit to what that loader would look like.
- A template renders against a small context. In v0 the only context field is `entry_point`, taken directly from `--entry-point`.
- `--entry-point` is never checked against the filesystem. Its existence, meaning, and interpretation are entirely up to the chosen template's own documentation.
- `--entry-point` is validated lexically: it must be non-empty and must not contain a NUL byte, line feed, or carriage return. A template that embeds `--entry-point` into a generated shell script must single-quote it (escaping any embedded single quote) so that no value can break out of its quoting or inject a second shell command.

## Template addition policy

Adding a template means adding an entry to the builtin registry (name plus a render function), not modifying the scaffold command itself. See `reportage_core::shim_scaffold::TemplateRegistry` and the module-level documentation in `shim_scaffold.rs` for the exact seam.

## `typescript-c8-tsx` template

```sh
reportage shim scaffold --template typescript-c8-tsx --entry-point my-app/index.ts --out shims/my-app
```

This template targets a common Node.js/TypeScript pairing: [`c8`](https://github.com/bcoe/c8) for Node.js/V8 coverage collection, and [`tsx`](https://github.com/privatenumber/tsx) to run a TypeScript entry point directly without a separate build step. The generated shim is a POSIX `sh` script that single-quotes `--entry-point` (reusing the same quoting `reportage_core::shim_scaffold::single_quote` applies to every template, not a template-specific reimplementation) and execs `npx c8 ... npx tsx "$entry_point" "$@"`, so the shim's own exit status is whatever `c8`/`tsx` produced and every extra argument passed to the shim reaches the entry point unchanged.

The generated shim `cd`s into its own directory (derived from `$0`) before doing anything else, so `--entry-point` and `--reports-dir` are resolved relative to where the shim file itself lives, not relative to whatever directory the shim happens to be invoked from. This matters because a command wired up through reportage runs with the case workspace as its working directory, not the shim's own directory (see [Execution model](execution-model.md)); if `--entry-point` or this project's `node_modules` live somewhere other than the shim's own directory, edit the `cd` target and these paths after generation to match.

The generated shim passes `--clean=false` to `c8`. A suite typically invokes the same shim once per test case, each as a separate `npx c8` process; `c8`'s own default (`--clean=true`) erases coverage from the temp directory before every run, so without `--clean=false` only the last invocation's coverage would survive in the final report. This accumulation has no notion of a suite run boundary — `c8` does not know when one suite run ends and the next begins — so a project that re-runs its suite without clearing `--reports-dir` in between risks a stale file from an earlier run masking a real coverage regression. Clearing `--reports-dir` before each fresh suite run is this project's responsibility, not something the shim or `scaffold` does.

At scaffold time, reportage does not read this project's `package.json`, `tsconfig.json`, or any `c8` configuration file, and does not check that `c8` or `tsx` are installed. When the generated shim later runs, `c8` itself may still perform its own configuration resolution (for example a `.c8rc` file or a `c8` field in `package.json`); reportage does not interpret or validate whatever `c8` finds there.

The generated shim's `npx c8` / `npx tsx` invocations are an initial scaffold, not a dependency-pinning mechanism: `npx` can resolve `c8`/`tsx` from wherever npm's package resolution finds them, which does not guarantee a project-local, version-pinned install. Manage `c8` and `tsx` as `devDependencies` in this project's `package.json`, and let the project's package manager pin their versions, the same way the generated file's own comments recommend.

### Assumptions and limits

This template provides only an initial `c8 + tsx` scaffold, not a complete or guaranteed-working TypeScript execution setup. Depending on the project, the generated file may need further edits after generation:

- replacing the `npx` invocations with a package-manager-specific run command (for example `pnpm exec` or `yarn`);
- replacing `tsx` with `ts-node` or a project-specific loader;
- running already-built JavaScript instead of a TypeScript source file directly;
- changing the `c8` reporter flags or `--reports-dir` output location;
- adjusting the `cd` target if `--entry-point` or `node_modules` live somewhere other than the shim's own directory — for example if `--out` places the shim in a nested `shim/` subdirectory of the project, as in this repo's own [`examples/shims/javascript`](../../examples/shims/javascript/).

## `golang` template

```sh
reportage shim scaffold --template golang --entry-point cli.go --out shims/my-app
```

This template assumes Go 1.20 or later, which introduced integration coverage via `go build -cover` plus the `GOCOVERDIR` environment variable. The generated shim is a POSIX `sh` script that single-quotes `--entry-point` (reusing the same quoting `reportage_core::shim_scaffold::single_quote` applies to every template) and, on every invocation, builds a coverage-instrumented binary with `go build -cover -o "$bin_path" "$entry_point"`, then execs that binary with `GOCOVERDIR` set to a fixed coverage output directory.

`--entry-point` is a `go build` target, not necessarily a file path: `cli.go`, `.`, and `./cmd/my-app` are all valid values, and the template embeds whatever is given verbatim.

Go coverage instrumentation is a build-time flag, not a runtime one, so the generated shim rebuilds the binary on every invocation rather than exec-ing a pre-built one. A project that wants to run an already-built binary instead must edit the generated shim after scaffold to remove the `go build` step and point `bin_path` at that binary directly.

At scaffold time, reportage does not read this project's `go.mod`, does not detect the installed Go version, and does not check that the `go` command exists.

By default, `go build -cover` instruments only packages in the main module; it does not instrument the standard library or external dependencies. Add `-coverpkg` to the generated shim's `go build` invocation if this project needs a different instrumentation scope.

Go coverage data is written when the program returns normally from `main` or exits via `os.Exit`. If the program terminates through an unrecovered panic or a fatal exception, coverage data from that run may be lost; reportage neither detects nor works around this. `GOCOVERDIR` accumulates a new file per process run rather than overwriting the previous one, which is what lets coverage from multiple invocations within one suite run add up correctly; it also means a stale file left over from an earlier suite run stays in `cover_dir` and can mask a real coverage regression unless this project clears `cover_dir` before each fresh suite run — the generated shim does not do this itself.

`work_dir`, `bin_path`, and `cover_dir` in the generated shim are fixed initial values, not derived from `--out`: v0's template context carries only `entry_point`, so scaffold has no project-specific destination to embed here even if it wanted to. Edit these paths to fit the project after generation.

The generated shim `cd`s into its own directory (derived from `$0`) before doing anything else, so `entry_point`, `work_dir`, and `cover_dir` are resolved relative to where the shim file itself lives, not relative to whatever directory the shim happens to be invoked from. This matters because a command wired up through reportage runs with the case workspace as its working directory, not the shim's own directory (see [Execution model](execution-model.md)); if `entry_point` or this project's `go.mod` live somewhere other than the shim's own directory, edit the `cd` target and these paths after generation to match.

### Assumptions and limits

This template assumes Go 1.20 or later (`go build -cover` and `GOCOVERDIR` were introduced in that release) and a build-on-run workflow. Depending on the project, the generated file may need further edits after generation:

- changing the `go build` target or flags, for example adding `-coverpkg`;
- running an already-built binary instead of building on every invocation;
- changing `work_dir`, `bin_path`, or `cover_dir` to project-specific locations;
- reconciling `GOCOVERDIR` with an existing coverage-collection setup;
- adjusting the `cd` target if `entry_point` or `go.mod` live somewhere other than the shim's own directory — for example if `--out` places the shim in a nested `shim/` subdirectory of the project, as in this repo's own [`examples/shims/go`](../../examples/shims/go/).

## `rust` template

```sh
reportage shim scaffold --template rust --entry-point my-app --out shims/my-app
```

This template targets Rust's LLVM source-based coverage instrumentation (`-C instrument-coverage`, stable since Rust 1.60). The generated shim is a POSIX `sh` script that single-quotes `--entry-point` (reusing the same quoting `reportage_core::shim_scaffold::single_quote` applies to every template) and, on every invocation, builds a coverage-instrumented binary with `RUSTFLAGS='-C instrument-coverage' cargo build --quiet --bin "$entry_point" --target-dir "$target_dir"`, then execs that binary with `LLVM_PROFILE_FILE` pointing into a fixed coverage output directory.

`--entry-point` is a cargo binary target name (the value `cargo build --bin` accepts), not a file path: the generated shim both selects the build target with it and derives the built binary's path (`$target_dir/debug/$entry_point`) from it.

`-C instrument-coverage` is a build-time rustc flag, not a runtime one, so the generated shim rebuilds the binary on every invocation rather than exec-ing a pre-built one, for the same reason the `golang` template documents above; cargo's incremental compilation keeps rebuilds after the first one cheap. The build uses its own target directory under `work_dir` so instrumented artifacts do not share (and repeatedly invalidate) the project's regular `target/` cache. A project that wants to run an already-built binary instead must edit the generated shim after scaffold to remove the `cargo build` step and point `bin_path` at that binary directly.

The generated shim passes `--quiet` to `cargo build` so a successful build stays silent: cargo's normal progress output goes to stderr, and a test asserting on the wired-up command's stderr would otherwise see build progress it did not expect.

At scaffold time, reportage does not read the project's `Cargo.toml`, does not detect the installed Rust version, and does not check that the `cargo` command exists.

Rust/LLVM coverage data is written when the program exits normally, including via `std::process::exit`. If the program terminates through an abort or a fatal signal, coverage data from that run may be lost; reportage neither detects nor works around this. `LLVM_PROFILE_FILE` names one `.profraw` file per process run (`%p` expands to the process ID, `%m` to the instrumented binary's signature), which is what lets coverage from multiple invocations within one suite run add up correctly; it also means a stale file left over from an earlier suite run stays in `cover_dir` and can mask a real coverage regression unless the project clears `cover_dir` before each fresh suite run — the generated shim does not do this itself. Turning the accumulated `.profraw` files into a report needs LLVM's `llvm-profdata`/`llvm-cov` (shipped by rustup's `llvm-tools` component) or a wrapper such as `cargo-llvm-cov`; the shim only collects the data.

`work_dir`, `target_dir`, `bin_path`, and `cover_dir` in the generated shim are fixed initial values, not derived from `--out`: v0's template context carries only `entry_point`, so scaffold has no project-specific destination to embed here even if it wanted to. Edit these paths to fit the project after generation.

The generated shim `cd`s into its own directory (derived from `$0`) before doing anything else, so cargo resolves the project's `Cargo.toml` from where the shim file itself lives, and `work_dir`/`cover_dir` are created there rather than in whatever directory the shim happens to be invoked from. This matters because a command wired up through reportage runs with the case workspace as its working directory, not the shim's own directory (see [Execution model](execution-model.md)); if the project's `Cargo.toml` lives somewhere other than the shim's own directory, edit the `cd` target after generation to match. `LLVM_PROFILE_FILE` is made absolute (prefixed with the shim's `$PWD` after that `cd`) so the coverage data still lands under the project even if the program changes its own working directory before exiting.

### Assumptions and limits

This template assumes `-C instrument-coverage` support (stable since Rust 1.60), a `cargo`-managed project whose `Cargo.toml` lives in the shim's own directory, and a build-on-run workflow. Depending on the project, the generated file may need further edits after generation:

- selecting a different build target or profile, for example `--release`, `-p <package>` in a cargo workspace, or feature flags;
- running an already-built binary instead of building on every invocation;
- changing `work_dir`, `target_dir`, `bin_path`, or `cover_dir` to project-specific locations;
- reconciling `RUSTFLAGS`/`LLVM_PROFILE_FILE` with an existing coverage-collection setup (for example `cargo-llvm-cov`, which manages both itself);
- adjusting the `cd` target if the project's `Cargo.toml` lives somewhere other than the shim's own directory — for example if `--out` places the shim in a nested `shim/` subdirectory of the project, as in this repo's own [`examples/shims/rust`](../../examples/shims/rust/).

## Output path policy

- The parent directory of `--out` is created automatically if it does not already exist.
- If `--out` already exists as a regular file, `scaffold` fails unless `--force` is given; with `--force` the file is overwritten.
- If `--out` already exists as a directory, `scaffold` always fails, regardless of `--force`.
- If `--out` is a symlink, `scaffold` always fails, regardless of `--force` and regardless of what the symlink points to (including a dangling symlink). Writing through a symlink means the file actually written is not the path the caller named, so v0 refuses it outright rather than trying to infer caller intent.
- A generated file is given the owner-execute permission bit. No group or other execute bit is added; whatever those bits were after ordinary file creation (subject to the process umask) is left as-is.

## Failure diagnostics

An unknown `--template` value fails with a message that names the requested template and lists every template name currently registered (or states that none are registered, for an embedder that constructs an empty registry directly; the CLI's own registry always has at least `golang`, `rust`, and `typescript-c8-tsx`). `--template`, `--entry-point`, and `--out` each fail the same way whether the flag was omitted entirely or given an explicit empty value.

`--template`, `--entry-point`, and `--out` are validated independently of each other. If more than one is empty, missing, or (for `--entry-point`) lexically unsafe in the same invocation, every one of those problems is reported together in a single failure, not just the first one `scaffold` happens to check. A caller who fixes the reported problems should not have to rerun `scaffold` once per remaining problem it already could have reported.

Every `--out` conflict message (existing file, existing directory, existing symlink) names `--out` explicitly and includes the concrete path that was rejected, so the message is unambiguous about which argument and which path it concerns.

## Related documents

- [Shims](shims.md)
- [Execution model](execution-model.md)
- [Exit codes](exit-codes.md)
- [ADR: Shim Scaffold Command](../../docs/adr/20260708T062146Z_shim-scaffold-command.md)
