# reportage-syntax-change

Use this skill when adding, removing, or changing reportage DSL syntax.

This skill is required for changes that affect any of the following:

- `.reportage` source syntax
- pest grammar rules
- parser behavior
- AST shape caused by syntax
- syntax-related diagnostics
- semantic validation rules for newly parsed syntax
- generated language documentation derived from grammar or syntax definitions

## Required work

When adding or changing syntax, do all of the following.

### 1. Add valid examples

Add examples that show the new syntax being used in realistic reportage files.

The examples must include:

- at least one minimal valid example
- at least one realistic example that resembles how a user would write the syntax in practice

The examples should not exist only as parser unit tests.
They must be visible as examples or fixtures that future maintainers and AI agents can inspect.

### 2. Add syntax error examples

Add examples that demonstrate inputs rejected at parse time.

A syntax error example must satisfy all of the following:

- the file cannot be parsed into a valid AST
- the failure is caused by source syntax, not by semantic validation
- the example documents what shape is intentionally invalid
- the expected failure is tested or covered by an existing example-validation mechanism

Do not classify an example as a syntax error if it parses successfully and is rejected later.

### 3. Add semantic error examples

Add examples that parse successfully but are rejected by semantic validation.

A semantic error example must satisfy all of the following:

- the source file is syntactically valid
- the parser can construct an AST
- the semantic evaluator rejects the program
- the example documents which semantic rule is violated

Do not classify an example as a semantic error if the parser rejects it.

If no meaningful semantic error exists for the syntax change, explicitly document why.
Do not create artificial semantic-invalid examples that misrepresent the language model.

### 4. Update parser and semantic tests

The implementation must include tests covering:

- valid parsing of the new syntax
- rejection of syntax-invalid examples
- semantic rejection of semantic-invalid examples, when applicable
- preservation of existing syntax behavior

If diagnostic code validation is not yet stable, tests may validate the failure class instead of the exact diagnostic code.
Do not silently omit diagnostics from consideration.

### 5. Update documentation or generated documentation

If the grammar, syntax reference, generated language docs, or examples index is affected, update it in the same change.

If generated documentation exists, run the project’s documentation generation/check command and commit the resulting changes when appropriate.

## Review checklist

Before considering the change complete, verify:

- [ ] The new syntax has at least one valid example.
- [ ] Syntax-invalid examples exist and fail at parse time.
- [ ] Semantic-invalid examples exist, or their absence is explicitly justified.
- [ ] Syntax error and semantic error examples are not mixed up.
- [ ] Parser tests cover the new syntax.
- [ ] Semantic validation tests cover the new rule, if any.
- [ ] Documentation/examples reflect the new syntax.
- [ ] Existing examples still pass.
- [ ] Generated docs or snapshots are updated intentionally.

## Common mistakes

Avoid these mistakes:

- adding parser support without user-facing examples
- testing only the happy path
- treating semantic rejection as syntax rejection
- creating invalid examples that are not executed or checked anywhere
- updating grammar without updating generated docs
- changing AST shape without checking semantic tests
- relying only on inline unit tests when external examples would be more inspectable
