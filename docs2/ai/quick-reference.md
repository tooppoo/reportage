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

- Full grammar: [`crates/reportage-core/src/reportage.pest`](../../crates/reportage-core/src/reportage.pest) — the normative source of truth for what is valid.
- Assertion/expectation semantics: [the language semantics reference](../reference/semantics.md) and the semantic rule specs under [`spec/language/semantics/`](../../spec/language/semantics/README.md).
- Known-good scripts to adapt from: [`examples/`](../../examples/) and [`tests/fixtures/syntax/valid/`](../../tests/fixtures/syntax/valid/).

## Prohibitions

- Do not use anything absent from the grammar at [`crates/reportage-core/src/reportage.pest`](../../crates/reportage-core/src/reportage.pest).
- Do not treat [deferred topics](../planning/TBD.md) entries as usable syntax.
- Do not invent constructs that merely look plausible; see [the generation rules](generation-rules.md) and [common mistakes](common-mistakes.md).

## Validate after editing

```sh
reportage <file.repor> --format=json
```

See [the validation flow](validation-flow.md) for how to read the output.

## Full reading order

See [the AI documentation guide](README.md).
