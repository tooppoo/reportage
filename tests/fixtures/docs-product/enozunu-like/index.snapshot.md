# Documentation

## Contents

- [Getting started](#group-1-getting-started)
  - [Initializing a workspace](#section-1-1-initializing-a-workspace)
    - [Creating the configuration](#example-1-1-1-creating-the-configuration)
    - [Re-running init](#example-1-1-2-re-running-init)
  - [Installing a skill](#section-1-2-installing-a-skill)
    - [Installing from a catalog](#example-1-2-1-installing-from-a-catalog)
    - [Installing an unknown skill](#example-1-2-2-installing-an-unknown-skill)

<a id="group-1-getting-started"></a>
## Getting started

<a id="section-1-1-initializing-a-workspace"></a>
### Initializing a workspace

A workspace is described by a single `demo.kdl` file that lists the
catalogs a project installs from.

<a id="example-1-1-1-creating-the-configuration"></a>
#### Creating the configuration

The configuration file is written first; `demo init` then records a lock
file describing exactly what it resolved.

`demo.kdl`

```
demo {
  catalog "github.com/example/catalog"
}
```

```sh
demo init
```

**Verified outcome**

- the exit code is 0
- standard output contains `resolved 1 catalog`
- `demo.lock.json` exists
- `demo.lock.json` contains `github.com/example/catalog`

<a id="example-1-1-2-re-running-init"></a>
#### Re-running init

A second run reuses the recorded lock file instead of resolving again.

`demo.kdl`

```
demo {
  catalog "github.com/example/catalog"
}
```

```sh
demo init
```

```sh
demo init
```

**Verified outcome**

- the exit code is 0
- standard output contains `up to date`
- not:
  - standard error contains `error`

<a id="section-1-2-installing-a-skill"></a>
### Installing a skill

<a id="example-1-2-1-installing-from-a-catalog"></a>
#### Installing from a catalog

**Preparation**

`demo.kdl`

```
demo {
  catalog "github.com/example/catalog"
}
```

```sh
demo init
```

**Steps**

```sh
demo install review
```

**Verified outcome**

- the exit code is 0
- the directory `.demo/skills` has an entry named `review`
- `.demo/skills/review/SKILL.md` exists

```sh
demo list
```

**Verified outcome**

- standard output contains `review`

<a id="example-1-2-2-installing-an-unknown-skill"></a>
#### Installing an unknown skill

An unknown name fails without touching the workspace.

**Preparation**

`demo.kdl`

```
demo {
  catalog "github.com/example/catalog"
}
```

```sh
demo init
```

**Steps**

```sh
demo install no-such-skill
```

**Verified outcome**

- the exit code is 1
- standard error contains `unknown skill`
- not:
  - the directory `.demo/skills` has an entry named `no-such-skill`
