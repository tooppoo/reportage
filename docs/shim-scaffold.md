# Shim Scaffold

This document describes `reportage shim scaffold`: a command that generates a coverage-integration shim file from a static builtin template.

For PATH-overlay command shims (the mechanism `.repor` actions resolve through at runtime), see [shims.md](shims.md).

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

See [ADR 20260708T062146Z](adr/20260708T062146Z_shim-scaffold-command.md) for the reasoning behind treating this as a scaffold rather than a managed integration.

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

- `typescript-c8-tsx` and `golang` (both documented below) are the only builtin templates today. Any other `--template` name currently fails as unknown.
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

A relative `--entry-point` is resolved from the generated shim's runtime working directory when it executes, not from the directory `scaffold` was run in.

At scaffold time, reportage does not read this project's `package.json`, `tsconfig.json`, or any `c8` configuration file, and does not check that `c8` or `tsx` are installed. When the generated shim later runs, `c8` itself may still perform its own configuration resolution (for example a `.c8rc` file or a `c8` field in `package.json`); reportage does not interpret or validate whatever `c8` finds there.

The generated shim's `npx c8` / `npx tsx` invocations are an initial scaffold, not a dependency-pinning mechanism: `npx` can resolve `c8`/`tsx` from wherever npm's package resolution finds them, which does not guarantee a project-local, version-pinned install. Manage `c8` and `tsx` as `devDependencies` in this project's `package.json`, and let the project's package manager pin their versions, the same way the generated file's own comments recommend.

### Assumptions and limits

This template provides only an initial `c8 + tsx` scaffold, not a complete or guaranteed-working TypeScript execution setup. Depending on the project, the generated file may need further edits after generation:

- replacing the `npx` invocations with a package-manager-specific run command (for example `pnpm exec` or `yarn`);
- replacing `tsx` with `ts-node` or a project-specific loader;
- running already-built JavaScript instead of a TypeScript source file directly;
- changing the `c8` reporter flags or `--reports-dir` output location.

## `golang` template

```sh
reportage shim scaffold --template golang --entry-point cli.go --out shims/my-app
```

This template assumes Go 1.20 or later, which introduced integration coverage via `go build -cover` plus the `GOCOVERDIR` environment variable. The generated shim is a POSIX `sh` script that single-quotes `--entry-point` (reusing the same quoting `reportage_core::shim_scaffold::single_quote` applies to every template) and, on every invocation, builds a coverage-instrumented binary with `go build -cover -o "$bin_path" "$entry_point"`, then execs that binary with `GOCOVERDIR` set to a fixed coverage output directory.

`--entry-point` is a `go build` target, not necessarily a file path: `cli.go`, `.`, and `./cmd/my-app` are all valid values, and the template embeds whatever is given verbatim.

Go coverage instrumentation is a build-time flag, not a runtime one, so the generated shim rebuilds the binary on every invocation rather than exec-ing a pre-built one. A project that wants to run an already-built binary instead must edit the generated shim after scaffold to remove the `go build` step and point `bin_path` at that binary directly.

At scaffold time, reportage does not read this project's `go.mod`, does not detect the installed Go version, and does not check that the `go` command exists.

By default, `go build -cover` instruments only packages in the main module; it does not instrument the standard library or external dependencies. Add `-coverpkg` to the generated shim's `go build` invocation if this project needs a different instrumentation scope.

Go coverage data is written when the program returns normally from `main` or exits via `os.Exit`. If the program terminates through an unrecovered panic or a fatal exception, coverage data from that run may be lost; reportage neither detects nor works around this.

`work_dir`, `bin_path`, and `cover_dir` in the generated shim are fixed initial values, not derived from `--out`: v0's template context carries only `entry_point`, so scaffold has no project-specific destination to embed here even if it wanted to. Edit these paths to fit the project after generation.

### Assumptions and limits

This template assumes Go 1.20 or later (`go build -cover` and `GOCOVERDIR` were introduced in that release) and a build-on-run workflow. Depending on the project, the generated file may need further edits after generation:

- changing the `go build` target or flags, for example adding `-coverpkg`;
- running an already-built binary instead of building on every invocation;
- changing `work_dir`, `bin_path`, or `cover_dir` to project-specific locations;
- reconciling `GOCOVERDIR` with an existing coverage-collection setup.

## Output path policy

- The parent directory of `--out` is created automatically if it does not already exist.
- If `--out` already exists as a regular file, `scaffold` fails unless `--force` is given; with `--force` the file is overwritten.
- If `--out` already exists as a directory, `scaffold` always fails, regardless of `--force`.
- If `--out` is a symlink, `scaffold` always fails, regardless of `--force` and regardless of what the symlink points to (including a dangling symlink). Writing through a symlink means the file actually written is not the path the caller named, so v0 refuses it outright rather than trying to infer caller intent.
- A generated file is given the owner-execute permission bit. No group or other execute bit is added; whatever those bits were after ordinary file creation (subject to the process umask) is left as-is.

## Failure diagnostics

An unknown `--template` value fails with a message that names the requested template and lists every template name currently registered (or states that none are registered, for an embedder that constructs an empty registry directly; the CLI's own registry always has at least `golang` and `typescript-c8-tsx`). `--template`, `--entry-point`, and `--out` each fail the same way whether the flag was omitted entirely or given an explicit empty value.

`--template`, `--entry-point`, and `--out` are validated independently of each other. If more than one is empty, missing, or (for `--entry-point`) lexically unsafe in the same invocation, every one of those problems is reported together in a single failure, not just the first one `scaffold` happens to check. A caller who fixes the reported problems should not have to rerun `scaffold` once per remaining problem it already could have reported.

Every `--out` conflict message (existing file, existing directory, existing symlink) names `--out` explicitly and includes the concrete path that was rejected, so the message is unambiguous about which argument and which path it concerns.

## Related documents

- [shims.md](shims.md)
- [exit-codes.md](exit-codes.md)
- [ADR 20260708T062146Z](adr/20260708T062146Z_shim-scaffold-command.md)
