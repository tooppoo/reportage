# Authoring and Review

## Purpose

This document defines how an agent should create, edit, and review reportage scenarios after obtaining the required version-matched documentation.

## Requirement Analysis

Before writing or changing a `.repor` file, separate:

1. The application's required behavior.
2. The observable evidence that would demonstrate that behavior.
3. The reportage syntax available in the installed version.
4. The reportage semantics governing that syntax.
5. Environment assumptions required to execute the scenario.
6. Safety constraints on dynamic execution.

Do not begin from a preferred syntax and force the requirement into it.

If the requested behavior cannot be expressed by the installed reportage version, state that limitation explicitly.

## Normative Source Priority

Use this priority when sources appear to disagree:

1. Version-matched normative syntax, semantics, diagnostics, execution, and schema documents.
2. Version-matched generated semantic catalogs and schemas.
3. Version-matched AI guides.
4. Known-good examples and fixtures.
5. Existing project scenarios.
6. Memory or analogy with other tools.

A lower-priority source cannot establish support for a construct absent from higher-priority normative documentation.

## Using Existing Project Files

Inspect nearby `.repor` files and project conventions before authoring.

Existing project files may establish:

- naming conventions
- directory placement
- test organization
- preferred command wrappers
- common setup patterns
- local artifact expectations

They do not prove that their syntax is valid for the currently installed reportage version.

When an existing file conflicts with the current version-matched specification, do not copy the conflict into new work.

## Using Examples and Fixtures

Prefer adapting a known-good example over constructing a complex scenario from grammar fragments alone.

Examples and valid fixtures can help with:

- concrete block shape
- ordering
- quoting
- combinations of individually valid constructs
- minimal working scenarios

Still verify every used construct against the current syntax and semantics documents.

Do not infer a general rule from one example when the normative documentation is narrower.

## Authoring Procedure

For a new scenario:

1. State the behavior being tested.
2. Identify the action or actions that produce observable behavior.
3. Identify the minimum evidence required to distinguish success from accidental execution.
4. Select documented reportage constructs that express those checks.
5. Confirm semantic constraints on each construct.
6. Draft the smallest scenario that fully tests the stated behavior.
7. Review it statically.
8. Perform safety review before dynamic validation.
9. Validate using the advertised command when safe.
10. Interpret results using version-matched contracts.

For an edit:

1. Determine the existing scenario's intended requirement.
2. Identify whether the change affects syntax, semantics, execution order, environment, or expected behavior.
3. Preserve valid existing assertions unless the requirement itself changed.
4. Make the smallest justified change.
5. Re-evaluate assertion adequacy.
6. Review safety and validate when appropriate.

## Do Not Invent Syntax

Do not create:

- new keywords
- aliases
- infix operators
- assertion modifiers
- block types
- config fields
- commands
- output fields
- diagnostic meanings

A construct that appears plausible is not available unless it is documented for the current version.

Deferred items and design discussions are not usable features.

## Unsupported Behavior

When the user's requested behavior is not expressible:

1. Identify the exact missing capability.
2. Distinguish language limitation from application limitation.
3. Do not fabricate a workaround that changes the requirement.
4. Offer only alternatives that preserve the intended verification boundary.
5. State what remains unverified.

A shell command inside an action is not automatically an acceptable substitute for a missing reportage assertion if it hides evidence or shifts correctness into opaque script logic.

## Assertion Adequacy

A scenario is not adequate merely because it executes successfully.

For each requirement, ask:

- What observable result would prove the requirement?
- Could the scenario pass if the application did nothing?
- Could it pass if the wrong command ran?
- Could it pass with incomplete or malformed output?
- Could it pass while leaving unintended state behind?
- Does it check only process completion when content or artifacts matter?
- Does it assert output that was produced?
- Does it distinguish expected failure from infrastructure failure?

Prefer assertions that directly correspond to the stated requirement.

Do not add assertions merely to increase count. Each assertion should protect a meaningful property.

## Unasserted Evidence

When an action produces evidence that appears relevant but no assertion evaluates it, identify that gap.

Examples include:

- stdout or stderr that carries the result
- a generated file
- a state transition
- an artifact manifest
- a command exit status
- a structured response

