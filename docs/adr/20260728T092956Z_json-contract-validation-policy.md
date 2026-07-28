# JSON Contract Validation Policy

- Status: Accepted
- Created: 2026-07-28T09:29:56Z

## Context

Reportage publishes two machine-readable JSON contracts: the `reportage run --format=json` stdout document ([`spec/output/json-report/schema.json`](../../spec/output/json-report/schema.json)) and the artifact run result manifest ([`spec/artifacts/run-result/schema.json`](../../spec/artifacts/run-result/schema.json)). Both are specified as JSON Schema Draft 2020-12 documents, and both are the contract external consumers write against.

Until now, CI enforced them by deserializing representative fixture output into typed Rust structs marked `#[serde(deny_unknown_fields)]`. [The superseded validation policy](20260707T050100Z_json-output-schema-and-validation-policy.md) and [the semantic specs ADR](20260630T000000Z_json-semantic-specs.md) both chose that approach to keep contract checking inside `cargo nextest`, without adding a Node.js or Python validator step to CI. That constraint has not changed and is not being revisited.

What has changed is the claim made about what the approach guarantees. Both ADRs describe typed deserialization as "schema validation", or as validation "equivalent" to what the JSON Schema states. It is not. Serde checks that a document's fields and types map onto a Rust shape; JSON Schema states considerably more than a Rust shape can:

- value constraints: `const`, `pattern`, `minimum`, `enum` beyond what a Rust enum's variant set happens to cover;
- combinator constraints: `oneOf`, `allOf`, and conditional `if` / `then` requirements between fields of the same object;
- numeric domain: JSON Schema `integer` is unbounded, while a Rust field is a fixed-width integer that rejects values the contract permits;
- coverage: a typed model that only covers fixture-exercised shapes says nothing about the rest of the contract.

Each of those gaps is a way for producer output to violate the published contract while every CI test passes. The gap is not hypothetical for a contract that already uses `const` for `schemaVersion`, `pattern` for diagnostic codes and evidence digests, `minimum` for counts, `oneOf` for diagnostic origins and expectation kinds, and `if` / `then` for the contents-comparison shapes.

A second, smaller problem is that representative fixtures are representative. They exist to show that realistic runs produce well-formed documents, not to show that each constraint bites. A constraint no fixture violates is never exercised, so it could be silently wrong in the schema and nothing would notice.

[The schema artifact generation ADR](20260727T151234Z_json-schema-artifact-generation.md) introduced a second file per contract: the metadata-bearing `schema.internal.json` maintainers edit, and the generated public `schema.json`. That raises a question this ADR has to answer as well: which of the two a producer document is validated against, and what relates the two validation results.

