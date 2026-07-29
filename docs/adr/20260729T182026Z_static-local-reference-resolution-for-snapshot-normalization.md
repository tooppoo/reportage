# Static Local Reference Resolution for Snapshot Normalization

- Status: Accepted
- Created: 2026-07-29T18:20:26Z

## Context

Snapshot normalization carries its policy in the schema, as `x-reportage-snapshot` annotations placed beside the fields they stabilize. [The normalization foundation ADR](20260723T160117Z_json-schema-driven-snapshot-normalization-foundation.md) decided that schema preparation compiles those annotations into a normalization plan, that a limited static local `$ref` capability is part of that foundation, and that the resolver itself would be decided separately. This ADR is that decision (issue #193).

References are not optional for this design. Reportage's contract schemas express envelope and reusable structures through `$defs`, and both annotations they currently carry are only reachable through a reference: `tool.version` is defined in `$defs/Tool` in both [`spec/output/json-report/schema.internal.json`](../../spec/output/json-report/schema.internal.json) and [`spec/artifacts/run-result/schema.internal.json`](../../spec/artifacts/run-result/schema.internal.json). A normalizer that does not follow `$ref` cannot reach the fields the annotation mechanism exists for.

The difficulty is the other direction. Resolving references in full generality means base-URI rebasing through `$id`, embedded resource registries, plain-name anchors, dynamic scope, external retrieval, and URI syntax rules. Implementing that inside a snapshot harness would build a second JSON Schema implementation next to the validator the contract suites already use, and every part of it would be a way to normalize the wrong field without anything reporting it. The decision needed is therefore not how to resolve references, but exactly how little to resolve, and where the boundary of what is inspected lies.

## Decision

### Support exactly one reference spelling

A reference the normalizer follows must be the string `#/$defs/<token>`, where `<token>` is a single RFC 6901 reference token naming a direct member of the document root's `$defs`.

A `$ref` reached by normalization traversal is checked in this order, and each step narrows what the next may assume:

1. the value is a string;
2. it begins with the literal prefix `#/$defs/`;
3. the remainder contains no further `/`, so it is one reference token;
4. the remainder contains no `%`;
5. every `~` in it begins a `~0` or `~1` escape;
6. the token is decoded, `~1` to `/` and `~0` to `~`;
7. the document root has a `$defs` member and it is an object;
8. the decoded token names a member of that object; and
9. that member is an object or a boolean.

Step 2 is a literal prefix test, not URI parsing. A URI parser would accept spellings whose meaning depends on machinery this profile does not implement — percent-decoding, relative resolution, base URIs — so the accepted form must be recognizable by reading it. `%` is rejected outright for the same reason: a percent-encoded reference that is never decoded would otherwise resolve to a definition whose name happens to contain a literal `%`, which is not what the author wrote.

Step 9 is what keeps a reference from targeting arbitrary JSON. Without it, `#/$defs/Tool` could name a `properties` map or an annotation object, which the traversal would then walk as though its members were schema keywords.

Definition names are not restricted beyond this. A name may be empty, contain non-ASCII characters, or contain `/` and `~` written as `~1` and `~0`.

### Treat a boolean target as a terminal preserve

`true` and `false` are schemas, so they are valid targets. Neither can carry annotations or subschemas, so reaching one produces no instruction and the instance positions it describes keep their observed values. Whether any instance can satisfy `false` is a validation question and belongs to contract validation (issue #192); the normalizer does not re-check it.

### Check a reference target's type, but skip a keyword position that holds no schema

The type check above applies to what a reference resolves to. A value reached by descending into `properties` or `items` that is neither an object nor a boolean is skipped instead: it produces no instruction and no error.

The asymmetry is deliberate. A reference is the mechanism by which a definition is named, so what a reference names must be checked or the traversal would walk arbitrary JSON as though it were a schema. A keyword holding a non-schema is simply a malformed schema document, which contract validation decides; restating that verdict here would make normalization a second, partial validity check that has to agree with the first.

### Inspect only what normalization traversal reaches

Reference resolution, cycle detection, and the compatibility rules apply to schema nodes normalization traversal actually reaches: the root schema, object `properties`, homogeneous array `items`, and the targets of supported references.

An unsupported reference, a cycle, a nested `$id`, or a dynamic keyword that exists only inside a keyword the normalizer does not enter — `oneOf`, `patternProperties`, schema-form `additionalProperties` — or only in a `$defs` entry no supported reference reaches, must not fail schema preparation.

This is deliberately not a statement that the document is a valid JSON Schema. Document validity is decided by [the JSON contract validation policy](20260728T092956Z_json-contract-validation-policy.md) and is checked against the Draft 2020-12 meta-schema. Normalization answers only whether the part of the schema it has to interpret is interpretable.

### Allow `$id` on the document root and reject it below

References resolve against the document root. A nested `$id` starts a new resource whose base URI subordinate references resolve against, so following a reference from inside such a subtree against the document root would silently reach the wrong schema. A nested `$id` reached by traversal is therefore a schema preparation error, while the root `$id` both contract schemas declare is accepted.

The root allowance is about where a document declares its own identity. It is not an exemption from the sibling rule below: a root schema object holding both `$ref` and `$id` is rejected as a sibling form.

### Require a reference object to hold `$ref` alone

A schema object reached by traversal that holds `$ref` must hold no other member, including `description`, `$comment`, `$id`, `x-reportage-snapshot`, and unknown extension keywords.

In Draft 2020-12 a sibling is evaluated alongside the referenced schema. Ignoring siblings would silently drop them, and honoring them would require the applicator semantics this profile does not implement. Annotations therefore belong in the referenced definition, never beside the reference. This is a restriction of the normalization profile, not a claim that such a schema is invalid.

### Detect cycles in the collector, from the active expansion stack

The collector holds the references currently being expanded. A reference is a cycle when its resolved target is already on that stack; identity is the decoded JSON Pointer of the resolved target relative to the document root.

Reaching a definition that is not currently being expanded is reuse, not a cycle. The same definition may be referenced from several properties, from several array paths, and repeatedly down one chain. This is what lets one definition produce one instruction per instance location.

A cycle is an error rather than something to unroll, because the plan describes instance locations statically: a recursive definition has no bounded set of locations to compile, and any depth limit would be a number chosen without reference to any instance.

### Keep the resolver pure

The resolver maps a document and a `$ref` value to a target schema and its location. It does not know the instance location, does not collect annotations, does not detect cycles, and caches nothing.

Following a reference must not change the instance location: the referring schema and the referenced schema describe the same instance positions, and only descending into `properties` or `items` extends the location. Because the same definition is normally reached from several instance locations, any cache of instance-location-dependent results inside the resolver would be wrong for every reuse but the first.

Each unsupported form is likewise an independent compatibility rule over one reached node, so supporting a form later means removing one rule and adding one collector.

Because this change introduces the traversal, it also implements the `prefixItems` rejection [the foundation ADR](20260723T160117Z_json-schema-driven-snapshot-normalization-foundation.md) requires, as one more such rule. Without it the traversal would treat `items` as describing every element of a tuple-prefixed array and collect instructions for positions that schema does not describe.

### Classify schema preparation failures

Schema preparation must distinguish at least: non-string reference, unsupported reference form, invalid reference container, unresolved reference, invalid resolved target, nested `$id`, dynamic reference or anchor, `$ref` sibling, and reference cycle.

They are separate because they are separate repairs, and a caller must be able to act on the classification rather than on message text. In particular, "the document has no object `$defs`" and "`$defs` has no such member" are different defects, and neither is "this spelling is outside the profile".

Every failure carries the schema location of the offending keyword as an RFC 6901 JSON Pointer from the document root, with the root itself being the empty pointer, plus the offending literal value where one exists. A cycle additionally carries the location of the `$ref` that closed it, the active reference chain as `$ref` location and resolved target pairs, and the target the cycle re-entered.

## Non-Goals

This decision does not add external, remote, relative, anchor-based, or dynamic reference support; percent-decoding or general URI resolution; a nested `$id` resource registry; targets below a direct `$defs` entry; normalization of recursive schemas; traversal of applicator keywords; or any validity guarantee about the schema document as a whole.

It also does not make normalization available to user-supplied schemas. The facility is internal to this repository's snapshot harness.

## Alternatives Considered

### Resolve references with a general JSON Schema library

Rejected. The contract suites already compile these schemas with the `jsonschema` crate, but a validator resolves references in order to evaluate an instance; it does not hand back the annotated schema nodes, their document locations, and the instance locations they describe, which is the whole output of schema preparation. Driving normalization from a general resolver would mean adopting base-URI rebasing, anchors, and dynamic scope as normalization semantics, and a mis-resolution there is invisible: the plan simply normalizes a different field.

### Parse the reference as a URI

Rejected. It would accept forms whose resolution this profile does not implement, so the check for "is this supported" would move from the reference's spelling into the resolution logic, where the answer is much harder to read off. A literal prefix keeps the supported set legible in the schema itself.

### Allow any JSON Pointer into the document

Rejected. A pointer such as `#/properties/x/properties/y` addresses a schema today and an arbitrary object after an unrelated edit, and `#/$defs/Foo/properties/bar` addresses part of a definition whose surrounding constraints would be silently dropped. Restricting targets to direct `$defs` entries makes "this reference names a definition" a property of the reference's text.

### Check every reference in the document

Rejected. It would make normalization support a constraint on parts of the contract normalization has no opinion about: a schema could not use `oneOf` over a recursive expectation definition, which both contracts do, without the normalizer refusing the whole document. Reference checking is scoped to what the normalizer must interpret, and document validity is checked separately.

### Ignore unsupported references instead of failing

Rejected. Silently skipping a reference leaves the annotations behind it uncollected, which puts a volatile value into a snapshot with nothing reporting why. A rejected schema is a visible defect; a skipped subtree is a snapshot that fails later somewhere else.

### Detect cycles in the resolver by remembering resolved targets

Rejected. The resolver would need traversal state it deliberately does not have, and "already resolved" is not "currently being expanded": every ordinary reuse of a definition would be reported as a cycle.

## Consequences

### Positive Consequences

- The supported reference set is readable from the schema text, so whether the normalizer will follow a reference does not depend on resolution logic.
- Reportage's existing contract schemas prepare unchanged, including their root `$id` and their applicator keywords.
- A definition may be reused freely, and each use produces instructions for its own instance locations.
- Schema defects are found once, against the schema, with a JSON Pointer to the keyword to edit and a classification stating which repair is needed.
- Supporting a further form later is a local change: one compatibility rule removed, one collector added.

### Negative Consequences

- A schema that uses references in a legitimate but unsupported way must be restructured before normalization can be applied to it, even though it is a valid JSON Schema.
- Annotations cannot be placed beside a `$ref`, so annotating one use of a shared definition differently requires a separate definition.
- Recursive structures cannot be normalized at all, and would need a different plan representation to support.
- The traversal-reachability boundary means the same defect is an error in one part of a document and unexamined in another, which has to be understood to read a diagnostic.

### Neutral Consequences

- The resolver, the compatibility rules, and the collector are separate components; how they are composed in Rust is an implementation choice.
- Instruction deduplication, conflicting instructions, and applying a plan to a document are decided by the foundation ADR and implemented with instance processing in issue #114.
