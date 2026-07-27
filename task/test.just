
# run all tests and generate coverage report
[group('check')]
test:
  #!/usr/bin/env sh
  set -eu

  cargo llvm-cov --locked --all-features --workspace --no-report nextest
  cargo llvm-cov report --codecov --output-path cov.json --ignore-filename-regex "cli/src/main"
  cargo llvm-cov report \
    --fail-under-functions 80 \
    --fail-under-lines 80 \
    --fail-under-file-lines 80 \
    --fail-under-regions 80 \
    --ignore-filename-regex "cli/src/main|src/bin/gen_semantic_docs|src/bin/gen_ai_reading_order|model"

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
