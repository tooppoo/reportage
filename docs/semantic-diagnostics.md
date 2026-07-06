# Semantic and Assertion Diagnostics

This document defines the diagnostic model and stable diagnostic code system for semantic errors and assertion failures produced by the semantic evaluator, and the format for verifying expected diagnostic codes in semantic conformance cases.

See [`20260702T133734Z_semantic-and-assertion-diagnostic-model.md`](adr/20260702T133734Z_semantic-and-assertion-diagnostic-model.md) for the decision record.

This document is a specification. It does not require immediate, full application to the parser, evaluator, or CLI diagnostic rendering; that application is handled by follow-up issues as needed.

## Relationship to Parse Diagnostics

[`diagnostics.md`](diagnostics.md) defines the `parse.*` namespace and the minimal diagnostic model for parser and validator errors. That document reserved a `semantic.*` namespace without defining it. This document defines the `semantic.*` and `assertion.*` namespaces and extends the diagnostic model with `severity`, `origin`, and a range-capable `location`.

This document does not redesign the `parse.*` codes or the parse-side diagnostic model. Applying the extended model (severity, origin, source ranges) to existing parse diagnostics is follow-up work.

## Top-Level Namespaces

Diagnostic codes are dot-separated strings, consistent with the `parse.*` convention. The top-level namespaces are:

- `parse.*` — syntax parse or parse-domain validation failures (defined in [`diagnostics.md`](diagnostics.md)).
- `semantic.*` — the script, normalized semantic model, or expectation definition is invalid, and the evaluator must reject it before evidence comparison can begin.
- `assertion.*` — the script and expectation are valid, but the observed evidence does not satisfy the expectation.
- `step.*` — a side-effecting step (e.g. `write`) is valid, but failed while actually running: a runtime step error. Unlike `assertion.*`, there is no expectation being compared against evidence; unlike `semantic.*`, the step was not rejected before it ran. See [`docs/semantics.md`](semantics.md) — Write step.

Uppercase prefix forms such as `RPT-ASSERT-EXIT-MISMATCH` are **not** adopted.

## Naming Convention

Semantic and assertion codes use the form:

```text
<namespace>.<subject>.<reason>
```

All segments are lowercase; multi-word segments use `snake_case`.

Examples:

```text
assertion.exit.mismatch
assertion.stdout.contains_mismatch
assertion.stderr.contains_mismatch
assertion.file.exists_missing
assertion.file.contains_mismatch
assertion.dir.exists_missing
assertion.dir.contains_entry_missing
semantic.file_path.absolute
semantic.file_path.dot_segment
semantic.dir_entry_name.empty
semantic.dir_entry_name.path_separator
semantic.dir_entry_name.dot_entry
semantic.dir_entry_name.control_char
semantic.expectation.unsupported
semantic.expectation.empty_block
semantic.workspace_path.empty
semantic.workspace_path.absolute
semantic.workspace_path.dot_segment
semantic.literal.kind_mismatch
step.write.target_exists
step.write.parent_not_a_directory
step.write.io_error
```

As with `parse.*` codes, a diagnostic code is **not** the Rust error enum variant name that produces it. Internal enum structure may be renamed or restructured freely; the published code string is the external, stable identifier that tests and tooling depend on.

## Semantic Errors vs. Assertion Failures

Reportage separates two failure kinds on the semantic evaluator side:

```text
semantic error:
  The script, normalized semantic model, or expectation definition is invalid, and the evaluator must reject it before entering evidence comparison.

assertion failure:
  The script and expectation are valid, but evidence acquisition or comparison shows the expectation is not satisfied.
```

Assertion failures **are** diagnostics. They are represented in the same diagnostic model as errors, distinguished by namespace and severity.

Classification examples:

- `exit 0` observed with actual exit code `1` — assertion failure.
- A path policy violation such as `file <"../secret.txt"> exists` — semantic error.
- An unsupported expectation form — semantic error, or a parse-domain validation error depending on where it is detected.
- A valid path whose target does not exist for `file <"path"> exists` — assertion failure.
- A valid path whose target does not exist for `file <"path"> contains "text"` — assertion failure.
- A valid path whose target is a directory or has non-UTF-8 content for `file <"path"> contains "text"` — assertion failure in principle, because the expectation itself is valid and the observed evidence fails the predicate's requirement.
- An invalid entry name such as `dir <"artifacts"> contains "a/b"` — semantic error, for the same reason as a path policy violation: the value violates a policy the evaluator must reject before evidence comparison.
- A literal of the wrong kind, such as `file "out.txt" exists` (a `StringLiteral` where the `file` subject requires a `WorkspacePath`) — semantic error (`semantic.literal.kind_mismatch`), detected during AST construction. The script parses at the grammar level; the diagnostic names the expected kind, the actual kind, and the suggested replacement (`use <"out.txt"> instead`). See [`docs/semantics.md`](semantics.md) — Value literals.
- A valid `dir <"path"> exists` whose target does not exist, or is a regular file rather than a directory — assertion failure.

