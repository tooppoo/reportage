
default:
  @just check

check:
  @just test lint fmt build

test:
		cargo lcov
		cargo lcov-json
		cargo lcov-assert

fmt:
  cargo fmt --all --check

lint:
  cargo clippy --locked -- -D warnings

build:
  cargo build --locked

build-release:
  cargo build --release --locked
