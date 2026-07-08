# Semantic Rules

<!-- GENERATED FILE: do not edit directly. Regenerate with `just semantic-docs-gen`. -->

This is the generated semantic rule catalog. It is generated from `spec/language/semantics/*.json`. The JSON specs are the source of truth for each rule's normative fields and conformance cases; this catalog is a read-only view of that content.

The inventory of which semantic rules exist, and which ones require a spec, conformance cases, or a catalog entry here, is owned separately by the Rust const registry (`reportage_core::semantic_rule_registry::SEMANTIC_RULE_REGISTRY`), checked in CI by `just semantic-rule-coverage-check`. See `spec/language/semantics/README.md` and docs/adr/20260708T065700Z_semantic-rule-coverage-registry.md for the full source-of-truth split.

The conformance case lists below are read-only views derived from the JSON specs. Change the JSON specs, then regenerate this file.

Semantic conformance verifies the expected pass/fail result by passing the normalized assertion representation and checkpoint data from each JSON case to the semantic evaluator. Parser/source consistency is checked separately. The diagnostic code contract is defined in [`semantic-diagnostics.md`](../semantic-diagnostics.md); expected diagnostic code checks remain optional until semantic conformance enables code verification. Cases without diagnostic codes are verified by pass/fail result only.

## assertion.dir.contains

- Source: `spec/language/semantics/assertion.dir.contains.json`
- Syntax form: `dir <"path"> contains "name"`
- Category: `assertion`

### Normative Fields

| Field | Value |
|---|---|
| checkpointField | `"dir"` |
| expectedValueType | `"utf8String"` |
| matchSemantics | `{"comparison":"entryNameEquality"}` |
| operator | `"contains"` |
| referencedValueReferenceRule | `"value-reference.workspace-path.resolve"` |

### Conformance Cases

| Description | Assertion source | Normalized assertion | Checkpoint | Expected result | Expected diagnostic |
|---|---|---|---|---|---|
| dir contains passes when an entry with the exact name exists directly under the path | `"dir <\\"outdir\\"> contains \\"result.json\\""` | `{"expected":"result.json","operator":"contains","path":"outdir","subject":"dir"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"files":[{"contents":{"data":"","encoding":"base64","text":""},"path":"outdir/result.json"}]}}` | `pass` | - |
| dir contains fails when no entry has that exact name | `"dir <\\"outdir\\"> contains \\"missing.json\\""` | `{"expected":"missing.json","operator":"contains","path":"outdir","subject":"dir"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"files":[{"contents":{"data":"","encoding":"base64","text":""},"path":"outdir/result.json"}]}}` | `fail` | `assertion.dir.contains_entry_missing` |
| dir contains matches the full entry name only, never a substring of it | `"dir <\\"outdir\\"> contains \\"result\\""` | `{"expected":"result","operator":"contains","path":"outdir","subject":"dir"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"files":[{"contents":{"data":"","encoding":"base64","text":""},"path":"outdir/result.json"}]}}` | `fail` | `assertion.dir.contains_entry_missing` |
| dir contains fails when the subject path does not exist | `"dir <\\"missing_dir\\"> contains \\"x\\""` | `{"expected":"x","operator":"contains","path":"missing_dir","subject":"dir"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""}}` | `fail` | `assertion.dir.contains_subject_missing` |
| dir contains fails when the subject path is a regular file, not a directory | `"dir <\\"notadir\\"> contains \\"x\\""` | `{"expected":"x","operator":"contains","path":"notadir","subject":"dir"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"files":[{"contents":{"data":"","encoding":"base64","text":""},"path":"notadir"}]}}` | `fail` | `assertion.dir.contains_subject_not_directory` |

## assertion.dir.exists

- Source: `spec/language/semantics/assertion.dir.exists.json`
- Syntax form: `dir <"path"> exists`
- Category: `assertion`

### Normative Fields

| Field | Value |
|---|---|
| checkpointField | `"dir"` |
| expectedValueType | `"none"` |
| matchSemantics | `{"comparison":"existence"}` |
| operator | `"exists"` |
| referencedValueReferenceRule | `"value-reference.workspace-path.resolve"` |

### Conformance Cases

