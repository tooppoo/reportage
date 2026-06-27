# Syntax

This document describes the intended v0 syntax for reportage scripts.

The syntax is intentionally small. A test file is a module. The module may define one `before_each` block and multiple `case` blocks. Each `case` may optionally define case-local `params`. Shell actions are written with `$`. Checkpoint-level verification blocks are written with `assert { ... }`.

## Overview

```reportage
before_each {
  file ".rellog/config.kdl" <<'KDL'
project "example"
kind "feature"
KDL
}

case "check output" {
  params {
    variant "human" {
      ARGS = ""
    }

    variant "json" {
      ARGS = "--json"
    }
  }

  file ".rellog/entries/001.kdl" <<'KDL'
entry "add feature" {
  kind "feature"
}
KDL

  $ rellog check ${ARGS}

  assert {
    exit 0
    stderr empty
  }
}
```

## Test module

A test module is one reportage script file.

A module may contain:

```text
before_each? case*
```

v0 rules:

- `before_each` is optional.
- At most one `before_each` block is allowed.
- `before_each` must appear before any `case` block.
- A module may contain multiple `case` blocks.
- `case` blocks cannot be nested.
- Grouping is done by file boundaries, not nested `describe`-style syntax.

## `before_each`

`before_each` defines setup steps that run before every concrete case in the module.

```reportage
before_each {
  file ".rellog/config.kdl" <<'KDL'
project "example"
kind "feature"
KDL
}
```

`before_each` is intended for deterministic module-level setup. v0 does not provide `before_all`, `after_all`, or `after_each`.

Recommended v0 restriction:

- `before_each` should contain setup steps such as `file` and ordinary shell setup steps.
- If setup needs normal filesystem operations, use `$` steps such as `$ mkdir -p ...` or `$ cp -R ... ...`.
- Assertion blocks and primary system-under-test actions should normally live in `case` blocks.

## `case`

A `case` is a test case.

```reportage
case "valid entry" {
  file ".rellog/entries/001.kdl" <<'KDL'
entry "valid entry" {
  kind "feature"
}
KDL

  $ rellog check

  assert {
    exit 0
  }
}
```

A `case` may contain:

- an optional `params` block at the beginning;
- setup steps such as `file` and `$` shell setup commands;
- `$` shell steps for system-under-test actions;
- `assert { ... }` blocks for checkpoint-level verification.

## Case-local `params`

v0 supports case-local parameterized tests.

```reportage
case "check output" {
  params {
    variant "human" {
      ARGS = ""
    }

    variant "json" {
      ARGS = "--json"
    }
  }

  $ rellog check ${ARGS}

  assert {
    exit 0
  }
}
```

Rules:

- `params` is optional.
- At most one `params` block is allowed per `case`.
- `params` must appear at the beginning of the `case` block.
- `params` contains one or more `variant` blocks.
- `variant` names must be unique within the `params` block.
- v0 does not support module-scope `param` blocks.

Each `variant` expands the case into a concrete case.

For example:

```reportage
case "check output" {
  params {
    variant "human" {
      ARGS = ""
    }

    variant "json" {
      ARGS = "--json"
    }
  }

  $ rellog check ${ARGS}

  assert {
    exit 0
  }
}
```

expands into:

```text
check output / human
check output / json
```

## `variant`

A `variant` defines parameter bindings for one concrete case.

```reportage
variant "json" {
  ARGS = "--json"
  EXPECT_EXIT = "0"
}
```

Bindings are written as:

```text
NAME = "VALUE"
```

v0 rules:

- Binding names should be valid POSIX-style environment names: `[A-Za-z_][A-Za-z0-9_]*`.
- Binding values are strings in v0.
- The DSL does not require an `env` keyword; bindings are parameter values.
- Implementations may pass bindings to shell steps as environment variables.
- Bindings may be used in `${NAME}` expansions where expansion is enabled.

## File heredocs

`file` writes a file into the current concrete case workspace.

```reportage
file ".rellog/config.kdl" <<'KDL'
project "example"
kind "feature"
KDL
```

The heredoc delimiter must appear alone on its terminating line.

File heredoc content is raw by default. Parameter expansion is not performed inside raw file heredocs.

```reportage
file "example.txt" <<'TXT'
${NAME}
TXT
```

This writes the literal text `${NAME}`.

## File template heredocs

Use `template` when file content should expand parameter bindings.

```reportage
file ".rellog/entries/001.kdl" template <<'KDL'
entry "entry" {
  kind "${ENTRY_KIND}"
}
KDL
```

