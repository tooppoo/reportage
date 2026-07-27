# JSON Schema Artifact Generation

- Status: Accepted
- Created: 2026-07-27T15:12:34Z

## Context

Reportage publishes JSON Schemas for two machine-readable contracts: the `reportage run --format=json` stdout document ([`spec/output/json-report/schema.json`](../../spec/output/json-report/schema.json)) and the artifact run result manifest ([`spec/artifacts/run-result/schema.json`](../../spec/artifacts/run-result/schema.json)). Both are referenced by documentation and by external consumers who want editor integration against a stable path.

[The snapshot normalization foundation](20260723T160117Z_json-schema-driven-snapshot-normalization-foundation.md) makes JSON Schema annotations the carrier for snapshot normalization policy, and issue #114 migrates both existing snapshot suites onto that mechanism. That requires the schema the snapshot harness reads to carry `x-reportage-snapshot` metadata, while the schema external consumers read must not: normalization policy is a repository testing concern and does not belong in a published contract.

Two schemas that must stay identical apart from that metadata cannot be maintained by hand. Fields, constraints, and descriptions drift apart the first time a contract change lands in only one of them.

This ADR records how the two artifacts relate, which one is edited, how the other is produced, and where the boundary sits against the issues that own normalization (#114), contract validation (#192), and static local `$ref` resolution (#193).

## Decision

### The metadata-bearing internal source schema is the only editing target

Each contract has one hand-edited file, `schema.internal.json`, holding the full contract shape plus its `x-reportage-snapshot` annotations. Every change to a contract is made there.

A single editing target is the only arrangement in which the two files cannot disagree. Any scheme where both are editable reintroduces the drift the split was meant to remove.

### The internal source schema is itself the internal artifact

The snapshot harness reads `schema.internal.json` directly. No separate generated internal copy exists.

A generated internal copy would be byte-identical to its source, so it would add a build step and a staleness failure mode without adding information. The harness gains nothing from reading a duplicate.

### The existing `schema.json` path stays, as the generated public artifact

Generation writes to the path that already exists rather than introducing a new public path such as `schema.public.json`.

Documentation, references, and external consumers already point at `schema.json`. Moving the public artifact would break every one of those references to express a change that does not alter the contract at all.

The corollary is that `schema.json` must never be hand-edited. Both spec directory READMEs state this, and `just schema-artifacts-check` fails when it drifts from what the internal source generates.

### Both files are committed

`schema.internal.json` and the generated `schema.json` are tracked in git.

Committing the generated file keeps the public schema fetchable from GitHub at a stable path, makes each contract change reviewable as a diff in the pull request that causes it, and lets documentation reference the public schema without assuming anyone has run a generator. CI verifies freshness instead of producing the artifact, so a stale commit is a failing check rather than a silently different published contract.

### `schema.internal.json` is not an external compatibility contract

The internal source schema is visible in a public repository, but it is repository tooling input. Its annotations, and its existence at that path, may change whenever the repository's own tooling needs change.

The stable path for external consumers is the generated public `schema.json`.

### Only `x-reportage-snapshot` is stripped

Generation removes exactly one member name, not the `x-reportage-*` prefix as a class.

A prefix rule would silently strip a future Reportage extension that external consumers should see. Extension keywords are a normal way to publish additional contract information, and being Reportage-specific is not a reason to withhold one. Each new extension is a deliberate decision about whether it belongs in the public artifact, and the allowlist makes that decision explicit rather than implied by a naming convention.

### Stripping is a structural transformation over an allowlisted set of locations

The generator is not schema-aware. It walks every object in the parsed document, including objects inside arrays, deletes members whose key is exactly `x-reportage-snapshot`, and changes nothing else. Objects left empty by a deletion are kept.

Defining the transformation structurally rather than through JSON Schema semantics keeps the tool small and its output predictable, and avoids taking on the JSON Schema evaluation model that [the normalization foundation](20260723T160117Z_json-schema-driven-snapshot-normalization-foundation.md) already scopes down for the normalizer. It has one consequence: `x-reportage-snapshot` becomes a reserved object member name across the whole internal source schema document. It cannot be used as an instance property name, a `$defs` definition name, or a member inside literal data such as `const`, `default`, or `examples`.

To keep that reservation from turning into an invisible trap, the generator holds an allowlist of the JSON Pointers at which the annotation may appear. An occurrence anywhere else fails generation and the check, and so does the absence of an allowlisted occurrence. A dropped annotation would otherwise weaken snapshot normalization silently: the suite would keep passing while a volatile value stopped being replaced.

### Object member order follows the source

Generated output preserves the internal source schema's member order at every level and never sorts keys.

The two files are reviewed as a pair. Source order keeps them diffable against each other and keeps a contract review free of reordering noise unrelated to the change. This differs deliberately from the recursive key sorting [the normalization foundation](20260723T160117Z_json-schema-driven-snapshot-normalization-foundation.md) specifies for formatted snapshots: that rule exists to make snapshot comparison independent of producer ordering, which is not a concern for a file whose ordering a human chose.

Order preservation constrains the implementation, because `serde_json::Value` sorts object members. The generator therefore parses into its own order-preserving representation rather than enabling the `serde_json/preserve_order` feature, which would unify onto every workspace crate and change the reportage CLI's own JSON output ordering.

The rest of the output format is fixed: two-space indentation, LF, exactly one trailing newline, UTF-8, array order preserved, and numbers serialized by `serde_json`.

### Bundling is not `$ref` elimination, and local `$ref` is preserved

The generator does not inline, dereference, or otherwise remove `$ref`.

Both schemas use `$defs` and fragment-only local `$ref` to express reusable structures, and both keep them. Inlining would duplicate every shared definition at each use site, inflate the public schema, and destroy the structure that makes the contract readable. Per [the normalization foundation](20260723T160117Z_json-schema-driven-snapshot-normalization-foundation.md), the snapshot normalizer resolves those references through the resolver tracked by #193; the artifact generator has no reason to resolve anything.

### External, remote, anchor, and dynamic references are out of scope

The source schemas use no external file references, remote URL references, `$anchor`, `$dynamicRef`, or `$dynamicAnchor`, and this ADR does not add support for bundling them.

Bundling a remote reference means fetching it, rebasing `$id`, and embedding it as a resource — a substantially different tool with network behavior and cache semantics. Nothing in the current contracts needs it.

### The public `$id` is kept on both files

The internal source schema keeps the same `$id` as the public schema it generates.

They are the same contract at different processing stages, and nothing registers both in one resolver registry at the same time, which is the situation a distinct identity would guard against. If internal-specific resource identity becomes necessary, `$id` rewriting during generation is a separate decision.

### The implementation is a repository tool, not a general schema tool

Generation lives in a non-published workspace crate, `crates/xtask`, invoked through `just schema-artifacts-gen` and `just schema-artifacts-check`, with the latter wired into `just check`.

Keeping it out of the shipped crates means the reportage CLI does not grow a dependency, a binary, or a public API for a repository maintenance concern. Keeping its scope at "strip one annotation deterministically" means it is not a JSON Schema compiler, optimizer, or validator; contract validation is tracked separately by #192.

## Alternatives Considered

### Hand-maintain both schemas

Rejected. Two files that must stay identical apart from annotations drift on the first contract change that lands in only one of them, and nothing detects the drift until a consumer hits it.

### Generate the public schema in CI instead of committing it

Rejected. External consumers could no longer fetch a stable path from GitHub, contract changes would not appear as reviewable diffs, and documentation would have to assume a build step. Committing the artifact and checking its freshness gives review visibility and a stale-artifact failure with none of that cost.

### Keep `schema.json` as the edited source and generate the internal schema

Rejected. The annotated document is the superset, so the generator would have to merge annotations in from a side file keyed by instance path. That reintroduces exactly the path-repetition and drift that [the normalization foundation](20260723T160117Z_json-schema-driven-snapshot-normalization-foundation.md) chose annotations to avoid.

### Publish the annotated schema and let consumers ignore the extension

Rejected. JSON Schema tolerates unknown keywords, so this would work mechanically. It would still publish an internal testing policy as part of an external contract, invite consumers to depend on it, and make every change to snapshot placeholders a visible change to the published schema.

### Strip every `x-reportage-*` member

Rejected. The prefix says who defined a keyword, not who should see it. A future Reportage extension that is useful to external consumers would be removed by default, and the removal would be invisible because no rule would name it.

### Treat the allowlist as a permitted subset rather than an exact set

Rejected. Permitting a missing annotation makes a deleted placeholder a silent normalization regression: the snapshot suite keeps passing while a volatile value stops being replaced. Requiring the exact set turns that into a failing check.

### Sort object keys in the generated public schema

Rejected. Sorting would make the public schema unreviewable against its own source, since every diff would mix contract changes with reordering. Deterministic output does not require sorting when the input order is itself deterministic and committed.

### Implement generation as a shell script over `jq`

Rejected. Preserving member order, comparing bytes, and classifying failures with paths and pointers is more than a pipeline expresses clearly, and the rules deserve tests. A small Rust crate makes the contract testable in the same suite as everything else.

### Add generation to the reportage CLI

Rejected. It is a repository maintenance task with no user-facing value, and it would put a schema transformation tool inside the published binary.

## Consequences

### Positive Consequences

- A contract change is made in one file, and the public schema follows mechanically.
- External consumers keep the path they already use, and its content is unchanged apart from formatting.
- A stale or hand-edited public schema fails `just check` rather than reaching a consumer.
- Snapshot normalization policy lives beside the fields it applies to without leaking into the published contract.
- A dropped or relocated annotation is a loud failure instead of a silent normalization regression.
- Generated and source schemas diff cleanly against each other, so contract review sees only the contract change.

### Negative Consequences

- `x-reportage-snapshot` is reserved as an object member name throughout the internal source schemas, including inside literal data such as `const` and `examples`.
- Adding or moving an annotation requires updating the allowlist in the same change.
- The repository carries an order-preserving JSON representation that duplicates a small part of what `serde_json/preserve_order` would provide, because enabling that feature would alter the reportage CLI's own output ordering.
- Contributors must run `just schema-artifacts-gen` after editing a contract, and commit its output.
- The first generation reformats both public schemas, producing a large formatting-only diff.

### Neutral Consequences

- Whether the public schemas are valid JSON Schema, and whether they agree with the typed Rust models, remains outside this decision and is tracked by #192.
- Resolving the local `$ref` references both schemas keep remains the normalizer's concern, tracked by #193.
- Adding a third contract means adding an entry to the generator's contract table; nothing else about the arrangement changes.
