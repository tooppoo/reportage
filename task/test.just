
# run all tests and generate coverage report
[group('check')]
test:
  #!/usr/bin/env sh
  set -eu

  cargo llvm-cov --locked --all-features --workspace --no-report nextest

  # Every alternative in both exclusions below is a whole repository-relative path, anchored at both
  # ends. They are regexes matched against absolute paths, so an alternative that stops short of that
  # can also match a directory the build sits under rather than a file in it: a bare `model` matched
  # a git worktree named `…-model-guard` and excluded the entire workspace. Nothing says so — llvm-cov
  # prints a table of `-` and exits 1 with no message — so the failure reads as the thresholds being
  # missed while every test passes.
  cargo llvm-cov report --codecov --output-path cov.json \
    --ignore-filename-regex "/crates/reportage-cli/src/main\.rs$"
  cargo llvm-cov report \
    --fail-under-functions 80 \
    --fail-under-lines 80 \
    --fail-under-file-lines 80 \
    --fail-under-regions 80 \
    --ignore-filename-regex "/crates/reportage-cli/src/main\.rs$|/crates/reportage-core/src/bin/gen_semantic_docs\.rs$|/crates/reportage-cli/src/bin/gen_ai_reading_order\.rs$|/crates/reportage-core/src/model\.rs$"

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

# run the --format=json representative-fixture schema/snapshot conformance tests
[group('check')]
json-report-fixtures-check:
  cargo nextest run --locked -p reportage-cli --test json_report_fixtures

# run the artifact result.json representative-fixture schema/snapshot/parity conformance tests
[group('check')]
run-result-fixtures-check:
  cargo nextest run --locked -p reportage-cli --test run_result_fixtures

# run the `reportage references --format=json` schema/repository-consistency conformance tests
[group('check')]
references-index-check:
  cargo nextest run --locked -p reportage-cli --test references_index

# run the JSON contract schema document validity and schema feature coverage tests
[group('check')]
json-contract-schemas-check:
  cargo nextest run --locked -p reportage-cli --test json_contract_schemas

# run the snapshot normalization schema preparation tests
[group('check')]
snapshot-normalization-check:
  cargo nextest run --locked -p reportage-cli --test snapshot_normalization
