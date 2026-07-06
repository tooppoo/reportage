
default:
  @just check

get-version:
  @sh scripts/get-version.sh

# run all check actions
[group('check')]
check: test lint fmt build semantic-docs-check

# run all semantic-specs tests
[group('docs')]
[group('check')]
semantic-specs-check:
  cargo nextest run --locked --test semantic_specs

# generate language semantics documentation
[group('docs')]
semantic-docs-gen:
  cargo run --locked -p reportage-core --bin gen_semantic_docs -- docs/language/semantics.md

# check if language semantics documentation is up to date
[group('docs')]
[group('check')]
semantic-docs-check:
  #!/usr/bin/env sh
  set -eu
  tmp=$(mktemp)
  trap "rm -f '$tmp'" EXIT
  cargo run --locked -p reportage-core --bin gen_semantic_docs -- "$tmp" > /dev/null
  if ! diff -q docs/language/semantics.md "$tmp" > /dev/null 2>&1; then
    echo "docs/language/semantics.md is stale. Run 'just semantic-docs-gen' to regenerate."
    diff docs/language/semantics.md "$tmp" || true
    exit 1
  fi
  echo "docs/language/semantics.md is up to date."

# generate language grammar documentation
[group('docs')]
lang-docs-gen:
  #!/usr/bin/env sh
  set -eu
  bash scripts/gen-grammar-doc.sh

# check if language grammar documentation is up to date
[group('docs')]
[group('check')]
lang-docs-check:
  #!/usr/bin/env sh
  set -eu
  tmp=$(mktemp)
  trap "rm -f '$tmp'" EXIT
  bash scripts/gen-grammar-doc.sh "$tmp" > /dev/null
  if ! diff -q docs/syntax.md "$tmp" > /dev/null 2>&1; then
    echo "docs/syntax.md is stale. Run 'just lang-docs-gen' to regenerate."
    diff docs/syntax.md "$tmp" || true
    exit 1
  fi
  echo "docs/syntax.md is up to date."

# run all tests and generate coverage report
[group('check')]
test:
		cargo llvm-cov --locked --all-features --workspace --no-report nextest
		cargo llvm-cov report --codecov --output-path cov.json --ignore-filename-regex "cli/src/main"
		cargo llvm-cov report --fail-under-functions 80 --fail-under-lines 80 --fail-under-file-lines 80 --fail-under-regions 80 --ignore-filename-regex "cli/src/main|src/bin/gen_semantic_docs|model"

# run all formatting checks
[group('check')]
fmt:
  cargo fmt --all --check

# fix all formatting issues
[group('check')]
fmt-fix:
  cargo fmt --all

# run all lint checks
[group('check')]
lint:
  cargo clippy --locked -- -D warnings

# build as debug
[group('check')]
[group('build')]
build:
  cargo build --locked

# create archives for release
[group('build')]
archive dist:
  @sh scripts/release/archive.sh {{ dist }}

# install reportage-self into the current environment
[group('check')]
self-install:
  cargo install --path crates/reportage-cli --locked --force

# install the vscode extension for reportage-self into the current environment
[group('check')]
vscode-install:
  sh scripts/dev/setup-reportage-vscode-extension.sh

# create release tag(version pin) for the given version
[group('release')]
release-tag version:
  rellog ready {{ version }}
  git tag -a {{ version }} -m "Release {{ version }}"

# create release tag(latest) for the given version
[group('release')]
release-latest version:
  rellog ready {{ version }}
  git tag -d latest || true
  git tag -a latest -m "Release latest({{ version }})"
