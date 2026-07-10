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

- Full grammar: [`docs/syntax.md`](../syntax.md) — the only source of truth for what is valid.
- Assertion/expectation semantics: [`docs/semantics.md`](../semantics.md) and the generated [`docs/language/semantic-rules.md`](../language/semantic-rules.md).
- Known-good scripts to adapt from: `examples/*.repor` and `tests/fixtures/syntax/valid/`.

## Prohibitions

- Do not use anything absent from [`docs/syntax.md`](../syntax.md).
- Do not treat [`docs/TBD.md`](../TBD.md) entries as usable syntax.
- Do not invent constructs that merely look plausible; see [`docs/ai/generation-rules.md`](generation-rules.md) and [`docs/ai/common-mistakes.md`](common-mistakes.md).

## Validate after editing

```sh
reportage <file.repor> --format=json
```

See [`docs/ai/validation-flow.md`](validation-flow.md) for how to read the output.

## Full reading order

See [`docs/ai/reading-order.generated.md`](reading-order.generated.md).
