# Add command invocation contracts to the E2E assertion DSL

- Status: Rejected
- Created: 2026-07-01T04:18:11Z

## Context

Reportage treats PATH shims and adapters as part of its execution boundary. Protocol-compliant shims may emit invocation metadata, and adapters may provide execution metadata that helps diagnose failures or reproduce a case.

One considered option was to expose expected command invocation contracts as ordinary E2E assertions. A scenario could then assert how a registered command was called.

```reportage
assert {
  command "app" invoked once
  command "app" args contains "--json"
}
```

This would make shim or adapter invocation metadata directly assertable from the public scenario syntax.

The problem is that command invocation contracts primarily test the route by which behavior was produced, not the externally observable behavior itself. That moves ordinary E2E scenarios toward spy/mock/white-box testing and can over-couple scenarios to implementation details.

Reportage's main assertion boundary is externally observable evidence: exit status, stdout, stderr, files, selected workspace deltas, HTTP status/body when supported, and other evidence visible at the process or system boundary.

## Decision

Reportage will not add expected command invocation contracts to the ordinary E2E assertion DSL.

The following style of assertion is not part of the ordinary public scenario DSL:

```reportage
assert {
  command "app" invoked once
  command "app" args contains "--json"
}
```

Shim and adapter invocation metadata may still be captured as diagnostic, reproduction, or explanatory evidence. It may be used by failure reproduction bundles, action timelines, evidence dependency maps, internal adapter tests, or debug tooling.

The durable boundary is:

```text
Allowed as evidence / diagnostics:
  invocation metadata emitted by shims or adapters

Rejected as ordinary E2E assertion DSL:
  expected command invocation contracts such as invoked once / called with args
```

## E2E Assertion Boundary

Ordinary Reportage assertions should evaluate externally observable evidence from the subject under test or the test workspace boundary.

Examples that fit this boundary:

- exit status;
- stdout and stderr content;
- generated files;
- selected workspace deltas;
- structured output queries;
- future HTTP response status/body evidence.

Expected command invocation contracts do not fit this boundary by default because they assert an internal collaboration route. They are closer to spy/mock assertions, even when the metadata is collected through Reportage's shim boundary rather than by application instrumentation.

This distinction does not mean invocation metadata is useless. It means invocation metadata is primarily evidence for diagnosis and reproduction, not ordinary user-facing E2E correctness.

## Alternatives Considered

### Add `command invoked once` and related assertions to public syntax

This was rejected because it would encourage scenarios to specify implementation routing rather than externally observable behavior.

For example, a scenario could fail because an application changed its delegation strategy even though the externally visible output, files, and exit status remained correct. That coupling is acceptable in some unit or integration tests, but it should not define Reportage's ordinary E2E assertion model.

### Allow invocation assertions only for registered commands

Restricting the feature to registered commands would reduce ambiguity, but it would not solve the boundary problem. The assertion would still make the scenario depend on how behavior was produced rather than what behavior was observed.

Registered command metadata remains useful for diagnostics and reproduction artifacts.

### Treat invocation contracts as adapter self-test syntax

This remains possible as a separate future design. Adapter self-tests or internal runner tests may need stronger contracts over invocation metadata.

If such a feature is introduced, it should be explicitly separated from ordinary E2E assertions and should document why it is not the default user-facing scenario model.

## Consequences

### Positive Consequences

- Ordinary scenarios remain focused on externally observable behavior.
- Reportage avoids becoming a general spy/mock assertion framework.
- Tests are less likely to overfit implementation routing details.
- Shim and adapter metadata can still improve failure analysis without changing the public assertion boundary.

### Negative Consequences

- Users cannot directly express `command invoked once` as an ordinary E2E assertion.
- Some adapter or delegation correctness checks will need another testing layer or a future explicitly separated self-test mode.
- Debugging workflows must present invocation metadata as evidence rather than as ordinary pass/fail assertions.

### Neutral Consequences

- Failure reproduction bundles may include invocation metadata when it helps reconstruct the execution context.
- Action timeline evidence may reference shim or adapter events as explanatory data.
- Evidence dependency maps may include invocation metadata only where it represents an explicit Reportage-observed relation, not inferred internal behavior.

## References

- [#45: failure reproduction bundle を設計する](https://github.com/tooppoo/reportage/issues/45)
- [#49: action timeline evidence を設計する](https://github.com/tooppoo/reportage/issues/49)
- [#50: evidence dependency map を設計する](https://github.com/tooppoo/reportage/issues/50)
- [#51: 不採用案の理由をADRとして記録する](https://github.com/tooppoo/reportage/issues/51)
- [Use shim-emitted events for shim invocation observability](20260629T000000Z_use-shim-emitted-events-for-shim-invocation-observability.md)
