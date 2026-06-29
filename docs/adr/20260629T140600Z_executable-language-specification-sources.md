# Use Executable and Machine-Readable Sources for Language Specification

- Status: Accepted
- Created: 2026-06-29T14:06:00Z

## Context

Reportage is a new DSL. It does not have a large public corpus, established conventions, or broad model familiarity. That creates a practical risk that AI agents and human contributors will infer unsupported syntax or semantics from adjacent tools such as POSIX shell scripts, Go `testscript`, Cucumber, Playwright, RSpec, or other E2E testing tools.

This risk is especially important because Reportage is intended to remain small, explicit, shell-like, and coverage-aware. The language should not accidentally expand because examples, documentation, parser behavior, or AI-generated scripts drift apart.

The project needs a way to keep the following artifacts mechanically aligned:

- syntax definition
- parser behavior
- syntax documentation
- valid and invalid syntax examples
- semantic rules
- semantic conformance tests
- semantic documentation
- AI-facing authoring references

A hand-written EBNF document would be useful for human readers, but it would also introduce another source of truth unless it directly generated the parser and documentation. In the Rust ecosystem, EBNF can be used as a descriptive notation, but the project does not currently have a lightweight, standard path from EBNF to Rust parser generation, diagnostics, documentation generation, and conformance testing.

Pest grammar is a better fit for the initial syntax layer because it can act as an executable grammar for Rust parser generation while also constraining the language to remain small and line-oriented.

Semantic behavior cannot be fully defined by grammar. For example, grammar can parse `stdout contains "ok"`, but it cannot by itself prove that `contains` means substring matching, that an assertion is evaluated against a particular checkpoint, or that a file assertion uses a particular path policy.

Therefore, semantic rules need their own machine-readable source of truth and executable conformance cases.

Related issues:

