# Reportage Examples

## Contents

- [Actions](#group-1-actions)
  - [Line continuation in an action](#file-1-1-line-continuation-in-an-action)
    - [long pipeline split across lines with backslash continuation](#case-1-1-1-long-pipeline-split-across-lines-with-backslash-continuation)
  - [Any shell command as an action](#file-1-2-any-shell-command-as-an-action)
    - [action is shell, so it is possible HTTP request!](#case-1-2-1-action-is-shell-so-it-is-possible-http-request)
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
    - [Workspace isolation](#case-6-1-2-workspace-isolation)
    - [a later case still sees the pristine seeded state](#case-6-1-3-a-later-case-still-sees-the-pristine-seeded-state)
  - [Writing a file with a heredoc](#file-6-2-writing-a-file-with-a-heredoc)
    - [create file with heredoc](#case-6-2-1-create-file-with-heredoc)

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

A `before_each` block seeds every case's isolated workspace with the same files, so each case starts from an identical, explicit state.
It holds only `write` steps — setup commands belong in each case body — and, like a `document file` block, it appears at most once, before the first case.

<a id="case-6-1-1-the-seeded-files-are-present-before-any-action"></a>
#### the seeded files are present before any action

```reportage
case "the seeded files are present before any action" {
  assert {
    file <"config.yml"> exists
    file <"input/message.txt"> contains "hello"
  }
}
```

<a id="case-6-1-2-workspace-isolation"></a>
#### Workspace isolation

Each case gets its own copy of the seeded state, so a case may modify or delete a seeded file without affecting any other case.
The next case below still sees the pristine seeded files.

```reportage
case "a case mutates only its own copy of the seeded state" {
  $ rm config.yml
  assert {
    exit 0
    not {
      file <"config.yml"> exists
    }
  }
}
```

<a id="case-6-1-3-a-later-case-still-sees-the-pristine-seeded-state"></a>
#### a later case still sees the pristine seeded state

```reportage
case "a later case still sees the pristine seeded state" {
  $ grep "retries" config.yml
  assert {
    exit 0
    stdout contains "retries: 3"
  }
}
```

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
