# Reportage Documentation

## Contents

- [Guides](#group-1-guides)
  - [Getting started](#file-1-1-getting-started)
    - [Hello world](#case-1-1-1-hello-world)
    - [undocumented sibling case](#case-1-1-2-undocumented-sibling-case)
  - [Second guide](#file-1-2-second-guide)
    - [second guide case](#case-1-2-1-second-guide-case)
  - [Unordered guide](#file-1-3-unordered-guide)
    - [unordered guide case](#case-1-3-1-unordered-guide-case)
- [Index](#group-2-index)
  - [Case-less file](#file-2-1-case-less-file)
  - [undocumented](#file-2-2-undocumented)
    - [fallback case one](#case-2-2-1-fallback-case-one)
    - [fallback case two](#case-2-2-2-fallback-case-two)
- [advanced](#group-3-advanced)
  - [Lowercase group](#file-3-1-lowercase-group)
    - [lowercase group case](#case-3-1-1-lowercase-group-case)

<a id="group-1-guides"></a>
## Guides

<a id="file-1-1-getting-started"></a>
### Getting started

Source: sources/z-first.repor

The first guide.

Declared order 1, so it comes first in its group
even though its path sorts last.

<a id="case-1-1-1-hello-world"></a>
#### Hello world

Runs the smallest possible command.

```reportage
case "prints hello" {
  $ echo hello

  assert {
    exit 0
    stdout contains "hello"
  }
}
```

<a id="case-1-1-2-undocumented-sibling-case"></a>
#### undocumented sibling case

```reportage
case "undocumented sibling case" {
  $ true

  assert {
    exit 0
  }
}
```

<a id="file-1-2-second-guide"></a>
### Second guide

Source: sources/a-second.repor

<a id="case-1-2-1-second-guide-case"></a>
#### second guide case

```reportage
case "second guide case" {
  $ true

  assert {
    exit 0
  }
}
```

<a id="file-1-3-unordered-guide"></a>
### Unordered guide

Source: sources/m-unordered.repor

<a id="case-1-3-1-unordered-guide-case"></a>
#### unordered guide case

```reportage
case "unordered guide case" {
  $ true

  assert {
    exit 0
  }
}
```

<a id="group-2-index"></a>
## Index

<a id="file-2-1-case-less-file"></a>
### Case-less file

Source: sources/no-cases.repor

A valid source with zero cases still appears in the document.

<a id="file-2-2-undocumented"></a>
### undocumented

Source: sources/undocumented.repor

<a id="case-2-2-1-fallback-case-one"></a>
#### fallback case one

```reportage
case "fallback case one" {
  $ echo one

  assert {
    exit 0
  }
}
```

<a id="case-2-2-2-fallback-case-two"></a>
#### fallback case two

```reportage
case "fallback case two" {
  $ echo two

  assert {
    exit 0
  }
} # trailing comment, and the file ends without a final newline
```

<a id="group-3-advanced"></a>
## advanced

<a id="file-3-1-lowercase-group"></a>
### Lowercase group

Source: sources/advanced.repor

<a id="case-3-1-1-lowercase-group-case"></a>
#### lowercase group case

```reportage
case "lowercase group case" {
  $ true

  assert {
    exit 0
  }
}
```
