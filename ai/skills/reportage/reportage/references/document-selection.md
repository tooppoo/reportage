# Document Selection

## Purpose

This document defines which version-matched reportage documents an agent should read for each task.

The document index returned by:

```sh
reportage docs --format=json
```

is the authority for available document IDs, paths, URLs, order, version, and validation command.

## General Rules

Use document IDs as the primary lookup key.

Do not select documents by title alone because titles may change while IDs remain stable.

Preserve the relative order of selected entries as they appear in `documents[]`.

Read only the documents needed for the task, but do not omit a normative source required to justify syntax, semantics, diagnostics, output interpretation, or execution behavior.

Guides under `docs/ai/` help navigate the specification. They are not substitutes for normative syntax, semantics, diagnostics, or contract documents.

When a guide conflicts with a normative document, the normative document wins.

## Baseline Orientation

For an unfamiliar reportage repository or a first task in a session, read:

- `ai-readme`
- `ai-quick-reference`

These establish the documentation hierarchy and provide a minimal orientation.

Do not use the quick reference as a complete syntax or semantics specification.

## Creating a New Scenario

Read at least:

- `ai-readme`
- `ai-quick-reference`
- `syntax`
- `semantics`
- `semantic-rules`
- `ai-generation-rules`
- `ai-validation-flow`
- `ai-common-mistakes`

Also read:

- `execution-model` when the scenario depends on workspaces, checkpoints, action ordering, isolation, or command resolution
- `configuration` when creating or depending on `reportage.kdl`
- `json-report` when the task includes consuming structured execution output
- `run-result` when the task depends on persistent artifacts or evidence

Known-good examples and valid fixtures may be inspected when the version-matched AI generation guide points to them. They are examples, not normative specifications.

## Editing an Existing Scenario

Read at least:

- `syntax`
- `semantics`
- `semantic-rules`
- `ai-generation-rules`
- `ai-validation-flow`

Read `ai-common-mistakes` when changing:

- logical composition
- negation
- assertions
- diagnostic handling
- validation commands
- result interpretation

Read `execution-model` when changing:

- action order
- assertion checkpoints
- workspace assumptions
- command invocation behavior
- isolation behavior

Read `configuration` when the edited scenario depends on registered commands, path rules, adapters, or other config-defined behavior.

## Reviewing a Scenario

Read at least:

- `syntax`
- `semantics`
- `semantic-rules`
- `ai-generation-rules`
- `ai-common-mistakes`

Read `execution-model` when review findings depend on runtime ordering, checkpoint behavior, workspace lifecycle, or command resolution.

Read `ai-validation-flow` when dynamic validation is in scope.

Read `configuration` when the scenario cannot be understood without `reportage.kdl`.

The review must not stop at syntax validity. Use the authoring and review reference to assess assertion adequacy, reproducibility, safety, and evidence quality.

## Diagnosing a Failed Run

Read:

- `ai-validation-flow`
- `diagnostics`
- `exit-codes`
- `json-report`

Also read:

- `syntax` when the parser rejected the script
- `semantics` and `semantic-rules` when semantic evaluation rejected the script
- `execution-model` when failure depends on action order, workspace state, checkpoint selection, or command execution
- `configuration` when configuration resolution or registered commands are involved
- `run-result` when persistent artifacts are needed to reconstruct the failure

A diagnostic document may link to another version-matched normative document, such as a semantic-diagnostic catalog. Follow that version-matched link when needed.

Do not infer a diagnostic meaning from its code name alone.

## Working with Configuration

Read:

- `configuration`

Also read:

- `execution-model` when config changes affect command resolution, adapters, shims, or runtime behavior
- `syntax` and `semantics` when configuration changes alter how scenarios are interpreted
- `ai-validation-flow` when the config change must be exercised by a scenario run

Do not infer configuration fields from common conventions or another tool's configuration format.

## Reasoning About Execution

Read:

- `execution-model`

Also read:

- `semantics` when execution behavior affects assertion meaning
- `configuration` when execution depends on registered commands or adapters
- `run-result` when reasoning about persisted evidence
- `json-report` when reasoning about stdout JSON results

Use the execution model for:

- case workspace lifecycle
- ordering of actions and assertions
- checkpoints
- isolation
- command resolution
- adapter and shim responsibilities
- artifact timing

## Processing JSON Execution Reports

Read:

- `json-report`
- `ai-validation-flow`
- `exit-codes`

Read `diagnostics` when the report contains diagnostics.

Read `run-result` only when correlating the stdout report with persisted artifact data.

Do not rely on a summary in the AI guide when exact field names, allowed values, nullability, or schema behavior matters.

## Processing Artifacts and Evidence

Read:

- `run-result`

Follow version-matched links from the run-result contract to general artifact documentation when needed.

Also read:

- `execution-model` when artifact timing or workspace lifecycle matters
- `json-report` when correlating artifacts with stdout
- `ai-validation-flow` when artifacts were produced by a validation run

Do not infer artifact layout from a previous version.

## Writing Tooling Around Reportage

When generating code that invokes reportage or consumes its output, read:

- `json-report`
- `exit-codes`
- `diagnostics`
- `ai-validation-flow`

Also read:

- `run-result` when tooling consumes artifact manifests
- `configuration` when tooling writes or interprets `reportage.kdl`
- `execution-model` when tooling controls execution or isolation

For tooling, exact schemas and contracts take precedence over human-oriented summaries.

## Unsupported or Missing IDs

If a required document ID is absent from the current index:

1. Check whether a selected indexed document contains a version-matched relative link to the required normative material.
2. Follow and cache that link according to the storage reference.
3. Do not guess a URL based only on the repository's current layout.
4. Do not substitute default-branch content.

If the required normative material cannot be discovered through the current version-matched documentation graph, state that the installed version's documentation is insufficient for the requested determination.

## Examples and Fixtures

Examples and fixtures may be used to reduce construction errors.

Treat them as:

- known-good usage samples
- evidence of supported combinations in that version
- local style guidance when they belong to the current project

Do not treat them as:

- a replacement for syntax documentation
- a replacement for semantic rules
- proof that undocumented syntax is supported
- proof that every behavior shown is stable API

If an example conflicts with a normative document, follow the normative document and report the inconsistency when relevant.

## Minimal Reading Is Not Minimal Reasoning

Selecting fewer documents does not justify weaker analysis.

Even when a task needs only a small document set, distinguish:

- what the documents directly state
- what follows by necessary inference
- what remains uncertain
- what depends on application requirements rather than reportage rules

Fetch additional documents whenever the requested conclusion cannot be justified from the selected set.