This classification is a premise for the diagnostic design of file assertions (#24), logical composition (#25), and directory assertions (#66).

## Severity

Severity is classified as:

```text
error
  Parse / validation / semantic errors. The script or semantic model is invalid, and normal assertion evaluation cannot proceed.

failure
  Assertion failures. The script is valid, but evidence does not satisfy the expectation.

warning
  Reserved for future non-fatal diagnostics. Not required in v0.
```

Semantic errors have severity `error`. Assertion failures have severity `failure`. An assertion failure may count as a failed CI / test result, but that is distinct from the script or semantic model being invalid; the two must not be conflated into a single severity.

## Diagnostic Structure

The minimal diagnostic structure separates:

- `code` — the machine-readable, stable diagnostic code.
- `severity` — `error` / `failure` / `warning`.
- `message` — a human-readable view (not a stable contract; see below).
- `location` — a range in source text. Optional, because some input paths have no source text.
- `origin` — where the diagnostic came from: which input, spec, or case.
- `details` — structured auxiliary information defined per code.

A diagnostic carries an `origin` in principle. At minimum, a diagnostic without a `location` **must** carry an `origin`, so that no diagnostic is untraceable.

Example (source-derived):

```json
{
  "code": "assertion.exit.mismatch",
  "severity": "failure",
  "message": "expected exit code 0, but got 1",
  "origin": {
    "kind": "source",
    "source": "tests/example.rpt"
  },
  "location": {
    "source": "tests/example.rpt",
    "start": { "line": 3, "column": 3 },
    "end": { "line": 3, "column": 9 }
  },
  "details": {
    "expected": 0,
    "actual": 1
  }
}
```

Example (no source text — a semantic conformance case):

```json
{
  "code": "assertion.stdout.contains_mismatch",
  "severity": "failure",
  "message": "stdout did not contain expected substring",
  "origin": {
    "kind": "semantic_conformance_case",
    "spec_id": "stdout.contains",
    "rule_id": "stdout-contains-substring",
    "case_id": "stdout_contains_mismatch"
  },
  "location": null,
  "details": {
    "expected_substring": "PASS"
  }
}
```

## Location and Source Ranges

A diagnostic `location` is not a single line / column point. It is a model capable of expressing a **source range**, represented by start / end line / column positions:

```json
{
  "source": "tests/example.rpt",
  "start": { "line": 4, "column": 8 },
  "end": { "line": 4, "column": 23 }
}
```

The range points at the source node the diagnostic is about. Examples:

- a path policy violation — the range of the path literal;
- an unsupported expectation form — the range of the whole expectation;
- an assertion mismatch — the range of the expectation, or of the literal / predicate at the center of the problem.

`SourceRange.end` is an **exclusive** position: it points at the position immediately after the last character of the diagnosed range, not at the last character itself. This aligns with Rust ranges, string slices, and LSP ranges.

Reportage is a testing DSL, so locating a failed test quickly matters. For diagnostics generated from source DSL text — parse errors, validation errors, semantic errors, and assertion failures alike — the goal is to carry a `location` whenever a corresponding source node exists.

However, `location` is optional in the diagnostic model as a whole. Input paths such as semantic conformance cases have no `.rpt` source text, so no Reportage source location can be produced naturally. When `location` is absent, `origin` must carry identifying information — spec id / rule id / case id — so the failure site can still be tracked.

v0 does not require byte offsets in the stable contract. If needed, they are an implementation detail or a later extension.

## Origin

`origin` identifies which input, spec, or case a diagnostic came from.

- For source-derived diagnostics, `origin` identifies the source input (e.g. `{ "kind": "source", "source": "tests/example.rpt" }`).
- For semantic conformance cases, `origin` carries spec id / rule id / case id, so the failing case can be identified without any source location.

## Message Stability

`message` is a human-readable view, **not** a stable contract. Treating message text as stable would make every wording improvement a breaking change.

The stable contract is limited, in principle, to `code` and the stable `details` fields defined per code. Tests and tooling must not depend on full-message matches.

## Details Stability

The `details` field itself is part of the diagnostic model, but `details` as a whole is **not** unconditionally stable API. Stable fields are defined per diagnostic code:

```text
assertion.exit.mismatch:
  stable details:
    expected: number
    actual: number

assertion.stdout.contains_mismatch:
  stable details:
    expected_substring: string

assertion.file.exists_missing:
  stable details:
    path: string

assertion.file.contains_mismatch:
  stable details:
    path: string
    expected_substring: string

assertion.dir.exists_missing:
  stable details:
    path: string

assertion.dir.contains_entry_missing:
  stable details:
    path: string
    expected_entry: string

semantic.literal.kind_mismatch:
  stable details:
    raw_value: string       # the offending literal as written, e.g. "out.txt" or <"out.txt">
    expected_kind: string   # WorkspacePath | TextValue | StringLiteral
    actual_kind: string     # StringLiteral | WorkspacePath | FixtureReference
```

`semantic.literal.kind_mismatch` also carries a `suggestion` detail (the suggested replacement, e.g. `<"out.txt">`). Like `message`, `suggestion` is free-form, human-facing text that may be improved over time; it is **not** part of the stable details contract. The `expected_kind` / `actual_kind` names above are the stable kind identifiers.

Structured `expected` / `actual` values such as the above **are** part of the stable contract for the codes that define them.

The following are **not** part of the v0 stable details contract:

```text
rendered_message
debug_dump
pest_message
full_actual_stdout
full_actual_stderr
full_actual_file_content
filesystem_error_message
```

Full actual stdout / stderr / file content is excluded because it can be large, volatile, and may contain sensitive data.

## Expected Diagnostic Codes in Semantic Conformance Cases

A semantic conformance case may specify an expected diagnostic code via the optional `expectedDiagnosticCode` field (see [`spec/language/semantics/README.md`](../spec/language/semantics/README.md)):

```json
{
  "description": "stdout missing the expected substring fails",
  "expectedResult": "fail",
  "expectedDiagnosticCode": "assertion.stdout.contains_mismatch"
}
```

The value is a diagnostic code string as defined by this document. At minimum, the code string itself is verifiable; richer per-diagnostic expectations (multiple diagnostics, details assertions) may be added later without invalidating this form.

Sequencing with #30:

- #30 may proceed with pass/fail result verification independently of this contract; expected diagnostic code verification was not mandatory there before this contract existed.
- Now that the code format is defined, #30 or a follow-up issue can enable code verification for cases that specify `expectedDiagnosticCode`.
- Cases without an expected diagnostic code are verified by pass/fail result only.

## Logical Composition and Nested Diagnostics

The logical composition diagnostics of #25 will want to preserve each alternative's failure reason when all alternatives of `any` / `or` fail.

This document does **not** require implementing nested / child diagnostics. It does, however, keep room for a future structure such as:

```json
{
  "code": "assertion.any.all_failed",
  "severity": "failure",
  "message": "all alternatives failed",
  "origin": { "kind": "source", "source": "tests/example.rpt" },
  "location": {
    "source": "tests/example.rpt",
    "start": { "line": 5, "column": 3 },
    "end": { "line": 10, "column": 4 }
  },
  "details": {},
  "children": [
    {
      "code": "assertion.file.exists_missing",
      "severity": "failure",
      "details": { "path": "a.txt" }
    },
    {
      "code": "assertion.stdout.contains_mismatch",
      "severity": "failure",
      "details": { "expected_substring": "PASS" }
    }
  ]
}
```

Nothing in this document's model prevents adding a `children` field later.

## Compatibility Policy

Semantic and assertion diagnostic codes follow the same compatibility policy as `parse.*` codes (see [`diagnostics.md`](diagnostics.md#compatibility-policy)):

- Renaming or removing an existing code is a breaking change.
- Adding a new code is a non-breaking change.
- Improving `message` text is a non-breaking change.
- Adding stable details fields to a code is a non-breaking change; removing or renaming a stable details field is a breaking change for that code.
- Changing non-stable details contents is a non-breaking change.

## Out of Scope

- Full application to the parser / evaluator / CLI diagnostic rendering (follow-up issues, as needed).
- Redesign of the `parse.*` codes defined for #35.
- Mandatory implementation of nested / child diagnostics.
- Byte offsets in the stable location contract.
- LSP / editor integration, localization / i18n.
