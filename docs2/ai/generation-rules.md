# AI generation rules

Rules for generating or editing `.repor` files. Read [the generated syntax reference](../../docs/syntax.md) and [the language semantics reference](../reference/semantics.md) first — this document does not repeat their content, only states what to do and not do when using them.

## Use only documented syntax

Every construct used in a `.repor` file must appear in [the generated syntax reference](../../docs/syntax.md), the grammar reference generated from [`crates/reportage-core/src/reportage.pest`](../../crates/reportage-core/src/reportage.pest). If a construct is not there, it is not part of this version of reportage, regardless of how plausible it looks.

For the semantic rules that govern how a construct behaves once parsed (assertion evaluation, logical composition, expectations, and so on), use [the language semantics reference](../reference/semantics.md) and [the generated semantic rule catalog](../../docs/language/semantic-rules.md) the same way.

## Never treat deferred topics as available syntax

[The deferred topics document](../planning/TBD.md) records intentionally deferred features and design topics. Nothing listed there is usable syntax, however concrete it reads. Do not generate a script that relies on one of its entries, and do not present a deferred item to a user as if it already worked.

## Never invent future syntax

Do not extrapolate a plausible-looking extension of an existing construct (a new keyword, a new assertion form, a new block type) unless it is already documented. If a script requires behavior that does not exist yet, say so explicitly instead of writing something that merely looks like it would work.

## Prefer examples over first-principles construction

[`examples/*.repor`](../../examples/) and the fixtures under [`tests/fixtures/syntax/valid/`](../../tests/fixtures/syntax/valid/) are known-good scripts. Prefer adapting one of these over constructing a script from the grammar alone — it lowers the chance of combining individually valid constructs into a form the parser rejects. See [Syntax conformance fixtures](../design/testing/syntax-conformance.md) for how the fixtures under `tests/fixtures/syntax/` are organized.

## Verify after generating

Generating or editing a `.repor` file is not the end of the task. See [the validation flow](validation-flow.md) for the command to run afterward.
