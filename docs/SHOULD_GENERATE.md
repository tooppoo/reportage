# Files that should be generated

This file records every document in this tree that is produced by a generator or protected by a mechanical drift check, plus the hand-written sections that are candidates for the same treatment. Per this repository's documentation policy, generated output is never hand-edited: change the source or the generator, then regenerate.

## Generated documents in this tree

| Path | Generator and source | Reason |
| --- | --- | --- |
| [`docs/reference/syntax.md`](reference/syntax.md) | [`scripts/gen-grammar-doc.sh`](../scripts/gen-grammar-doc.sh) (`just lang-docs-gen`, drift-checked by `just lang-docs-check`), from [`crates/reportage-core/src/reportage.pest`](../crates/reportage-core/src/reportage.pest) | The pest file is the normative syntax source; a hand-written grammar reference would drift from it. |
| [`docs/reference/semantic-rules.md`](reference/semantic-rules.md) | [`crates/reportage-core/src/bin/gen_semantic_docs.rs`](../crates/reportage-core/src/bin/gen_semantic_docs.rs) (`just semantic-docs-gen`, drift-checked by `just semantic-docs-check`), from the JSON specs under [`spec/language/semantics/`](../spec/language/semantics/README.md) | The JSON specs are the source of truth for each rule's normative fields and conformance cases; the catalog is a read-only view of them. |
| [`docs/ai/reading-order.generated.md`](ai/reading-order.generated.md) | [`crates/reportage-cli/src/bin/gen_ai_reading_order.rs`](../crates/reportage-cli/src/bin/gen_ai_reading_order.rs) (`just ai-docs-gen`, drift-checked by `just ai-docs-check`), from the `DOCUMENTS` table in [`crates/reportage-cli/src/references.rs`](../crates/reportage-cli/src/references.rs) | Generating the reading order from the same table `reportage references --format=json` reads keeps the two from drifting apart. |

## Mechanically checked documents in this tree

| Path | Check | Reason |
| --- | --- | --- |
| [`docs/reference/artifacts.md`](reference/artifacts.md) | `run_result_fixtures.rs::docs_artifacts_examples_match_their_fixture_snapshots` | The `<!-- checked-against: ... -->` JSON examples are verified byte-for-byte against the `tests/fixtures/run_result/` snapshots, so they cannot silently drift from real output. |

## New generation candidates

These are hand-written today but state facts that are derivable from the implementation or a registry. They are candidates for generation or for embedded generated sections; each stays hand-written until a generator exists.

| Path in this tree | Reason |
| --- | --- |
| [`docs/reference/semantic-diagnostics.md`](reference/semantic-diagnostics.md) — the code inventory ("Naming Convention" examples list) and the per-code stable `details` list | The set of `semantic.*` / `assertion.*` / `step.*` codes is owned by `reportage_core::semantic_rule_registry::SEMANTIC_RULE_REGISTRY` and the diagnostic constructors, and CI already checks rule/code correspondence ([`crates/reportage-core/tests/semantic_rule_coverage.rs`](../crates/reportage-core/tests/semantic_rule_coverage.rs)). A generated inventory (or a drift check against the registry) would stop the hand-maintained list from silently missing newly added codes. The surrounding model/policy prose should stay hand-written. |
| [`docs/reference/diagnostics.md`](reference/diagnostics.md) — the `parse.*` code examples list | The `parse.*` codes are defined by `DiagnosticCode::as_str()` in [`crates/reportage-core/src/diagnostic.rs`](../crates/reportage-core/src/diagnostic.rs). Generating the code list (or checking it against the enum) would keep it complete as codes are added. The naming convention, granularity policy, and compatibility policy should stay hand-written. |
| [`docs/reference/exit-codes.md`](reference/exit-codes.md) — the exit code tables | Exit codes are asserted by integration tests and e2e self-tests; the tables could be checked against a small registry (or the tests' expected values) so a new subcommand's exit code cannot be documented inconsistently. The policy and precedence prose should stay hand-written. |
| [`docs/reference/configuration.md`](reference/configuration.md) — the per-node rule lists ("Rules:" bullets) and the validation error list | These enumerate exactly what `config.rs` validation accepts and rejects, and drift is only caught by review today. A config-schema-derived section or a fixture-backed check (valid/invalid config fixtures asserted against the documented rules) would protect them. The loading model and examples should stay hand-written. |
| [`docs/reference/shim-scaffold.md`](reference/shim-scaffold.md) — the builtin template list and per-template generated-script descriptions | The template registry (`reportage_core::shim_scaffold::TemplateRegistry`) owns which templates exist, and the described script behavior restates the template bodies. A generated template inventory, or a drift check that the documented flags (`--clean=false`, `--quiet`, `cd` behavior) appear in the rendered templates, would keep this in sync. The purpose/assumptions prose should stay hand-written. |
