
default:
  @just check

check:
  @just test lint fmt build

lang-docs-gen:
  #!/usr/bin/env bash
  set -euo pipefail
  bash scripts/gen-grammar-doc.sh

lang-docs-check:
  #!/usr/bin/env bash
  set -euo pipefail
  tmp=$(mktemp)
  trap "rm -f '$tmp'" EXIT
  bash scripts/gen-grammar-doc.sh "$tmp" > /dev/null
  if ! diff -q docs/syntax.md "$tmp" > /dev/null 2>&1; then
    echo "docs/syntax.md is stale. Run 'just lang-docs-gen' to regenerate."
    diff docs/syntax.md "$tmp" || true
    exit 1
  fi
  echo "docs/syntax.md is up to date."

test:
		cargo llvm-cov --locked --all-features --workspace --no-report nextest
		cargo llvm-cov report --codecov --output-path cov.json --ignore-filename-regex "cli/src/main"
		cargo llvm-cov report --fail-under-functions 80 --fail-under-lines 80 --fail-under-file-lines 80 --fail-under-regions 80 --ignore-filename-regex "cli/src/main|model"

fmt:
  cargo fmt --all --check

fmt-fix:
  cargo fmt --all

lint:
  cargo clippy --locked -- -D warnings

build:
  cargo build --locked

build-release:
  cargo build --release --locked
