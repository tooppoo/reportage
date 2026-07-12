# Files that should be generated

This file records every document in this tree that should be produced by a generator or protected by a mechanical drift check, rather than written or copied by hand. Per this repository's documentation policy, generated output is never hand-edited; hand-copying a generated file here would create a second, driftable copy, so those files are deliberately absent from this tree until their generators are repointed.

## Generated documents not yet materialized in this tree

These already have working generators that currently emit into [`docs/`](../docs/). Migrating this tree to authoritative status means changing each generator's output path (and every check that reads it), then generating — never copying the current output by hand.

| Planned path | Current generated output | Reason |
| --- | --- | --- |
| `docs2/reference/syntax.md` | [`docs/syntax.md`](../docs/syntax.md) | The grammar reference is generated from [`crates/reportage-core/src/reportage.pest`](../crates/reportage-core/src/reportage.pest) by [`scripts/gen-grammar-doc.sh`](../scripts/gen-grammar-doc.sh) (`just lang-docs-gen`). The pest file is the normative syntax source; a hand-written copy would drift from the grammar. |
| `docs2/reference/semantic-rules.md` | [`docs/language/semantic-rules.md`](../docs/language/semantic-rules.md) | The semantic rule catalog is generated from `spec/language/semantics/*.json` by [`crates/reportage-core/src/bin/gen_semantic_docs.rs`](../crates/reportage-core/src/bin/gen_semantic_docs.rs) (`just semantic-docs-gen`). The JSON specs are the source of truth for normative fields and conformance cases. |
| `docs2/ai/reading-order.generated.md` | [`docs/ai/reading-order.generated.md`](../docs/ai/reading-order.generated.md) | The AI reading order is generated from the `DOCUMENTS` table in [`crates/reportage-cli/src/references.rs`](../crates/reportage-cli/src/references.rs) by [`crates/reportage-cli/src/bin/gen_ai_reading_order.rs`](../crates/reportage-cli/src/bin/gen_ai_reading_order.rs) (`just ai-docs-gen`, checked by `just ai-docs-check`), so the reading order and `reportage references --format=json` never drift apart. The table's `path` entries also need updating to this tree's layout when repointed. |

## Checks that must be repointed at migration

| Path in this tree | Check | Reason |
| --- | --- | --- |
| [`docs2/reference/artifacts.md`](reference/artifacts.md) | `run_result_fixtures.rs::docs_artifacts_examples_match_their_fixture_snapshots` | The `<!-- checked-against: ... -->` JSON examples are verified byte-for-byte against `tests/fixtures/run_result/` snapshots, but the test currently reads [`docs/artifacts.md`](../docs/artifacts.md). Until it is repointed, only the [`docs/`](../docs/) copy is drift-protected. |

## New generation candidates

These are hand-written today (in both trees) but state facts that are derivable from the implementation or a registry. They are candidates for generation or for embedded generated sections; each stays hand-written until a generator exists.

| Path in this tree | Reason |
| --- | --- |
| [`docs2/reference/semantic-diagnostics.md`](reference/semantic-diagnostics.md) — the code inventory ("Naming Convention" examples list) and the per-code stable `details` list | The set of `semantic.*` / `assertion.*` / `step.*` codes is owned by `reportage_core::semantic_rule_registry::SEMANTIC_RULE_REGISTRY` and the diagnostic constructors, and CI already checks rule/code correspondence ([`crates/reportage-core/tests/semantic_rule_coverage.rs`](../crates/reportage-core/tests/semantic_rule_coverage.rs)). A generated inventory (or a drift check against the registry) would stop the hand-maintained list from silently missing newly added codes. The surrounding model/policy prose should stay hand-written. |
| [`docs2/reference/diagnostics.md`](reference/diagnostics.md) — the `parse.*` code examples list | The `parse.*` codes are defined by `DiagnosticCode::as_str()` in [`crates/reportage-core/src/diagnostic.rs`](../crates/reportage-core/src/diagnostic.rs). Generating the code list (or checking it against the enum) would keep it complete as codes are added. The naming convention, granularity policy, and compatibility policy should stay hand-written. |
| [`docs2/reference/exit-codes.md`](reference/exit-codes.md) — the exit code tables | Exit codes are asserted by integration tests and e2e self-tests; the tables could be checked against a small registry (or the tests' expected values) so a new subcommand's exit code cannot be documented inconsistently. The policy and precedence prose should stay hand-written. |
| [`docs2/reference/configuration.md`](reference/configuration.md) — the per-node rule lists ("Rules:" bullets) and the validation error list | These enumerate exactly what `config.rs` validation accepts and rejects, and drift is only caught by review today. A config-schema-derived section or a fixture-backed check (valid/invalid config fixtures asserted against the documented rules) would protect them. The loading model and examples should stay hand-written. |
| [`docs2/reference/shim-scaffold.md`](reference/shim-scaffold.md) — the builtin template list and per-template generated-script descriptions | The template registry (`reportage_core::shim_scaffold::TemplateRegistry`) owns which templates exist, and the described script behavior restates the template bodies. A generated template inventory, or a drift check that the documented flags (`--clean=false`, `--quiet`, `cd` behavior) appear in the rendered templates, would keep this in sync. The purpose/assumptions prose should stay hand-written. |
