# VS Code Extension Repository and Release Units

- Status: Accepted
- Created: 2026-07-10T21:05:26Z

## Context

reportage includes VS Code language support under the repository's editor-support area. The extension is currently a language-support package: TextMate grammar, language configuration, examples, snippets, and package metadata.

This raises two separate questions that should not be collapsed into one decision:

1. whether the VS Code extension should continue to be developed in the reportage repository or move to a separate repository;
2. whether the VS Code extension should be released from the same Git tag as the reportage CLI and language implementation.

These concerns have different coupling characteristics.

The extension's source is closely coupled to the reportage language syntax, examples, and documentation. In the early v0 phase, syntax changes and editor grammar updates should be reviewable together, and keeping them in the same repository reduces coordination cost.

The extension artifact, however, is a distinct distribution unit. It has its own VS Code extension manifest, package version, Marketplace/Open VSX publishing process, release notes, and failure modes. A CLI-only release should not necessarily consume a VS Code extension version, and an editor-only fix should not require a reportage CLI release.

## Decision

The VS Code extension will remain in the reportage repository for the current development phase.

The extension's release unit must be separate from the reportage CLI release unit.

Concretely:

- VS Code extension source should remain under the reportage repository's editor-support tree for now.
- The reportage CLI and core project release tags must remain the normal project tags, such as `v0.1.0`.
- The VS Code extension must use a separate tag namespace for publishing, such as `vscode-reportage-v0.0.1`.
- A reportage project release tag may run a VS Code extension package check, but it must not publish the VS Code extension by default.
- Publishing the VS Code extension should be triggered only by the extension-specific tag namespace or by an explicitly documented manual process.
- The extension version in `package.json` must be treated as the extension artifact version, not as the reportage CLI version.

This decision separates repository locality from release identity: the extension may be co-developed in the same repository while still being released as an independent artifact.

## Alternatives Considered

### Move the VS Code extension to a separate repository immediately

This was rejected for the current phase.

The extension is still primarily language support, and its source changes are expected to track reportage syntax changes closely. A separate repository would add issue, branch, review, CI, and publishing surface area before the extension has enough independent behavior to justify that cost.

A separate repository remains a future option if the extension grows into a more independent tool.

### Publish the VS Code extension from every reportage release tag

This was rejected.

It would make `v0.1.0` ambiguous: the tag would represent both the reportage CLI/core release and the VS Code extension release. It would also force editor-extension versions to be consumed for CLI-only changes, and would make editor-only fixes depend on the CLI release cadence.

The extension can still document compatibility with a reportage language version in its changelog or README without sharing the same release tag.

### Keep both repository and release fully unified

This was rejected because the extension artifact has different release mechanics and failure modes from the CLI/core artifact.

Repository co-location is useful for development. Release unification is not required for that benefit and creates unnecessary coupling.

## Future Re-evaluation Criteria

Repository separation should be reconsidered if one or more of the following become true:

- the VS Code extension gains substantial TypeScript implementation beyond static language support;
- the extension includes a language client, diagnostics, formatting, runner integration, or command execution behavior;
- the extension has an independent roadmap, issue flow, or contributor group;
- Marketplace/Open VSX release engineering becomes large enough to dominate the editor-support workflow;
- editor support expands beyond VS Code and a separate `reportage-editors` repository becomes a better organizing unit.

If repository separation happens later, this ADR does not require the extension-specific tag namespace to remain unchanged, but the release-unit separation should remain unless a later ADR explicitly supersedes it.

## Consequences

### Positive Consequences

- Syntax, examples, snippets, and grammar changes can be reviewed together with the language implementation while reportage is still evolving.
- The VS Code extension can be published only when its own artifact changes.
- CLI-only releases do not consume editor-extension versions.
- Editor-only fixes can be released without forcing a reportage CLI/core release.
- Release workflows can apply different safeguards to CLI/core artifacts and extension artifacts.

### Negative Consequences

- The repository will contain more than one release workflow and more than one tag namespace.
- Contributors must understand that repository co-location does not imply release co-versioning.
- Release documentation must explain which tags publish which artifacts.

### Neutral Consequences

- A reportage release workflow may still package-check the VS Code extension to catch breakage caused by syntax or example changes.
- The extension may use the same numeric version as a compatible reportage release when that is useful, but the tag namespace should remain distinct.
- Future LSP or diagnostics work may require a new ADR if it changes the coupling between the CLI/core implementation and the extension artifact.
