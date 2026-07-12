# AI documentation guide

This directory is the entrypoint for AI agents that write, edit, or review `.repor` files for reportage.

## What this directory is

The documents under this directory are guides, not the specification. They exist to help an AI reach the right normative document quickly, avoid fabricating syntax that does not exist, and validate a generated or edited `.repor` file with a command that actually works today.

They deliberately do not redefine syntax, semantics, diagnostics, or JSON output. Those are owned by [the generated syntax reference](../../docs/syntax.md), [the language semantics reference](../reference/semantics.md), [the generated semantic rule catalog](../../docs/language/semantic-rules.md), [the parse diagnostics reference](../reference/diagnostics.md), [the semantic and assertion diagnostics reference](../reference/semantic-diagnostics.md), [the execution model reference](../reference/execution-model.md), and [the artifacts reference](../reference/artifacts.md). If a guide in this directory appears to disagree with one of those, the normative document wins.

## Where to start

Read the generated reading order at [`docs/ai/reading-order.generated.md`](../../docs/ai/reading-order.generated.md) next. It lists, in recommended order, every document an AI should read before generating or editing a `.repor` file, including this one's siblings ([generation rules](generation-rules.md), [validation flow](validation-flow.md), [common mistakes](common-mistakes.md), [quick reference](quick-reference.md)) and the normative references above.

That file is generated — do not edit it by hand. It is produced from the same document table `reportage references --format=json` reads, so the reading order and that command's `documents[]` field never drift apart. Regenerate it with `just ai-docs-gen`; `just ai-docs-check` (part of `just check`) fails the build if it goes stale. Its home in this tree is pending generator repointing; see [`SHOULD_GENERATE.md`](../SHOULD_GENERATE.md).

## The one rule that matters most

Only use syntax that appears in [the generated syntax reference](../../docs/syntax.md), the file generated from the grammar. If a construct is not documented there, it does not exist in this version of reportage — do not invent it, and do not treat [the deferred topics document](../planning/TBD.md) as a list of usable syntax. See [the generation rules](generation-rules.md) for the full rule set.

## After editing a `.repor` file

Run the validation command described in [the validation flow](validation-flow.md) — the same invocation `reportage references --format=json`'s `validation.command` field advertises — before treating a generated or edited script as done.
