
# run all semantic-specs tests
[group('docs')]
[group('check')]
semantic-specs-check:
  cargo nextest run --locked --test semantic_specs

# generate the semantic rule catalog
[group('docs')]
semantic-docs-gen:
  cargo run --locked -p reportage-core --bin gen_semantic_docs -- docs2/reference/semantic-rules.md

# check if the semantic rule catalog is up to date
[group('docs')]
[group('check')]
semantic-docs-check:
  #!/usr/bin/env sh
  set -eu
  tmp=$(mktemp)
  trap "rm -f '$tmp'" EXIT
  cargo run --locked -p reportage-core --bin gen_semantic_docs -- "$tmp" > /dev/null
  if ! diff -q docs2/reference/semantic-rules.md "$tmp" > /dev/null 2>&1; then
    echo "docs2/reference/semantic-rules.md is stale. Run 'just semantic-docs-gen' to regenerate."
    diff docs2/reference/semantic-rules.md "$tmp" || true
    exit 1
  fi
  echo "docs2/reference/semantic-rules.md is up to date."

# run the semantic rule coverage check (registry vs. semantic specs vs. generated docs)
[group('docs')]
[group('check')]
semantic-rule-coverage-check:
  cargo nextest run --locked --test semantic_rule_coverage

# generate language grammar documentation
[group('docs')]
lang-docs-gen:
  #!/usr/bin/env sh
  set -eu
  bash scripts/gen-grammar-doc.sh

# check if language grammar documentation is up to date
[group('docs')]
[group('check')]
lang-docs-check:
  #!/usr/bin/env sh
  set -eu
  tmp=$(mktemp)
  trap "rm -f '$tmp'" EXIT
  bash scripts/gen-grammar-doc.sh "$tmp" > /dev/null
  if ! diff -q docs2/reference/syntax.md "$tmp" > /dev/null 2>&1; then
    echo "docs2/reference/syntax.md is stale. Run 'just lang-docs-gen' to regenerate."
    diff docs2/reference/syntax.md "$tmp" || true
    exit 1
  fi
  echo "docs2/reference/syntax.md is up to date."

# generate the AI documentation reading order
[group('docs')]
ai-docs-gen:
  cargo run --locked -p reportage-cli --bin gen_ai_reading_order -- docs2/ai/reading-order.generated.md

# check if the AI documentation reading order is up to date
[group('docs')]
[group('check')]
ai-docs-check:
  #!/usr/bin/env sh
  set -eu
  tmp=$(mktemp)
  trap "rm -f '$tmp'" EXIT
  cargo run --locked -p reportage-cli --bin gen_ai_reading_order -- "$tmp" > /dev/null
  if ! diff -q docs2/ai/reading-order.generated.md "$tmp" > /dev/null 2>&1; then
    echo "docs2/ai/reading-order.generated.md is stale. Run 'just ai-docs-gen' to regenerate."
    diff docs2/ai/reading-order.generated.md "$tmp" || true
    exit 1
  fi
  echo "docs2/ai/reading-order.generated.md is up to date."

[group('check')]
[group('docs')]
find-hardcode-path *args:
  #!/usr/bin/env bash
  matches=$(scripts/docs/find-bare-markdown-path.sh {{ args }})
  if [ "$(printf "$matches" | wc -l)" = "0" ]; then
    echo "No hardcoded paths found in docs."
  else
    echo "Found hardcoded paths in docs:"
    echo "$matches"
    exit 1
  fi
