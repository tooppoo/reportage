
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
  if ! diff -q docs/language/grammar.md "$tmp" > /dev/null 2>&1; then
    echo "docs/language/grammar.md is stale. Run 'just lang-docs-gen' to regenerate."
    diff docs/language/grammar.md "$tmp" || true
    exit 1
  fi
  echo "docs/language/grammar.md is up to date."

test:
		cargo lcov
		cargo lcov-json
		cargo lcov-assert

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
