# Semantic Rules

<!-- GENERATED FILE: do not edit directly. Regenerate with `just semantic-docs-gen`. -->

This is the generated semantic rule catalog. It is generated from `spec/language/semantics/*.json`. The JSON specs are the source of truth for each rule's normative fields and conformance cases; this catalog is a read-only view of that content.

The inventory of which semantic rules exist, and which ones require a spec, conformance cases, or a catalog entry here, is owned separately by the Rust const registry (`reportage_core::semantic_rule_registry::SEMANTIC_RULE_REGISTRY`), checked in CI by `just semantic-rule-coverage-check`. See `spec/language/semantics/README.md` and docs/adr/20260708T065700Z_semantic-rule-coverage-registry.md for the full source-of-truth split.

The conformance case lists below are read-only views derived from the JSON specs. Change the JSON specs, then regenerate this file.

Semantic conformance verifies the expected pass/fail result by passing the normalized assertion representation and checkpoint data from each JSON case to the semantic evaluator. Parser/source consistency is checked separately. The diagnostic code contract is defined in [`semantic-diagnostics.md`](../semantic-diagnostics.md); expected diagnostic code checks remain optional until semantic conformance enables code verification. Cases without diagnostic codes are verified by pass/fail result only.

## assertion.exit.equals

- Source: `spec/language/semantics/assertion.exit.equals.json`
- Syntax form: `exit <code>`
- Category: `assertion`

### Normative Fields

| Field | Value |
|---|---|
| checkpointField | `"exitCode"` |
| expectedValueType | `"uint8"` |
| matchSemantics | `{"comparison":"exact"}` |
| operator | `"equals"` |

### Conformance Cases

| Description | Assertion source | Normalized assertion | Checkpoint | Expected result | Expected diagnostic |
|---|---|---|---|---|---|
| exit code 0 matches expected 0 | `"exit 0"` | `{"expected":0,"operator":"equals","subject":"exit"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""}}` | `pass` | - |
| exit code 1 matches expected 1 | `"exit 1"` | `{"expected":1,"operator":"equals","subject":"exit"}` | `{"exitCode":1,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""}}` | `pass` | - |
| exit code 1 does not match expected 0 | `"exit 0"` | `{"expected":0,"operator":"equals","subject":"exit"}` | `{"exitCode":1,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""}}` | `fail` | - |
| exit code 0 does not match expected 1 | `"exit 1"` | `{"expected":1,"operator":"equals","subject":"exit"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""}}` | `fail` | - |
| non-zero exit code 42 matches expected 42 | `"exit 42"` | `{"expected":42,"operator":"equals","subject":"exit"}` | `{"exitCode":42,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""}}` | `pass` | - |

## assertion.stderr.contains

- Source: `spec/language/semantics/assertion.stderr.contains.json`
- Syntax form: `stderr contains <string>`
- Category: `assertion`

### Normative Fields

| Field | Value |
|---|---|
| checkpointField | `"stderr"` |
| expectedValueType | `"utf8String"` |
| matchSemantics | `{"caseSensitive":true,"comparison":"byteSubstring","emptyExpectedAlwaysMatches":true,"lineEndingNormalization":false}` |
| operator | `"contains"` |

### Conformance Cases

