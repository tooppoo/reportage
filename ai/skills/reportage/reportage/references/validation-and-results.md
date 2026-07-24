# Validation and Result Interpretation

## Purpose

This document defines how to decide whether dynamic validation is safe, execute the advertised validation command, capture its results, and interpret them without conflating distinct failure categories.

## Validation Is Potentially Destructive

The command in `validation.command` may execute the scenario.

It is not necessarily:

- a parser-only check
- a semantic-only check
- side-effect-free
- confined to harmless local state

Treat validation as execution of the scenario's actions.

Do not run it solely because the file was generated successfully.

## Pre-Execution Safety Review

Before dynamic validation, inspect the scenario and relevant configuration for:

- file deletion
- recursive deletion
- writes outside an isolated workspace
- absolute-path mutation
- writes through symbolic links
- network requests
- database mutation
- queue or message publication
- deployment
- package publication
- release creation
- credential use
- secret exposure
- calls to production or shared services
- irreversible operations
- high-cost operations
- commands whose effects are not understood

Also inspect invoked scripts when their behavior determines safety.

Do not assume that reportage workspace isolation makes arbitrary shell commands safe.

## Validation Decision

Dynamic validation may proceed when:

- the scenario's actions are reasonably understood
- expected side effects are acceptable within the user's request
- shared or production resources are not at risk
- required credentials are intentionally available
- the execution environment is suitable
- rollback is unnecessary or feasible

Do not run when:

- side effects cannot be assessed
- commands are opaque and potentially destructive
- production targets may be selected
- required authorization is unclear
- the user requested static review only
- execution would exceed the task's legitimate scope

When validation is not performed, state:

- that only static review was completed
- the specific action or uncertainty that blocked execution
- what remains unverified

## Use the Advertised Command

Obtain the validation command from the current documentation index:

```json
{
  "validation": {
    "command": "..."
  }
}
```

Do not assume that a remembered command still exists.

Do not substitute:

- `reportage check`
- a command from another version
- a repository script that appears equivalent
- a human-guide example when the index differs

The index field is the source of truth for the running binary's advertised validation invocation.

## Expanding the Command Template

Replace the documented file placeholder with the actual scenario path.

Do not use `eval`.

Prefer argument-based process execution.

When a shell is unavoidable:

- quote the path safely
- preserve the command structure
- do not allow path content to introduce additional arguments or shell operators

A filename containing spaces or shell metacharacters must remain one path argument.

For multiple files, validate each separately unless version-matched documentation explicitly defines multi-file behavior.

## Working Directory

Run validation from the project location required by the scenario and configuration.

Before execution, determine whether the command depends on:

- repository root
- config discovery
- relative fixture paths
- registered command paths
- environment setup

Do not arbitrarily change the working directory.

Record the effective working directory when it matters to interpretation.

## Capture Requirements

Capture:

- process exit status
- stdout
- stderr
- executed command
- working directory
- relevant environment assumptions
- produced artifact location when reported

Do not discard stdout because the process exited nonzero.

Do not merge stdout and stderr when the documented contract distinguishes them.

## Parsing JSON Output

When the advertised command uses JSON output:

1. Capture stdout completely.
2. Attempt to parse it as one JSON document.
3. Validate its shape against the version-matched JSON report contract when exact interpretation matters.
4. Preserve raw stdout when parsing fails.
5. Preserve stderr separately.

Do not parse JSON with regular expressions.

Do not infer success from the presence of a particular substring.

## Exit Status Is Not the Whole Result

Do not determine the result from the shell exit status alone.

The process exit status may distinguish categories, but detailed interpretation belongs to the version-matched:

- JSON report contract
- exit-code documentation
- diagnostics documentation
- execution model

Attempt to parse documented output even when the exit status is nonzero.

## Separate Exit-Code Domains

Keep separate:

- the reportage process exit code
- an exit code produced by an action inside a scenario

An action exit code is evidence evaluated by the scenario.

The reportage process exit code describes the overall reportage invocation according to its documented contract.

Do not report one as the other.

## Passed, Failed, and Error

Interpret status values according to the current JSON report contract.

At a conceptual level:

- `passed` means the scenario ran and its assertions passed
- `failed` means the scenario was valid enough to run, but at least one assertion failed
- `error` means the invocation could not produce an ordinary pass/fail result because of parsing, validation, configuration, semantic, runtime infrastructure, or another documented error condition

Do not reduce these to a boolean.

