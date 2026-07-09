# AI generation rules

Rules for generating or editing `.repor` files. Read [`docs/syntax.md`](../syntax.md) and [`docs/semantics.md`](../semantics.md) first — this document does not repeat their content, only states what to do and not do when using them.

## Use only documented syntax

Every construct used in a `.repor` file must appear in [`docs/syntax.md`](../syntax.md), the grammar reference generated from `crates/reportage-core/src/reportage.pest`. If a construct is not there, it is not part of this version of reportage, regardless of how plausible it looks.

For the semantic rules that govern how a construct behaves once parsed (assertion evaluation, logical composition, expectations, and so on), use [`docs/semantics.md`](../semantics.md) and the generated [`docs/language/semantic-rules.md`](../language/semantic-rules.md) the same way.

## Never treat `docs/TBD.md` as available syntax

[`docs/TBD.md`](../TBD.md) records intentionally deferred features and design topics. Nothing listed there is usable syntax, however concrete it reads. Do not generate a script that relies on a `TBD.md` entry, and do not present a `TBD.md` item to a user as if it already worked.

## Never invent future syntax

Do not extrapolate a plausible-looking extension of an existing construct (a new keyword, a new assertion form, a new block type) unless it is already documented. If a script requires behavior that does not exist yet, say so explicitly instead of writing something that merely looks like it would work.

## Prefer examples over first-principles construction

`examples/*.repor` and the fixtures under `tests/fixtures/syntax/valid/` are known-good scripts. Prefer adapting one of these over constructing a script from the grammar alone — it lowers the chance of combining individually valid constructs into a form the parser rejects. See [`docs/syntax-conformance.md`](../syntax-conformance.md) for how the fixtures under `tests/fixtures/syntax/` are organized.

## Verify after generating

Generating or editing a `.repor` file is not the end of the task. See [`docs/ai/validation-flow.md`](validation-flow.md) for the command to run afterward.