| Description | Assertion source | Normalized assertion | Checkpoint | Expected result | Expected diagnostic |
|---|---|---|---|---|---|
| dir exists passes when the path is a directory | `"dir <\\"outdir\\"> exists"` | `{"operator":"exists","path":"outdir","subject":"dir"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"dirs":["outdir"]}}` | `pass` | - |
| dir exists fails when the path does not exist | `"dir <\\"missing_dir\\"> exists"` | `{"operator":"exists","path":"missing_dir","subject":"dir"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""}}` | `fail` | `assertion.dir.exists_missing` |
| dir exists fails when the path is a regular file, not a directory | `"dir <\\"notadir\\"> exists"` | `{"operator":"exists","path":"notadir","subject":"dir"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"files":[{"contents":{"data":"","encoding":"base64","text":""},"path":"notadir"}]}}` | `fail` | `assertion.dir.exists_not_directory` |

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

## assertion.file.contains

- Source: `spec/language/semantics/assertion.file.contains.json`
- Syntax form: `file <"path"> contains <text_literal>`
- Category: `assertion`

### Normative Fields

| Field | Value |
|---|---|
| checkpointField | `"file"` |
| expectedValueType | `"utf8String"` |
| matchSemantics | `{"caseSensitive":true,"comparison":"textSubstring","emptyExpectedAlwaysMatches":true,"lineEndingNormalization":false}` |
| operator | `"contains"` |

### Conformance Cases

| Description | Assertion source | Normalized assertion | Checkpoint | Expected result | Expected diagnostic |
|---|---|---|---|---|---|
| file contains passes when the expected text is a substring of the file's decoded UTF-8 text | `"file <\\"actual.txt\\"> contains \\"world\\""` | `{"expected":"world","operator":"contains","path":"actual.txt","subject":"file"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"files":[{"contents":{"data":"aGVsbG8gd29ybGQ=","encoding":"base64","text":"hello world"},"path":"actual.txt"}]}}` | `pass` | - |
| file contains fails when the expected text is not a substring | `"file <\\"actual.txt\\"> contains \\"xyz\\""` | `{"expected":"xyz","operator":"contains","path":"actual.txt","subject":"file"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"files":[{"contents":{"data":"aGVsbG8gd29ybGQ=","encoding":"base64","text":"hello world"},"path":"actual.txt"}]}}` | `fail` | `assertion.file.contains_mismatch` |
| empty expected text always matches an existing, readable, UTF-8 file | `"file <\\"actual.txt\\"> contains \\"\\""` | `{"expected":"","operator":"contains","path":"actual.txt","subject":"file"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"files":[{"contents":{"data":"aGVsbG8=","encoding":"base64","text":"hello"},"path":"actual.txt"}]}}` | `pass` | - |
| file contains fails when the file does not exist; missing/unreadable/non-UTF-8 are a single precondition-unmet failure category | `"file <\\"missing.txt\\"> contains \\"x\\""` | `{"expected":"x","operator":"contains","path":"missing.txt","subject":"file"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""}}` | `fail` | `assertion.file.contains_precondition_unmet` |
| match is case-sensitive; uppercase file content does not match a lowercase expected substring | `"file <\\"actual.txt\\"> contains \\"hello\\""` | `{"expected":"hello","operator":"contains","path":"actual.txt","subject":"file"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"files":[{"contents":{"data":"SEVMTE8=","encoding":"base64","text":"HELLO"},"path":"actual.txt"}]}}` | `fail` | `assertion.file.contains_mismatch` |

## assertion.file.contents_equals

- Source: `spec/language/semantics/assertion.file.contents_equals.json`
- Syntax form: `file <ActualValue<WorkspacePath>> contents_equals <ExpectedValue<FileContentsReference>>`
- Category: `assertion`

### Normative Fields

| Field | Value |
|---|---|
| checkpointField | `"file"` |
| expectedValueType | `"fileContentsReference"` |
| matchSemantics | `{"comparison":"byteExact"}` |
| operator | `"contentsEquals"` |
| referencedValueReferenceRule | `"value-reference.file-contents-reference.resolve"` |

### Conformance Cases

