
default:
  @just check

check:
  @just lint fmt test build

test:
		cargo llvm-cov --locked --all-features --workspace --no-report
		cargo llvm-cov report --codecov --output-path cov.json \
				--ignore-filename-regex 'cli/src/main'
		# validation/schema is excluded: its builder functions contain map_err closures on
		# schema compilation that are unreachable at runtime because all schemas are hardcoded
		# via include_str! and are always syntactically valid at build time.
		# cli/src/main and plugins/*/src/main are excluded: all are thin binary entry
		# points (arg parsing / stdin→stdout dispatch) with no behavioral logic to test.
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
