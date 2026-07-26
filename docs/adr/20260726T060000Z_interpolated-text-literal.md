# Interpolated text literals

- Status: Accepted
- Created: 2026-07-26T06:00:00Z

## Context

[Case-local immutable runtime evidence bindings](20260725T180000Z_case-local-runtime-evidence-bindings.md) made a whole captured value referenceable as `&name`.
Combining a captured value with surrounding literal text still required writing a placeholder and rewriting the file with `sed`, or collapsing the comparison into a shell test whose only structured evidence is an exit status.

Reportage also has to keep raw literals predictable.
Scripts write shell scripts, configuration files, and other template engines' syntax as literal content, so a construct that expands text on sight would break those payloads.

## Decision

Reportage supports interpolated text literals: an `&`-prefixed string literal or heredoc literal whose `&{name}` markers reference case-local bindings.

Interpolation is a typed text value expression, not a matcher modifier.
`TextValueExpression` keeps raw text, a direct binding reference, and an interpolated literal apart at the source level, and runtime consumers resolve it through one fallible, context-taking resolver rather than branching on the variant.

Raw literals stay non-interpolating.
`&{name}` inside `"..."` or a plain heredoc is literal text, the raw string literal escape set is unchanged, and no `template` keyword is introduced.

Inside an interpolated literal `&` is reserved, and a literal one is written `\&`.
Escapes resolve before interpolation markers, left to right, in a single pass; an interpolated string extends the raw escape set with `\&`, while an interpolated heredoc adds only `\\` and `\&` and keeps every other backslash literal.

A binding's exact UTF-8 text is substituted, with no escaping, quoting, trimming, indentation, newline normalization, or recursive interpolation.
An interpolated heredoc is dedented first, and the binding value is then inserted into the dedented text unchanged.

The grammar exposes two categories, `inline_text_value_expression` and `heredoc_text_value_expression`.
A `TextValue` consumer references a category and declares one surface capability — inline only, or inline and heredoc — instead of enumerating the forms itself.
Version zero applies this to `write` content and to the `contains` and `text_equals` matchers of `file`, `stdout`, and `stderr`.

Interpolation is confined to the `TextValue` domain.
`WorkspacePath`, `FixtureReference`, `FileContentsReference`, `contents_equals`, exit codes, and action command sources are not text value positions, so an interpolated literal there is a plain syntax error.

## Alternatives Considered

### Implicit interpolation in raw literals

Expanding `&{name}` everywhere would silently rewrite shell scripts, configuration files, and other template engines' payloads.
An explicit `&` prefix keeps a raw literal's content exactly what the author typed.

### A `template` keyword

A keyword would introduce a second text construct that every `TextValue` position has to accept separately, and would suggest a general template engine with conditions, loops, and filters.
The `&` prefix reuses the binding sigil already established for `&name` and stays a literal form.

### Interpolation as a matcher modifier

Attaching interpolation to `contains` and `text_equals` would tie it to assertion syntax, leaving `write` — the position that motivated the feature — without it, and would require a new modifier for every future consumer.

### Resolving interpolated literals at parse time

Evaluating to a `TextValue` during parsing would erase which form the author wrote, so diagnostics, AST snapshots, and provenance could no longer distinguish an interpolated literal from raw text that happens to look like one.

### `AsRef` / `Deref` / `Into` for resolution

Resolution needs the binding environment and must be able to report which binding was unavailable, so an infallible context-free conversion cannot express it.
A trait that takes a context and returns a `Result` keeps the failure visible at every call site.

### Interpolating into action command sources

Inserting a captured value into command source would put Reportage in charge of shell quoting and injection.
Passing bindings to actions is left to explicit environment projection.

### Defining escape behavior by backslash parity

A parity rule would have to be restated for each literal form and re-derived by every reader.
Sequential left-to-right evaluation produces the same results and is stated once.

### Reproducing a resolved interpolation result in output

The resolved value mixes script text with captured process output, which may contain credentials or arbitrarily large data.
Human output and the artifact manifest describe an interpolated expected value by form, source position, and referenced binding names instead.

## Consequences

### Positive Consequences

- A runtime-captured value can be embedded in expected text and in written files without a rewrite step.
- A future `TextValue` consumer gains raw, direct binding, and interpolated forms by referencing the shared grammar category and resolver, with no per-form branch.
- Diagnostics distinguish a malformed marker, an unterminated reference, an empty binding name, and an invalid identifier, each at its own source span.

### Negative Consequences

- Interpolated literals carry an escape set that differs from raw literals, so `&` now needs escaping in one context and not the other.
- An interpolated heredoc's markers are recognized after dedenting, so the parser has to carry a mapping from the dedented text back to the original source.
- Failure output for an interpolated expected value is less direct than for a raw literal, because the resolved value is deliberately not printed.

### Neutral Consequences

- An interpolated literal that references no binding is redundant but legal, so it remains usable where no binding scope exists.
- `before_each` uses the same text value expression model with a statically empty binding scope, rather than a raw-text-only input type.