| Description | Assertion source | Normalized assertion | Checkpoint | Expected result | Expected diagnostic |
|---|---|---|---|---|---|
| stderr contains the expected string | `"stderr contains \\"error\\""` | `{"expected":"error","operator":"contains","subject":"stderr"}` | `{"exitCode":1,"stderr":{"data":"ZXJyb3IK","encoding":"base64","text":"error\\n"},"stdout":{"data":"","encoding":"base64","text":""}}` | `pass` | - |
| stderr does not contain the expected string | `"stderr contains \\"error\\""` | `{"expected":"error","operator":"contains","subject":"stderr"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""}}` | `fail` | - |
| empty expected string always matches any stderr | `"stderr contains \\"\\""` | `{"expected":"","operator":"contains","subject":"stderr"}` | `{"exitCode":1,"stderr":{"data":"ZXJyb3IK","encoding":"base64","text":"error\\n"},"stdout":{"data":"","encoding":"base64","text":""}}` | `pass` | - |
| empty expected string matches empty stderr | `"stderr contains \\"\\""` | `{"expected":"","operator":"contains","subject":"stderr"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""}}` | `pass` | - |
| match is case-sensitive; uppercase does not match lowercase expected | `"stderr contains \\"error\\""` | `{"expected":"error","operator":"contains","subject":"stderr"}` | `{"exitCode":1,"stderr":{"data":"RVJST1IK","encoding":"base64","text":"ERROR\\n"},"stdout":{"data":"","encoding":"base64","text":""}}` | `fail` | - |
| match finds the expected string as a substring in the middle of stderr | `"stderr contains \\"rro\\""` | `{"expected":"rro","operator":"contains","subject":"stderr"}` | `{"exitCode":1,"stderr":{"data":"ZXJyb3IK","encoding":"base64","text":"error\\n"},"stdout":{"data":"","encoding":"base64","text":""}}` | `pass` | - |
| line endings are not normalized; an LF expectation does not match CRLF-only stderr | `"stderr contains \\"a\\\\n\\""` | `{"expected":"a\\n","operator":"contains","subject":"stderr"}` | `{"exitCode":1,"stderr":{"data":"YQ0K","encoding":"base64","text":"a\\r\\n"},"stdout":{"data":"","encoding":"base64","text":""}}` | `fail` | - |

## assertion.stdout.contains

- Source: `spec/language/semantics/assertion.stdout.contains.json`
- Syntax form: `stdout contains <string>`
- Category: `assertion`

### Normative Fields

| Field | Value |
|---|---|
| checkpointField | `"stdout"` |
| expectedValueType | `"utf8String"` |
| matchSemantics | `{"caseSensitive":true,"comparison":"byteSubstring","emptyExpectedAlwaysMatches":true,"lineEndingNormalization":false}` |
| operator | `"contains"` |

### Conformance Cases

| Description | Assertion source | Normalized assertion | Checkpoint | Expected result | Expected diagnostic |
|---|---|---|---|---|---|
| stdout contains the expected string | `"stdout contains \\"hello\\""` | `{"expected":"hello","operator":"contains","subject":"stdout"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"aGVsbG8K","encoding":"base64","text":"hello\\n"}}` | `pass` | - |
| stdout does not contain the expected string | `"stdout contains \\"hello\\""` | `{"expected":"hello","operator":"contains","subject":"stdout"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"ZXJyb3IK","encoding":"base64","text":"error\\n"}}` | `fail` | - |
| empty expected string always matches any stdout | `"stdout contains \\"\\""` | `{"expected":"","operator":"contains","subject":"stdout"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"aGVsbG8K","encoding":"base64","text":"hello\\n"}}` | `pass` | - |
| empty expected string matches empty stdout | `"stdout contains \\"\\""` | `{"expected":"","operator":"contains","subject":"stdout"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""}}` | `pass` | - |
| match is case-sensitive; uppercase does not match lowercase expected | `"stdout contains \\"hello\\""` | `{"expected":"hello","operator":"contains","subject":"stdout"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"SEVMTE8K","encoding":"base64","text":"HELLO\\n"}}` | `fail` | - |
| match finds the expected string as a substring in the middle of stdout | `"stdout contains \\"ell\\""` | `{"expected":"ell","operator":"contains","subject":"stdout"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"aGVsbG8K","encoding":"base64","text":"hello\\n"}}` | `pass` | - |
| line endings are not normalized; an LF expectation does not match CRLF-only stdout | `"stdout contains \\"a\\\\n\\""` | `{"expected":"a\\n","operator":"contains","subject":"stdout"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"YQ0K","encoding":"base64","text":"a\\r\\n"}}` | `fail` | - |
| non-UTF-8 stdout is not rejected; byte substring match still finds a valid-UTF-8 prefix | `"stdout contains \\"ok\\""` | `{"expected":"ok","operator":"contains","subject":"stdout"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"b2vD","encoding":"base64"}}` | `pass` | - |

