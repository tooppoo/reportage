# Configuration

This document describes the current v0 configuration specification for reportage.

The configuration format is KDL v2. The default config file is `reportage.kdl`.

## Loading

v0 behavior:

- by default, read `./reportage.kdl`;
- support `--config <path>` for explicit config selection;
- do not perform upward directory discovery in v0.

## Root and version

A config document must use `reportage` as its root node.

```kdl
reportage {
  config {
    version 1
  }
}
```

Rules:

- `config` is required;
- `version` is required;
- `version` must be an integer;
- v0 supports version `1`;
- unsupported versions are config errors.

## Commands

Commands are registered under `reportage.commands`.

```kdl
reportage {
  config {
    version 1
  }

  commands {
    command "rellog" {
      exec "target/debug/rellog"
    }
  }
}
```

Rules:

- `commands` is optional;
- `command` may appear multiple times;
- each `command` node requires exactly one string argument;
- command ids must be unique;
- command ids follow the same validity rules as a shim file name: non-empty, no `/`, not `.` or `..`, no NUL byte;
- each command requires one `exec` node in v0;
- `exec` requires exactly one string argument;
- `exec` is a path-like value (relative, no absolute paths, no dot segments);
- unknown nodes inside `commands` or inside a `command` block are config errors, not silently ignored.

The command id is the name used in `$` shell steps. For each concrete case, the runner generates a case-local PATH shim with that name in a fresh `<workspace>/bin` directory and prepends it to the action shell's `PATH`. The configured `exec` value is the executable target used by that shim. See [Shims](shims.md) for the PATH overlay shim model.

### `exec` path resolution

`exec` is a path relative to the config file's directory, never to the process's current working directory.

Resolution happens at run setup, after config parsing: `exec` is joined onto the config file's directory and turned into an absolute path via lexical absolutization (no filesystem access, comparable to Rust's `std::path::absolute`). This means:

- resolution does not require the target executable to already exist at config-load time (it commonly does not — e.g. a not-yet-built `target/debug/myapp`);
- resolution does not follow symlinks in the path; the resulting absolute path is not canonicalized.

The absolute path produced by this resolution becomes the shim's executable invocation target (see [Shims](shims.md) — Executable invocation targets).

### v0 scope

- v0's `commands.command.exec` is a single program path with no fixed arguments. Interpreter-plus-script invocations (e.g. `ruby tool.rb`) and a config syntax for fixed arguments are shim concepts (see [Shims](shims.md)) but are not exposed through `reportage.kdl` in v0; both are separate, not-yet-decided follow-ups.
- explicit script mode (`reportage <script>...`) never reads a config file, so it never registers commands. Config-driven command registration requires `--config <path>` or the default config mode (`reportage` with no script arguments). Explicit script mode is not planned to gain a separate command-registration option.

## Tests

Test discovery is configured under `reportage.tests`.

```kdl
reportage {
  config {
    version 1
  }

  tests {
    path "e2e/**/*.repor"
  }
}
```

Rules:

- `tests` is required unless test files are provided explicitly by CLI arguments;
- `path` may appear multiple times;
- each `path` node requires exactly one string argument;
- each `path` value is a glob pattern;
- each `path` value is also a path-like value;
- each pattern must match at least one file;
- matched files are deduplicated and sorted before execution.

See [Path matching](path-matching.md).

## Complete example

```kdl
reportage {
  config {
    version 1
  }

  commands {
    command "rellog" {
      exec "target/debug/rellog"
    }
  }

  tests {
    path "e2e/**/*.repor"
    path "tests/e2e/**/*.repor"
  }
}
```

## Validation errors

Invalid configuration should fail before test execution starts.

Typical config errors include:

- missing `reportage.config.version`;
- unsupported config version;
- duplicate command id;
- invalid command id (empty, contains `/`, `.`, `..`, or a NUL byte);
- missing command `exec`;
- more than one `exec` node in a `command` block;
- non-string `command` id or `exec` argument;
- unknown node inside `commands` or inside a `command` block;
- absolute path-like value;
- dot segment in a path-like value;
- `tests.path` pattern that matches no files.