Related issue: [#192](https://github.com/tooppoo/reportage/issues/192).

## Decision

### Only the superseded ADR's validation policy is replaced

[`20260707T050100Z_json-output-schema-and-validation-policy.md`](20260707T050100Z_json-output-schema-and-validation-policy.md) is marked superseded by this ADR, but only its answer to "how does CI enforce the schema" changes. Its schema design decisions remain current project policy and are not restated here: the external contract's independence from `ExecutionReport`, camelCase field naming, `schemaVersion` and `additionalProperties`, document-local ids not being long-term stable identifiers, the representative fixture and snapshot policy, and the `location` / `origin` fallback.

The same applies to the validation paragraph in [the artifact run result canonical manifest ADR](20260708T130500Z_artifact-run-result-canonical-manifest.md), which carries an amendment note pointing here. Nothing else it decides changes.

### JSON Schema remains the authoritative specification

The schema documents under `spec/` state what the contracts are. No Rust type, test, or documentation page is a second definition of them.

Everything below follows from that: if the schema is authoritative, then CI must check producer output against the schema itself, and anything else CI checks is a different property that needs its own name.

### The `jsonschema` crate performs JSON Schema validation

Contract validation uses the [`jsonschema`](https://docs.rs/jsonschema/) crate, as a dev-dependency of `reportage-cli`, run inside `cargo nextest`.

This satisfies the constraint that motivated the superseded policy — no non-Rust step in CI — while removing the reason that policy accepted a weaker check. A Rust JSON Schema validator did not have to be traded against an external toolchain; the trade was only ever between an external validator and no validator.

The dependency version is pinned in `Cargo.toml` and fixed by `Cargo.lock`. This ADR deliberately records no specific version: which version is compatible is an implementation fact, not a design decision, and freezing one here would make an ordinary dependency bump look like a policy change.

### Draft 2020-12 is stated, not detected

Validation uses the crate's explicit Draft 2020-12 API rather than relying on automatic draft detection from `$schema`.

The schemas do declare `$schema`, and that declaration stays. Stating the draft in the harness as well means an edit to a schema's `$schema`, or a change in a future version's detection heuristics, cannot silently change the semantics a contract is evaluated under. A contract check should fail loudly when its own premises change, not evaluate a different contract.

### External resource resolution is disabled

The crate is depended on with `default-features = false`, which excludes its HTTP and file retrievers, and the test harness additionally installs a retriever that rejects every external reference.

Both are deliberate. Turning off the features expresses the policy; installing the rejecting retriever makes the policy hold regardless of which features a future dependency change happens to enable. Contract validation must produce the same result on a machine with no network access, and an external reference introduced by mistake must be a clear failure rather than a fetch that succeeds locally and fails in CI.

The initial contracts use fragment-only local `$ref` exclusively, and a test asserts that they continue to. [The normalization `$ref` resolver](https://github.com/tooppoo/reportage/issues/193) is a limited resolver for snapshot normalization and must not be reused here: reference semantics for validation belong to the validator.

### Validators are built once per schema artifact and reused

Each of the four schema artifacts — two contracts, internal source and public — compiles to one validator that every instance check reuses.

Compilation is the expensive half of validation, and these suites check many fixture documents against the same four schemas.

### Producer output is validated against both the internal source and the public schema

Every representative fixture's output is validated against its contract's internal source schema and its generated public schema, and the two must agree.

The public schema is what external consumers read, so a conformance claim is really about that file. The internal source schema is what maintainers edit, so validating it too keeps a defect visible in the artifact that has to be fixed rather than only in its generated output. Agreement between the two is the instance-level counterpart to the generation check: [issue #115](https://github.com/tooppoo/reportage/issues/115) guarantees the documents differ only by stripped metadata, and this guarantees that the stripped metadata was not load-bearing.

Full extensional equivalence of the two schemas is not proved. The generation check plus agreement over the representative fixtures and the schema feature cases is what is claimed.

### Typed Rust deserialization is consumer compatibility, not validation

The typed structs stay, under an accurate name: they are a *typed Rust consumer compatibility test*.

What they establish is that a Rust consumer in this repository can read real producer output, that a new field or variant does not silently break such a consumer, and that the Rust-side representation has the shape the code expects. They are kept because that is a real requirement with no other test, not because they check the contract.

What they do not establish is anything about the schema's value constraints, combinators, or conditional requirements, and the set of instances they accept is not the set the schema accepts. Test names, comments, and documentation must not describe them as JSON Schema validation.

Whether a contract's typed model covers the whole contract or only fixture-exercised shapes is now a consumer-side question. The artifact manifest keeps its full model for the reason [the canonical manifest ADR](20260708T130500Z_artifact-run-result-canonical-manifest.md) gave, and the stdout document keeps its fixture-exercised subset; neither choice affects contract coverage any more, because contract coverage is the validator's job.

### Domain invariants are separate tests again

Properties that JSON Schema cannot state stay in their own tests, with their own failure messages: `diagnosticRef` resolving to a diagnostic in the same document, a logical composition's children never carrying their own `diagnosticRef`, evidence files existing with the size and digest the manifest records, summary counts agreeing with the results they count, recorded source paths existing, and projection parity between the two contracts.

A schema violation means the producer broke the published contract. An invariant violation means the producer wrote a self-inconsistent document the contract was never able to rule out. Collapsing the two would make every failure ambiguous about which one happened.

The converse also applies: a conditional requirement the schema does state is enforced by validating against the schema, and must not be restated in Rust. Two hand-maintained copies of one rule give one of them no way to notice it has fallen behind.

### Schema feature tests are separate from producer fixtures

Each JSON Schema keyword the contracts rely on gets valid and invalid instances constructed directly in the test suite, independent of any fixture run.

Representative fixtures cannot serve this purpose. Making a producer emit a contract violation would mean adding test-only behavior to the runtime, and a keyword can only be shown to bite by feeding it something that must fail. The instances are therefore hand-built, as edits to a valid document so that an invalid case fails for the reason it names.

Which keywords need cases is derived from the schemas rather than listed by hand. Every keyword occurring in a schema position must either map to a covered feature or be recorded as structural, so adding a constraint to a contract without exercising it fails the suite.

One case per keyword is not always enough. A contract states the same constraint in many definitions independently — `additionalProperties: false` in more than twenty, the contents-comparison conditional in four — and producer fixtures emit only conforming instances, so a single definition quietly losing its constraint would go unnoticed. That is how the contents-comparison conditional came to be wrong in all four of its definitions while every check passed.

Coverage is therefore per site for the constraints that repeat. The case suite builds one base document instantiating every closed shape both contracts define, and generates from it: every object in that document must reject an undefined member, and every value must reject one of the wrong JSON type. A separate check fails when a contract gains a closed shape the base document does not build, so the generated coverage cannot fall behind the schemas. `oneOf` and the conditional are written out per site, four each.

`const`, `enum`, `pattern`, `minimum`, `required`, and `items` are covered once each. One definition dropping one of those is a gap this suite does not close.

This is also what covers the `jsonschema` crate's own Draft 2020-12 conformance for the keywords Reportage actually uses. Proving the crate's conformance in general is not Reportage's job; noticing that a keyword this repository depends on stopped being enforced is.

### `format` stays an annotation

No published schema uses `format` as a constraint, and validation does not enable format assertion.

Draft 2020-12 treats `format` as an annotation by default. Leaving it there means the schema does not impose requirements it does not visibly state. If a contract later needs `format` as an assertion, that is a deliberate change to make then, with the validator configured explicitly rather than by default.

### Contract validation runs before snapshot normalization

Raw producer output is validated before [snapshot normalization](20260708T125940Z_snapshot-normalization-policy.md) rewrites its volatile fields.

Normalization exists to make documents comparable, not to repair them. A snapshot recorded from a document that never satisfied the schema would pin a contract violation as expected output.

### What CI guarantees, and what it does not

CI guarantees, for both contracts:

- each schema artifact is a valid Draft 2020-12 document, and a malformed one fails before any instance is validated;
- every representative fixture's producer output conforms to both the internal source and the public schema, with all violations reported together and each locating itself by instance path, schema path, and evaluation path;
- each JSON Schema keyword the contracts rely on accepts and rejects the instances it is supposed to, and no keyword occurs in a schema without such cases;
- `additionalProperties: false` is exercised at every closed shape the contracts define, `oneOf` at each of its four sites, the contents-comparison conditional at each of the four definitions stating it, and `type` at every value the base document carries;
- typed Rust consumers can deserialize real producer output;
- the domain invariants listed above hold.

CI does not guarantee:

- that the set of instances the schema accepts equals the set the Rust types accept;
- that representative fixtures cover the whole space the schema describes;
- extensional equivalence of the internal source and public schemas over all instances;
- the `jsonschema` crate's conformance to the JSON Schema specification beyond the keywords tested here;
- anything about external, remote, or user-supplied schemas.

The semantic specifications contract ([`spec/language/semantics/`](../../spec/language/semantics/README.md)) and the references index contract ([`spec/output/references-index/`](../../spec/output/references-index/README.md)) are out of scope. Applying this policy to them is a separate decision.

## Alternatives Considered

### Keep typed Rust deserialization as the only contract check

Rejected. It is the status quo whose guarantee this ADR exists to correct. The objection was never that it checks nothing, but that what it checks was described as schema validation, which left every value constraint and combinator in the published schemas unenforced.

### Add an external JSON Schema validator (ajv, `jsonschema-cli`)

Rejected, for the reason the superseded ADRs gave: it requires a Node.js or Python step in the `cargo nextest` pipeline. With a Rust validator available, this alternative costs a toolchain and buys nothing over it.

### Generate Rust types from the schemas

Rejected. It would replace one incomplete correspondence with a more automated incomplete correspondence: a generator still has to project JSON Schema's value constraints and combinators onto Rust types that cannot express them, and it would make the typed consumer model a derived artifact just as it stops being a contract check at all. Developing or adopting a general schema-to-Rust generator is out of proportion to the problem.

### Validate only against the generated public schema

Rejected. It is the file consumers read, so validating it is necessary, but a failure would then point at generated output rather than at the internal source schema a maintainer has to edit. Validating both, and requiring agreement, costs one extra validator per contract.

### Derive schema feature coverage from producer fixtures

Rejected. Covering an invalid case would require the runtime to be able to emit an invalid document on demand, which means test-only behavior in production code paths. Constructing instances directly keeps that pressure off the runtime.

## Consequences

### Positive Consequences

- The published schemas are enforced as written, including the `const`, `pattern`, `minimum`, `oneOf`, `allOf`, and `if` / `then` constraints that no previous check reached.
- Each of the three checks — schema conformance, consumer compatibility, domain invariants — fails with a message that identifies which kind of thing broke.
- A validation failure reports every violation at once, located by instance path, schema path, and evaluation path.
- Schema feature tests make a constraint's enforcement visible even when no realistic run violates it, and detect a keyword regressing in the validator.
- Documentation and test names now describe what the tests actually establish.

### Negative Consequences

- CI gains a dev-dependency, and with it a dependency to keep current and a compile-time cost in the `reportage-cli` test profile.
- A contract change now touches the schema, the typed consumer model, the fixtures, the snapshots, and possibly the feature cases; the pieces are more clearly separated, but there are more of them.
- Feature cases are hand-maintained. The coverage check catches a keyword with no cases anywhere, and per-site generation catches one definition weakening for the four constraints it covers, but neither catches a case that still runs and has stopped being about what its description claims.
- Per-site coverage extends to four constraints. A `const`, `enum`, `pattern`, `minimum`, `required`, or `items` constraint removed from one definition still passes, as does a `type` removed from an optional property no instance in the base document carries.

### Neutral Consequences

- The typed structs are unchanged in shape; only their name, documentation, and the conditional assertions the schema now covers changed.
- `format` remains an annotation, deferring the assertion decision to whenever a contract first needs it.
- The semantic specifications and references index contracts continue under [the semantic specs ADR](20260630T000000Z_json-semantic-specs.md)'s typed-deserialization policy until a separate decision changes them.
