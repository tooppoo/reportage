# Separate Filesystem Observation from Expectation Comparison

- Status: Accepted
- Created: 2026-07-25T01:51:00Z

## Context

The evaluator has separate execution, expectation, and observation modules, but the filesystem observation functions for `file contains`, `file contents_equals`, `file text_equals`, and `dir contains` also accepted expected values and performed matcher-specific work.
They decoded UTF-8, compared bytes or names, and constructed runtime result enums containing comparison outcomes.
This made the observation layer responsible for both acquiring actual-side state and deciding what that state meant for a particular expectation.

The existing runtime result enums are consumed by workspace code and serialized into CLI and artifact JSON.
This decision must therefore improve the internal boundary without changing those compatibility models or observable behavior.

## Decision

Filesystem observation must depend only on the workspace root and subject path.
It must return evaluator-internal raw observations that distinguish missing paths, wrong filesystem types, unreadable subjects, and acquired actual values.
File acquisition returns bytes without decoding them.
Directory acquisition returns direct-child entry names while preserving the existing rule that errors for individual entries are ignored.

The expectation layer must own expected-value resolution, UTF-8 decoding, substring matching, byte-for-byte comparison, exact entry-name matching, pass/fail decisions, and conversion into the existing runtime result model.
The dependency direction is `expectation -> observation`; observation must not depend on expectation or matcher-specific runtime result types.

The existing `FileContentObservation`, `ContentsEqualsObservation`, and `DirContainsObservation` types remain runtime compatibility models.
They are not actual-side acquisition models, and comparison-bearing variants must be constructed by the expectation layer.
Raw filesystem observation types must remain private to the evaluator module and must not create a new production public surface.

Future filesystem matchers should preserve the same boundary unless a later ADR replaces this decision.

## Alternatives Considered

### Keep matcher-specific observation functions

Rejected because passing expected values into filesystem acquisition makes the observation layer own decode, comparison, and result construction.
It also prevents multiple matchers from sharing a single expected-independent actual-side model.

### Move the existing runtime result enums into the observation module

Rejected because those enums contain interpreted comparison outcomes and are serialized compatibility models.
Treating them as raw observations would preserve the responsibility overlap and risk changing workspace consumers and output shapes.

### Expose raw observation types publicly

Rejected because the types are an evaluator implementation boundary, not a library contract.
A public surface would unnecessarily constrain later changes to acquisition details.

## Consequences

### Positive Consequences

- Filesystem acquisition can be tested independently of expected values and matcher semantics.
- Matcher-specific interpretation and compatibility-model construction have one owner in the expectation layer.
- Observation has no dependency on comparison types or comparison-bearing result enums.

### Negative Consequences

- The expectation layer contains explicit conversion code from raw acquisition states into each runtime compatibility model.

### Neutral Consequences

- Filesystem path resolution, symlink following, error classification, direct-child traversal, ignored individual-entry errors, diagnostics, pass/fail conditions, exit codes, and JSON shapes remain unchanged.
- `file exists` and `dir exists` continue to use their existing actual-side classifications because they require no expected-value comparison.