| Description | Assertion source | Normalized assertion | Checkpoint | Expected result | Expected diagnostic |
|---|---|---|---|---|---|
| contents_equals passes on a byte-for-byte match against a workspace expected file | `"file <\\"actual.txt\\"> contents_equals <\\"expected.txt\\">"` | `{"expected":{"kind":"workspacePath","value":"expected.txt"},"operator":"contentsEquals","path":"actual.txt","subject":"file"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"files":[{"contents":{"data":"aGVsbG8=","encoding":"base64","text":"hello"},"path":"actual.txt"},{"contents":{"data":"aGVsbG8=","encoding":"base64","text":"hello"},"path":"expected.txt"}]}}` | `pass` | - |
| contents_equals fails on a byte mismatch against a workspace expected file | `"file <\\"actual.txt\\"> contents_equals <\\"expected.txt\\">"` | `{"expected":{"kind":"workspacePath","value":"expected.txt"},"operator":"contentsEquals","path":"actual.txt","subject":"file"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"files":[{"contents":{"data":"aGVsbG8=","encoding":"base64","text":"hello"},"path":"actual.txt"},{"contents":{"data":"d29ybGQ=","encoding":"base64","text":"world"},"path":"expected.txt"}]}}` | `fail` | `assertion.file.contents_equals_mismatch` |
| contents_equals fails on a missing trailing newline; comparison is byte-exact with no normalization | `"file <\\"actual.txt\\"> contents_equals <\\"expected.txt\\">"` | `{"expected":{"kind":"workspacePath","value":"expected.txt"},"operator":"contentsEquals","path":"actual.txt","subject":"file"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"files":[{"contents":{"data":"aGVsbG8=","encoding":"base64","text":"hello"},"path":"actual.txt"},{"contents":{"data":"aGVsbG8K","encoding":"base64","text":"hello\\n"},"path":"expected.txt"}]}}` | `fail` | `assertion.file.contents_equals_mismatch` |
| contents_equals fails as an assertion failure, not a script error, when the actual file is missing | `"file <\\"actual.txt\\"> contents_equals <\\"expected.txt\\">"` | `{"expected":{"kind":"workspacePath","value":"expected.txt"},"operator":"contentsEquals","path":"actual.txt","subject":"file"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"files":[{"contents":{"data":"aGVsbG8=","encoding":"base64","text":"hello"},"path":"expected.txt"}]}}` | `fail` | `assertion.file.contents_equals_actual_missing` |
| contents_equals passes against a fixture-file expected value, resolved relative to repor_dir | `"file <\\"actual.txt\\"> contents_equals @\\"expected.txt\\""` | `{"expected":{"kind":"fixtureReference","value":"expected.txt"},"operator":"contentsEquals","path":"actual.txt","subject":"file"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"files":[{"contents":{"data":"aGVsbG8=","encoding":"base64","text":"hello"},"path":"actual.txt"}],"reporDirFiles":[{"contents":{"data":"aGVsbG8=","encoding":"base64","text":"hello"},"path":"expected.txt"}]}}` | `pass` | - |
| contents_equals is a script error, not an assertion failure, when the workspace expected value is missing | `"file <\\"actual.txt\\"> contents_equals <\\"missing-expected.txt\\">"` | `{"expected":{"kind":"workspacePath","value":"missing-expected.txt"},"operator":"contentsEquals","path":"actual.txt","subject":"file"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"files":[{"contents":{"data":"aGVsbG8=","encoding":"base64","text":"hello"},"path":"actual.txt"}]}}` | `scriptError` | `semantic.file_contents_reference.missing` |
| invalid: contents_equals rejects a bare string literal as the expected value | `"file <\\"actual.txt\\"> contents_equals \\"expected text\\""` | - | - | `parseError` | `semantic.literal.kind_mismatch` |

## assertion.file.exists

- Source: `spec/language/semantics/assertion.file.exists.json`
- Syntax form: `file <"path"> exists`
- Category: `assertion`

### Normative Fields

| Field | Value |
|---|---|
| checkpointField | `"file"` |
| expectedValueType | `"none"` |
| matchSemantics | `{"comparison":"existence"}` |
| operator | `"exists"` |

### Conformance Cases

| Description | Assertion source | Normalized assertion | Checkpoint | Expected result | Expected diagnostic |
|---|---|---|---|---|---|
| file exists passes when the path is a regular file | `"file <\\"actual.txt\\"> exists"` | `{"operator":"exists","path":"actual.txt","subject":"file"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"files":[{"contents":{"data":"aGVsbG8=","encoding":"base64","text":"hello"},"path":"actual.txt"}]}}` | `pass` | - |
| file exists fails when the path does not exist | `"file <\\"missing.txt\\"> exists"` | `{"operator":"exists","path":"missing.txt","subject":"file"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""}}` | `fail` | `assertion.file.exists_missing` |
| file exists fails when the path is a directory, not a regular file | `"file <\\"adir\\"> exists"` | `{"operator":"exists","path":"adir","subject":"file"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"dirs":["adir"]}}` | `fail` | `assertion.file.exists_not_a_file` |

