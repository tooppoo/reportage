# JSON Schema-Driven Snapshot Normalization Foundation

- Status: Accepted
- Created: 2026-07-23T16:01:17Z

## Context

Reportage uses snapshots to detect regressions in machine-readable output and
other observed evidence. Some values, such as tool versions, run identifiers,
temporary paths, and artifact roots, are intentionally volatile and must be
stabilized before snapshot comparison. #113 established that this operation is
`snapshot normalization`: a deterministic comparison policy owned by the
snapshot harness, not part of the Reportage DSL semantics.

#114 adopts JSON Schema annotations as the policy carrier for JSON snapshot
normalization. Keeping the policy beside the field definition avoids repeating
instance paths in each fixture and reduces drift when internal JSON structures
change. However, an annotation shape alone does not define how a schema is
prepared, how annotations become instance operations, how unsupported JSON
Schema structures behave, or how failures and formatted snapshots are
classified.

The schemas in scope are maintained by Reportage for its own JSON contracts.
This decision does not introduce a facility for users to supply arbitrary JSON
Schemas, and it does not require the normalizer to implement the complete JSON
Schema evaluation model. Raw JSON contract validation is a separate concern
tracked by #192.

Reportage schemas also use `$defs` and `$ref` to express envelope-style and
reusable structures. A limited static local `$ref` capability is therefore part
of the initial normalization foundation, while its resolver implementation is
tracked separately by #193.

Related issues: #113, #114, #115, #162, #163, #164, #165, #192, and #193.

## Decision

### Separate schema preparation from instance processing

Snapshot normalization must use two distinct phases.

Schema preparation must:

1. read the Reportage-maintained internal schema;
2. parse `x-reportage-snapshot` annotations into typed metadata;
3. check whether schema structures reached by normalization traversal are
   supported;
4. resolve supported static local `$ref` references;
5. collect normalization instructions;
6. detect duplicates and conflicts; and
7. compile an immutable normalization plan.

Instance processing must:

1. receive a raw JSON document after the applicable contract validation has
   completed;
2. apply the compiled normalization plan;
3. apply deterministic snapshot formatting; and
4. compare the result with the expected snapshot.

Instance processing must not reinterpret the JSON Schema for every document.
Schema-side failures that do not depend on an instance should be detected during
schema preparation.

The normalized document is a snapshot-comparison representation. It is not
required to satisfy the source JSON Schema after placeholder replacement.

### Keep preparation checks internally removable and extensible

Compatibility checks in schema preparation must be implemented as independent
internal rules so that future support can remove one rejection rule and add one
collector without restructuring the entire normalizer.

This is an internal maintainability requirement. Users must not be able to
select, add, disable, or reorder these rules as a plugin system. The supported
normalization profile is fixed by the Reportage version.

The implementation may use ordinary functions, enums, or traits. Dynamic plugin
loading is not required.

### Limit the initial normalization traversal

The initial normalization traversal supports:

- the root schema;
- object `properties`;
- homogeneous array `items`; and
- supported static local `$ref` references to schemas under `$defs`.

`$defs` does not independently correspond to an instance location. A schema
under `$defs` is traversed only when it is reached through a supported `$ref`.

An annotation under `items` applies to every element of the corresponding
homogeneous array.

If a schema object reached by normalization traversal contains `prefixItems`,
schema preparation must fail. In JSON Schema Draft 2020-12, `items` applies only
after the tuple prefix when `prefixItems` is present, so that schema object
cannot be treated as homogeneous. Future `prefixItems` support must be addable
by removing this compatibility check and adding a dedicated collector.

### Ignore unsupported keyword subtrees for normalization

The following schema-bearing keywords are not traversed by the initial
normalizer:

- `oneOf`, `allOf`, and `anyOf`;
- `not`;
- `if`, `then`, and `else`;
- `patternProperties`;
- schema-form `additionalProperties`;
- `dependentSchemas`;
- `propertyNames`;
- `contains`;
- `unevaluatedProperties` and `unevaluatedItems`; and
- any other schema-bearing keyword outside the initial traversal subset.

Their presence in a JSON Schema is allowed. The normalizer must not treat their
presence as a schema preparation error or warning.

