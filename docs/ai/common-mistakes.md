# AI common mistakes

Short wrong/correct examples of mistakes AI agents commonly make when writing or reviewing `.repor` files. For the full picture behind any example here, follow its link — this document intentionally stays short rather than becoming a second example collection; see `examples/*.repor` and `tests/fixtures/syntax/valid/` / `tests/fixtures/syntax/invalid/` for a fuller set.

## Fabricating logical composition syntax

Reportage rejects infix `and`/`or` and `and { }` / `or { }` aliases; only block-form `all { }` / `any { }` / `not { }` are accepted. See [`docs/semantics.md`](../semantics.md) (Logical composition) and [ADR: block-form logical composition](../adr/20260704T150000Z_block-form-logical-composition.md).

Wrong:

```reportage
assert {
  exit 0 and stdout empty
}
```

Correct:

```reportage
assert {
  all {
    exit 0
    stdout empty
  }
}
```

## Fabricating predicate-level negation

`not` is a block that wraps expectations; it is not a modifier on a single predicate. See [`docs/semantics.md`](../semantics.md) (Logical composition).

Wrong:

```reportage
assert {
  file <"out.txt"> not exists
}
```

Correct:

```reportage
assert {
  not {
    file <"out.txt"> exists
  }
}
```

## Confusing a syntax error with a semantic error

A syntax error means the parser rejected the script (`docs/diagnostics.md`, `script_error`, exit code `2`). A semantic error means the script parsed but a semantic rule rejected it during evaluation (`docs/semantic-diagnostics.md`, `semantic.*` / `assertion.*` / `step.*` codes). An assertion failure is neither: the script was valid and ran, but a check inside `assert { }` did not pass (`test_failed`, exit code `1`). Do not report one as another when explaining a failure — the `category` and `code` fields in `--format=json` diagnostics distinguish them; see [`docs/ai/validation-flow.md`](validation-flow.md).

## Suggesting `reportage check` for validation

`reportage check <file> --format=json` does not exist in the current CLI. The command that exists today is:

```sh
reportage <file.repor> --format=json
```

Always use the `validation.command` field from `reportage references --format=json` rather than assuming `reportage check` has shipped by the time this guide is read.