- [#26](https://github.com/tooppoo/reportage/issues/26)
- [#27](https://github.com/tooppoo/reportage/issues/27)
- [#28](https://github.com/tooppoo/reportage/issues/28)
- [#29](https://github.com/tooppoo/reportage/issues/29)
- [#30](https://github.com/tooppoo/reportage/issues/30)
- [#31](https://github.com/tooppoo/reportage/issues/31)

## Decision

Reportage must manage its language specification through executable and machine-readable sources of truth.

### Syntax

`reportage.pest` is the normative syntax source for v0.

The parser must be generated from, or directly driven by, the pest grammar. Syntax not represented in the pest grammar must not be treated as part of the Reportage language specification.

Human-readable syntax documentation must be generated from `reportage.pest`. A hand-written EBNF file must not be introduced as a separate normative syntax source for v0.

Generated syntax documentation must be checked in CI. If the pest grammar changes and generated syntax documentation is stale, CI must fail.

### Syntax conformance

The project must maintain valid and invalid syntax fixtures.

Valid fixtures must parse successfully. Invalid fixtures must be rejected. Where the diagnostic model supports stable diagnostic codes, invalid fixtures should assert those codes.

AST snapshots may be added when the AST shape is stable enough. If AST snapshots are not yet practical, that deferral should be explicit.

### Semantics

Semantic rules must be represented as machine-readable JSON specifications for v0.

The semantic spec JSON must be validated by JSON Schema or equivalent typed loading. Unknown fields should be rejected. Required fields must be enforced.

Each semantic rule should include executable conformance cases that can be run against the semantic evaluator without executing external commands.

The semantic specification JSON should include structured normative data and conformance cases. It must not include free-form explanatory prose in v0.

The following fields are intentionally out of scope for v0 semantic specs:

- `notes`
- `explanation`
- `aiNote`
- `rationale`
- `status: tbd`

Deferred questions and open design topics must remain in `docs/TBD.md`, not inside semantic spec JSON.

### Semantic documentation

`docs/language/semantics.md` must be generated from semantic spec JSON.

Generated semantic documentation should include rule identifiers, syntax forms, categories, structured normative fields, conformance cases, expected results, and expected diagnostic codes where applicable.

`semantics.md` must not be a hand-written normative source for v0.

### Format boundary

The project uses different formats according to the primary consumer:

- human-authored Reportage scripts: Reportage DSL
- human-authored configuration: KDL
- parser-readable syntax definition: pest grammar
- machine-readable semantic specification: JSON
- public language documentation: generated Markdown

KDL remains appropriate for user-authored configuration such as `reportage.kdl`, where direct readability and editability matter. JSON is preferred for semantic specification data because the primary consumers are tools, validators, test runners, and documentation generators.

## Alternatives Considered

### Maintain hand-written EBNF as the normative syntax specification

Rejected for v0.

EBNF is useful as a human-readable notation, but it would create a second source of truth unless the parser, documentation, and conformance tests were generated from it. Building or adopting that full EBNF-to-parser-and-docs pipeline would add integration cost before the Reportage grammar is large enough to justify it.

### Maintain hand-written syntax documentation alongside the parser

Rejected.

This would be simple initially, but it would allow syntax documentation to drift from parser behavior. That is exactly the failure mode this decision is intended to avoid.

### Use KDL for semantic specifications

Rejected for v0 semantic specs.

KDL is appropriate for human-authored configuration, but semantic specifications are primarily machine-consumed. JSON has stronger tooling for schema validation, typed loading through Serde, generated docs, and CI integration.

### Put explanatory notes in semantic spec files

Deferred.

Human-authored explanations may become useful later, especially for AI authoring guidance or edge-case documentation. For v0, they are excluded to avoid creating unchecked natural-language specification fragments. If explanatory fields are introduced later, they must be clearly separated from mechanically checked normative fields unless they are themselves generated from structured data.

### Track TBD semantic items inside semantic spec JSON

Rejected.

Open or deferred decisions should remain in `docs/TBD.md`. Duplicating TBD state inside semantic spec JSON would create unnecessary coordination overhead and another possible drift point.

### Add Tree-sitter grammar now

Deferred.

Tree-sitter may be useful for editor support later, but adding it now would introduce another grammar that must be kept in sync with the runtime parser grammar. Runtime syntax correctness takes priority in v0.

## Consequences

### Positive Consequences

- Parser behavior and syntax specification are tied to the same pest grammar.
- Syntax documentation can be proven to derive from the parser grammar.
- Valid and invalid syntax fixtures make accepted and rejected forms explicit.
- Semantic rules become testable through JSON conformance cases.
- Semantic documentation can be generated from the same data used by tests.
- AI-facing references can later be generated from the same syntax and semantic sources.
- The format boundary is explicit: KDL for human-authored configuration, JSON for machine-consumed specification data.
- The language is less likely to grow accidentally through undocumented parser behavior or AI-inferred syntax.

### Negative Consequences

- The project must maintain documentation generation and stale-docs CI checks.
- JSON semantic specs are more verbose than KDL or Markdown prose.
- Contributors must understand that generated documentation should not be edited directly.
- Some semantic explanations will be minimal in v0 because free-form explanatory prose is intentionally excluded.
- Pest-specific syntax becomes part of the implementation architecture for v0.

### Neutral Consequences

- The language specification is split by responsibility rather than stored in one file: pest grammar for syntax, JSON for semantics, Markdown for generated public docs.
- Generated documentation may be less polished initially than hand-written prose, but it will be mechanically traceable.
- Future explanatory prose, AI authoring hints, or Tree-sitter grammar can be added later through separate decisions if the need becomes concrete.

## Non-Goals

This decision does not attempt to prove all Reportage behavior from natural language documentation.

This decision does not define the full v0 syntax or all v0 semantics. It defines how syntax and semantics should be represented and kept mechanically consistent.

This decision does not require Tree-sitter support.

This decision does not require human-authored explanatory semantic prose for v0.

## Summary

Reportage will use executable and machine-readable language specification sources. Syntax is defined by pest grammar. Semantics are defined by JSON specs with executable conformance cases. Public language documentation is generated from those sources. CI must verify generated documentation freshness and conformance behavior so the language, parser, and docs do not silently drift apart.
