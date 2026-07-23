# reportage documentation

## How this tree is organized

Documents are separated by audience and by role, so each substantive fact has exactly one home:

| Section | Audience | Contains |
| --- | --- | --- |
| [`guide/`](guide/README.md) | Users deciding on or using reportage | Positioning, decision guidance, and task-oriented navigation into the exact references. |
| [`examples/`](examples/index.md) | Examples of e2e tests by `reportage` |
| [`reference/`](reference/README.md) | Anyone needing the exact contract | Normative behavior: syntax, semantics, execution model, configuration, diagnostics, exit codes, artifacts, shims. Several documents are generated; see below. |
| [`design/`](design/README.md) | Maintainers | Philosophy, design principles, technical selection, and testing strategy — the why behind the contracts. |
| [`planning/`](planning/TBD.md) | Maintainers | Intentionally deferred features and undecided topics. Nothing here is implemented behavior. |
| [`ai/`](ai/README.md) | AI agents | A thin navigation layer: reading order, authoring constraints, and validation steps. Not a specification. |
| [`adr/`](adr/README.md) | Maintainers | Architecture decision records: the context, alternatives, and trade-offs behind durable decisions. |

## Generated documents

Reference material that can be derived from an executable specification or the implementation is generated, never hand-written.

The generated documents in this tree are [the syntax reference](reference/syntax.md), [the semantic rule catalog](reference/semantic-rules.md), [examples](examples/index.md), and [the AI reading order](ai/reading-order.generated.md);

edit their sources and regenerate, never the files themselves.

[`SHOULD_GENERATE.md`](SHOULD_GENERATE.md) records each generated document's source and generator, plus the hand-written sections that are candidates for generation.

## Where to start

- New to reportage: start at [the user guide index](guide/README.md).
- Need the exact behavior of a construct or command: start at [the reference index](reference/README.md).
- Changing reportage itself: start at [the design index](design/README.md), then the ADRs under [`adr/`](adr/README.md).
- An AI agent writing `.repor` files: start at [the AI guide](ai/README.md).
