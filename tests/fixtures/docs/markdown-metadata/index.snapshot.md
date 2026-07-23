# Reportage Documentation

## Contents

- [# Raw *Group*](#group-1-raw-group)
  - [**Bold** <em>html</em> [link](https://example.com)](#file-1-1-bold-em-html-em-link-https-example-com)
    - [## fence stress](#case-1-1-1-fence-stress)
- [Slug Cases](#group-2-slug-cases)
  - [Dup Title](#file-2-1-dup-title)
    - [same case](#case-2-1-1-same-case)
    - [same case](#case-2-1-2-same-case)
  - [Dup Title](#file-2-2-dup-title)
    - [sibling with the same file title](#case-2-2-1-sibling-with-the-same-file-title)
  - [dup?title](#file-2-3-dup-title)
    - [normalizes to the same slug as Dup Title](#case-2-3-1-normalizes-to-the-same-slug-as-dup-title)
- [日本語グループ](#group-3)
  - [日本語ガイド](#file-3-1)
    - [ファイル生成](#case-3-1-1)

<a id="group-1-raw-group"></a>
## # Raw *Group*

<a id="file-1-1-bold-em-html-em-link-https-example-com"></a>
### **Bold** <em>html</em> [link](https://example.com)

Source: sources/raw-markdown.repor

First paragraph with **markdown** and <b>raw html</b>.

Second paragraph after a blank line.

<a id="case-1-1-1-fence-stress"></a>
#### ## fence stress

A backtick run inside the source forces a longer fence.

``````reportage
case "echoes backticks" {
  $ echo '`````'

  assert {
    exit 0
    stdout contains "`````"
  }
}
``````

<a id="group-2-slug-cases"></a>
## Slug Cases

<a id="file-2-1-dup-title"></a>
### Dup Title

Source: sources/duplicate-a.repor

<a id="case-2-1-1-same-case"></a>
#### same case

```reportage
case "first duplicate case" {
  $ true

  assert {
    exit 0
  }
}
```

<a id="case-2-1-2-same-case"></a>
#### same case

```reportage
case "second duplicate case" {
  $ true

  assert {
    exit 0
  }
}
```

<a id="file-2-2-dup-title"></a>
### Dup Title

Source: sources/duplicate-b.repor

<a id="case-2-2-1-sibling-with-the-same-file-title"></a>
#### sibling with the same file title

```reportage
case "sibling with the same file title" {
  $ true

  assert {
    exit 0
  }
}
```

<a id="file-2-3-dup-title"></a>
### dup?title

Source: sources/normalized-duplicate.repor

<a id="case-2-3-1-normalizes-to-the-same-slug-as-dup-title"></a>
#### normalizes to the same slug as Dup Title

```reportage
case "normalizes to the same slug as Dup Title" {
  $ true

  assert {
    exit 0
  }
}
```

<a id="group-3"></a>
## 日本語グループ

<a id="file-3-1"></a>
### 日本語ガイド

Source: sources/non-ascii.repor

ASCII slug を持たない title と group の例。

<a id="case-3-1-1"></a>
#### ファイル生成

コマンドによってファイルが生成されたことを検証する。

```reportage
case "file exists" {
  $ touch test.txt

  assert {
    file <"test.txt"> exists
  }
}
```
