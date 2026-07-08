
# Tag-Based GitHub URLs and `reportage docs` as the v0 AI Documentation Discovery Path

- Status: Proposed
- Created: 2026-07-08T18:00:00Z

## Context

Reportage is a new tool. General-purpose AI/LLM assistants cannot be expected to already know its exact grammar, semantics, execution model, or how to read its diagnostics.

Expanding the pest grammar and writing more documentation is necessary but not sufficient. Before an AI can use that information, it first has to discover where the authoritative information lives.

This matters most not inside the reportage development repository itself, but inside a user's project that merely depends on reportage, where an AI is asked to write, edit, or review `.repor` files. In that setting, the AI needs to discover, without prior knowledge:

- that the project uses reportage at all;
- where the authoritative grammar, semantics, and execution-result model are documented;
- which version of reportage is installed locally;
- the official documentation URL that corresponds to that version;
- which command to run to validate a `.repor` file after generating or editing it;
- which documentation URL to consult from a diagnostic or JSON output.

A dedicated Web site or GitHub Pages deployment could serve as a discovery hub, but building and operating one is a separate cost that should not block a v0 discovery path. AI consumers also often prefer raw Markdown over rendered HTML, since raw Markdown is cheaper to fetch and parse.

This is the first of three ADRs recorded from issue #136, a policy issue that surveys how reportage communicates information to AI agents. It covers the core v0 discovery path.

Related issues: #136, #137, #142.

## Decision

v0 does not require a dedicated Web site or GitHub Pages deployment. The core documentation discovery command is `reportage docs`.

`reportage docs` must build a `v{version}` tag from the reportage binary's own runtime version, and print, for each known document, both a human URL and an AI-readable URL derived from that tag.

```text
version = 0.1.0
tag     = v0.1.0
```

```text
human URL:
  https://github.com/tooppoo/reportage/blob/<tag>/<path>

AI-readable URL:
  https://raw.githubusercontent.com/tooppoo/reportage/<tag>/<path>
```

- The human URL must be an ordinary GitHub `blob` URL.
- The AI-readable URL must be a `raw.githubusercontent.com` URL that resolves to the raw Markdown source.
- `reportage docs` must resolve the tag from the reportage binary's own runtime version. It must not accept an arbitrary version argument in v0.
- `reportage docs` does not verify that the resolved tag exists or that the URLs are reachable. Tag existence is a release-process concern, not a `reportage docs` concern.
- v0 must not support a `latest`- or `main`-based URL. `main` and `latest` change over time; using them would let the resolved documentation drift away from the reportage version actually installed, and could cause an AI to generate syntax that is not yet implemented, or that has already been removed.

`reportage docs --format=json` must produce a machine-readable docs index, using the same `--format=json` convention already established by [`reportage run`](20260707T045900Z_json-output-as-structured-execution-report.md). `reportage docs --ai` and `reportage docs --json` are not introduced in v0, because:

- `reportage docs` itself is already the documentation discovery command; a separate `--ai` mode is not needed.
- The AI-readable URL for each document is represented per-document (e.g. as a `urls.ai` field), not as a separate output mode.
- JSON output should follow the project's existing `--format=json` convention rather than introduce a second flag spelling.

`reportage --help` must not enumerate the full set of documentation URLs. It must instead point to `reportage docs`:

```text
Documentation:
  Run `reportage docs` to list versioned documentation URLs.
```

Repository owner/name changes are treated as a future concern and are out of scope for this ADR.

## Alternatives Considered

### Require a dedicated Web site or GitHub Pages deployment for v0

Rejected for v0. Building and operating a Web site is a separate implementation and maintenance cost. Tag-based GitHub URLs provide a working, versioned discovery path without that cost, and remain compatible with a future Web site.

### Support a `latest`/`main` documentation URL

Rejected. A `latest` or `main` URL would drift from the reportage version actually installed by the user, and could cause an AI to reference future or removed syntax. Every generated URL must correspond to the runtime version's tag.

### Add `reportage docs --ai` / `reportage docs --json` flags

Rejected. `reportage docs` is already the discovery command, and a second flag spelling would compete with the project's existing `--format=json` convention. Per-document AI URLs are represented as data (`urls.ai`), not as a separate CLI mode.

## Consequences

### Positive Consequences

- Versioned documentation URLs are available without building a Web site.
- An AI with no prior knowledge of reportage can reach documentation by running `reportage --help` and then `reportage docs`.
- The documentation URL always corresponds to the reportage version actually installed.
- Human and AI-readable URLs are explicitly distinguished.
- `--format=json` lets AI and external tooling consume the docs index programmatically.

### Negative Consequences

- Development or prerelease builds without a matching tag will produce URLs that do not resolve.
- The URL scheme is coupled to the `tooppoo/reportage` GitHub owner/repository name.
- Without a dedicated site, the human browsing experience is limited to GitHub's own rendering.

### Neutral Consequences

- Tag existence and URL reachability are not verified by `reportage docs` itself.
- A future Web site or `llms.txt` may be layered on top of this discovery path without replacing it; see [Supplementary AI Documentation Discovery Paths for v0](20260708T180200Z_supplementary-ai-documentation-discovery-paths.md).

## References

- [#136](https://github.com/tooppoo/reportage/issues/136)
- [#137](https://github.com/tooppoo/reportage/issues/137)
- [#142](https://github.com/tooppoo/reportage/issues/142)