## assertion.file.text_equals

- Source: `spec/language/semantics/assertion.file.text_equals.json`
- Syntax form: `file <ActualValue<WorkspacePath>> text_equals <ExpectedValue<TextValue>>`
- Category: `assertion`

### Normative Fields

| Field | Value |
|---|---|
| checkpointField | `"file"` |
| expectedValueType | `"utf8String"` |
| matchSemantics | `{"comparison":"byteExact"}` |
| noImplicitConversionFrom | `["fileContentsReference"]` |
| operator | `"textEquals"` |

### Conformance Cases

| Description | Assertion source | Normalized assertion | Checkpoint | Expected result | Expected diagnostic |
|---|---|---|---|---|---|
| text_equals passes on a byte-for-byte match against a string literal | `"file <\\"actual.txt\\"> text_equals \\"hello\\\\n\\""` | `{"expected":"hello\\n","operator":"textEquals","path":"actual.txt","subject":"file"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"files":[{"contents":{"data":"aGVsbG8K","encoding":"base64","text":"hello\\n"},"path":"actual.txt"}]}}` | `pass` | - |
| text_equals fails on a missing trailing newline; comparison is byte-exact with no normalization | `"file <\\"actual.txt\\"> text_equals \\"hello\\""` | `{"expected":"hello","operator":"textEquals","path":"actual.txt","subject":"file"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"files":[{"contents":{"data":"aGVsbG8K","encoding":"base64","text":"hello\\n"},"path":"actual.txt"}]}}` | `fail` | `assertion.file.text_equals_mismatch` |
| text_equals fails when actual is CRLF and expected is LF; no line-ending normalization is applied | `"file <\\"actual.txt\\"> text_equals \\"hello\\\\n\\""` | `{"expected":"hello\\n","operator":"textEquals","path":"actual.txt","subject":"file"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"files":[{"contents":{"data":"aGVsbG8NCg==","encoding":"base64","text":"hello\\r\\n"},"path":"actual.txt"}]}}` | `fail` | `assertion.file.text_equals_mismatch` |
| text_equals is an assertion failure, not a script error, when the actual file is missing | `"file <\\"missing.txt\\"> text_equals \\"hello\\""` | `{"expected":"hello","operator":"textEquals","path":"missing.txt","subject":"file"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""}}` | `fail` | `assertion.file.text_equals_actual_missing` |
| invalid: text_equals rejects a workspace path literal as the expected value; TextValue and FileContentsReference do not implicitly convert | `"file <\\"actual.txt\\"> text_equals <\\"expected.txt\\">"` | - | - | `parseError` | `semantic.literal.kind_mismatch` |
| invalid: text_equals rejects a fixture reference literal as the expected value; TextValue and FileContentsReference do not implicitly convert | `"file <\\"actual.txt\\"> text_equals @\\"expected.txt\\""` | - | - | `parseError` | `semantic.literal.kind_mismatch` |

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

## assertion.stderr.empty

- Source: `spec/language/semantics/assertion.stderr.empty.json`
- Syntax form: `stderr empty`
- Category: `assertion`

### Normative Fields

| Field | Value |
|---|---|
| checkpointField | `"stderr"` |
| expectedValueType | `"none"` |
| matchSemantics | `{"comparison":"emptiness"}` |
| operator | `"empty"` |

### Conformance Cases

| Description | Assertion source | Normalized assertion | Checkpoint | Expected result | Expected diagnostic |
|---|---|---|---|---|---|
| stderr empty passes when stderr captured zero bytes | `"stderr empty"` | `{"operator":"empty","subject":"stderr"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""}}` | `pass` | - |
| stderr empty fails when stderr contains any text | `"stderr empty"` | `{"operator":"empty","subject":"stderr"}` | `{"exitCode":1,"stderr":{"data":"aGVsbG8K","encoding":"base64","text":"hello\\n"},"stdout":{"data":"","encoding":"base64","text":""}}` | `fail` | `assertion.stderr.not_empty` |
| stderr empty fails on a single newline byte; whitespace-only output is still output | `"stderr empty"` | `{"operator":"empty","subject":"stderr"}` | `{"exitCode":1,"stderr":{"data":"Cg==","encoding":"base64","text":"\\n"},"stdout":{"data":"","encoding":"base64","text":""}}` | `fail` | `assertion.stderr.not_empty` |
| stderr empty fails on a single space byte | `"stderr empty"` | `{"operator":"empty","subject":"stderr"}` | `{"exitCode":1,"stderr":{"data":"IA==","encoding":"base64","text":" "},"stdout":{"data":"","encoding":"base64","text":""}}` | `fail` | `assertion.stderr.not_empty` |

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

