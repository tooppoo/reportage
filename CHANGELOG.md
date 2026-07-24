# CHANGELOG

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