Template expansion is opt-in so that raw file contents, such as JSON, KDL, shell, JavaScript, Ruby, PHP, or other source code, do not accidentally collide with reportage parameters.

## Shell steps

A `$` step is executed by a POSIX shell.

```reportage
$ rellog check --json
```

Pipelines and normal shell syntax are allowed because the step is passed to the shell.

```reportage
$ rellog check --json | jq .
```

Use shell commands for ordinary filesystem operations, including directory creation and copying fixtures.

```reportage
$ mkdir -p .rellog/entries
$ cp -R fixtures/${FIXTURE}/. .
```

v0 rules:

- `$` steps use POSIX shell execution.
- Native Windows shell execution is out of scope in v0.
- Registered commands may be intercepted through PATH shims.
- The runner does not parse and rewrite arbitrary shell syntax in v0.
- v0 does not define a dedicated `copy` syntax.

## Assertion blocks

An `assert` statement defines a checkpoint-level verification block.

```reportage
assert {
  exit 0
  stderr empty
}
```

An assertion block verifies the current checkpoint. It is not attached to the nearest preceding action. See [semantics.md](semantics.md) for the checkpoint model.

### Block syntax

An assertion block uses `{` and `}` delimiters.

Valid — multi-line:

```reportage
assert {
  exit 0
  stderr empty
}
```

Valid — single-line (single expectation):

```reportage
assert { exit 0 }
```

Invalid in v0 — single-line with multiple expectations (no separator defined):

```reportage
assert { exit 0 stderr empty }
```

Invalid in v0 — single-line `assert ${expectation}` form:

```reportage
assert exit 0
```

Deferred / future candidate — single-line with `;` as expectation separator:

```reportage
assert { exit 0; stderr empty }
```

Rules:

- `assert` defines a block; the block must use `{` and `}`.
- A block must contain at least one expectation. An empty `assert { }` is a script error.
- Indentation is recommended but not syntax-significant.
- Multiple expectations in one line are not part of v0.
- If a single-line multiple-expectation form is added in a future version, `;` is the candidate separator.
- `$` actions may not appear inside an `assert` block.

### Exit expectations

```reportage
assert {
  exit 0
}

assert {
  exit 1
}

assert {
  exit nonzero
}
```

### stdout and stderr expectations

```reportage
assert {
  stdout empty
}

assert {
  stderr empty
}

assert {
  stdout contains "created"
}

assert {
  stderr contains "unknown kind"
}

assert {
  stdout matches /release [0-9]+\.[0-9]+\.[0-9]+/
}

assert {
  stdout not contains "panic"
}
```

### jq expectations

`jq` expectations evaluate JSON using external `jq` in v0.

```reportage
assert {
  stdout jq '.ok == true'
}

assert {
  stdout jq '.diagnostics | length == 1'
  stdout jq '.diagnostics[0].code == "UNKNOWN_KIND"'
}
```

v0 requires `jq` to be available on `PATH` when `stdout jq` or `stderr jq` is used.

### File expectations

```reportage
assert {
  file exists "CHANGELOG.md"
}

assert {
  file not exists ".rellog/tmp"
}

assert {
  file contains "CHANGELOG.md" "Added"
}

assert {
  file matches "CHANGELOG.md" /## v[0-9]+\.[0-9]+\.[0-9]+/
}
```

### Directory expectations

```reportage
assert {
  dir exists .rellog
}

assert {
  dir not exists .rellog
}
```

### File count expectations

```reportage
assert {
  file-count ".rellog/entries/*.kdl" == 0
}

assert {
  file-count ".rellog/entries/*.kdl" == 1
}

assert {
  file-count ".rellog/entries/*.kdl" >= 1
}
```

Glob evaluation is performed by the runner, not by the shell.

## Parameter expansion

Parameter expansion uses `${NAME}`.

Expansion is enabled in:

- `$` shell steps, by shell environment expansion;
- expectation string arguments;
- `file ... template` heredoc bodies.

Expansion is disabled by default in:

- raw `file` heredoc bodies;
- raw expected-output heredocs, if such assertions are added later.

Because `$` steps are shell steps, `${NAME}` in commands such as `$ cp -R fixtures/${FIXTURE}/. .` follows shell expansion rules.

## Indentation

Indentation is recommended for readability but is not syntax-significant in v0.

Blocks are delimited by `{` and `}`. Heredocs are delimited by their explicit heredoc marker.
