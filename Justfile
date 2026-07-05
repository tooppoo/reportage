
default:
  @just check

[group('check')]
check: test lint fmt build semantic-docs-check

[group('docs')]
[group('check')]
semantic-specs-check:
  cargo nextest run --locked --test semantic_specs

[group('docs')]
semantic-docs-gen:
  cargo run --locked -p reportage-core --bin gen_semantic_docs -- docs/language/semantics.md

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

[group('docs')]
lang-docs-gen:
  #!/usr/bin/env sh
  set -eu
  bash scripts/gen-grammar-doc.sh

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

[group('check')]
test:
		cargo llvm-cov --locked --all-features --workspace --no-report nextest
		cargo llvm-cov report --codecov --output-path cov.json --ignore-filename-regex "cli/src/main"
		cargo llvm-cov report --fail-under-functions 80 --fail-under-lines 80 --fail-under-file-lines 80 --fail-under-regions 80 --ignore-filename-regex "cli/src/main|src/bin/gen_semantic_docs|model"

[group('check')]
fmt:
  cargo fmt --all --check

[group('check')]
fmt-fix:
  cargo fmt --all

[group('check')]
lint:
  cargo clippy --locked -- -D warnings

[group('check')]
[group('build')]
build:
  cargo build --locked

[group('build')]
archive dist:
  @sh scripts/release/archive.sh {{ dist }}

[group('build')]
[group('check')]
archive-assert dist:
  @sh scripts/release/assertion/assert_archive.sh {{ dist }}

[group('build')]
archive-collect src dist:
  @sh scripts/release/collect.sh {{ src }} {{ dist }}

archive-extract src dist:
  @sh scripts/release/extract.sh {{ src }} {{ dist }}
