# Product Documentation Projection

- Status: Proposed
- Created: 2026-09-08T13:11:34Z

## Context

[ADR: Reportage-Source Documentation Is Its Own Subcommand](20260907T230710Z_reportage-source-documentation-subcommand.md) split documentation generation into a product-facing generator (`reportage docs`) and a Reportage-source one (`reportage docs-reportage`), and deliberately left the product projection's own model to the ADR written with it.
This is that ADR.

The projection's input is the parser's source-level model: `document file` / `document case` metadata, an optional `before_each`, and each case's semantic step sequence.
Its output must describe the product a `.repor` file tests, for that product's users, without exposing the DSL that expresses it.

The difficult part is not hiding syntax; it is deciding what a case *means* once its syntax is gone.
A reportage case is not a list of commands with their results: an `assert` block verifies the current checkpoint, which may be the initial one, and several actions may run between two checkpoints.
An expectation is likewise not a sentence; it is a subject and an operation on it, and the same operation word means different things on different subjects.
Both facts are easy to lose in a projection and expensive to recover afterwards, because a renderer built on a lossy model cannot reconstruct what the model dropped.

## Decision

### The product projection is a separate catalog, not a generalization of the source one

`ProductDocumentationCatalog` is built from the semantic step model and must not carry `.repor` source text in any field.
The Reportage-source `DocumentationCatalog` keeps carrying exactly that text.
Neither may be generalized into a universal document model to serve both.

The two share only what genuinely answers the same question: the `document file` / `document case` display fallbacks and the grouping and ordering contract, which both projections read from the same blocks.
That sharing lives in one module (`docs::metadata`) so the two cannot drift into two ordering contracts for the same metadata.

### A case becomes an ordered step sequence, not commands plus results

An example must hold one step sequence in source order, whose elements are a prepared file, a command, or a verification.
It must not be flattened into a `commands[]` and a `results[]`.

Reportage assertions verify the current checkpoint, not the action immediately before them.
Pairing each assertion with a preceding action would state a causal relationship the language does not define, and a case with two actions before one `assert`, or with an `assert` before any action, has no such pairing to state.
Keeping the sequence also keeps the checkpoint boundaries visible: where one verification ends and the next command begins is exactly what a reader follows.

### An expectation is a subject and an operation, never a rendered phrase

A documented expectation must be a structured pair — what is observed (exit status, stdout, stderr, a file, a directory) and what is asserted about it (exists, is empty, is exactly, contains, does not contain, matches) — with the expected value carried as its own structured value.
It must not be the expectation's source text, and it must not be a pre-rendered sentence.

This is what keeps the model consistent with the language's own assertion model, where a subject takes predicates (see [Language semantics](../reference/semantics.md) — File assertions).
It is also what lets a renderer phrase `file <"f"> contains "x"` (a substring of the file) and `dir <"d"> contains "x"` (an entry named `x`) differently, even though both spell `contains`: the operation alone is ambiguous, and only the pair resolves it.
A pre-rendered phrase would additionally fix wording across every format, and could not be re-phrased per audience later.

Logical compositions (`not` / `all` / `any`) must stay nested rather than being flattened, because `not { A B }` negates the two together and a flattened form would state a different condition.

### `write` is an input example, never a hidden fixture

Every `write` step must appear as a file example with its path and content.
Files a `.repor` writes are, in practice, the configuration and input files a product's users write themselves, so treating them as test scaffolding to hide would delete the most directly reusable part of the example.

A `write` that names a `mode` explicitly must carry it: an example whose file has to be executable is not reproducible without its permission bits.

An unnamed `mode` must not be surfaced, even though it is not absent.
Every `write` applies a mode, and an unnamed one is reportage's fixed `0o600` default (see [Language semantics](../reference/semantics.md) — File mode).
That default describes the case workspace reportage creates, not a permission a product's reader should reproduce; stating it in product documentation would present a test-harness detail as product guidance.
An explicitly named mode is different: the source named it because the example depends on it.

Content assembled from values captured during the run must keep its literal parts and name the binding filling each gap, rather than collapsing into a single "captured" marker.
A configuration file with one interpolated field is otherwise almost entirely literal text, and that text is the part a reader copies.

### Actions are not classified as product operations or setup

Every `$` action must appear as a command example, in source order.
Reportage records no distinction between "an operation the product's user performs" and "a command that only makes the example work", so inferring one would attribute meaning the source never carried.
Step-level visibility metadata is out of scope until a real need for it exists.

### `before_each` becomes each example's preparation

