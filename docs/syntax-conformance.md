# Syntax Conformance Fixtures

Syntax conformance fixtures live under `tests/fixtures/syntax/`.

- `valid/*.repor` fixtures must parse successfully.
- `invalid/*.repor` fixtures must be rejected by the production `parse()` entrypoint.
- `ast_snapshots/*.json` files record the parsed AST shape for every valid fixture.

The AST snapshots are JSON because they are easier to review than Rust `Debug`
output. Snapshot serialization uses a test-only intermediate representation
rather than adding snapshot-specific serde constraints to the production AST
model.

## Updating AST snapshots

When a syntax fixture is added, renamed, removed, or intentionally changes AST
shape, refresh the snapshots:

```sh
UPDATE_AST_SNAPSHOTS=1 cargo test -p reportage-core --test syntax_conformance ast_snapshots_for_valid_syntax_fixtures_are_current
```

Then run:

```sh
cargo test -p reportage-core --test syntax_conformance
```

Review the JSON diff before committing. Field order is defined by the snapshot
intermediate representation, and files are written with stable pretty JSON plus
a trailing newline so ordinary line-oriented diffs remain readable.