Do not describe an `error` as a failed application assertion.

## Assertion Failures and Diagnostics

Keep assertion failures separate from diagnostics.

An assertion failure means:

- the scenario was accepted sufficiently to run
- observed behavior did not satisfy an expectation

A diagnostic may represent:

- syntax rejection
- validation rejection
- semantic rejection
- configuration failure
- step execution problem
- infrastructure failure
- another documented category

A failed assertion is not automatically a diagnostic.

Do not guess the meaning of a diagnostic code from its name.

Read the version-matched diagnostic documentation.

## Syntax, Semantic, and Runtime Boundaries

When explaining a failure, identify its stage.

### Syntax error

The parser rejected the scenario's form.

### Semantic error

The scenario parsed, but a documented semantic rule rejected its meaning or use.

### Assertion failure

The scenario ran, but observed evidence did not satisfy an expectation.

### Configuration error

Required configuration was missing, invalid, or inconsistent.

### Execution or infrastructure error

The runner could not execute the scenario or collect a normal result.

Do not use these terms interchangeably.

## Diagnostic Preservation

When reporting a diagnostic, preserve:

- category
- code
- message
- file
- line
- column
- relevant structured details

Do not rewrite the code into a friendlier but inaccurate label.

A concise explanation may accompany the original fields, but it must be grounded in the version-matched documentation.

## Result Analysis Procedure

When validation does not pass:

1. Preserve raw process data.
2. Parse the JSON report when possible.
3. Identify the top-level status.
4. Identify the reportage process exit code.
5. Separate diagnostics from test assertion results.
6. Locate the failing case, step, or assertion.
7. Compare observed and expected evidence.
8. Determine whether the defect is in:
   - the scenario
   - configuration
   - environment
   - application behavior
   - expectation
   - infrastructure
9. Make the smallest justified correction.
10. Re-run only when safety conditions still hold.

## Correcting Failures

Do not automatically edit the scenario when validation fails.

First determine whether:

- the scenario contains invalid syntax
- the scenario misstates the requirement
- application behavior is defective
- environment setup is incomplete
- expected evidence is wrong
- a nondeterministic dependency changed
- infrastructure prevented execution

Do not modify application code unless the user requested that scope.

Do not weaken assertions solely to obtain a passing status.

## Safe Re-Execution

Before re-running after a change, reassess:

- whether the change introduced new side effects
- whether the previous run left mutable state
- whether cleanup is required
- whether external state has changed
- whether repeated execution is idempotent
- whether credentials or rate limits are affected

A command that was safe once is not necessarily safe to repeat.

## Artifacts

When validation produces artifacts:

- record the artifact root
- preserve the version-matched artifact contract
- use artifact files for retained evidence
- do not rely only on memory of stdout
- correlate artifact data with the execution report when needed

Do not infer artifact structure from another version.

## Partial Validation

When only some files or cases were run, report the exact scope.

Do not say:

- “reportage tests passed”
- “the scenario is validated”
- “all cases pass”

unless the executed scope justifies that statement.

Use precise language such as:

```text
Dynamic validation was performed for tests/example.repor only.
The invocation returned status "passed".
Other reportage scenarios were not run.
```

## Static Review Versus Dynamic Validation

Use distinct claims.

Static review can support:

- “No undocumented syntax was found.”
- “The assertions appear insufficient for the stated requirement.”
- “The scenario contains a potentially destructive command.”

Dynamic validation can support:

- “The advertised validation command was executed.”
- “The invocation returned status `passed`.”
- “Case X failed assertion Y.”

Neither alone proves the application's general correctness.

## Completion Report

Include:

- reportage version and tag
- documentation IDs consulted
- documentation cache directory
- scenario files validated
- exact command executed
- working directory when relevant
- whether execution was dynamic or static only
- process exit status
- documented report status
- relevant diagnostic categories and codes
- failing cases or assertions
- artifact root when relevant
- remaining unverified behavior
- any safety condition that prevented execution

Never claim validation when only static inspection occurred.

## Prohibited Behavior

Do not:

- execute without reviewing side effects
- use `eval`
- assume `reportage check` exists
- infer the result from exit status alone
- ignore nonempty stderr without analysis
- discard stdout on nonzero exit
- conflate action and reportage exit codes
- conflate assertion failure and diagnostic error
- guess diagnostic meanings
- rerun destructive or non-idempotent scenarios automatically
- weaken expectations to manufacture a pass
- overstate the scope of validation