A file's `before_each` must reach every example as its preparation, and must never be named `before_each` in the output: that name is reportage's, not the product's.
Its steps use the same step projection as a case body, so a step surface shared between setup and case body stays shared in the documentation.

A file that declares setup but no case produces no example, and therefore no preparation.
There is nothing for a reader to prepare for.
The Reportage-source projection deliberately differs here and still shows that setup, because it documents the file as written rather than as an example.

### Binding declarations produce no step

A `let` binding declares where a captured value comes from.
It writes no file, runs no command, and verifies nothing, so it has no product-facing meaning of its own and must not become a step.
Its effect stays visible wherever the bound value is used, as a named captured segment of that value.

## Alternatives Considered

### Pair each assertion with the preceding action

Rendering "run this, get that" reads well and matches how readers think about examples.
Rejected: it is not what reportage evaluates. The pairing does not exist for an assertion at the initial checkpoint, and is arbitrary when several actions precede one assertion; a model that asserts it cannot represent the cases where it is false.

### Carry each expectation as its source text

Reusing the assertion's source text is the smallest possible model and always renders something.
Rejected: the acceptance criterion for this projection is that the DSL does not reach the output, and source text is the DSL. It would also make every renderer a string formatter over syntax it must not show.

### Carry each expectation as a rendered sentence

Building "exit code is 0" in the catalog is simpler for renderers.
Rejected: it fixes wording for every format at once, cannot be adapted per subject or audience, and puts presentation in the model — the same mistake the Reportage-source Catalog avoids by keeping presentation in renderers.

### Collapse any interpolated value into a single "captured" marker

Treating a value with any captured part as opaque is the smallest model, and matches how the execution model answers "can this be resolved without a binding environment".
Rejected: that question is not this projection's. Discarding the literal segments would delete most of a file whose content is one interpolated field inside otherwise static configuration, which is the common shape in real suites.

### Surface the effective file mode on every file example

Resolving an unnamed `mode` to the `0o600` default would make every file example state the permissions the file ends up with.
Rejected: those permissions are reportage's workspace default, not something the product's user does; stating them would document the harness. The alternative of resolving the default inside a renderer was rejected for the same reason, plus it would duplicate a `reportage-core` semantics constant in the documentation layer.

### Hide `write` steps as test fixtures

Treating `write` as scaffolding would keep the output focused on commands.
Rejected: in real suites those files are the product's own input format, and hiding them would remove the part of the example a reader is most likely to copy.

### Generalize the existing Catalog to serve both projections

One catalog with an optional source field, or a universal document AST, would avoid a second model.
Rejected: the two projections agree only on the `document` block metadata; every case-level field differs. A shared model would be a union of two shapes whose invariants contradict, and it is explicitly out of scope for this work.

## Consequences

### Positive Consequences

- A renderer cannot print Reportage DSL from the product catalog, because no field holds any.
- Checkpoint semantics survive into the documentation: order and verification boundaries are preserved rather than reinterpreted.
- Expectation rendering stays a renderer decision, so plain text and Markdown can phrase the same condition differently without changing the model.
- The two projections share exactly one ordering and fallback contract.

### Negative Consequences

- Two catalogs must be maintained, and a `document` block field added later has to be threaded into both (though only through the shared metadata module).
- The expectation model must stay exhaustive over the language's expectations; a new expectation kind is a change here as well as in the evaluator.
- A captured value is documented by the binding's name and the literal text around it, never by the value itself, so an example whose interesting part is that value reads thinner than one with literal content. Resolving it would require running the scenario, which documentation generation must not do.
- The projection does not state where a captured value comes from, even though the source model records it (a binding declares a stream and a capture mode). Naming the binding was enough to keep interpolated content readable; carrying the capture's origin is a further step, deliberately not taken until a renderer needs it.

### Neutral Consequences

- The catalog carries each source's display path even though a product document need not show it; whether to print it is a renderer decision.
- A file with `before_each` and no cases documents nothing but its own metadata, which differs from the Reportage-source projection's treatment of the same file.
- Expectation kinds that the v0 grammar does not produce are still represented, so the projection stays total over the expectation model rather than depending on which kinds happen to be parsed today.

## References

- Issue: [#257](https://github.com/tooppoo/reportage/issues/257)
- [ADR: Reportage-Source Documentation Is Its Own Subcommand](20260907T230710Z_reportage-source-documentation-subcommand.md)
- [ADR: Documentation Generation Command](20260723T070556Z_documentation-generation-command.md)
- [Language semantics](../reference/semantics.md)
- [Execution model](../reference/execution-model.md)
