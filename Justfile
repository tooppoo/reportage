import 'task/test.just'
import 'task/release.just'
import 'task/docs.just'
import 'task/schema.just'

mod examples-shim 'examples/shims/Justfile'
mod vscode-ext 'editors/vscode/Justfile'

default:
  @just check

get-version:
  @sh scripts/get-version.sh

# run all check actions
#
# schema-artifacts-check runs before test: the JSON contract suites validate producer output
# against the committed public schemas, and a stale one would report conformance against an
# artifact that no longer reflects its internal source.
[group('check')]
check: examples-shim::go-build examples-shim::rust-build examples-shim::js-install schema-artifacts-check test lint fmt build semantic-docs-check semantic-specs-check semantic-rule-coverage-check ai-docs-check examples-docs-check
  just find-hardcode-path -n

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

source-lines:
  @find crates -type f -name '*.rs' | xargs -i wc -l {} | sort -nr
