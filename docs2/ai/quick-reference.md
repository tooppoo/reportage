# AI quick reference

The shortest path to a valid `.repor` file, for a fast orientation pass. This is a minimal reference, not a full syntax or semantics reference — follow the links below for anything beyond this page.

## Minimal shape

```reportage
case "always pass" {
  $ true
  assert {
    exit 0
  }
}
```

A module is one or more `case "<name>" { ... }` blocks. Inside a case, an action (`$ <command>`) precedes an `assert { }` block, which contains one or more expectation expressions (`exit <code>`, `stdout empty`, `stdout contains <string>`, `stderr empty`, `stderr contains <string>`, file/dir assertions, and more).

## Where the rest of the syntax lives

- Full grammar: [the generated syntax reference](../../docs/syntax.md) — the only source of truth for what is valid.
- Assertion/expectation semantics: [the language semantics reference](../reference/semantics.md) and [the generated semantic rule catalog](../../docs/language/semantic-rules.md).
- Known-good scripts to adapt from: [`examples/`](../../examples/) and [`tests/fixtures/syntax/valid/`](../../tests/fixtures/syntax/valid/).

## Prohibitions

- Do not use anything absent from [the generated syntax reference](../../docs/syntax.md).
- Do not treat [deferred topics](../planning/TBD.md) entries as usable syntax.
- Do not invent constructs that merely look plausible; see [the generation rules](generation-rules.md) and [common mistakes](common-mistakes.md).

## Validate after editing

```sh
reportage <file.repor> --format=json
```

See [the validation flow](validation-flow.md) for how to read the output.

## Full reading order

See the generated reading order at [`docs/ai/reading-order.generated.md`](../../docs/ai/reading-order.generated.md).