## assertion.stdout.empty

- Source: `spec/language/semantics/assertion.stdout.empty.json`
- Syntax form: `stdout empty`
- Category: `assertion`

### Normative Fields

| Field | Value |
|---|---|
| checkpointField | `"stdout"` |
| expectedValueType | `"none"` |
| matchSemantics | `{"comparison":"emptiness"}` |
| operator | `"empty"` |

### Conformance Cases

| Description | Assertion source | Normalized assertion | Checkpoint | Expected result | Expected diagnostic |
|---|---|---|---|---|---|
| stdout empty passes when stdout captured zero bytes | `"stdout empty"` | `{"operator":"empty","subject":"stdout"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""}}` | `pass` | - |
| stdout empty fails when stdout contains any text | `"stdout empty"` | `{"operator":"empty","subject":"stdout"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"aGVsbG8K","encoding":"base64","text":"hello\\n"}}` | `fail` | `assertion.stdout.not_empty` |
| stdout empty fails on a single newline byte; whitespace-only output is still output | `"stdout empty"` | `{"operator":"empty","subject":"stdout"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"Cg==","encoding":"base64","text":"\\n"}}` | `fail` | `assertion.stdout.not_empty` |
| stdout empty fails on a single space byte | `"stdout empty"` | `{"operator":"empty","subject":"stdout"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"IA==","encoding":"base64","text":" "}}` | `fail` | `assertion.stdout.not_empty` |

## logical-composition.expectation.all

- Source: `spec/language/semantics/logical-composition.expectation.all.json`
- Syntax form: `all { <expectation>... }`
- Category: `logical-composition`

### Normative Fields

| Field | Value |
|---|---|
| emptyBlockDiagnosticCode | `"semantic.expectation.empty_block"` |
| emptyBlockPolicy | `"semanticError"` |
| evaluatesAllChildren | `true` |
| operator | `"all"` |
| passCondition | `"allChildrenPassed"` |

### Conformance Cases

| Description | Assertion source | Normalized assertion | Checkpoint | Expected result | Expected diagnostic |
|---|---|---|---|---|---|
| all passes when every child passes | `"all {\\n      exit 0\\n      stdout empty\\n    }"` | `{"children":[{"expected":0,"operator":"equals","subject":"exit"},{"operator":"empty","subject":"stdout"}],"operator":"all","subject":"logical"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""}}` | `pass` | - |
| all fails when one child fails, even though another child passes | `"all {\\n      exit 0\\n      stdout contains \\"nope\\"\\n    }"` | `{"children":[{"expected":0,"operator":"equals","subject":"exit"},{"expected":"nope","operator":"contains","subject":"stdout"}],"operator":"all","subject":"logical"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"aGVsbG8K","encoding":"base64","text":"hello\\n"}}` | `fail` | - |
| invalid: an empty all block is a semantic error, not evaluated as vacuously true | `"all {\\n    }"` | - | - | `parseError` | `semantic.expectation.empty_block` |

## logical-composition.expectation.any

- Source: `spec/language/semantics/logical-composition.expectation.any.json`
- Syntax form: `any { <expectation>... }`
- Category: `logical-composition`

### Normative Fields

| Field | Value |
|---|---|
| emptyBlockDiagnosticCode | `"semantic.expectation.empty_block"` |
| emptyBlockPolicy | `"semanticError"` |
| evaluatesAllChildren | `true` |
| operator | `"any"` |
| passCondition | `"anyChildPassed"` |

### Conformance Cases