The initial normalizer does not enter those subtrees. Any
`x-reportage-snapshot` annotation reachable only through an unsupported keyword
is ignored for normalization, and the corresponding instance value is preserved
under the default-preserve policy.

This decision concerns normalization only. It does not deny the JSON Schema
validity or validation semantics of those keywords. Dedicated support is
tracked separately, including #163, #164, and #165.

### Support a limited static local `$ref` profile

Static local `$ref` is part of the initial normalization foundation because
Reportage schemas use envelope and reusable definitions.

The initial supported profile is limited to:

- references within the same schema document;
- fragment-only references;
- JSON Pointer syntax; and
- targets under `$defs`.

The following forms must fail during schema preparation:

- external file references;
- remote URL references;
- `$anchor`-based references;
- `$dynamicRef` and `$dynamicAnchor`;
- references requiring `$id` rebasing or an embedded resource registry;
- unresolved references;
- reference cycles; and
- a `$ref` with sibling schema keywords in the same schema object.

Annotations must be placed in the referenced schema rather than beside `$ref`.
The resolver, cycle detection, diagnostics, and representative tests are the
responsibility of #193.

### Use one typed annotation shape

The initial annotation name is `x-reportage-snapshot`.

```json
{
  "x-reportage-snapshot": {
    "operation": "replace",
    "value": "<reportage:tool.version>"
  }
}
```

The annotation must be an object with exactly these members:

- `operation`: required string;
- `value`: required string.

Unknown members, a missing member, a non-object annotation, or an unknown
operation must be a schema preparation error.

The initial and only operation is `replace`. Reportage must parse the annotation
into a typed internal representation during schema preparation. The initial
implementation must not introduce a custom JSON Schema meta-schema or vocabulary
only to validate this annotation.

### Restrict replacement values and targets

The initial replacement value is a string.

`replace` may target only scalar JSON values:

- string;
- number;
- boolean; or
- null.

Replacing an object or array with a string placeholder would hide an entire
contract or evidence structure. It must therefore be a normalization application
error. The same restriction applies to an annotation on the root schema.

The property or array element itself remains present. Removing members or array
elements is not part of the initial operation set.

### Delegate requiredness to contract validation

Normalization metadata must not introduce a separate `required` flag.

- If an optional property is absent, its instruction is not applied and
  normalization succeeds.
- If a property is present with `null`, it is a scalar target and may be
  replaced.
- If a required property is absent, that is outside normalization and belongs to
  the applicable raw contract validation policy tracked by #192.

### Deduplicate identical instructions and reject conflicts

The normalizer must not assign an implicit priority based on schema member order,
traversal order, or source order.

When multiple schema paths produce instructions for the same instance location:

- instructions with the same operation and value are deduplicated and applied
  once;
- instructions with different operations or values are a schema preparation
  conflict error.

A conflict diagnostic should include every known source schema location that
contributed to the conflict.

### Classify failures by phase

The normalization pipeline must distinguish at least these categories.

#### Schema preparation error

A schema or metadata failure detectable before instance processing, including:

- malformed annotation metadata;
- an unknown operation;
- an unknown annotation member;
- `prefixItems` on a schema object reached by traversal;
- an unsupported `$ref` form;
- an unresolved reference;
- a reference cycle; or
- conflicting instructions.

#### Normalization application error

A failure that depends on the concrete instance, including:

- replacing an object or array; or
- an unexpected mismatch between the compiled plan and the instance shape.

#### Snapshot mismatch

Normalization and formatting succeeded, but the actual formatted snapshot does
not equal the expected snapshot.

#### Harness internal error

An implementation failure that cannot be classified under the normalization
contract above.

Observed JSON parsing and raw contract validation errors are outside this ADR and
are tracked by #192 or the corresponding contract-test policy.

Schema preparation errors should include a schema location whenever possible.
Normalization application errors should include the instance location as a JSON
Pointer and the source schema location that produced the instruction.

### Use deterministic snapshot formatting, not canonical JSON

The formatted snapshot must follow these rules:

- recursively sort object keys in lexicographic ascending order;
- preserve array order;
- use two-space indentation;
- use LF line endings;
- end with exactly one trailing newline;
- serialize parsed numbers through `serde_json`, without preserving the input
  lexical representation; and