Unasserted evidence may indicate that the scenario confirms execution without confirming correctness.

Treat this as a review concern, not automatically as an error, unless the requirement clearly depends on that evidence.

## Do Not Weaken Assertions to Obtain a Pass

When actual behavior differs from expected behavior:

1. Verify the requirement.
2. Verify the scenario.
3. Verify the environment.
4. Determine whether the application or expectation is wrong.
5. Change the expectation only when the requirement justifies it.

Do not:

- remove a failing assertion without explanation
- replace an exact check with a vague check solely to pass
- ignore stderr solely because it is inconvenient
- accept a broader exit-code range without a requirement
- treat nondeterminism as success

Explain any reduction in verification strength.

## Review Dimensions

Review each scenario along separate dimensions.

### Syntactic validity

Does every construct appear in the current syntax documentation?

### Semantic validity

Does the scenario satisfy the documented semantic rules?

### Requirement correctness

Does the scenario test the behavior the user or project actually requires?

### Assertion adequacy

Would the scenario fail when the required behavior is absent or incorrect?

### Execution safety

Can dynamic execution mutate shared, external, or production state?

### Reproducibility

Does the scenario depend on uncontrolled host state, time, network, locale, ordering, or credentials?

### Maintainability

Is the scenario explicit, localized, and understandable without hidden shell logic?

### Evidence quality

Does the scenario retain and evaluate the evidence needed to explain the result?

Do not collapse these dimensions into one pass/fail judgment.

## Environment and Reproducibility

Check for accidental dependence on:

- current working directory outside the case workspace
- absolute paths
- files left by earlier runs
- host-installed commands not declared by the project
- network availability
- external service state
- current time or timezone
- locale
- random values without control
- environment variables or credentials
- execution order across independent cases
- shared mutable directories

When dependence is intentional, it should be documented or constrained by project configuration.

## Shell Actions

Shell-like actions may conceal complexity.

Review whether an action:

- combines multiple responsibilities
- performs assertion logic internally
- suppresses relevant errors
- redirects evidence away from reportage
- mutates external state
- relies on shell-specific behavior not guaranteed by the project
- uses unsafe interpolation
- makes failure attribution difficult

Prefer explicit reportage assertions over shell code that converts rich evidence into one opaque exit status, when the documented language supports the direct assertion.

## Checkpoints and Ordering

When assertions depend on prior actions, verify the documented checkpoint and ordering semantics.

Do not assume:

- every assertion observes final state
- actions and assertions can be reordered freely
- a later action cannot invalidate earlier evidence
- a checkpoint is global when it is local
- isolated cases share state

Read the execution model whenever ordering affects correctness.

## Configuration Dependencies

When a scenario relies on `reportage.kdl`, review both together.

Check:

- command registration
- executable invocation
- path assumptions
- adapter behavior
- environment setup
- versioned configuration fields
- missing or conflicting settings

Do not review a config-dependent scenario as self-contained.

## Review Findings

A useful finding should state:

1. The affected file and location.
2. The observed construct or behavior.
3. The violated requirement or documented rule.
4. The consequence.
5. The smallest justified correction.
6. Whether the finding was established statically or dynamically.

Distinguish direct documentation findings from inference.

Example structure:

```text
Finding:
The scenario checks only exit status, although the requirement is about generated JSON content.

Basis:
The action can exit successfully while producing incorrect content.

Consequence:
The test can pass without verifying the stated requirement.

Correction:
Add a documented output or file-content assertion that checks the required field.
```

## Static Review Limits

A static review can establish:

- use of undocumented syntax
- obvious semantic conflicts
- missing assertions
- unsafe commands visible in the scenario
- dependence on undeclared state
- inconsistency with configuration

It cannot establish that:

- the application behaves correctly
- the scenario passes
- external commands exist
- runtime evidence matches expectations
- all side effects are safe

Report these limits explicitly.

## Completion Checklist

Before treating authoring or review as complete, verify:

- version-matched documents were used
- every construct is documented
- semantic constraints were considered
- application requirements are explicit
- assertions correspond to those requirements
- relevant evidence is asserted
- environment assumptions are identified
- dynamic execution safety was assessed
- static review and dynamic validation are not conflated
- unsupported behavior is reported rather than invented
