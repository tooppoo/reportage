# Give `write` an Optional POSIX File Mode

- Status: Accepted
- Created: 2026-08-24T14:00:00Z

## Context

[The `write` step ADR](20260704T183546Z_write-step-and-per-case-workspace-isolation.md) listed "file mode / executable bit" among its explicit non-goals, on the reasoning that `write` privileges *text file content* and nothing else about the file.

Real use has since produced a case that reasoning does not cover: authoring a fake command. A fake `git` on `PATH` is a fixture, not behavior under test, yet without a file mode it takes two steps:

````reportage
write <"bin/git"> ```
  #!/bin/sh
  echo "fake git"
  ```

$ chmod u+x bin/git
````

The `chmod` action is pure setup that a reader has to recognize as incidental, and it splits one fixture across two steps — the same problem the `write` step was introduced to remove for file content. The permission bit is not a separate concern from the content here; a script that writes a shell script and cannot mark it executable has not finished creating the fixture.

This reverses a decision recorded in an accepted ADR, so the reversal and its reasoning need their own durable record rather than living only in the issue.

## Decision

### 1. An optional `mode` property, not an `executable` modifier

````reportage
write <"bin/git"> mode=0o755 ```
  #!/bin/sh
  echo "fake git"
  ```

write <"secret.txt"> mode=0o640 "secret"
````

The motivating case is the executable bit, and a dedicated `executable` modifier would express it more directly. It was rejected because it answers only one of the permission questions a fixture raises: a fixture that must be group-readable but not world-readable (`0o640`), or deliberately unreadable (`0o000`), is just as real, and each would need its own modifier. One property whose value is the permission bit set covers all of them and introduces exactly one new concept.

`mode` is a property of the file being created, so it sits with the other things `write` says about that file — after the path, before the content — rather than as a trailing option.

### 2. Exactly `mode=0oXYZ`, and nothing else

The value is the `0o` prefix followed by three octal digits, with no whitespace around `=`. `mode = 0o755`, `0O755`, `0755`, `755`, `0o75`, `0o1000`, `0o888`, and symbolic chmod syntax such as `u+x` are all syntax errors.

One spelling per value means a reader never has to work out which radix a number is in — the failure mode of C-style `0755`, which reads as decimal to anyone who has not memorized the convention. Fixing the digit count at three is also what confines the value to `0o000`–`0o777`, so setuid, setgid, and sticky are absent from the language rather than merely unimplemented.

Symbolic syntax (`u+x`, `go-r`) is rejected for a different reason: it is *relative*, and describes a change to a mode the file already has. `write` creates the file, so there is no prior mode for a relative expression to be relative to. An absolute bit set is the only thing that has an unambiguous meaning in this position.

### 3. `mode` is a domain type, not a number carried to the filesystem

```rust
pub struct FileMode(u32);

impl FileMode {
    pub fn from_bits(bits: u32) -> Result<Self, FileModeError> { ... }
}

pub struct WriteFileStep {
    pub path: WorkspacePath,
    pub content: TextValueExpression,
    pub mode: Option<FileMode>,
}
```

`FileMode::from_bits` rejects everything above `0o777`, mirroring what [`WorkspacePath`](20260704T183546Z_write-step-and-per-case-workspace-isolation.md) does for path safety: the guarantee is attached to the type rather than to one parser, so a value that reaches the filesystem is in range no matter which caller built it. The grammar already rejects an out-of-range literal, so this is redundant for the parser and load-bearing for everyone else.

### 4. No semantic-invalid case exists for `mode`

Every rejected `mode` is rejected by the grammar, and every value the grammar accepts is a valid `FileMode`. There is therefore no shape that parses into an AST and is then refused by semantic validation, and no `mode`-specific parse-domain diagnostic. This is a deliberate consequence of decision 2, not a gap in validation: the surface syntax is narrow enough that the set of "parses but is meaningless" values is empty.

### 5. The mode is applied before the file is published, and never depends on the umask

`write` already writes content to a temporary file in the target's parent directory and publishes it with `persist_noclobber`, so a failed write never leaves a partial file visible at the target. The mode joins that guarantee rather than sitting outside it:

1. create a temporary file in the target's parent directory
2. write the content to it
3. apply the mode to it — the one the step named, or the default
4. publish it to the target with `persist_noclobber`

A file visible at the target therefore always already carries the mode that was applied, and a failed mode application publishes nothing.

The mode is applied with `chmod` on the open handle rather than through the file's *creation* mode. This is what makes the result independent of the reportage process's umask: the kernel masks a creation mode and does not mask `chmod`. Applying it to the handle rather than to the path also means no other process can substitute a different file between the write and the mode change.

