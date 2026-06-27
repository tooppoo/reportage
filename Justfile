
setup:
  ./scripts/dev/setup-libs.sh

default:
  @just setup

check:
  @just lint fmt test build

test:
		cargo llvm-cov --locked --all-features --workspace --no-report
		cargo llvm-cov report --codecov --output-path cov.json \
				--ignore-filename-regex 'cli/src/main'
		cargo llvm-cov report \
				--fail-under-functions 80 \
				--fail-under-lines 80 \
				--fail-under-file-lines 80 \
				--fail-under-regions 80 \
				--ignore-filename-regex 'cli/src/main'

fmt:
  cargo fmt --all --check

lint:
  cargo clippy --all-targets --locked -- -D warnings

build:
  cargo build --locked

build-release:
  cargo build --release --locked