- emit UTF-8 without forcing ASCII escapes beyond those required by JSON.

This is called deterministic snapshot formatting. It is not RFC 8785 canonical
JSON.

The existing `serde_json::to_string_pretty` plus trailing newline behavior may be
used as the base, but object ordering must be made explicit and deterministic.

## Non-Goals

This decision does not define:

- raw JSON contract validation or the relationship between typed Rust models and
  JSON Schema validation; see #192;
- the implementation details of static local `$ref`; see #193;
- general JSON Schema validation or evaluation;
- user-supplied JSON Schema support;
- normalization support for the initially ignored keywords;
- external, remote, anchor-based, dynamic, or recursive references;
- a `remove` operation;
- arbitrary JSONPath or transformation languages; or
- the snapshot approval workflow.

## Alternatives Considered

### Interpret the schema during every instance normalization

Rejected. Schema defects would be rediscovered for each fixture, conflict
handling would be tied to traversal order, and instance processing would have to
carry schema-evaluation responsibilities. Preparing one normalization plan gives
a stable boundary and makes schema-side failures independent of the instance.

### Reject schemas that contain unsupported JSON Schema keywords

Rejected. Reportage schemas already use JSON Schema features for contract
expression that the initial normalizer does not need to understand. Rejecting the
whole schema would improperly conflate JSON Schema validity with normalization
capability and would prevent gradual normalization support.

### Fail when an annotation appears below an unsupported keyword

Rejected for the initial profile. The normalizer deliberately does not traverse
those subtrees and preserves the corresponding values. This keeps unsupported
normalization features non-destructive and allows the schema to continue using
those keywords for contract purposes. Later issues may add traversal semantics
without changing existing preserved values unexpectedly.

### Support the complete JSON Schema evaluation model

Rejected. The schemas are controlled by Reportage, and normalization requires
only a bounded structural subset. A complete evaluator would introduce
substantial complexity unrelated to the immediate snapshot policy.

### Allow arbitrary JSON replacement values and structured targets

Rejected. Replacing an object or array can erase the shape that snapshots are
intended to protect. A string placeholder applied only to scalar values provides
a clear and reviewable initial contract.

### Apply the first or last instruction on conflict

Rejected. Schema order is not a policy priority. First-wins or last-wins would
make normalization results dependent on incidental document structure and could
silently hide conflicting intent.

### Use canonical JSON

Rejected. Snapshot readability requires pretty formatting, while the project
does not need RFC 8785 byte-level canonicalization. The chosen formatting rules
are deterministic and human-reviewable without claiming canonical JSON
compliance.

## Consequences

### Positive Consequences

- Schema defects and annotation conflicts are detected once during preparation.
- Instance normalization is deterministic and does not depend on schema traversal
  order.
- Unsupported JSON Schema features remain usable for contract description while
  their values remain visible in snapshots.
- The initial implementation is bounded but has explicit extension points for
  `prefixItems`, applicators, dynamic property schemas, and broader references.
- Scalar-only replacement reduces the risk that normalization hides contract or
  evidence structure.
- Failure categories and diagnostic locations make schema defects distinguishable
  from snapshot regressions.
- Deterministic formatting reduces snapshot churn from object ordering and output
  formatting.

### Negative Consequences

- An annotation placed only under an unsupported keyword has no effect and
  produces no warning in the initial implementation.
- Reportage must maintain a normalization-specific traversal profile in addition
  to its JSON contract schemas.
- Static local `$ref` requires a dedicated resolver and cycle detection even
  though broader JSON Schema resolution remains unsupported.
- Scalar-only replacement may require additional contract tests or future
  operations for use cases that genuinely need structured normalization.
- Compiling a plan introduces an additional internal representation and diagnostic
  model.

### Neutral Consequences

- The raw document's contract validation mechanism remains unchanged by this ADR
  and is decided separately in #192.
- The exact Rust abstraction used for preparation checks is an implementation
  choice as long as checks can be added and removed locally.
- Later support for ignored keywords will require dedicated decisions defining
  branch selection, matching, evaluated-property tracking, and conflict behavior.
