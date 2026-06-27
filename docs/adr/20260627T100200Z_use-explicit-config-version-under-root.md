# Use an Explicit Config Version under the Reportage Root

- Status: Accepted
- Created: 2026-06-27T10:02:00Z

## Context

reportage configuration is expected to evolve. The runner should detect incompatible configuration before execution starts.

## Decision

Use a `reportage` root node and declare the config version under `reportage.config.version`.

```kdl
reportage {
  config {
    version 1
  }
}
```

## Alternatives Considered

`reportage 1` was considered, but it uses the root node argument for versioning and leaves less room for future metadata. A top-level `config-version 1` was considered, but it makes the document shape more fragmented.

## Consequences

### Positive Consequences

- Unsupported config versions can be rejected early.
- Future config metadata has a stable home.
- The document shape remains clearly namespaced.

### Negative Consequences

- The minimal config is slightly more verbose.

### Neutral Consequences

- `commands`, `tests`, and future settings should live under the `reportage` root.
