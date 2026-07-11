# AI Documentation Guide Structure and Reading Order Generation

- Status: Accepted
- Created: 2026-07-09T09:00:00Z

> **Update (2026-07-11, issue #166):** the discovery command referred to as `reportage docs` below was renamed to `reportage references`, and version tags carry no `v` prefix; see [Rename `reportage docs` to `reportage references` and Reserve `docs` for Documentation Generation](20260711T070008Z_rename-docs-command-to-references.md).
> The decisions recorded here are otherwise unaffected.

## Context

[Tag-Based GitHub URLs and `reportage docs` as the v0 AI Documentation Discovery Path](20260708T180000Z_ai-documentation-discovery-core-path.md) gives an AI a mechanical way to discover *where* reportage's documentation lives, and #137 built `reportage docs` as the command that prints that index. Neither one tells an AI *what order* to read those documents in, *what rules* to follow while generating a `.repor` file, or *how* to validate its own output afterward.

Reportage already has authoritative documents for syntax (`docs/syntax.md`, generated), semantics (`docs/semantics.md`, `docs/language/semantic-rules.md`), diagnostics (`docs/diagnostics.md`, `docs/semantic-diagnostics.md`), and JSON/artifact output (`docs/artifacts.md`, `spec/output/json-report/`, `spec/artifacts/run-result/`). Duplicating any of that content into a new AI-facing document would create a second copy that can drift from the source of truth. At the same time, an AI benefits from a short, enumerative guide that states prohibitions (no `TBD.md` syntax, no invented future syntax) and points at the right document instead of guessing.

This is the second ADR recorded from issue #142, which built `docs/ai/`. Related issues: #136, #137, #142.

## Decision

`docs/ai/` holds thin guides, not a second specification. Each guide points at an existing normative document rather than redefining its content: [`docs/ai/README.md`](../ai/README.md) (hand-written entrypoint), [`docs/ai/quick-reference.md`](../ai/quick-reference.md), [`docs/ai/generation-rules.md`](../ai/generation-rules.md), [`docs/ai/validation-flow.md`](../ai/validation-flow.md), and [`docs/ai/common-mistakes.md`](../ai/common-mistakes.md).

`crates/reportage-cli/src/docs.rs`'s `DOCUMENTS` table — already the source `reportage docs --format=json` reads — is extended with the five `docs/ai/*.md` guides above and two new internal-only fields per entry, `role` and `note`. Neither field is serialized into `reportage docs --format=json`; that output contract still carries only `id`, `title`, `path`, and `urls` (`spec/output/docs-index/schema.json`, unchanged).

[`docs/ai/reading-order.generated.md`](../ai/reading-order.generated.md) is generated from that same `DOCUMENTS` table by `crates/reportage-cli/src/bin/gen_ai_reading_order.rs`, a second binary in the `reportage-cli` crate. No second reading-order source is introduced: the generator reads `DOCUMENTS` directly (via a small `reportage-cli` lib target added for this purpose), so the JSON index and the generated reading order can never disagree about which documents exist or in what order. `just ai-docs-gen` regenerates the file; `just ai-docs-check`, wired into `just check`, fails the build when it goes stale, mirroring the existing `semantic-docs-gen`/`semantic-docs-check` and `lang-docs-gen`/`lang-docs-check` pattern.

`docs/ai/reading-order.generated.md` itself is not added to `DOCUMENTS`: it is a derived view of the table, not a document with its own role in the reading order.

## Alternatives Considered

### Hand-maintain the reading order in `docs/ai/README.md`

Rejected. A hand-written list next to a generated `documents[]` array is exactly the kind of duplicate that drifts: adding, removing, or reordering a document would require remembering to update two places, and nothing would catch the omission until a human noticed the mismatch.

### Give `reading-order.generated.md` its own manifest file, separate from `DOCUMENTS`

Rejected. A second manifest is a second source of truth with the same drift risk as hand-maintaining the list, just moved one layer down. Reading `DOCUMENTS` directly, at generation time, is the only way to guarantee the two outputs describe the same set of documents in the same order.

### Add `role` to the `reportage docs --format=json` output contract

Rejected for v0. `role` and `note` are useful for a human- or AI-facing reading order rendered as prose, but adding them to the JSON contract would grow a schema that already has external consumers (`spec/output/docs-index/schema.json`) for a benefit specific to one consumer (the generated reading-order file). If a future consumer needs `role` from the JSON output directly, that is a deliberate, separately reviewed schema change, not a side effect of this guide's existence.

## Consequences

### Positive Consequences

- Adding, removing, or reordering a document in `DOCUMENTS` automatically keeps `reportage docs --format=json` and `docs/ai/reading-order.generated.md` in agreement; there is no second list to forget.
- The AI-facing guides stay short because they point at normative documents instead of restating them, so they cannot drift out of sync with the generated syntax/semantic-rule catalogs the way a hand-written reference could.
- The generation/check pattern (`just ai-docs-gen` / `just ai-docs-check`) matches two existing precedents in this repository, so contributors already familiar with `semantic-docs-gen` or `lang-docs-gen` need no new mental model.

### Negative Consequences

- `crates/reportage-cli` now has a `lib.rs` in addition to `main.rs`, solely so `src/bin/gen_ai_reading_order.rs` can reach `DOCUMENTS`; this is a small structural change for a single internal consumer.
- `DOCUMENTS` entries now carry two fields (`role`, `note`) that `render_json` never reads, which a future contributor must recognize as deliberate internal-only metadata rather than dead code.

## References

- [#136](https://github.com/tooppoo/reportage/issues/136)
- [#137](https://github.com/tooppoo/reportage/issues/137)
- [#142](https://github.com/tooppoo/reportage/issues/142)
- [Tag-Based GitHub URLs and `reportage docs` as the v0 AI Documentation Discovery Path](20260708T180000Z_ai-documentation-discovery-core-path.md)
- [Supplementary AI Documentation Discovery Paths for v0](20260708T180200Z_supplementary-ai-documentation-discovery-paths.md)