A step that names no `mode` goes through the same step, with a fixed default of `0o600` — owner read and write, nothing for group or other, never executable. Leaving the temporary file's creation mode in place instead would have made the one case a script does not spell out the one case whose result depends on the environment: under `umask 0400` the same step yields `0o200`. A default that is stated rather than inherited means the permission bits of every file a `write` step creates are a property of the script.

The cost is that `chmod` is now on the path of every `write` step, not only one that names a mode: on a filesystem that refuses it, a step that previously succeeded becomes a `step.write.io_error`. That is accepted because the alternative is a default whose value depends on the environment, which is the thing this decision exists to remove.

### 6. A failed mode is a runtime step error, reusing `step.write.io_error`

A `write` step has no expectation to compare against evidence, so its failure is never an assertion failure — decision 5 of [the `write` step ADR](20260704T183546Z_write-step-and-per-case-workspace-isolation.md) applies unchanged. A mode that cannot be applied reuses the existing `step.write.io_error` code rather than introducing a new one: it is an OS-level I/O failure like the others already in that class, and a new code would oblige every consumer to learn a distinction that changes nothing about how the failure is handled. The message names the mode as the failing part and repeats the value that was refused, so the diagnostic is still specific.

### 7. Linux and macOS only, and a mode that cannot be applied fails loudly

Windows permission semantics are out of scope, consistent with [no native Windows execution](20260627T120000Z_no-windows-native-execution.md).

Off-Unix the mode application returns an error rather than being skipped. Since every step applies a mode, that means no `write` step at all can succeed there, not merely one that names a mode. This deliberately differs from `shim.rs` and `shim_scaffold.rs`, which wrap their `chmod` in a bare `#[cfg(unix)]` block and silently do nothing elsewhere. The difference is who asked: there the `0o755` is an internal implementation detail, while here it is a contract the language states — the mode the script wrote down, or the `0o600` default it can rely on having instead. Silently ignoring either would hand back a fixture that is not the one the language promised, reported as a success.

## Alternatives Considered

### An `executable` modifier

Directly expresses the motivating case and needs no octal literacy. Rejected because it generalizes badly: selectively shared fixtures (`0o640`), owner-only executables (`0o700`), and deliberately unreadable ones (`0o000`) are equally real, and each would need a modifier of its own, leaving the language with several partly-overlapping concepts instead of one.

### Symbolic chmod syntax (`mode=u+x`)

Familiar to anyone who uses `chmod`. Rejected because symbolic syntax is relative to an existing mode, and `write` creates the file — there is nothing for it to be relative to. It would also make the final permission bits depend on the umask, contradicting decision 5.

### Accept several octal spellings (`0755`, `755`, `0o755`)

More forgiving of habit. Rejected because `0755` and `755` are the same characters with different meanings depending on a convention the reader has to know, and a mode that is silently misread is a fixture that is silently wrong. One spelling makes the radix explicit at every call site.

### Keep `chmod` in a `$` action

Adds nothing to the language. Rejected because it leaves fixture creation split across two steps and mixes setup with behavior under test — the same argument that justified the `write` step in the first place.

### Apply the mode after publishing the file

Simpler to write. Rejected because it breaks the atomicity `write` already guarantees: the file would be briefly visible at the target with a mode the script did not ask for, and a failed `chmod` would leave that file behind.

## Consequences

### Positive Consequences

- A fake command fixture is one step, expressed entirely in the DSL, with no incidental `chmod` action for a reader to classify as setup.
- Permission-sensitive fixtures beyond the executable bit — owner-only secrets, deliberately unreadable files — are expressible without further language additions.
- The final permission bits are a property of the script, not of the environment it runs in: the same script produces the same mode under any umask.
- `FileMode` makes setuid / setgid / sticky unrepresentable rather than merely undocumented, so the out-of-scope decision cannot be eroded by a future caller.

### Negative Consequences

- `write`'s surface syntax gains an optional element between two existing positions, which every future reader of the grammar and every `write`-aware tool has to account for.
- The scope decision recorded in [the `write` step ADR](20260704T183546Z_write-step-and-per-case-workspace-isolation.md) is now partly superseded; that ADR's non-goal list can no longer be read on its own.
- The non-Unix branch is unreachable on every platform CI builds, so it is not exercised by the test suite.

### Neutral Consequences

- Directory modes, symlink creation, and permission assertions remain out of scope, unchanged from the original `write` non-goals.
- `mode` contributes no new diagnostic code and no new semantic rule; it is entirely a grammar-level addition plus a runtime application step.
