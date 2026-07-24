---
name: reportage-syntax-change
description: Require valid, syntax-invalid, and semantic-invalid examples plus parser, semantic, and documentation coverage when adding, removing, or changing reportage DSL syntax, grammar, parser behavior, AST shape, or syntax-related diagnostics.
---

# reportage-syntax-change

Use this skill when adding, removing, or changing reportage DSL syntax.

Apply it to changes that affect any of the following:

- `.repor` source syntax
- pest grammar rules in [`crates/reportage-core/src/reportage.pest`](../../../crates/reportage-core/src/reportage.pest)
- parser behavior
- AST shape caused by syntax
- syntax-related diagnostics
- semantic validation rules for newly parsed syntax
- generated language documentation derived from grammar or syntax definitions

## Read the current references

For a repository change, start with [the generated AI reading order](../../../docs/ai/reading-order.generated.md) and read the local documents it lists.
The checked-out files describe the current repository revision, including unreleased changes.

For an installed reportage version, run `reportage references --format=json` and read the returned `documents[].urls.ai` entries in order.
Do not use `reportage docs` for reference discovery: that command generates documentation from `.repor` sources.
The URLs from `reportage references` are pinned to the running binary's version tag, so do not substitute them for the local documents when changing the current checkout.

## Required work

When adding or changing syntax, do all of the following.

### 1. Add valid examples

Add examples that show the new syntax being used in realistic reportage files.

A valid example must satisfy both of the following:

- it is syntactically valid and can be parsed into a valid AST
- it is semantically valid and is accepted by semantic validation

In this skill, "valid example" does not mean "parseable example".
A source file that parses successfully but is rejected by semantic validation is a semantic error example, not a valid example.

The valid examples must include:

- at least one minimal valid fixture under [`tests/fixtures/syntax/valid/`](../../../tests/fixtures/syntax/valid/)
- at least one realistic example under [`examples/`](../../../examples/) or an executable scenario under [`e2e/`](../../../e2e/)

The examples should not exist only as parser unit tests.
They must be visible as examples or fixtures that future maintainers and AI agents can inspect.

### 2. Add syntax error examples

Add examples that demonstrate inputs rejected at parse time.

A syntax error example must satisfy all of the following:

- the example lives under [`tests/fixtures/syntax/invalid/`](../../../tests/fixtures/syntax/invalid/)
- the file cannot be parsed into an AST
- the failure is caused by source syntax, not by semantic validation
- the example documents what shape is intentionally invalid
- the production parser rejects it through the syntax conformance test

Do not classify an example as a syntax error if it parses successfully and is rejected later.

### 3. Add semantic error examples

Add examples that parse successfully but are rejected by semantic validation.

A semantic error example must satisfy all of the following:

- the source file is syntactically valid
- the parser can construct an AST
- the semantic evaluator rejects the program
- the example documents which semantic rule is violated
- the rule and its conformance cases are updated under [`spec/language/semantics/`](../../../spec/language/semantics/) when the semantic spec model covers the rule

Do not classify an example as a semantic error if the parser rejects it.

If no meaningful semantic error exists for the syntax change, explicitly document why.
Do not create artificial semantic-invalid examples that misrepresent the language model.

### 4. Update parser and semantic tests

The implementation must include tests covering:

- valid parsing of the new syntax
- rejection of syntax-invalid examples
- semantic rejection of semantic-invalid examples, when applicable
- preservation of existing syntax behavior
- intentional AST snapshot changes for every changed valid syntax fixture

If diagnostic code validation is not yet stable, tests may validate the failure class instead of the exact diagnostic code.
Do not silently omit diagnostics from consideration.

### 5. Update documentation or generated documentation

If the grammar, syntax reference, generated language docs, or examples index is affected, update it in the same change.

Regenerate affected files from their authoritative source rather than editing generated output.
Use the current repository recipes:

```sh
UPDATE_AST_SNAPSHOTS=1 cargo test --locked -p reportage-core --test syntax_conformance ast_snapshots_for_valid_syntax_fixtures_are_current
cargo test --locked -p reportage-core --test syntax_conformance
just lang-docs-gen
just lang-docs-check
```

When semantic rules change, also run:

```sh
just semantic-docs-gen
just semantic-docs-check
just semantic-specs-check
just semantic-rule-coverage-check
```

Review every generated diff before committing it.

### 6. Run repository-level coverage

Run the focused fixture and self-test coverage after the generated files are current:

```sh
cargo nextest run --locked -p reportage-core --test grammar_fixtures
cargo nextest run --locked -p reportage-cli --test self_test
```

Finish with `just check`.

## Review checklist

Before considering the change complete, verify:

* [ ] The new syntax has at least one syntactically and semantically valid example.
* [ ] Valid examples are not merely parseable examples.
* [ ] Syntax-invalid examples exist and fail at parse time.
* [ ] Semantic-invalid examples parse successfully but fail semantic validation, or their absence is explicitly justified.
* [ ] Syntax error and semantic error examples are not mixed up.
* [ ] Parser tests cover the new syntax.
* [ ] Semantic validation tests cover the new rule, if any.
* [ ] Documentation/examples reflect the new syntax.
* [ ] Existing examples still pass.
* [ ] Generated docs or snapshots are updated intentionally.
* [ ] The current local reading order and references were used instead of version-tag documentation for another version.
* [ ] Focused checks and `just check` pass.

## Common mistakes

Avoid these mistakes:

- adding parser support without user-facing examples
- testing only the happy path
- treating semantic rejection as syntax rejection
- creating invalid examples that are not executed or checked anywhere
- updating grammar without updating generated docs
- changing AST shape without checking semantic tests
- relying only on inline unit tests when external examples would be more inspectable
