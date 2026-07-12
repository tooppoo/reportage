# reportage documentation

## How this tree is organized

Documents are separated by audience and by role, so each substantive fact has exactly one home:

| Section | Audience | Contains |
| --- | --- | --- |
| [`guide/`](guide/README.md) | Users deciding on or using reportage | Positioning, decision guidance, and task-oriented navigation into the exact references. |
| [`reference/`](reference/README.md) | Anyone needing the exact contract | Normative behavior: syntax, semantics, execution model, configuration, diagnostics, exit codes, artifacts, shims. Several documents are generated; see below. |
| [`design/`](design/README.md) | Maintainers | Philosophy, design principles, technical selection, and testing strategy — the why behind the contracts. |
| [`planning/`](planning/TBD.md) | Maintainers | Intentionally deferred features and undecided topics. Nothing here is implemented behavior. |
| [`ai/`](ai/README.md) | AI agents | A thin navigation layer: reading order, authoring constraints, and validation steps. Not a specification. |

Architecture decision records are not part of this tree. They stay at [`docs/adr/`](../docs/adr/README.md), and documents here link to them there.

## Generated documents

Reference material that can be derived from an executable specification or the implementation is generated, never hand-written. The generated documents (the grammar reference, the semantic rule catalog, and the AI reading order) are not hand-copied into this tree; until their generators are repointed here, their current output lives under [`docs/`](../docs/). [`SHOULD_GENERATE.md`](SHOULD_GENERATE.md) records every document in this tree that is, or should become, generated, with the reason and the generation path.

## Where to start

- New to reportage: start at [the user guide index](guide/README.md).
- Need the exact behavior of a construct or command: start at [the reference index](reference/README.md).
- Changing reportage itself: start at [the design index](design/README.md), then the ADRs under [`docs/adr/`](../docs/adr/README.md).
- An AI agent writing `.repor` files: start at [the AI guide](ai/README.md).
