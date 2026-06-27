# Design Principles

reportage is an E2E-oriented test runner. It should make E2E scenarios easier to write, execute, and inspect, but it must not add avoidable overhead to environments that are already heavy by nature.

The goal is not to make E2E itself cheap. Databases, Redis, containers, browsers, web frameworks, and external service fixtures may be expensive to start and run. That cost belongs to the target system and its test environment. The goal of reportage is to keep its own overhead small, predictable, and structurally bounded.

```text
total cost
= target system cost
+ scenario execution cost
+ reportage core overhead
+ shim / adapter overhead
```

reportage is responsible for the last two terms. It should not hide or amplify the cost of the first two.

## Core: thin execution and assertion engine

The core should remain a thin runner, parser, and assertion engine.

Core responsibilities are limited to:

- parsing scenario files;
- building an execution plan;
- spawning commands;
- capturing stdout, stderr, exit status, and declared artifacts;
- applying assertions;
- reporting results.

The core should not own heavyweight lifecycle management. In particular, the core should not directly manage:

- database servers;
- Redis or other backing services;
- container orchestration;
- browser automation;
- framework-specific application lifecycles;
- coverage instrumentation internals;
- long-running background daemons;
- repository-wide source analysis.

Those concerns may still be useful, but they belong to external tools, shims, adapters, or post-processing systems.

## Structural performance constraints

The default execution path should be approximately linear in:

- the number of scenario steps;
- the number of assertions;
- the size of captured outputs;
- the number of declared artifacts.

The default execution path must not implicitly perform work proportional to:

- the repository size;
- the dependency graph size;
- the number of source files;
- the number of installed packages;
- the number of available framework components.

Any operation with those costs must be explicit and isolated behind an adapter, evaluator, reporter, or other opt-in extension point.

## Shims: transparent PATH proxies

A shim exists to route command execution without changing the user's scenario syntax. It should behave as a transparent PATH proxy.

A shim may:

- resolve the original command;
- inject required environment variables;
- route execution through a coverage tool;
- preserve stdout, stderr, and exit status semantics;
- collect minimal execution metadata.

A shim should avoid:

- deep stdout or stderr interpretation;
- full process-tree analysis unless explicitly requested;
- continuous filesystem watching;
- daemon processes;
- framework-specific behavior;
- coverage result interpretation;
- language-runtime helpers that are expensive to load for every command.

Coverage integration should usually be a connection mechanism, not an implementation of coverage measurement. reportage should rely on each language ecosystem's coverage tooling and consume the resulting artifacts when needed.

## Adapters: opt-in, lazy, and thin

Adapters are the boundary for heavier integrations. They may connect reportage to web frameworks, browsers, containers, coverage systems, or other runtime-specific tools, but they must not turn the core into a framework-specific runner.

Adapters should be:

- opt-in: unused adapters must not affect the default execution path;
- lazy: adapter setup should happen only when a scenario asks for it;
- thin: adapters should normalize evidence for the core rather than expanding the core's domain model;
- isolated: framework-specific behavior should stay inside the adapter.

An adapter should return normalized evidence, such as command results, HTTP responses, generated files, or coverage artifact locations. The core should not need to understand the internals of the framework that produced that evidence.

## Default and optional capabilities

The default core should prioritize lightweight capabilities:

- scenario parsing;
- command execution;
- stdout, stderr, and exit-status assertions;
- file existence and file-content assertions;
- external tool integration for structured assertions, such as jq;
- simple result and artifact output.

Heavier capabilities should remain optional:

- coverage adapters;
- web framework adapters;
- browser or Playwright integration;
- container lifecycle helpers;
- rich report generation;
- LLM-based evaluation;
- testography integration;
- repository-wide static or runtime analysis.

Optional capabilities may be valuable, but they should not change the cost model of the default runner.

## Web E2E boundary

reportage may support web E2E, but it should not become a browser automation framework by default.

The primary web-oriented use case should be declarative assertion over observable outputs:

- HTTP status, headers, and body;
- HTML structure;
- JSON responses;
- generated files;
- logs;
- coverage artifacts.

Complex interaction testing, such as clicking, dragging, form choreography, SPA state transitions, and visual timing behavior, should be delegated to specialized tools when needed. reportage may call those tools or validate their artifacts, but it should not make them part of the default execution model.

## Evidence first, analysis later

reportage should produce evidence. Heavy analysis should happen after evidence is produced.

This keeps the runner small and allows other tools to evaluate the same evidence in different ways. For example, testography, LLM evaluators, or rich reporters may analyze reportage outputs, but they should not be required for ordinary scenario execution.

This separation preserves the main architectural boundary:

```text
reportage
  -> execute scenarios
  -> collect evidence
  -> apply direct assertions

post-processing tools
  -> evaluate coverage meaning
  -> inspect test patterns
  -> generate rich reports
  -> support human review
```

## Summary

The design constraint is simple:

```text
core is thin
shims are transparent
adapters are opt-in
heavy lifecycle management is external
analysis is post-processing
```

reportage should be useful for E2E without inheriting all heavyweight behavior into its own core. It should make expensive systems testable without becoming an expensive system itself.
