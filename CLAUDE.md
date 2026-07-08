
## File changes

- When making file changes, always use the `git-kura` skill unless the user explicitly instructs otherwise.
- Follow the workflow defined by the `git-kura` skill. Do not duplicate or reinterpret that workflow in this file.
- After completing file edits, always use the `subagent-review-loop` skill to review and revise the changes before reporting completion, unless the user explicitly instructs otherwise.
- When updating documentation, always use the `semantic-line-breaks` skill.
- When writing or modifying code comments, always use both the `semantic-line-breaks` skill and the `code-comment` skill.

## Development `reportage`

When a change affects reportage syntax or runtime behavior, use the appropriate reportage skill before editing.

- Use `reportage-syntax-change` for DSL syntax, grammar, parser, AST, or semantic validation changes.
- Use `reportage-behavior-change` for CLI-visible behavior, execution behavior, output, exit code, diagnostics, JSON, artifacts, or evidence changes.

If a change affects both syntax and behavior, use both skills.

Do not treat parser tests, unit tests, examples, and e2e tests as interchangeable.
Each protects a different contract:

- examples document intended user-facing usage
- syntax-invalid examples document parser rejection
- semantic-invalid examples document language-level rejection after parsing
- unit tests protect internal implementation behavior
- e2e tests protect observable CLI behavior
