# Reference Index

This directory records the external contract for `reportage references --format=json`'s CLI stdout document:
the versioned documentation URL index (issue #137, renamed from `reportage docs` in issue #166).

## Relationship to other machine-readable outputs

This is **not** the run report document (`reportage run --format=json`, [`spec/output/json-report/schema.json`](../json-report/schema.json)).
Both are selected with a flag spelled `--format=json`, but each `--format` belongs to its own (sub)command and the two output contracts are independent: they version independently and share no structure.

`reportage references` is a side-effect-free reference discovery command.
It runs no test scripts, loads no config, writes no artifacts, never creates or updates `.reportage/`, and performs no network access.
It also does not verify that the tag exists, that the URLs are reachable, or that `documents[].path` exists — see the validation policy below.
See [`docs/adr/20260708T180000Z_ai-documentation-discovery-core-path.md`](../../../docs/adr/20260708T180000Z_ai-documentation-discovery-core-path.md).

## Contract highlights

- `schema_version` is the integer `1`.
- `tool.name` is the binary name `"reportage"`, not the Rust package name.
- `tool.tag` is identical to `tool.version`, with no `v` prefix, built mechanically from the runtime version, even for dev/prerelease builds whose tag may not exist yet.
- `documents[]` is ordered: the order is the recommended reading order for AI consumers and must stay stable.
- `documents[].id` is a stable identifier, unique within `documents[]`, that survives title and path renames.
- `urls.human` is a GitHub `blob` URL; `urls.ai` is a `raw.githubusercontent.com` URL serving the raw source.
- `validation.command` is the CLI invocation to run after editing a `.repor` file, and always points at an invocation that exists in the version that produced the index.

## JSON Schema

`schema.json` defines the expected structure of the `reportage references --format=json` document and is useful for editor integration (autocomplete, inline validation).

CI validation is performed by typed Rust deserialization in `crates/reportage-cli/tests/references_index.rs`, following the same approach as [`spec/output/json-report/README.md`](../json-report/README.md): the command's JSON output is deserialised into Rust structs marked `#[serde(deny_unknown_fields)]`, which rejects unknown fields and enforces required fields, without an external JSON Schema validator dependency.

## Validation policy

Schema validation is limited to output structure.

The repository-existence of each `documents[].path` is a repository consistency property, not a structural one, so it is verified by a separate check in `crates/reportage-cli/tests/references_index.rs` that resolves each path against the workspace root.
Keeping the two checks separate keeps their failure causes distinct: a schema failure means the output contract regressed, a path failure means the repository moved or removed a document the index still references.

Tag existence and URL reachability are not verified anywhere in this contract's checks: `reportage references` performs no network access, and tag existence is a release-process concern.

## Decision records

- [`docs/adr/20260708T180000Z_ai-documentation-discovery-core-path.md`](../../../docs/adr/20260708T180000Z_ai-documentation-discovery-core-path.md) — tag-based GitHub URLs and the reference discovery command as the v0 AI documentation discovery path.
- [`docs/adr/20260708T180100Z_ai-facing-docs-are-a-guide-not-source-of-truth.md`](../../../docs/adr/20260708T180100Z_ai-facing-docs-are-a-guide-not-source-of-truth.md) — AI-facing docs are a guide over the authoritative sources.
- [`docs/adr/20260708T180200Z_supplementary-ai-documentation-discovery-paths.md`](../../../docs/adr/20260708T180200Z_supplementary-ai-documentation-discovery-paths.md) — supplementary discovery paths layered on top of this one.
- [`docs/adr/20260711T070008Z_rename-docs-command-to-references.md`](../../../docs/adr/20260711T070008Z_rename-docs-command-to-references.md) — the rename of `reportage docs` to `reportage references` and the unprefixed tag convention.
