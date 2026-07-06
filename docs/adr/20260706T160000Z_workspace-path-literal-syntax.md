# Workspace Path Literal Syntax

- Status: Accepted
- Created: 2026-07-06T16:00:00Z

## Context

Until #93, the quoted literal `"..."` carried two unrelated meanings depending on context. In a `write` step's content position or a `contains` expectation's expected position it was text content, while in a `write` step's path position or a `file` / `dir` checkpoint subject position it was a filesystem path resolved inside the case workspace.

Two developments made this contextual typing untenable as a long-term language rule:

- #86 defined `TextValue = StringLiteral | HeredocLiteral` as the text-domain value category shared by `write` and `file ... contains`.
- #92 decided to introduce the fixture reference value `@"<path>"`, and #87 / #88 defined future `contents_equals` / `text_equals` expectations whose expected values differ in kind (`FileContentReference` vs. `TextValue`).

With those in place, `stdout text_equals "expected.txt"` and `stdout contents_equals "expected.txt"` would have contained the *same* literal with *different* meanings — a text value in one, a workspace file reference in the other. The same surface syntax having a different semantic type per expectation is ambiguity that every layer would have to re-explain: the language spec, the parser and AST, semantic diagnostics, generated docs, syntax highlighting, and AI-facing documentation.

Reportage is a young DSL with no meaningful migration burden yet, so this was the moment to decide whether to keep contextual typing or separate the surface syntaxes. This decision creates a durable language rule that affects the public syntax contract, which is why it is recorded as an ADR.

## Decision

Introduce a dedicated workspace path literal syntax and make each single-line literal kind map to exactly one semantic domain, independent of context:

````text
"..."      = StringLiteral        (text domain)
```...```  = HeredocLiteral       (text domain)
<"...">    = WorkspacePath        (case-workspace filesystem reference)
@"..."     = FixtureReference     (test-definition-side file reference; reserved for #92)
````

with the value categories:

```text
TextValue            = StringLiteral | HeredocLiteral
FileContentReference = WorkspacePath | FixtureReference
```

Specifically:

1. **Text value and filesystem reference are different semantic domains.** A `TextValue` is content to write or compare; a `WorkspacePath` is a reference resolved against the concrete case's workspace root; a `FixtureReference` is a reference to a static file near the `*.repor` source. They may hold the same string data internally, but there is **no implicit conversion** in any direction: `TextValue` ↔ `FileContentReference`, `WorkspacePath` ↔ `TextValue`, and `WorkspacePath` ↔ `FixtureReference` all require the author to write the other literal kind explicitly.

2. **Path-taking positions of existing constructs take a `WorkspacePath`:**

   ```text
   write <WorkspacePath> <TextValue>

   file <WorkspacePath> exists
   file <WorkspacePath> contains <TextValue>

   dir <WorkspacePath> exists
   dir <WorkspacePath> contains <existing-dir-contains-expected>
   ```

   `dir contains`'s expected side keeps its existing entry-name semantics (a plain string literal); only the `dir` subject moved to `WorkspacePath`.

3. **The `file` / `dir` checkpoint subject accepts `WorkspacePath` only — never `FixtureReference`.** Assertions observe the workspace produced by the code under test; fixtures supply expected content. This asymmetry is deliberate: a fixture reference is valid as a future `contents_equals` expected value (`FileContentReference`), but a checkpoint subject that pointed at the test definition's own files would invert what the assertion observes.

4. **Design constraint for future expectations:** `text_equals` takes a `TextValue`; `contents_equals` takes a `FileContentReference` as its expected value uniformly, regardless of subject (`file` / `stdout` / `stderr`). The uniformity means a reader who sees `contents_equals` always knows the expected side is `<"...">` or `@"..."` without consulting per-subject rules. The implementations, and their conformance cases, belong to #87 / #88 / #92.

5. **Literal kind mismatch is a semantic diagnostic, not a syntax error.** The grammar parses the kind-agnostic union `value_literal = workspace_path_literal | fixture_reference_literal | quoted_string` in every argument position; the required kind is checked during AST construction. A mismatch such as `file "out.txt" exists` produces `semantic.literal.kind_mismatch` with the expected kind, the actual kind, and a suggested replacement:

   ```text
   `file` checkpoint subject requires a WorkspacePath, but "out.txt" is a StringLiteral; use <"out.txt"> instead
   ```

   Literal kind mismatches are verified as semantic conformance cases (`tests/fixtures/syntax/invalid/`), and the valid forms as syntax conformance cases with AST snapshots.

6. **Escape and validation split.** The inner quoted content of `<"...">` reuses the string literal escape rules verbatim. Workspace path validation (non-empty, relative, no `.` / `..` segments) applies to the unescaped value, on the `WorkspacePath` side, exactly where it applied before: at AST construction for the `write` path, and in the semantic evaluator for `file` / `dir` subjects. This ADR does not define new traversal, symlink, absolute-path, dot-segment, or non-UTF-8 policies; existing policies carry over unchanged, and open questions remain TBD.

## Alternatives Considered

### Keep contextual typing (`"..."` for both text and paths)

Shorter to write, and no migration of existing scripts. Rejected because the same surface syntax would mean different types per expectation once `text_equals` / `contents_equals` / `@"..."` land, and because it would leave the category correspondence asymmetric:

````text
TextValue            = "..." | ```...```
FileContentReference = "..." | @"..."
````

With the dedicated syntax the correspondence is unambiguous:

````text
TextValue            = "..." | ```...```
FileContentReference = <"..."> | @"..."
````

Reportage is a new DSL that AI tooling cannot complete from prior knowledge, so "each surface form has exactly one meaning" is worth more than keystroke economy. Zero-base type visibility and semantic consistency were deliberately prioritized over migration cost and brevity.

### A keyword / constructor form such as `path("...")`

Explicit, but heavier at every use site, and it reads as a function call in a language that has no function-call syntax — inviting authors to expect other callables. `<"...">` is a lightweight bracket around the same quoted string, pairs visually with `@"..."`, and keeps the quoted content's escape rules obviously identical to a string literal's.

### Reject kind mismatches as plain syntax errors

Simpler grammar and parser. Rejected because a pest-level failure can only say "expected value_literal", which tells the author nothing about *which* literal kind the position wanted or how to fix it. Parsing the union and checking the kind at construction yields a diagnostic that names expected kind / actual kind / suggested replacement, matching the existing pattern where `semantic.workspace_path.*` and `semantic.expectation.empty_block` are parse-able semantic invalid cases. Actionability for language users was prioritized over implementation simplicity, and verifying these mismatches as semantic conformance cases pins that contract.

## Consequences

### Positive Consequences

- Surface syntax and semantic type correspond one-to-one; the spec, parser, AST, diagnostics, generated docs (which can now state each construct's type signature), syntax highlighting, and AI-facing docs all describe the same simple rule.
- `text_equals` vs. `contents_equals` expected values become visually distinguishable at the call site before those expectations even exist.
- Wrong-kind literals produce actionable diagnostics (`semantic.literal.kind_mismatch`) instead of opaque syntax errors.
- The VSCode grammar highlights `<"...">` and `@"..."` as distinct scopes, so the kinds are visible while editing.

### Negative Consequences

- Every existing path-taking script had to migrate from `file "p"` / `dir "p"` / `write "p"` to the `<"p">` form (done in this repo for examples, e2e scripts, fixtures, and docs; external early adopters must do the same).
- Paths are three characters heavier to write than plain strings.
- `@"..."` occupies grammar space for #92 before any position accepts it; until then every use is a kind mismatch.

### Neutral Consequences

- #87 / #88 / #92 must update their issue examples from `file "path"` to `file <"path">` before implementation.
- The grammar's `value_literal` union means new argument positions must explicitly choose a required kind; there is no default.