| Description | Assertion source | Normalized assertion | Checkpoint | Expected result | Expected diagnostic |
|---|---|---|---|---|---|
| any passes when at least one child passes | `"any {\\n      exit 0\\n      exit 1\\n    }"` | `{"children":[{"expected":0,"operator":"equals","subject":"exit"},{"expected":1,"operator":"equals","subject":"exit"}],"operator":"any","subject":"logical"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""}}` | `pass` | - |
| any fails when every child fails | `"any {\\n      exit 1\\n      exit 2\\n    }"` | `{"children":[{"expected":1,"operator":"equals","subject":"exit"},{"expected":2,"operator":"equals","subject":"exit"}],"operator":"any","subject":"logical"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""}}` | `fail` | - |
| invalid: an empty any block is a semantic error, not evaluated as vacuously false | `"any {\\n    }"` | - | - | `parseError` | `semantic.expectation.empty_block` |

## logical-composition.expectation.not

- Source: `spec/language/semantics/logical-composition.expectation.not.json`
- Syntax form: `not { <expectation>... }`
- Category: `logical-composition`

### Normative Fields

| Field | Value |
|---|---|
| emptyBlockDiagnosticCode | `"semantic.expectation.empty_block"` |
| emptyBlockPolicy | `"semanticError"` |
| evaluatesAllChildren | `true` |
| operator | `"not"` |
| passCondition | `"notAllChildrenPassed"` |

### Conformance Cases

| Description | Assertion source | Normalized assertion | Checkpoint | Expected result | Expected diagnostic |
|---|---|---|---|---|---|
| not passes when its single child fails | `"not { exit 1 }"` | `{"children":[{"expected":1,"operator":"equals","subject":"exit"}],"operator":"not","subject":"logical"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""}}` | `pass` | - |
| not fails when its single child passes | `"not { exit 0 }"` | `{"children":[{"expected":0,"operator":"equals","subject":"exit"}],"operator":"not","subject":"logical"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""}}` | `fail` | - |
| not with multiple children negates the group as a whole (implicit all), not each child individually: one child passes and the other fails, so the group fails and not passes | `"not {\\n      exit 0\\n      stdout contains \\"nope\\"\\n    }"` | `{"children":[{"expected":0,"operator":"equals","subject":"exit"},{"expected":"nope","operator":"contains","subject":"stdout"}],"operator":"not","subject":"logical"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"aGVsbG8K","encoding":"base64","text":"hello\\n"}}` | `pass` | - |
| invalid: an empty not block is a semantic error, not evaluated as vacuously true or false | `"not {\\n    }"` | - | - | `parseError` | `semantic.expectation.empty_block` |

## value-reference.file-contents-reference.resolve

- Source: `spec/language/semantics/value-reference.file-contents-reference.resolve.json`
- Syntax form: `<"path"> | @"path"`
- Category: `value-reference`

### Normative Fields

| Field | Value |
|---|---|
| diagnosticCodes | `{"workspacePathMissing":"semantic.file_contents_reference.missing","workspacePathNotARegularFile":"semantic.file_contents_reference.not_regular_file","workspacePathReadError":"semantic.file_contents_reference.read_error"}` |
| fixtureReferenceSyntax | `"@\\"path\\""` |
| fixtureVariantDiagnosticsOwnedBy | `"value-reference.fixture-reference.resolve"` |
| kind | `"union"` |
| usedBy | `["assertion.file.contents_equals"]` |
| variants | `["workspacePath","fixtureReference"]` |
| workspacePathSyntax | `"<\\"path\\">"` |

### Conformance Cases

| Description | Assertion source | Normalized assertion | Checkpoint | Expected result | Expected diagnostic |
|---|---|---|---|---|---|
| resolves to bytes when the workspace variant points at an existing regular file | `"file <\\"actual.txt\\"> contents_equals <\\"expected.txt\\">"` | `{"expected":{"kind":"workspacePath","value":"expected.txt"},"operator":"contentsEquals","path":"actual.txt","subject":"file"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"files":[{"contents":{"data":"aGVsbG8=","encoding":"base64","text":"hello"},"path":"actual.txt"},{"contents":{"data":"aGVsbG8=","encoding":"base64","text":"hello"},"path":"expected.txt"}]}}` | `pass` | - |
| invalid: the workspace variant is a script error when the expected workspace path does not exist | `"file <\\"actual.txt\\"> contents_equals <\\"missing-expected.txt\\">"` | `{"expected":{"kind":"workspacePath","value":"missing-expected.txt"},"operator":"contentsEquals","path":"actual.txt","subject":"file"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"files":[{"contents":{"data":"aGVsbG8=","encoding":"base64","text":"hello"},"path":"actual.txt"}]}}` | `scriptError` | `semantic.file_contents_reference.missing` |
| invalid: the workspace variant is a script error when the expected workspace path is a directory, not a regular file | `"file <\\"actual.txt\\"> contents_equals <\\"expected.txt\\">"` | `{"expected":{"kind":"workspacePath","value":"expected.txt"},"operator":"contentsEquals","path":"actual.txt","subject":"file"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"dirs":["expected.txt"],"files":[{"contents":{"data":"aGVsbG8=","encoding":"base64","text":"hello"},"path":"actual.txt"}]}}` | `scriptError` | `semantic.file_contents_reference.not_regular_file` |

