# Reportage Examples

## Contents

- [Actions](#group-1-actions)
  - [Line continuation in an action](#file-1-1-line-continuation-in-an-action)
    - [long pipeline split across lines with backslash continuation](#case-1-1-1-long-pipeline-split-across-lines-with-backslash-continuation)
  - [Any shell command as an action](#file-1-2-any-shell-command-as-an-action)
    - [action is shell, so it is possible HTTP request!](#case-1-2-1-action-is-shell-so-it-is-possible-http-request)
  - [Runtime evidence bindings](#file-1-3-runtime-evidence-bindings)
    - [capture exact stdout and write it to a file](#case-1-3-1-capture-exact-stdout-and-write-it-to-a-file)
    - [Single-line capture](#case-1-3-2-single-line-capture)
    - [capture exact stderr including newlines](#case-1-3-3-capture-exact-stderr-including-newlines)
    - [capture one stderr line for a later assertion](#case-1-3-4-capture-one-stderr-line-for-a-later-assertion)
  - [Interpolated text literals](#file-1-4-interpolated-text-literals)
    - [assert a captured revision inside surrounding expected text](#case-1-4-1-assert-a-captured-revision-inside-surrounding-expected-text)
    - [Interpolated heredoc](#case-1-4-2-interpolated-heredoc)
    - [Raw literals are never interpolated](#case-1-4-3-raw-literals-are-never-interpolated)
    - [Escaping a literal ampersand](#case-1-4-4-escaping-a-literal-ampersand)
- [Assertions](#group-2-assertions)
  - [stdout and stderr expectations](#file-2-1-stdout-and-stderr-expectations)
    - [stdout is empty](#case-2-1-1-stdout-is-empty)
    - [stdout contains text](#case-2-1-2-stdout-contains-text)
    - [stderr is empty](#case-2-1-3-stderr-is-empty)
    - [stderr contains text](#case-2-1-4-stderr-contains-text)
  - [text_equals](#file-2-2-text-equals)
    - [file text_equals against a string literal](#case-2-2-1-file-text-equals-against-a-string-literal)
    - [Heredoc literal form](#case-2-2-2-heredoc-literal-form)
    - [stdout text_equals against a string literal](#case-2-2-3-stdout-text-equals-against-a-string-literal)
    - [stdout and stderr text_equals against heredoc literals](#case-2-2-4-stdout-and-stderr-text-equals-against-heredoc-literals)
  - [contents_equals](#file-2-3-contents-equals)
    - [file contents_equals against a workspace file](#case-2-3-1-file-contents-equals-against-a-workspace-file)
    - [Fixture references](#case-2-3-2-fixture-references)
    - [stdout contents_equals against a fixture file](#case-2-3-3-stdout-contents-equals-against-a-fixture-file)
    - [stdout contents_equals against a workspace file](#case-2-3-4-stdout-contents-equals-against-a-workspace-file)
    - [file contents_equals accepts dot-segment-like names, not just dot segments](#case-2-3-5-file-contents-equals-accepts-dot-segment-like-names-not-just-dot-segments)
    - [Asserting a diagnostic](#case-2-3-6-asserting-a-diagnostic)
    - [file contents_equals rejects a dot segment in the actual path](#case-2-3-7-file-contents-equals-rejects-a-dot-segment-in-the-actual-path)
    - [contents_equals rejects a dot segment in a workspace-path expected value](#case-2-3-8-contents-equals-rejects-a-dot-segment-in-a-workspace-path-expected-value)
    - [contents_equals rejects a dot segment in a fixture-reference expected value](#case-2-3-9-contents-equals-rejects-a-dot-segment-in-a-fixture-reference-expected-value)
  - [Logic blocks](#file-2-4-logic-blocks)
    - [assert that all expectations match](#case-2-4-1-assert-that-all-expectations-match)
    - [assert that any expectation matches](#case-2-4-2-assert-that-any-expectation-matches)
    - [assert that no expectation matches](#case-2-4-3-assert-that-no-expectation-matches)
    - [Nested composite blocks](#case-2-4-4-nested-composite-blocks)
- [Basics](#group-3-basics)
  - [Minimal script](#file-3-1-minimal-script)
    - [always pass](#case-3-1-1-always-pass)
  - [Multiple cases in one file](#file-3-2-multiple-cases-in-one-file)
    - [always true](#case-3-2-1-always-true)
    - [always false](#case-3-2-2-always-false)
  - [Comments](#file-3-3-comments)
    - [grep finds the expected line](#case-3-3-1-grep-finds-the-expected-line)
    - [grep reports no match](#case-3-3-2-grep-reports-no-match)
- [Documentation](#group-4-documentation)
  - [File-scope documentation](#file-4-1-file-scope-documentation)
    - [file exists](#case-4-1-1-file-exists)
    - [file contains text](#case-4-1-2-file-contains-text)
  - [Case-scope documentation](#file-4-2-case-scope-documentation)
    - [Documented case](#case-4-2-1-documented-case)
    - [file contains text](#case-4-2-2-file-contains-text)
- [Filesystem](#group-5-filesystem)
  - [File assertions](#file-5-1-file-assertions)
    - [file exists](#case-5-1-1-file-exists)
    - [file contains text](#case-5-1-2-file-contains-text)
  - [Directory assertions](#file-5-2-directory-assertions)
    - [dir exists](#case-5-2-1-dir-exists)
    - [dir contains an entry](#case-5-2-2-dir-contains-an-entry)
- [Setup](#group-6-setup)
  - [before_each](#file-6-1-before-each)
    - [the seeded files are present before any action](#case-6-1-1-the-seeded-files-are-present-before-any-action)
    - [a case body sees what the setup command created](#case-6-1-2-a-case-body-sees-what-the-setup-command-created)
    - [Setup output as a binding](#case-6-1-3-setup-output-as-a-binding)
    - [Workspace isolation](#case-6-1-4-workspace-isolation)
    - [a later case still sees the pristine seeded state](#case-6-1-5-a-later-case-still-sees-the-pristine-seeded-state)
    - [The case body starts fresh](#case-6-1-6-the-case-body-starts-fresh)
  - [Writing a file with a heredoc](#file-6-2-writing-a-file-with-a-heredoc)
    - [create file with heredoc](#case-6-2-1-create-file-with-heredoc)
  - [Writing an executable file](#file-6-3-writing-an-executable-file)
    - [run a fake command created by a write step](#case-6-3-1-run-a-fake-command-created-by-a-write-step)
    - [keep a fixture readable only by its owner](#case-6-3-2-keep-a-fixture-readable-only-by-its-owner)

<a id="group-1-actions"></a>
## Actions

<a id="file-1-1-line-continuation-in-an-action"></a>
### Line continuation in an action

Source: examples/action-line-continuation.repor

A `$` action normally occupies one line.
A trailing backslash continues it onto the next line, so a long shell pipeline can be split for readability while remaining a single action.
The action body, including the backslash and newline, is passed to the shell unchanged, and the shell joins the lines.

<a id="case-1-1-1-long-pipeline-split-across-lines-with-backslash-continuation"></a>
#### long pipeline split across lines with backslash continuation

```reportage
case "long pipeline split across lines with backslash continuation" {
  $ printf 'one\ntwo\nthree\n' \
    | grep 'two' \
    | tr 'a-z' 'A-Z'

  assert {
    exit 0
    stdout contains "TWO"
  }
}
```

<a id="file-1-2-any-shell-command-as-an-action"></a>
### Any shell command as an action

Source: examples/curl-as-action.repor

A `$` action is an ordinary shell command line, so the action can be any program on PATH — here `curl` makes a real HTTP request.
Whatever the command writes to stdout and stderr is captured and can be checked with `stdout` / `stderr` expectations.

<a id="case-1-2-1-action-is-shell-so-it-is-possible-http-request"></a>
#### action is shell, so it is possible HTTP request!

```reportage
case "action is shell, so it is possible HTTP request!" {
  $ curl https://example.com

  assert {
    exit 0
    stdout contains "Example Domain"
    stdout contains "<!doctype html>"
  }
}
```

<a id="file-1-3-runtime-evidence-bindings"></a>
### Runtime evidence bindings

Source: examples/runtime-evidence-bindings.repor

A runtime evidence binding captures stdout or stderr from the preceding action as immutable, case-local text.
Use exact capture when newlines are significant, or a `_line` source when the action emits one logical line whose trailing newline should be removed.
A binding reference such as `&output` can supply text to `write`, `contains`, and `text_equals`.

<a id="case-1-3-1-capture-exact-stdout-and-write-it-to-a-file"></a>
#### capture exact stdout and write it to a file

```reportage
case "capture exact stdout and write it to a file" {
  $ printf 'hello\nworld\n'
  let output <- stdout

  write <"captured.txt"> &output

  assert {
    stdout text_equals &output
    file <"captured.txt"> text_equals &output
  }
}
```

<a id="case-1-3-2-single-line-capture"></a>
#### Single-line capture

`stdout_line` removes one trailing LF or CRLF before binding the value.
The captured line remains available only after its declaration and only within this case.

```reportage
case "capture one stdout line without its trailing newline" {
  $ printf 'reportage\n'
  let word <- stdout_line

  write <"word.txt"> &word

  assert {
    stdout contains &word
    file <"word.txt"> text_equals &word
  }
}
```

<a id="case-1-3-3-capture-exact-stderr-including-newlines"></a>
#### capture exact stderr including newlines

```reportage
case "capture exact stderr including newlines" {
  $ printf 'warning: retrying\nerror: unavailable\n' >&2
  let diagnostics <- stderr

  write <"diagnostics.txt"> &diagnostics

  assert {
    stderr text_equals &diagnostics
    file <"diagnostics.txt"> text_equals &diagnostics
  }
}
```

<a id="case-1-3-4-capture-one-stderr-line-for-a-later-assertion"></a>
#### capture one stderr line for a later assertion

```reportage
case "capture one stderr line for a later assertion" {
  $ printf 'warning: retrying\n' >&2
  let warning <- stderr_line

  assert {
    exit 0
    stderr contains &warning
  }
}
```

<a id="file-1-4-interpolated-text-literals"></a>
### Interpolated text literals

Source: examples/interpolated-text.repor

An interpolated text literal builds a `TextValue` from literal text and case-local bindings.
Prefix a string literal or a heredoc literal with `&`, then reference a binding as `&{name}` inside it.
Ordinary `"..."` and heredoc literals stay raw: `&{name}` written in one of those is literal text, which keeps shell scripts and other template syntax predictable.
Inside an interpolated literal `&` is reserved, so a literal ampersand is written `\&`.

<a id="case-1-4-1-assert-a-captured-revision-inside-surrounding-expected-text"></a>
#### assert a captured revision inside surrounding expected text

```reportage
case "assert a captured revision inside surrounding expected text" {
  $ printf 'abc123\n'
  let revision <- stdout_line

  write <"lock.json"> &"{\"resolved_revision\": \"&{revision}\"}\n"

  assert {
    file <"lock.json"> contains &"\"resolved_revision\": \"&{revision}\""
  }
}
```

<a id="case-1-4-2-interpolated-heredoc"></a>
#### Interpolated heredoc

An interpolated heredoc is dedented against its closing fence first, and the binding value is then inserted exactly as captured.
A multi-line binding value is never re-indented to match the reference's own indentation: the runtime value is inserted unchanged.

````reportage
case "render a fixture file from a value produced by the run" {
  $ printf 'file:///source-repo\n'
  let source_url <- stdout_line

  write <"provider.kdl"> &```
    provider {
      skills {
        skill "demo-skill" {
          url "&{source_url}"
          branch "main"
        }
      }
    }
    ```

  assert {
    file <"provider.kdl"> contains &"url \"&{source_url}\""
  }
}
````

<a id="case-1-4-3-raw-literals-are-never-interpolated"></a>
#### Raw literals are never interpolated

A raw literal keeps `&{name}` as literal text, so a shell script or another template engine's syntax can be written verbatim.
Only the `&`-prefixed forms interpolate.

````reportage
case "write a shell script whose own placeholder must survive verbatim" {
  $ printf 'v1\n'
  let tag <- stdout_line

  write <"release.sh"> ```
    echo "&{tag}"
    ```

  write <"release.txt"> &"released &{tag}"

  assert {
    file <"release.sh"> contains "&{tag}"
    file <"release.txt"> text_equals "released v1"
  }
}
````

<a id="case-1-4-4-escaping-a-literal-ampersand"></a>
#### Escaping a literal ampersand

`\&` produces a literal `&`, so `\&{name}` is the literal text `&{name}` rather than a binding reference.
An interpolated heredoc keeps every other backslash literal, exactly as a raw heredoc does.

````reportage
case "mix literal ampersands, escaped markers, and real references" {
  $ printf 'v1\n'
  let tag <- stdout_line

  write <"notes.txt"> &```
    matched by \d+ under C:\temp
    literal marker: \&{tag}
    interpolated:   &{tag}
    ```

  assert {
    file <"notes.txt"> text_equals ```
      matched by \d+ under C:\temp
      literal marker: &{tag}
      interpolated:   v1
      ```
  }
}
````

<a id="group-2-assertions"></a>
## Assertions

<a id="file-2-1-stdout-and-stderr-expectations"></a>
### stdout and stderr expectations

Source: examples/stdout-stderr.repor

Expectations that inspect the action's captured output streams.
`stdout empty` / `stderr empty` require the stream to have no output; `stdout contains` / `stderr contains` require the stream to include the given substring.

<a id="case-2-1-1-stdout-is-empty"></a>
#### stdout is empty

```reportage
case "stdout is empty" {
  $ true

  assert {
    stdout empty
  }
}
```

<a id="case-2-1-2-stdout-contains-text"></a>
#### stdout contains text

```reportage
case "stdout contains text" {
  $ echo "Hello, World!"

  assert {
    stdout contains "World"
  }
}
```

<a id="case-2-1-3-stderr-is-empty"></a>
#### stderr is empty

```reportage
case "stderr is empty" {
  $ true

  assert {
    stderr empty
  }
}
```

<a id="case-2-1-4-stderr-contains-text"></a>
#### stderr contains text

```reportage
case "stderr contains text" {
  $ cat nonexistent.file

  assert {
    stderr contains "No such file or directory"
  }
}
```

<a id="file-2-2-text-equals"></a>
### text_equals

Source: examples/text-equals.repor

`text_equals` requires the whole text to match a literal exactly, unlike `contains`, which only requires a substring.
The expected value may be a string literal (with `\n` escapes) or a triple-backtick heredoc literal (one line per line, dedented to the closing fence).
It applies to a workspace file with `file <"...">`, and to `stdout` / `stderr`.

<a id="case-2-2-1-file-text-equals-against-a-string-literal"></a>
#### file text_equals against a string literal

```reportage
case "file text_equals against a string literal" {
  $ printf 'hello\n' > actual.txt

  assert {
    file <"actual.txt"> text_equals "hello\n"
  }
}
```

<a id="case-2-2-2-heredoc-literal-form"></a>
#### Heredoc literal form

A heredoc literal spells the expected text out line by line instead of packing it into one string with `\n` escapes.
Each content line becomes one line of text, dedented relative to the closing fence, and a trailing newline is implied.

````reportage
case "file text_equals against a heredoc literal" {
  $ printf 'hello\nworld\n' > actual.txt

  assert {
    file <"actual.txt"> text_equals ```
    hello
    world
    ```
  }
}
````

<a id="case-2-2-3-stdout-text-equals-against-a-string-literal"></a>
#### stdout text_equals against a string literal

```reportage
case "stdout text_equals against a string literal" {
  $ printf 'hello\n'

  assert {
    stdout text_equals "hello\n"
  }
}
```

<a id="case-2-2-4-stdout-and-stderr-text-equals-against-heredoc-literals"></a>
#### stdout and stderr text_equals against heredoc literals

````reportage
case "stdout and stderr text_equals against heredoc literals" {
  $ sh -c 'printf "hello\nworld\n"; printf "warn\nline\n" >&2'

  assert {
    stdout text_equals ```
    hello
    world
    ```
    stderr text_equals ```
    warn
    line
    ```
  }
}
````

<a id="file-2-3-contents-equals"></a>
### contents_equals

Source: examples/contents-equals.repor

`contents_equals` compares one subject's bytes against another file's bytes.
The subject is a workspace `file <"...">` or `stdout` / `stderr`; the expected value is either a workspace path `<"...">` (a file the case produced) or a fixture reference `@"..."` (a file checked in under the test-definition tree).
Both path kinds forbid `.` and `..` path segments, so this file also shows the diagnostics raised when a path tries to escape its root.

<a id="case-2-3-1-file-contents-equals-against-a-workspace-file"></a>
#### file contents_equals against a workspace file

```reportage
case "file contents_equals against a workspace file" {
  $ printf 'hello\n' > expected.txt
  $ printf 'hello\n' > actual.txt

  assert {
    file <"actual.txt"> contents_equals <"expected.txt">
  }
}
```

<a id="case-2-3-2-fixture-references"></a>
#### Fixture references

A fixture reference `@"..."` names a file that lives with the test definition (here `fixtures/expected.txt`), not a file the case produced in its workspace.
Use it to compare output against checked-in expected data that every run shares.

```reportage
case "file contents_equals against a fixture file" {
  $ printf 'hello, fixture!\n' > actual.txt

  assert {
    file <"actual.txt"> contents_equals @"fixtures/expected.txt"
  }
}
```

<a id="case-2-3-3-stdout-contents-equals-against-a-fixture-file"></a>
#### stdout contents_equals against a fixture file

```reportage
case "stdout contents_equals against a fixture file" {
  $ printf 'hello, fixture!\n'

  assert {
    stdout contents_equals @"fixtures/expected.txt"
  }
}
```

<a id="case-2-3-4-stdout-contents-equals-against-a-workspace-file"></a>
#### stdout contents_equals against a workspace file

```reportage
case "stdout contents_equals against a workspace file" {
  $ printf 'hello\n' > expected.txt
  $ printf 'hello\n'

  assert {
    stdout contents_equals <"expected.txt">
  }
}
```

<a id="case-2-3-5-file-contents-equals-accepts-dot-segment-like-names-not-just-dot-segments"></a>
#### file contents_equals accepts dot-segment-like names, not just dot segments

```reportage
case "file contents_equals accepts dot-segment-like names, not just dot segments" {
  # ".." / "." *segments* (a whole path component equal to ".." or ".") are rejected — see the
  # invalid cases below — but a name that merely *starts* with dots is an ordinary file name and
  # is accepted, for both <"..."> and @"...".
  $ printf 'hello\n' > ..looks-like-parent.txt
  $ printf 'hello\n' > .hidden-expected.txt

  assert {
    file <"..looks-like-parent.txt"> contents_equals <".hidden-expected.txt">
    file <"..looks-like-parent.txt"> contents_equals @"fixtures/..looks-like-parent.txt"
  }
}
```

<a id="case-2-3-6-asserting-a-diagnostic"></a>
#### Asserting a diagnostic

reportage can test reportage: this case writes an inner `.repor` with `write`, runs it with `reportage inner.repor` as the action, and asserts the failing exit code and the diagnostic id on stderr.
The following cases reuse this pattern to pin down each path-segment diagnostic.

````reportage
case "file contents_equals rejects a fixture reference as the actual subject" {
  # A fixture reference (@"...") names test-definition-side content; it can only ever be used as
  # contents_equals's *expected* value, never as the `file` checkpoint subject (the actual side).
  write <"inner.repor"> ```
    case "inner" {
      $ printf hello > expected.txt
      assert {
        file @"actual.txt" contents_equals <"expected.txt">
      }
    }
    ```

  $ reportage inner.repor

  assert {
    exit 2
    stderr contains "semantic.literal.kind_mismatch"
  }
}
````

<a id="case-2-3-7-file-contents-equals-rejects-a-dot-segment-in-the-actual-path"></a>
#### file contents_equals rejects a dot segment in the actual path

````reportage
case "file contents_equals rejects a dot segment in the actual path" {
  write <"inner.repor"> ```
    case "inner" {
      $ printf hello > expected.txt
      assert {
        file <"../escape.txt"> contents_equals <"expected.txt">
      }
    }
    ```

  $ reportage inner.repor

  assert {
    exit 2
    stderr contains "semantic.file_path.dot_segment"
  }
}
````

<a id="case-2-3-8-contents-equals-rejects-a-dot-segment-in-a-workspace-path-expected-value"></a>
#### contents_equals rejects a dot segment in a workspace-path expected value

````reportage
case "contents_equals rejects a dot segment in a workspace-path expected value" {
  write <"inner.repor"> ```
    case "inner" {
      $ printf hello
      assert {
        stdout contents_equals <"../escape.txt">
      }
    }
    ```

  $ reportage inner.repor

  assert {
    exit 2
    stderr contains "semantic.workspace_path.dot_segment"
  }
}
````

<a id="case-2-3-9-contents-equals-rejects-a-dot-segment-in-a-fixture-reference-expected-value"></a>
#### contents_equals rejects a dot segment in a fixture-reference expected value

````reportage
case "contents_equals rejects a dot segment in a fixture-reference expected value" {
  write <"inner.repor"> ```
    case "inner" {
      $ printf hello > actual.txt
      assert {
        file <"actual.txt"> contents_equals @"../escape.txt"
      }
    }
    ```

  $ reportage inner.repor

  assert {
    exit 2
    stderr contains "semantic.fixture_reference.dot_segment"
  }
}
````

<a id="file-2-4-logic-blocks"></a>
### Logic blocks

Source: examples/use-logic-block.repor

Expectations inside an `assert` block are combined with implicit AND.
The `all` / `any` / `not` logic blocks override that: `all` requires every nested expectation, `any` requires at least one, and `not` requires none.
Logic blocks nest freely to express composite conditions.

<a id="case-2-4-1-assert-that-all-expectations-match"></a>
#### assert that all expectations match

```reportage
case "assert that all expectations match" {
  $ echo "assert with and"

  assert {
    all {
      exit 0
      stdout contains "assert"
    }
  }
}
```

<a id="case-2-4-2-assert-that-any-expectation-matches"></a>
#### assert that any expectation matches

```reportage
case "assert that any expectation matches" {
  $ echo "assert with or"

  assert {
    any {
      exit 1
      stdout contains "assert"
    }
  }
}
```

<a id="case-2-4-3-assert-that-no-expectation-matches"></a>
#### assert that no expectation matches

```reportage
case "assert that no expectation matches" {
  $ echo "assert with not"

  assert {
    not {
      exit 1
    }
  }
}
```

<a id="case-2-4-4-nested-composite-blocks"></a>
#### Nested composite blocks

`all`, `any`, and `not` can contain each other to any depth.
The top-level expectations still AND together, so this asserts the nested `any { ... }` and a bare `stderr empty` at once.

```reportage
case "composite logic blocks" {
  $ echo "composite logic blocks"

  assert {
    any {
      all {
        any {
          exit 1
          stdout contains "composite"
        }
        not {
          file <"nonexistent.file"> exists
        }
      }
      stdout empty
    }
    stderr empty
  }
}
```

<a id="group-3-basics"></a>
## Basics

<a id="file-3-1-minimal-script"></a>
### Minimal script

Source: examples/minimal.repor

The smallest useful reportage script: a single `case`, one `$` action line that runs a shell command, and an `assert` block.
`exit 0` checks the action's exit status.
Every reportage script is built from these three pieces.

<a id="case-3-1-1-always-pass"></a>
#### always pass

```reportage
case "always pass" {
  $ true
  assert {
    exit 0
  }
}
```

<a id="file-3-2-multiple-cases-in-one-file"></a>
### Multiple cases in one file

Source: examples/multi-case-1file.repor

A single script may hold any number of `case` blocks.
Each case runs in its own isolated workspace, so the cases are independent and their order does not couple them.
A case passes when every expectation in its `assert` block holds — including `exit 1`, which asserts the action exited with status 1 (a passing assertion, not a test failure).

<a id="case-3-2-1-always-true"></a>
#### always true

```reportage
case "always true" {
  $ true
  assert {
    exit 0
  }
}
```

<a id="case-3-2-2-always-false"></a>
#### always false

```reportage
case "always false" {
  $ false
  assert {
    exit 1
  }
}
```

<a id="file-3-3-comments"></a>
### Comments

Source: examples/commented-multi-case.repor

`#` comments may appear at the top level, between cases, inside a case, after an expectation on the same line, and after a closing brace.
They are discarded at parse time and never affect execution.
Blank lines and comment lines between cases belong to the file, not to any case.
(This file-level description comes from a `document` block, which is separate from `#` comments and is preserved as documentation metadata.)

<a id="case-3-3-1-grep-finds-the-expected-line"></a>
#### grep finds the expected line

````reportage
case "grep finds the expected line" {
  write <"notes.txt"> ```
  alpha
  beta
  gamma
  ```
  $ grep beta notes.txt
  assert {
    exit 0
    stdout contains "beta" # inline comments may follow an expectation
  }
}
````

<a id="case-3-3-2-grep-reports-no-match"></a>
#### grep reports no match

```reportage
case "grep reports no match" {
  write <"notes.txt"> "alpha\n"
  $ grep beta notes.txt
  assert {
    exit 1
    stdout empty
  }
} # comments may follow a closing brace
```

<a id="group-4-documentation"></a>
## Documentation

<a id="file-4-1-file-scope-documentation"></a>
### File-scope documentation

Source: examples/document-file.repor

A `document file` block attaches documentation metadata to the whole script.
It goes at the top, before any `before_each` or `case`, and may appear at most once.
Its fields are `title`, `group`, and `order` (which control how `reportage docs` labels and orders this file), plus a free-form `description` like this one.
The block is separate from `#` comments and never affects execution.

<a id="case-4-1-1-file-exists"></a>
#### file exists

```reportage
case "file exists" {
  $ touch test.txt

  assert {
    file <"test.txt"> exists
  }
}
```

<a id="case-4-1-2-file-contains-text"></a>
#### file contains text

```reportage
case "file contains text" {
  $ echo "FizzBuzz" > test.txt

  assert {
    file <"test.txt"> contains "FizzBuzz"
  }
}
```

<a id="file-4-2-case-scope-documentation"></a>
### Case-scope documentation

Source: examples/document-case.repor

A `document case` block attaches documentation to the case that immediately follows it.
Its fields are `title` and `description` only — ordering and grouping are file-scope concerns, so cases render in source order.
Case documentation is optional and per case.

<a id="case-4-2-1-documented-case"></a>
#### Documented case

This block documents the next case.
`reportage docs` uses its `title` as the case heading and renders this `description` above the case source.

```reportage
case "file exists" {
  $ touch test.txt

  assert {
    file <"test.txt"> exists
  }
}
```

<a id="case-4-2-2-file-contains-text"></a>
#### file contains text

```reportage
case "file contains text" {
  $ echo "FizzBuzz" > test.txt

  assert {
    file <"test.txt"> contains "FizzBuzz"
  }
}
```

<a id="group-5-filesystem"></a>
## Filesystem

<a id="file-5-1-file-assertions"></a>
### File assertions

Source: examples/file-assertions.repor

Expectations about a file in the case workspace, addressed by a workspace path `<"...">`.
`file <"..."> exists` checks that the file is present; `file <"..."> contains "..."` checks that its contents include the given substring.

<a id="case-5-1-1-file-exists"></a>
#### file exists

```reportage
case "file exists" {
  $ echo "file exists" > test.txt

  assert {
    file <"test.txt"> exists
  }
}
```

<a id="case-5-1-2-file-contains-text"></a>
#### file contains text

```reportage
case "file contains text" {
  $ echo "FizzBuzz" > test.txt

  assert {
    file <"test.txt"> contains "FizzBuzz"
  }
}
```

<a id="file-5-2-directory-assertions"></a>
### Directory assertions

Source: examples/dir-assertions.repor

Expectations about a directory in the case workspace, addressed by a workspace path `<"...">`.
`dir <"..."> exists` checks that the directory is present; `dir <"..."> contains "..."` checks that it holds an entry with the given name.

<a id="case-5-2-1-dir-exists"></a>
#### dir exists

```reportage
case "dir exists" {
  $ mkdir out

  assert {
    dir <"out"> exists
  }
}
```

<a id="case-5-2-2-dir-contains-an-entry"></a>
#### dir contains an entry

```reportage
case "dir contains an entry" {
  $ mkdir out && touch out/result.json

  assert {
    dir <"out"> contains "result.json"
  }
}
```

<a id="group-6-setup"></a>
## Setup

<a id="file-6-1-before-each"></a>
### before_each

Source: examples/before-each.repor

A `before_each` block seeds every case's isolated workspace before the case body's first step, so every case's starting state is produced by the same declared steps, written once.
It holds the same steps a case body holds — `$` actions, `assert` blocks, `let` bindings, and `write` steps — and, like a `document file` block, it appears at most once, before the first case.
Setup a `write` cannot express therefore belongs here too: creating an empty directory, initializing a tool, or reading a path that only exists at run time.

<a id="case-6-1-1-the-seeded-files-are-present-before-any-action"></a>
#### the seeded files are present before any action

````reportage
before_each {
  write <"config.yml"> ```
    retries: 3
    verbose: true
    ```

  write <"input/message.txt"> "hello reportage\n"

  $ mkdir -p repo/objects
  assert {
    exit 0
    dir <"repo/objects"> exists
  }

  $ pwd
  let workspace <- stdout_line

  write <"tool.config"> &```
    root = &{workspace}/repo
    mode = strict
    ```
}

case "the seeded files are present before any action" {
  assert {
    file <"config.yml"> exists
    file <"input/message.txt"> contains "hello"
  }
}
````

<a id="case-6-1-2-a-case-body-sees-what-the-setup-command-created"></a>
#### a case body sees what the setup command created

````reportage
before_each {
  write <"config.yml"> ```
    retries: 3
    verbose: true
    ```

  write <"input/message.txt"> "hello reportage\n"

  $ mkdir -p repo/objects
  assert {
    exit 0
    dir <"repo/objects"> exists
  }

  $ pwd
  let workspace <- stdout_line

  write <"tool.config"> &```
    root = &{workspace}/repo
    mode = strict
    ```
}

case "a case body sees what the setup command created" {
  assert {
    dir <"repo/objects"> exists
  }
}
````

<a id="case-6-1-3-setup-output-as-a-binding"></a>
#### Setup output as a binding

An action only updates the checkpoint, so a setup action is verified by an `assert` block written next to it, and its output is captured with `let`.
The binding is then usable for the rest of `before_each` — here, interpolated into a `write` — and for the whole case body.
Every step is replayed inside each concrete case's own workspace, so the captured value is that case's own.

````reportage
before_each {
  write <"config.yml"> ```
    retries: 3
    verbose: true
    ```

  write <"input/message.txt"> "hello reportage\n"

  $ mkdir -p repo/objects
  assert {
    exit 0
    dir <"repo/objects"> exists
  }

  $ pwd
  let workspace <- stdout_line

  write <"tool.config"> &```
    root = &{workspace}/repo
    mode = strict
    ```
}

case "a case body reads a binding the setup captured" {
  assert {
    file <"tool.config"> contains &"root = &{workspace}/repo"
  }
}
````

<a id="case-6-1-4-workspace-isolation"></a>
#### Workspace isolation

Each case gets its own copy of the seeded state, so a case may modify or delete a seeded file without affecting any other case.
The next case below still sees the pristine seeded files.

````reportage
before_each {
  write <"config.yml"> ```
    retries: 3
    verbose: true
    ```

  write <"input/message.txt"> "hello reportage\n"

  $ mkdir -p repo/objects
  assert {
    exit 0
    dir <"repo/objects"> exists
  }

  $ pwd
  let workspace <- stdout_line

  write <"tool.config"> &```
    root = &{workspace}/repo
    mode = strict
    ```
}

case "a case mutates only its own copy of the seeded state" {
  $ rm config.yml
  assert {
    exit 0
    not {
      file <"config.yml"> exists
    }
  }
}
````

<a id="case-6-1-5-a-later-case-still-sees-the-pristine-seeded-state"></a>
#### a later case still sees the pristine seeded state

````reportage
before_each {
  write <"config.yml"> ```
    retries: 3
    verbose: true
    ```

  write <"input/message.txt"> "hello reportage\n"

  $ mkdir -p repo/objects
  assert {
    exit 0
    dir <"repo/objects"> exists
  }

  $ pwd
  let workspace <- stdout_line

  write <"tool.config"> &```
    root = &{workspace}/repo
    mode = strict
    ```
}

case "a later case still sees the pristine seeded state" {
  $ grep "retries" config.yml
  assert {
    exit 0
    stdout contains "retries: 3"
  }
}
````

<a id="case-6-1-6-the-case-body-starts-fresh"></a>
#### The case body starts fresh

Workspace state carries over from `before_each`, but the last setup action's result does not: the case body starts at its own initial checkpoint.
`exit`, `stdout`, and `stderr` at the top of a case body would have no action to describe, so this case runs its own action first.
Its `stdout` is that action's — had the setup's evidence carried over, this would be the path the setup's last action printed.

````reportage
before_each {
  write <"config.yml"> ```
    retries: 3
    verbose: true
    ```

  write <"input/message.txt"> "hello reportage\n"

  $ mkdir -p repo/objects
  assert {
    exit 0
    dir <"repo/objects"> exists
  }

  $ pwd
  let workspace <- stdout_line

  write <"tool.config"> &```
    root = &{workspace}/repo
    mode = strict
    ```
}

case "process expectations describe the case body's own action" {
  $ grep "^mode = " tool.config
  assert {
    exit 0
    stdout contains "mode = strict"
  }
}
````

<a id="file-6-2-writing-a-file-with-a-heredoc"></a>
### Writing a file with a heredoc

Source: examples/create-file-with-heardoc.repor

A `write <"path"> ...` step creates a file in the case workspace before the action runs.
With a triple-backtick heredoc the content is written line by line and dedented relative to the closing fence, so the block can be indented to match the surrounding code without changing what is written.

<a id="case-6-2-1-create-file-with-heredoc"></a>
#### create file with heredoc

````reportage
case "create file with heredoc" {
  write <"test.txt"> ```
    Hello, Rerpotage!
    Indentation is deindented relative to the terminating character

    Therefore, the test will not fail even if you indent the text!
    ```

  $ cat test.txt | grep Therefore
  assert {
    exit 0
    not {
      stdout contains "Hello, Rerpotage!"
    }
    stdout contains "indent the text!"
  }
}
````

<a id="file-6-3-writing-an-executable-file"></a>
### Writing an executable file

Source: examples/write-file-mode.repor

A `write <"path"> mode=0oXYZ <content>` step fixes the POSIX permission bits of the file it creates, so a fake command can be authored and made executable in one step instead of being followed by a `chmod` action that is setup rather than behavior under test.
The mode is written as exactly three octal digits after the `0o` prefix, between the path and the content, and it is the file's final permission bits regardless of the umask reportage runs under.
`mode` is optional and defaults to `0o600`: without it the file is readable and writable by its owner only, and never executable. A mode never applies to the parent directories the step creates.

<a id="case-6-3-1-run-a-fake-command-created-by-a-write-step"></a>
#### run a fake command created by a write step

````reportage
case "run a fake command created by a write step" {
  write <"bin/git"> mode=0o755 ```
    #!/bin/sh
    echo "fake git 1.0"
    ```

  $ PATH="$PWD/bin:$PATH" git

  assert {
    exit 0
    stdout contains "fake git 1.0"
  }
}
````

<a id="case-6-3-2-keep-a-fixture-readable-only-by-its-owner"></a>
#### keep a fixture readable only by its owner

```reportage
case "keep a fixture readable only by its owner" {
  write <"secret.txt"> mode=0o600 "token\n"

  # The first ten characters of `ls -l` are the file type and permission bits.
  $ ls -l secret.txt | cut -c1-10

  assert {
    exit 0
    stdout text_equals "-rw-------\n"
  }
}
```
