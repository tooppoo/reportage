# AI documentation guide

This directory is the entrypoint for AI agents that write, edit, or review `.repor` files for reportage.

## What this directory is

The documents under this directory are guides, not the specification. They exist to help an AI reach the right normative document quickly, avoid fabricating syntax that does not exist, and validate a generated or edited `.repor` file with a command that actually works today.

They deliberately do not redefine syntax, semantics, diagnostics, or JSON output. Those are owned by the grammar at [`crates/reportage-core/src/reportage.pest`](../../crates/reportage-core/src/reportage.pest), [the language semantics reference](../reference/semantics.md), the semantic rule specs under [`spec/language/semantics/`](../../spec/language/semantics/README.md), [the parse diagnostics reference](../reference/diagnostics.md), [the semantic and assertion diagnostics reference](../reference/semantic-diagnostics.md), [the execution model reference](../reference/execution-model.md), and [the artifacts reference](../reference/artifacts.md). If a guide in this directory appears to disagree with one of those, the normative document wins.

## Where to start

Read this directory in this order: [quick reference](quick-reference.md) for the minimal valid shape, then the normative references above, then [generation rules](generation-rules.md), [validation flow](validation-flow.md), and [common mistakes](common-mistakes.md).

A generated reading order is planned at `ai/reading-order.generated.md` in this tree, produced from the same document table `reportage references --format=json` reads (`just ai-docs-gen` / `just ai-docs-check`), so the reading order and that command's `documents[]` field cannot drift apart. See [`SHOULD_GENERATE.md`](../SHOULD_GENERATE.md); until the generator emits here, use the order above and the `reportage references --format=json` document index.

## The one rule that matters most

Only use syntax that the grammar at [`crates/reportage-core/src/reportage.pest`](../../crates/reportage-core/src/reportage.pest) actually accepts. If a construct is not defined there, it does not exist in this version of reportage — do not invent it, and do not treat [the deferred topics document](../planning/TBD.md) as a list of usable syntax. See [the generation rules](generation-rules.md) for the full rule set.

## After editing a `.repor` file

Run the validation command described in [the validation flow](validation-flow.md) — the same invocation `reportage references --format=json`'s `validation.command` field advertises — before treating a generated or edited script as done.