## value-reference.fixture-reference.resolve

- Source: `spec/language/semantics/value-reference.fixture-reference.resolve.json`
- Syntax form: `@"path"`
- Category: `value-reference`

### Normative Fields

| Field | Value |
|---|---|
| diagnosticCodes | `{"absolute":"semantic.fixture_reference.absolute","dotSegment":"semantic.fixture_reference.dot_segment","empty":"semantic.fixture_reference.empty","escapesReporDirectory":"semantic.fixture_reference.escapes_repor_directory","missing":"semantic.fixture_reference.missing","notARegularFile":"semantic.fixture_reference.not_a_regular_file"}` |
| lexicalRejects | `["empty","absolute","dotSegment"]` |
| resolutionRejects | `["missing","notARegularFile","escapesReporDirectory"]` |
| resolvedRelativeTo | `"reporDir"` |
| usedBy | `["assertion.file.contents_equals"]` |

### Conformance Cases

| Description | Assertion source | Normalized assertion | Checkpoint | Expected result | Expected diagnostic |
|---|---|---|---|---|---|
| a relative fixture reference with no dot segments resolves | `"file <\\"actual.txt\\"> contents_equals @\\"expected.txt\\""` | - | - | `valid` | - |
| invalid: an empty fixture reference is rejected | `"file <\\"actual.txt\\"> contents_equals @\\"\\""` | - | - | `parseError` | `semantic.fixture_reference.empty` |
| invalid: an absolute fixture reference is rejected | `"file <\\"actual.txt\\"> contents_equals @\\"/etc/passwd\\""` | - | - | `parseError` | `semantic.fixture_reference.absolute` |
| invalid: a fixture reference with a '..' segment is rejected | `"file <\\"actual.txt\\"> contents_equals @\\"a/../b\\""` | - | - | `parseError` | `semantic.fixture_reference.dot_segment` |
| a fixture reference resolves and materializes when the fixture file exists next to repor_dir | `"file <\\"actual.txt\\"> contents_equals @\\"expected.txt\\""` | `{"expected":{"kind":"fixtureReference","value":"expected.txt"},"operator":"contentsEquals","path":"actual.txt","subject":"file"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"files":[{"contents":{"data":"aGVsbG8=","encoding":"base64","text":"hello"},"path":"actual.txt"}],"reporDirFiles":[{"contents":{"data":"aGVsbG8=","encoding":"base64","text":"hello"},"path":"expected.txt"}]}}` | `pass` | - |
| invalid: a fixture reference is a script error when the fixture file does not exist next to repor_dir | `"file <\\"actual.txt\\"> contents_equals @\\"expected.txt\\""` | `{"expected":{"kind":"fixtureReference","value":"expected.txt"},"operator":"contentsEquals","path":"actual.txt","subject":"file"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"files":[{"contents":{"data":"aGVsbG8=","encoding":"base64","text":"hello"},"path":"actual.txt"}]}}` | `scriptError` | `semantic.fixture_reference.missing` |
| invalid: a fixture reference is a script error when it resolves to a directory, not a regular file | `"file <\\"actual.txt\\"> contents_equals @\\"expected.txt\\""` | `{"expected":{"kind":"fixtureReference","value":"expected.txt"},"operator":"contentsEquals","path":"actual.txt","subject":"file"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"files":[{"contents":{"data":"aGVsbG8=","encoding":"base64","text":"hello"},"path":"actual.txt"}],"reporDirDirs":["expected.txt"]}}` | `scriptError` | `semantic.fixture_reference.not_a_regular_file` |
| invalid: a fixture reference is a script error when a symlink planted under repor_dir makes it escape repor_dir, even though the reference itself has no '.'/'..' segment | `"file <\\"actual.txt\\"> contents_equals @\\"escape/secret.txt\\""` | `{"expected":{"kind":"fixtureReference","value":"escape/secret.txt"},"operator":"contentsEquals","path":"actual.txt","subject":"file"}` | `{"exitCode":0,"stderr":{"data":"","encoding":"base64","text":""},"stdout":{"data":"","encoding":"base64","text":""},"workspace":{"files":[{"contents":{"data":"aGVsbG8=","encoding":"base64","text":"hello"},"path":"actual.txt"}],"reporDirSymlinks":[{"outsideDirFiles":[{"contents":{"data":"c2VjcmV0","encoding":"base64","text":"secret"},"path":"secret.txt"}],"path":"escape"}]}}` | `scriptError` | `semantic.fixture_reference.escapes_repor_directory` |

