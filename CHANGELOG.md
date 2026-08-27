# CHANGELOG

## 0.0.9

### added

#### core / cli / tests / docs

<!-- rellog:entry:start -->
<!-- rellog:body:start -->
`write` can fix the POSIX file mode of the file it creates.
`write <"bin/git"> mode=0o755 <content>` writes the file and sets its permission bits in one step, so a fake command fixture no longer needs a following `chmod` action. The mode is written as exactly three octal digits after `0o`, between the path and the content, and is accepted with every content form and in `before_each` as well as a case body. It is the target file's final permission bits regardless of the umask reportage runs under, it applies to the target file only and not to parent directories the step creates, and it does not relax create-only. `mode` defaults to `0o600`, so a file written without one is readable and writable by its owner only and never executable, regardless of umask. `mode = 0o755`, `0O755`, `0755`, `755`, `0o75`, `0o1000`, `0o888`, and symbolic chmod syntax are rejected before the script runs. Linux and macOS only.
<!-- rellog:body:end -->

Refs:
- https://github.com/tooppoo/reportage/issues/245
<!-- rellog:entry:end -->

## 0.0.8

### added

#### core / cli / docs

<!-- rellog:entry:start -->
<!-- rellog:body:start -->
`before_each` accepts the same steps as a case body.
`$` actions, `assert` blocks, `let` bindings, and `write` steps are all allowed in setup, so shared setup that needs a command no longer has to be repeated in every case body. A setup action's exit status does not by itself fail the case, so verify it with an `assert` block later in `before_each`; a setup failure then fails only the concrete case being set up. The case body starts at its own checkpoint, so its first `exit` / `stdout` / `stderr` never describes a setup action. Note that a setup `assert` does not satisfy a case's own requirement to contain an `assert` block, and a case body cannot redeclare a setup binding's name.
<!-- rellog:body:end -->

Refs:
- https://github.com/tooppoo/reportage/issues/227
<!-- rellog:entry:end -->

### changed

#### core / cli / docs

<!-- rellog:entry:start -->
<!-- rellog:body:start -->
remove the diagnostic codes that rejected steps inside `before_each`.
`parse.before_each.action_step`, `parse.before_each.assertion_block`, and `semantic.binding.before_each_forbidden` no longer exist. Shapes that stay invalid report ordinary step and binding codes instead: `parse.missing_assertion_block`, `semantic.binding.requires_action`, `semantic.binding.undefined`, and `semantic.binding.use_before_declaration`. The `before_each` write-step runtime failure and `parse.before_each.empty` messages are reworded, since setup is no longer write-only.
<!-- rellog:body:end -->

Refs:
- https://github.com/tooppoo/reportage/issues/227
<!-- rellog:entry:end -->

<!-- rellog:entry:start -->
<!-- rellog:body:start -->
the JSON and artifact contracts carry a step origin, at `schemaVersion` 2.
Every `actions[]` and `assertions[]` entry, and every step-attributed diagnostic, now carries `step: { phase, index }`, naming the block it came from and its 0-based position within that block. `checkpoint: "initial"` on an assertion means the phase-entry checkpoint of that assertion's own phase, not that no action has run in the case. Both `result.json` and `--format=json` move to `schemaVersion` 2: `step` is a new required property on objects declared `additionalProperties: false`, so a document produced now does not validate against either published v1 schema.
<!-- rellog:body:end -->

Refs:
- https://github.com/tooppoo/reportage/issues/227
<!-- rellog:entry:end -->

## 0.0.7

### added

#### core

<!-- rellog:entry:start -->
<!-- rellog:body:start -->
support `let` bindings.
user can capture stdout or stderr of an action and use on assertion
.
<!-- rellog:body:end -->
<!-- rellog:entry:end -->

#### core / docs

<!-- rellog:entry:start -->
<!-- rellog:body:start -->
support interpolated text literals.
`&"..."` and `&` + heredoc embed case-local bindings via `&{name}`, while raw literals stay non-interpolating.
<!-- rellog:body:end -->

Refs:
- https://github.com/tooppoo/reportage/issues/71
<!-- rellog:entry:end -->

