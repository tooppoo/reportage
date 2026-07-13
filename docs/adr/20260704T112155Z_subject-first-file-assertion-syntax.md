
# Adopt Subject-First File Assertion Syntax

- Status: Accepted
- Created: 2026-07-04T11:21:55Z

## Context

#13 established representative CLI-level self-testing for reportage. The remaining high-priority self-testing gap was artifact / evidence output: reportage's own self-tests could assert process-level evidence (`exit`, `stdout`, `stderr`) but had no way to assert that a file was created, or that a created file contained expected content, without delegating to shell helpers such as `test`, `grep`, or `cat`. That would make filesystem evidence a second-class citizen in the assertion model.

`model.rs` already defined `Expectation::File(FileExpectation)` and `FileMatcher` as forward-looking placeholders (see #13/#28-era work), but no grammar rule or parser code produced them, and the evaluator did not evaluate them.

This issue (#24) adds the first real file assertion syntax and evaluator support: `exists` and `contains`. It intentionally does not add a `dir` subject (deferred to #66), glob assertions, snapshot/equality assertions, or logical composition (`not` / `all` / `any`, deferred to #25).

Two syntax shapes were considered for the new expectation:

```reportage
file "path/to/file.txt" exists
file "path/to/file.txt" contains "expected text"
```

versus an expectation-first shape:

```reportage
file exists "path/to/file.txt"
file contains "path/to/file.txt" "expected text"
```

## Decision

reportage v0 adopts the **subject-first** form:

```text
file-expectation =
  "file" string-literal "exists"
  | "file" string-literal "contains" string-literal
```

`file "<path>"` is the common subject. `exists` and `contains` are predicates evaluated against that subject. The expectation-first form (`file <predicate> <path> <...args>`) is **not** adopted and is rejected as a plain syntax error (`parse.syntax`) in v0.

The choice is driven by implementation and extensibility properties, not by which form reads more naturally as English:

- **Shared subject construction.** `file "<path>"` can be validated once (path policy: relative only, no `.` / `..` segments) regardless of which predicate follows. Centralizing this in `semantic::validate_file_path` means every predicate — `exists`, `contains`, and future predicates — shares the same rejection rule instead of re-implementing it per predicate signature.
- **AST stability under predicate growth.** Adding a future predicate (e.g. `matches`, `size`, `type`) only adds a new predicate variant under the same subject; it does not change how the path argument is located or parsed. Under the expectation-first form, each new predicate would need its own argument-order convention, and argument position would carry predicate-specific meaning.
- **Evaluator symmetry with observation acquisition.** The evaluator reads filesystem evidence once per subject (`std::fs::metadata`, then `std::fs::read` for `contains`) and dispatches on the predicate. A subject-first AST mirrors that: `FileExpectation { path, matcher }` separates "what to observe" from "what to check," which keeps `evaluate_file_expectation` a straightforward match over the predicate.
- **Room for future logical composition.** #25 (`not` / `all` / `any`) will compose expectation *results*, not file predicates. Keeping predicates simple and subject-scoped avoids mixing logical operators into the file predicate grammar itself; composition stays a layer above individual expectations.

`file` is scoped to regular files only in v0. `dir` is deliberately deferred to #66 as a separate subject with its own predicate set (`dir "<path>" exists`, `dir "<path>" contains "<name>"`), rather than folding directory semantics into `file`. A single `file` subject that sometimes means "regular file" and sometimes means "directory" depending on the predicate would weaken the "subject declares what kind of evidence is being observed" property that motivated the subject-first form in the first place.

## Alternatives Considered

### Expectation-first form: `file exists "<path>"` / `file contains "<path>" "<text>"`

This reads closer to the `stdout contains "<text>"` / `stderr contains "<text>"` shape already in v0, and superficially looks more "shell-like."

Rejected because:

- path policy validation would need to be duplicated (or threaded through a shared helper called from two different argument positions) instead of living in one subject constructor;
- each predicate's argument order becomes predicate-specific instead of uniform ("path first, then predicate-specific arguments" only holds for `contains`; `exists` has no further arguments), which is a smaller but real AST/parser inconsistency;
- future predicates such as `size <path> <op> <n>` or `type <path> <kind>` would each need to independently place the path argument, rather than inheriting subject placement for free.

### Folding `dir` semantics into `file`

Considered adding a `kind` discriminator (`file` vs `directory`) on the same subject rather than introducing a separate `dir` subject in a later issue.

Rejected because it conflates two different evidence shapes (regular-file content/existence vs. directory-entry existence) under one subject name, and would force this issue to also define directory listing / containment semantics that are out of scope here. A separate `dir` subject (#66) keeps each subject's predicate set aligned with one evidence shape.

### Treating path policy violations as parse errors

Considered rejecting absolute paths and `.` / `..` segments directly in the parser (as `parse.*` diagnostics), matching how `config.rs` validates `tests.path` entries at config-parse time.

Rejected for file assertions specifically: the script and expectation shape are syntactically valid; only the *path value* violates a policy the evaluator must reject before evidence comparison, which is the definition of a semantic error, not a parse error, per [`docs/semantic-diagnostics.md`](../reference/semantic-diagnostics.md) and [20260702T133734Z_semantic-and-assertion-diagnostic-model.md](20260702T133734Z_semantic-and-assertion-diagnostic-model.md). Path policy validation therefore lives in `reportage_core::semantic`, runs per assertion block before any expectation in that block is evaluated, and produces `semantic.file_path.absolute` / `semantic.file_path.dot_segment` diagnostics distinct from `parse.syntax`.

## Consequences

### Positive Consequences

- File assertion path policy is defined once (`semantic::validate_file_path`) and reused by every predicate.
- Adding a future file predicate does not require re-deciding argument order.
- `file` and `dir` (#66) can evolve independently without one subject's predicates leaking into the other's evidence model.
- Logical composition (#25) can be layered on top of expectation results without needing to special-case file predicates.

### Negative Consequences

- The subject-first form is a departure from the `stdout contains` / `stderr contains` argument shape already in v0, so the grammar is not fully uniform across expectation kinds. This is an accepted, scoped inconsistency rather than an oversight.
- `file "<path>" exists` reads slightly less like a shell command than `file exists "<path>"` would, which is an explicit trade against v0's general "shell-like where reasonable" bias for the *action* side of the language. This trade-off is intentional: `docs/philosophy.md`'s shell-like readability goal is scoped to actions, not to the assertion/expectation grammar, where extensibility and AST stability are prioritized instead (see the issue's own framing).

### Neutral Consequences

- `FileMatcher::NotExists` and `FileMatcher::Matches` remain defined in `model.rs` as forward-looking placeholders but are not produced by the v0 parser or evaluated by the v0 evaluator; introducing them is left to a future issue.
- `not` / `all` / `any` logical composition (#25) and `dir` (#66) are explicitly out of scope here and are expected to build on top of this decision rather than revise it.
