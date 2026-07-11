
# Rename `reportage docs` to `reportage references` and Reserve `docs` for Documentation Generation

- Status: Proposed
- Created: 2026-07-11T07:00:08Z

## Context

[Tag-Based GitHub URLs and `reportage docs` as the v0 AI Documentation Discovery Path](20260708T180000Z_ai-documentation-discovery-core-path.md) introduced `reportage docs` as the command that prints the versioned official-reference URL index for the running binary, and #137 implemented it.

A follow-up issue plans a new command that generates documentation from a user's Reportage source.
Reference discovery and source documentation generation are different responsibilities: one lists the official reference URLs that correspond to the installed binary, the other produces new documents from user-authored input.
Both naturally want the name `docs`, so the name has to be assigned deliberately before the second command exists.

Separately, the core-path ADR specified a `v{version}` tag (`version = 0.1.0` → `tag = v0.1.0`), but the repository's actual tag convention has no `v` prefix (`version = 0.0.4` → `tag = 0.0.4`), and the implementation already emits unprefixed tags.
The JSON Schema still required `^v.+$`, so the contract, the ADR, and the implementation disagreed.

Related issue: #166.

## Decision

- `references` is the reference discovery command: it prints the official reference URL index corresponding to the running binary, exactly as the former `reportage docs` did, including `--format=json`.
- `docs` is assigned to the future source documentation generation command.
- The two are not merged into modes of a single command: their inputs, outputs, and responsibilities differ.
- Because reportage is in v0, the old `reportage docs` is not kept as a references alias, and no deprecation period is provided.
- Until the documentation generation command ships, `docs` stays registered as a reserved dummy subcommand: it never resolves as a positional script path, prints a not-implemented error to stderr, exits non-zero (exit code `2`, the "requested operation could not be treated as valid input" meaning shared with `shim scaffold` request errors), and is hidden from normal `--help` output.
- The machine-readable contract is renamed with the command: the authoritative contract path moves from `spec/output/docs-index/` to `spec/output/references-index/`, with the Schema `$id`, title, and description updated to references terminology. The old path is not kept as a compatibility alias.
- The JSON document's field structure and semantics do not change, so `schema_version` stays the integer `1`.
- The tag convention is recorded as it actually operates: version tags carry no `v` prefix, and `tool.tag == tool.version`. The Schema's tag and URL patterns are aligned to the unprefixed form.

## Alternatives Considered

### Keep `docs` as the reference discovery command and pick another name for generation

Rejected. "docs" reads as "produce/manage documentation" more than "list official reference URLs"; giving the generation command the `docs` name and the discovery command the more precise `references` name matches user expectations for both.

### Keep `reportage docs` as an alias, or add a deprecation warning

Rejected. reportage is in v0 with no compatibility guarantee, and an alias would permanently blur the responsibility split this rename exists to create. A hard cutover with a clear not-implemented error (which names `reportage references`) is cheaper now than an alias is later.

### Remove the `docs` subcommand entirely until the generation command ships

Rejected. Without a registered subcommand, `reportage docs` would be parsed as a positional script path named `docs`, producing a confusing script-not-found error and making the later introduction of the real command a behavior change for such invocations. Reserving the name keeps the future slot unambiguous.

## Consequences

### Positive Consequences

- The `docs` name is free for the documentation generation command, with no legacy discovery semantics attached.
- The discovery command's name states what it lists: official references for the installed version.
- Schema, ADRs, and implementation agree on the unprefixed tag convention.

### Negative Consequences

- Existing invocations and integrations using `reportage docs` (including `--format=json` consumers) break immediately and must switch to `reportage references`.
- External consumers of `spec/output/docs-index/` must update to `spec/output/references-index/`.

### Neutral Consequences

- The decisions of the core-path ADR other than the command name and tag spelling — tag-based URLs, the human URL / AI-readable URL distinction, side-effect-free discovery, no network access, no tag existence or URL reachability checks — remain in force unchanged.
- `documents[]` structure, the recommended reading order, and `validation.command` are unchanged.

## References

- [#166](https://github.com/tooppoo/reportage/issues/166)
- [Tag-Based GitHub URLs and `reportage docs` as the v0 AI Documentation Discovery Path](20260708T180000Z_ai-documentation-discovery-core-path.md)
