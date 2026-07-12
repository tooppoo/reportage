# Path Matching

This document defines the current v0 rules for path-like config values and test file discovery.

## Scope

These rules apply to config values that represent paths.

Current examples:

- `reportage.commands.command.exec`
- `reportage.tests.path`

Future path-like config values should follow the same rules unless their specification explicitly says otherwise.

## Common path-like value rules

Path-like values are project-local relative paths.

Rules:

- absolute paths are forbidden;
- dot segments are forbidden anywhere;
- `/` is the config-level path separator;
- values are resolved relative to the config file directory;
- path normalization must not be used to escape the config directory.

Dot segments are forbidden uniformly. This includes both `.` and `..` as path segments.

Forbidden examples:

```kdl
path "./e2e/**/*.repor"
path "../e2e/**/*.repor"
path "e2e/./case.repor"
path "e2e/../case.repor"
path "/tmp/e2e/*.repor"

exec "./target/debug/rellog"
exec "../target/debug/rellog"
exec "tools/./rellog"
exec "tools/../rellog"
exec "/usr/local/bin/rellog"
```

Allowed examples:

```kdl
path "e2e/**/*.repor"
exec "target/debug/rellog"
exec "tools/rellog"
```

## Rationale

Dot segments are forbidden to keep config paths simple, project-local, and reproducible.

A value such as `e2e/../tests` could normalize to a project-local path, but allowing it would make review and diagnostics weaker.

## Test path globbing

`reportage.tests.path` values are glob patterns.

Rules:

- multiple patterns are allowed;
- `**` is allowed;
- each pattern is resolved relative to the config file directory;
- each pattern must match at least one file;
- directories are not test files;
- matched files are deduplicated;
- matched files are sorted before execution.

A pattern that matches no files is a config error.

## Execution order

Matched files are deduplicated and sorted before execution.

The exact sorting should be stable and deterministic. v0 should prefer lexicographic ordering over filesystem traversal order.

## Hidden files and directories

The v0 specification does not yet define special hidden-file behavior beyond the selected glob implementation. If this becomes observable or confusing, the behavior should be documented explicitly before stabilization.
