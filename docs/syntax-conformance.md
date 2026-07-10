# Syntax Conformance Fixtures

Syntax conformance fixtures live under [`tests/fixtures/syntax/`](../tests/fixtures/syntax/).

- `valid/*.repor` fixtures must parse successfully and must produce an AST snapshot that matches the adjacent `valid/*.ast.json` file.
- `invalid/*.repor` fixtures must be rejected by the production `parse()` entrypoint.
- Each valid fixture has exactly one AST snapshot with the same base name: `valid/foo.repor` is paired with `valid/foo.ast.json`.
- Invalid fixtures do not have AST snapshots because they have no accepted AST shape to record.

The AST snapshots are JSON because they are easier to review than Rust `Debug` output. Snapshot serialization uses a test-only intermediate representation rather than adding snapshot-specific serde constraints to the production AST model.

## Updating AST snapshots

When a valid syntax fixture is added, renamed, removed, or intentionally changes AST shape, refresh the adjacent snapshots:

```sh
UPDATE_AST_SNAPSHOTS=1 cargo test -p reportage-core --test syntax_conformance ast_snapshots_for_valid_syntax_fixtures_are_current
```

Then run:

```sh
cargo test -p reportage-core --test syntax_conformance
```

Review the JSON diff before committing. Field order is defined by the snapshot intermediate representation, and files are written with stable pretty JSON plus a trailing newline so ordinary line-oriented diffs remain readable.

## Diagnostic codes

`invalid_syntax_fixtures_are_rejected` also asserts the stable diagnostic code (`ParseError::code()`) for fixtures where Reportage produces a fine-grained code beyond the generic `parse.syntax` wrapper. Not every invalid fixture needs an individual code assertion — see [`diagnostics.md`](diagnostics.md) for the naming convention and compatibility policy.
