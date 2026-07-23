# Reportage Documentation

## Contents

- [Filesystem](#group-1-filesystem)
  - [File assertions](#file-1-1-file-assertions)
    - [File creation](#case-1-1-1-file-creation)

<a id="group-1-filesystem"></a>
## Filesystem

<a id="file-1-1-file-assertions"></a>
### File assertions

Source: sources/file-assertions.repor

ファイルに対する assertion の使用例をまとめる。

<a id="case-1-1-1-file-creation"></a>
#### File creation

コマンドによってファイルが生成されたことを検証する。

```reportage
case "file exists" {
  $ touch test.txt

  assert {
    file <"test.txt"> exists
  }
}
```
