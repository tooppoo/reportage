# AI documentation guide

This directory is the entrypoint for AI agents that write, edit, or review `.repor` files for reportage.

## What this directory is

The documents under `docs/ai/` are guides, not the specification. They exist to help an AI reach the right normative document quickly, avoid fabricating syntax that does not exist, and validate a generated or edited `.repor` file with a command that actually works today.

They deliberately do not redefine syntax, semantics, diagnostics, or JSON output. Those are owned by the documents linked from [`docs/ai/reading-order.generated.md`](reading-order.generated.md), primarily [`docs/syntax.md`](../syntax.md), [`docs/semantics.md`](../semantics.md), [`docs/language/semantic-rules.md`](../language/semantic-rules.md), [`docs/diagnostics.md`](../diagnostics.md), [`docs/semantic-diagnostics.md`](../semantic-diagnostics.md), [`docs/execution-model.md`](../execution-model.md), and [`docs/artifacts.md`](../artifacts.md). If a guide in this directory appears to disagree with one of those, the normative document wins.

## Where to start

Read [`docs/ai/reading-order.generated.md`](reading-order.generated.md) next. It lists, in recommended order, every document an AI should read before generating or editing a `.repor` file, including this one's siblings ([generation rules](generation-rules.md), [validation flow](validation-flow.md), [common mistakes](common-mistakes.md), [quick reference](quick-reference.md)) and the normative references above.

That file is generated — do not edit it by hand. It is produced from the same document table `reportage references --format=json` reads, so the reading order and that command's `documents[]` field never drift apart. Regenerate it with `just ai-docs-gen`; `just ai-docs-check` (part of `just check`) fails the build if it goes stale.

## The one rule that matters most

Only use syntax that appears in [`docs/syntax.md`](../syntax.md), the file generated from the grammar. If a construct is not documented there, it does not exist in this version of reportage — do not invent it, and do not treat [`docs/TBD.md`](../TBD.md) as a list of usable syntax. See [`docs/ai/generation-rules.md`](generation-rules.md) for the full rule set.

## After editing a `.repor` file

Run the validation command described in [`docs/ai/validation-flow.md`](validation-flow.md) — the same invocation `reportage references --format=json`'s `validation.command` field advertises — before treating a generated or edited script as done.
