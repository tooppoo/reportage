---
name: reportage-behavior-change
description: Require appropriate end-to-end coverage when changing reportage runtime or CLI-visible behavior, including exit codes, command execution, output, diagnostics, JSON, artifacts, assertions, and error handling.
---

# reportage-behavior-change

Use this skill when changing reportage runtime behavior.

Apply it to changes that affect any of the following:

- reportage process exit code
- target command execution behavior
- stdout or stderr output
- diagnostic output
- JSON output
- artifact or evidence generation
- assertion evaluation behavior
- runtime error behavior
- script error behavior
- semantic error behavior
- command success or failure handling
- snapshot output
- CLI-visible behavior

## Read the current references

For a repository change, start with [the generated AI reading order](../../../docs/ai/reading-order.generated.md) and read the local documents it lists.
The checked-out files describe the current repository revision, including unreleased changes.

For an installed reportage version, run `reportage references --format=json` and read the returned `documents[].urls.ai` entries in order.
Do not use `reportage docs` for reference discovery: that command generates documentation from `.repor` sources.
The URLs from `reportage references` are pinned to the running binary's version tag, so do not substitute them for the local documents when changing the current checkout.

## Required work

When changing behavior, add or update e2e coverage.

### 1. Add e2e tests for observable behavior

Cover user-visible behavior with a `.repor` self-test under [`e2e/`](../../../e2e/).
Use [the testing guide](../../../docs/design/testing/README.md) to choose the narrowest additional Rust test layer needed for internal models, harness behavior, or structural contracts.

Observable behavior includes:

- process exit code
- stdout
- stderr
- diagnostics
- generated artifacts
- JSON output
- assertion result
- failure classification
- filesystem side effects
- ordering of emitted events when relevant

Do not replace CLI-visible coverage with only a Rust unit or integration test.
Do not duplicate a `.repor` assertion in Rust unless the Rust test verifies a boundary that self-testing cannot observe.

### 2. Cover both successful and failing command execution

When command execution behavior is involved, provide both:

- a case where the target command succeeds
- a case where the target command fails

The tests must distinguish:

- target command exit code
- reportage process exit code
- assertion failure
- runtime error
- script error
- semantic error, when relevant

Do not collapse all failures into a single generic failed case.

### 3. Verify exit code semantics

If exit code behavior changes, document and test the expected exit code.

The test must make clear whether the exit code belongs to:

- the target command
- the reportage CLI process
- a shell wrapper used by the test
- a fixture script used by the test

Avoid ambiguous assertions such as "the command fails" without stating which command and which process failed.

### 4. Verify output semantics

If stdout, stderr, diagnostics, or JSON output changes, add e2e assertions for the affected output.

The tests should verify:

- required output is present
- forbidden output is absent when relevant
- output is emitted to the correct stream
- structured output remains parseable when JSON is involved
- error output does not lose important diagnostic information

For JSON output, the e2e test should prefer structural assertions over brittle whole-string matching unless exact formatting is part of the contract.

### 5. Verify artifact and evidence changes

If artifacts or evidence output changes, e2e tests must check the generated files or directories.

The tests should verify:

- expected artifact paths exist
- unexpected artifact paths are not produced when relevant
- file contents match the intended contract
- failure cases still produce or suppress artifacts according to the specification

### 6. Keep success and failure examples inspectable

When possible, add examples or fixtures that clearly show:

- a successful execution case
- a target-command failure case
- an assertion failure case
- a runtime/script error case, if affected by the change

Prefer fixtures that can be read by future maintainers and AI agents without reconstructing the scenario from test code alone.

### 7. Run the current checks

Run the self-test suite that executes [`e2e/`](../../../e2e/) through the Cargo-built reportage binary:

```sh
cargo nextest run --locked -p reportage-cli --test self_test
```

Run the focused contract recipe when its output surface changes:

```sh
just json-report-fixtures-check
just run-result-fixtures-check
just references-index-check
just json-contract-schemas-check
```

Run `just json-contract-schemas-check` whenever a `schema.internal.json` changes: it owns schema document validity and the JSON Schema keyword coverage the fixture suites cannot provide.

Choose only the focused recipes relevant to the change, then finish with `just check`.
Do not invoke obsolete `just test-fixtures-valid` or `just test-fixtures-invalid` aliases; the current repository has no such recipes.

## Review checklist

Before considering the change complete, verify:

- [ ] The behavior change is covered by e2e tests.
- [ ] Success and failure command-execution cases both exist when command execution is involved.
- [ ] Target command exit code and reportage process exit code are not confused.
- [ ] stdout/stderr expectations are explicit when output changes.
- [ ] JSON output is structurally checked when relevant.
- [ ] Artifact/evidence output is checked when relevant.
- [ ] Failure classification is tested when relevant.
- [ ] Snapshots are updated intentionally, not accidentally.
- [ ] Documentation is updated if the user-visible contract changed.
- [ ] The current local reading order and references were used instead of version-tag documentation for another version.
- [ ] The self-test suite, relevant focused contract checks, and `just check` pass.

## Common mistakes

Avoid these mistakes:

- changing CLI behavior with only unit tests
- testing only successful command execution
- asserting only "non-zero" without checking the failure class
- confusing assertion failure with runtime error
- checking stdout while the actual contract is stderr
- snapshotting output without explaining the intended contract
- updating snapshots mechanically without reviewing what changed
- allowing output detail to regress below the normal human-readable output
