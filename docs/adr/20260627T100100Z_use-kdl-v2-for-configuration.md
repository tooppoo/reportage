# Use KDL v2 for Configuration

- Status: Accepted
- Created: 2026-06-27T10:01:00Z

## Context

reportage needs project configuration for config versioning, registered commands, test discovery patterns, and future adapter and artifact settings.

## Decision

Use KDL v2 for reportage configuration. The default config file is `reportage.kdl`.

## Alternatives Considered

TOML was considered but is less natural for nested command and test structures. YAML was rejected due to ambiguous parsing behavior. JSON was rejected as too verbose for hand-written project configuration.

## Consequences

### Positive Consequences

- Configuration remains concise and readable.
- Nested command and test discovery structures are natural.
- KDL Schema may be considered later for editor support.

### Negative Consequences

- The Rust implementation must select a KDL parser with KDL v2 support.
- Runtime validation must distinguish KDL parse success from reportage semantic validity.

### Neutral Consequences

- Runtime validation should initially be implemented by reportage's own validator.
