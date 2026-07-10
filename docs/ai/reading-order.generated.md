GENERATED FILE: do not edit directly. Regenerate with `just ai-docs-gen`.
(see [crates/reportage-cli/src/bin/gen_ai_reading_order.rs](../../crates/reportage-cli/src/bin/gen_ai_reading_order.rs))

# AI reading order

This is the recommended reading order for AI agents authoring, editing, or reviewing `.repor` files. It is generated from the same `DOCUMENTS` table `reportage docs --format=json` reads (`crates/reportage-cli/src/docs.rs`), so this list and that command's `documents[]` field never drift apart. See [`docs/ai/README.md`](README.md) for how to use this list.

`role` and `note` below are internal reading-order metadata. They are not part of the `reportage docs --format=json` output contract (`spec/output/docs-index/schema.json`), which carries only `id`, `title`, `path`, and `urls`.

## AI documentation guide

- Link: [AI documentation guide](README.md)
- Path: `docs/ai/README.md`
- Role: Entrypoint for AI-assisted authoring, editing, and review of .repor files
- Note: A guide, not a specification: it points at the normative documents below rather than redefining them.

## AI quick reference

- Link: [AI quick reference](quick-reference.md)
- Path: `docs/ai/quick-reference.md`
- Role: Shortest path to a minimal valid .repor file, for a fast orientation pass
- Note: Not a full syntax or semantics reference; follow its links for anything beyond the minimal shape.

## Syntax reference

- Link: [Syntax reference](../syntax.md)
- Path: `docs/syntax.md`
- Role: Normative syntax reference
- Note: Generated from the grammar; a construct absent here is not available, regardless of what seems plausible.

## Syntax conformance fixtures

- Link: [Syntax conformance fixtures](../syntax-conformance.md)
- Path: `docs/syntax-conformance.md`
- Role: Where the syntax conformance fixtures live: known-valid and known-invalid .repor examples with AST snapshots
- Note: Describes repository test fixtures; the fixtures under tests/fixtures/syntax/ are the example set itself.

## Semantics

- Link: [Semantics](../semantics.md)
- Path: `docs/semantics.md`
- Role: Overview and entrypoint for the semantics documentation set

## Semantic rule catalog

- Link: [Semantic rule catalog](../language/semantic-rules.md)
- Path: `docs/language/semantic-rules.md`
- Role: Generated catalog of language semantic rules
- Note: Generated from spec/language/semantics/*.json; do not hand-edit.

## Diagnostics

- Link: [Diagnostics](../diagnostics.md)
- Path: `docs/diagnostics.md`
- Role: Parser and validator diagnostic code reference

## Semantic and assertion diagnostics

- Link: [Semantic and assertion diagnostics](../semantic-diagnostics.md)
- Path: `docs/semantic-diagnostics.md`
- Role: Semantic, assertion, and step diagnostic code reference, extending the parse.* model above
- Note: A specification: parts may not yet be applied to the parser, evaluator, or CLI diagnostic rendering.

## Execution model

- Link: [Execution model](../execution-model.md)
- Path: `docs/execution-model.md`
- Role: Runner execution order and case workspace/checkpoint lifecycle

## Exit codes

- Link: [Exit codes](../exit-codes.md)
- Path: `docs/exit-codes.md`
- Role: Reportage process exit code reference

## Configuration

- Link: [Configuration](../configuration.md)
- Path: `docs/configuration.md`
- Role: reportage.kdl config file reference

## Artifacts

- Link: [Artifacts](../artifacts.md)
- Path: `docs/artifacts.md`
- Role: Artifact bundle overview: the .reportage/runs layout and result.json as the canonical run record
- Note: reportage run --format=json prints a projection derived from result.json, not the artifact document itself.

## JSON execution report contract

- Link: [JSON execution report contract](../../spec/output/json-report/README.md)
- Path: `spec/output/json-report/README.md`
- Role: Run JSON output contract

## Run result artifact contract

- Link: [Run result artifact contract](../../spec/artifacts/run-result/README.md)
- Path: `spec/artifacts/run-result/README.md`
- Role: Run result artifact JSON contract

## AI generation rules

- Link: [AI generation rules](generation-rules.md)
- Path: `docs/ai/generation-rules.md`
- Role: Rules and prohibitions for generating or editing .repor files
- Note: Read after the syntax and semantics references above; it does not repeat their content.

## AI validation flow

- Link: [AI validation flow](validation-flow.md)
- Path: `docs/ai/validation-flow.md`
- Role: How to validate a .repor file after generating or editing it
- Note: Only describes commands that exist in this CLI today; see the validation.command field for the current invocation.

## AI common mistakes

- Link: [AI common mistakes](common-mistakes.md)
- Path: `docs/ai/common-mistakes.md`
- Role: Short wrong/correct examples of mistakes AI agents commonly make
- Note: Points at existing fixtures and generated docs rather than collecting a full example set.

