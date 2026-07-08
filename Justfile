import 'task/test.just'
import 'task/release.just'
import 'task/docs.just'

mod examples-shim 'examples/shims/Justfile'

default:
  @just check

get-version:
  @sh scripts/get-version.sh

# run all check actions
[group('check')]
check: examples-shim::go-build test lint fmt build semantic-docs-check semantic-specs-check semantic-rule-coverage-check

# build as debug
[group('check')]
[group('build')]
build:
  cargo build --locked

# install reportage-self into the current environment
self-install:
  cargo install --path crates/reportage-cli --locked --force

# install the vscode extension for reportage-self into the current environment
vscode-install:
  sh scripts/dev/setup-reportage-vscode-extension.sh
