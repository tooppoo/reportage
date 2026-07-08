#!/usr/bin/env bash
# Generates docs/syntax.md from crates/reportage-core/src/reportage.pest.
# Usage: gen-grammar-doc.sh [OUTPUT_PATH]
#   OUTPUT_PATH defaults to docs/syntax.md (relative to repo root).
set -eu

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

GRAMMAR_SRC="$REPO_ROOT/crates/reportage-core/src/reportage.pest"
OUTPUT="${1:-$REPO_ROOT/docs/syntax.md}"

mkdir -p "$(dirname "$OUTPUT")"

{
  cat << 'HEADER'
Generated from [crates/reportage-core/src/reportage.pest](../crates/reportage-core/src/reportage.pest)
by [scripts/gen-grammar-doc.sh](../scripts/gen-grammar-doc.sh).
DO NOT EDIT MANUALLY — run `just lang-docs-gen` to regenerate.

# Reportage Grammar

> **This file is auto-generated.** Do not edit it manually.
> To update the grammar, modify
> [`crates/reportage-core/src/reportage.pest`](../crates/reportage-core/src/reportage.pest)
> and run `just lang-docs-gen`.

[crates/reportage-core/src/reportage.pest](../crates/reportage-core/src/reportage.pest) is the normative syntax source for
Reportage v0. Any syntax not expressible in that file is not part of v0.

## Syntax conformance vs. semantic conformance

This document covers *syntax* only — whether a script is accepted by the
parser. Semantic behaviour is defined separately: execution order and
workspace lifecycle in [`docs/execution-model.md`](execution-model.md),
and assertion evaluation in [`docs/semantics.md`](semantics.md).

## Grammar

```pest
HEADER

  cat "$GRAMMAR_SRC"

  printf '```\n'
} > "$OUTPUT"