## value-reference.literal.kind-mismatch

- Source: `spec/language/semantics/value-reference.literal.kind-mismatch.json`
- Syntax form: `"..." | <"..."> | @"..."`
- Category: `value-reference`

### Normative Fields

| Field | Value |
|---|---|
| diagnosticCode | `"semantic.literal.kind_mismatch"` |
| enforcedAt | `"parseTime"` |
| literalKinds | `["stringLiteral","workspacePath","fixtureReference"]` |
| mechanism | `"argumentPositionKindCheck"` |

### Conformance Cases

| Description | Assertion source | Normalized assertion | Checkpoint | Expected result | Expected diagnostic |
|---|---|---|---|---|---|
| a workspace path literal in a contents_equals expected-value position is valid | `"file <\\"actual.txt\\"> contents_equals <\\"expected.txt\\">"` | - | - | `valid` | - |
| invalid: a bare string literal in a contents_equals expected-value position is rejected; contents_equals requires a FileContentsReference | `"file <\\"actual.txt\\"> contents_equals \\"expected text\\""` | - | - | `parseError` | `semantic.literal.kind_mismatch` |
| invalid: a workspace path literal in a text_equals expected-value position is rejected; text_equals requires a TextValue | `"file <\\"actual.txt\\"> text_equals <\\"expected.txt\\">"` | - | - | `parseError` | `semantic.literal.kind_mismatch` |
| invalid: a fixture reference literal in a text_equals expected-value position is rejected; text_equals requires a TextValue | `"file <\\"actual.txt\\"> text_equals @\\"expected.txt\\""` | - | - | `parseError` | `semantic.literal.kind_mismatch` |
| invalid: a workspace path literal in a dir contains entry-name position is rejected; the entry name requires a plain string literal | `"dir <\\"out\\"> contains <\\"entry\\">"` | - | - | `parseError` | `semantic.literal.kind_mismatch` |
| invalid: a bare string literal as the file checkpoint subject is rejected; the subject requires a workspace path literal | `"file \\"out.txt\\" exists"` | - | - | `parseError` | `semantic.literal.kind_mismatch` |

## value-reference.workspace-path.resolve

- Source: `spec/language/semantics/value-reference.workspace-path.resolve.json`
- Syntax form: `<"path">`
- Category: `value-reference`

### Normative Fields

| Field | Value |
|---|---|
| diagnosticCodes | `{"absolute":"semantic.workspace_path.absolute","dotSegment":"semantic.workspace_path.dot_segment","empty":"semantic.workspace_path.empty"}` |
| rejects | `["empty","absolute","dotSegment"]` |
| usedBy | `["dir checkpoint subject","write step path","file contents_equals expected value"]` |

### Conformance Cases

| Description | Assertion source | Normalized assertion | Checkpoint | Expected result | Expected diagnostic |
|---|---|---|---|---|---|
| a relative path with no dot segments resolves | `"file <\\"actual.txt\\"> contents_equals <\\"expected.txt\\">"` | - | - | `valid` | - |
| invalid: an empty workspace path is rejected | `"file <\\"actual.txt\\"> contents_equals <\\"\\">"` | - | - | `parseError` | `semantic.workspace_path.empty` |
| invalid: an absolute workspace path is rejected | `"file <\\"actual.txt\\"> contents_equals <\\"/etc/passwd\\">"` | - | - | `parseError` | `semantic.workspace_path.absolute` |
| invalid: a workspace path with a '..' segment is rejected | `"file <\\"actual.txt\\"> contents_equals <\\"a/../b\\">"` | - | - | `parseError` | `semantic.workspace_path.dot_segment` |

