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

- v0 ships **no builtin templates**. Every `--template` name is "unknown" until later issues (`typescript-c8-tsx` and `golang`) add entries to the registry.
- Templates are resolved from a name through a registry built into the `reportage` binary. There is no support for loading a template file from disk in v0.
- The template resolution, template context, and rendering steps are kept as separate seams internally (see `reportage_core::shim_scaffold::ShimTemplate`), specifically so that a future external-template loader has somewhere to plug in without reworking the scaffold pipeline. v0 does not commit to what that loader would look like.
- A template renders against a small context. In v0 the only context field is `entry_point`, taken directly from `--entry-point`.
- `--entry-point` is never checked against the filesystem. Its existence, meaning, and interpretation are entirely up to the chosen template's own documentation.
- `--entry-point` is validated lexically: it must be non-empty and must not contain a NUL byte, line feed, or carriage return. A template that embeds `--entry-point` into a generated shell script must single-quote it (escaping any embedded single quote) so that no value can break out of its quoting or inject a second shell command.

## Template addition policy

Adding a template means adding an entry to the builtin registry (name plus a render function), not modifying the scaffold command itself. See `reportage_core::shim_scaffold::TemplateRegistry` and the module-level documentation in `shim_scaffold.rs` for the exact seam.

## Output path policy

- The parent directory of `--out` is created automatically if it does not already exist.
- If `--out` already exists as a regular file, `scaffold` fails unless `--force` is given; with `--force` the file is overwritten.
- If `--out` already exists as a directory, `scaffold` always fails, regardless of `--force`.
- If `--out` is a symlink, `scaffold` always fails, regardless of `--force` and regardless of what the symlink points to (including a dangling symlink). Writing through a symlink means the file actually written is not the path the caller named, so v0 refuses it outright rather than trying to infer caller intent.
- A generated file is given the owner-execute permission bit. No group or other execute bit is added; whatever those bits were after ordinary file creation (subject to the process umask) is left as-is.

## Failure diagnostics

An unknown `--template` value fails with a message that names the requested template and lists every template name currently registered (or states that none are registered, which is always true in v0). `--template`, `--entry-point`, and `--out` each fail the same way whether the flag was omitted entirely or given an explicit empty value.

## Related documents

- [shims.md](shims.md)
- [exit-codes.md](exit-codes.md)
- [ADR 20260708T062146Z](adr/20260708T062146Z_shim-scaffold-command.md)
