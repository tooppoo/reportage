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
- each command requires one `exec` node in v0;
- `exec` requires exactly one string argument;
- `exec` is a path-like value.

The command id is the name used in `$` shell steps. The runner generates a case-local PATH shim with that name. The configured `exec` value is the executable target used by that shim.

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

See [Path Matching](path-matching.md).

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
- missing command `exec`;
- absolute path-like value;
- dot segment in a path-like value;
- `tests.path` pattern that matches no files.