### fixed

#### core / cli

<!-- rellog:entry:start -->
<!-- rellog:body:start -->
correct the `file <"path"> contains` wrong-kind suggestion.
it named only the string literal form, though the position also accepts a heredoc literal; the accepted forms are now declared by the position instead of derived from its diagnostic label.
<!-- rellog:body:end -->

Refs:
- https://github.com/tooppoo/reportage/issues/71
<!-- rellog:entry:end -->

## 0.0.6

### added

#### core / cli

<!-- rellog:entry:start -->
<!-- rellog:body:start -->
`--index-file-name` option is supported for `reportage docs`.
User can specify filename for reportage document.
<!-- rellog:body:end -->
<!-- rellog:entry:end -->

### changed

#### cli

<!-- rellog:entry:start -->
<!-- rellog:body:start -->
Add reference to examples on `reportage references`
<!-- rellog:body:end -->
<!-- rellog:entry:end -->

## 0.0.5

### changed

#### cli

<!-- rellog:entry:start -->
<!-- rellog:body:start -->
Rename the reference discovery command `reportage docs` to `reportage references`, with the machine-readable contract moved to `spec/output/references-index/`. `docs` is reserved for a future documentation generation command and now fails as not implemented. No alias or deprecation period is provided.
<!-- rellog:body:end -->

Refs:
- https://github.com/tooppoo/reportage/issues/166
<!-- rellog:entry:end -->

### added

#### cli

<!-- rellog:entry:start -->
<!-- rellog:body:start -->
Add the `reportage docs` documentation generation command: glob-selected `.repor` sources are parsed (never executed) and aggregated into a single plain text document at `<out-dir>/index.txt`, with `document file` / `document case` metadata, display fallbacks, deterministic ordering, and existing-output-preserving replacement. Replaces the reserved not-implemented `docs` stub.
<!-- rellog:body:end -->

Refs:
- https://github.com/tooppoo/reportage/issues/170
<!-- rellog:entry:end -->

#### core

<!-- rellog:entry:start -->
<!-- rellog:body:start -->
support `before_each` block
<!-- rellog:body:end -->
<!-- rellog:entry:end -->

#### core / cli

<!-- rellog:entry:start -->
<!-- rellog:body:start -->
support `document` block and `docs` command to generate documents following to the block.
<!-- rellog:body:end -->
<!-- rellog:entry:end -->

## 0.0.4

### changed

#### cli / docs

<!-- rellog:entry:start -->
<!-- rellog:body:start -->
Add navigation to docs in `reportage docs`
<!-- rellog:body:end -->
<!-- rellog:entry:end -->

## 0.0.3

### added

#### core

<!-- rellog:entry:start -->
<!-- rellog:body:start -->
`shim` support Rust as template
<!-- rellog:body:end -->
<!-- rellog:entry:end -->

#### cli

<!-- rellog:entry:start -->
<!-- rellog:body:start -->
add `docs` subcommant.
it print references to documents.
it is expected to be read by not only human, but also AI sgent.
<!-- rellog:body:end -->
<!-- rellog:entry:end -->

## 0.0.2

### changed

#### tests / docs

<!-- rellog:entry:start -->
<!-- rellog:body:start -->
Internal changes.

* artifact result.json を canonical manifest 化し schema・fixture で検証可能にする by @tooppoo in https://github.com/tooppoo/reportage/pull/150
* Semantic rule identity を型化し、registry cross-reference を検証可能にする (#146) by @tooppoo in https://github.com/tooppoo/reportage/pull/152
* replace by shared reusable workflow by @tooppoo in https://github.com/tooppoo/reportage/pull/153
<!-- rellog:body:end -->
<!-- rellog:entry:end -->

## 0.0.1

### added

#### core

<!-- rellog:entry:start -->
<!-- rellog:body:start -->
add expectations
<!-- rellog:body:end -->
<!-- rellog:entry:end -->

## v0.0.0

### added

#### docs

<!-- rellog:entry:start -->
<!-- rellog:body:start -->
setup rellog changelog
<!-- rellog:body:end -->
<!-- rellog:entry:end -->
