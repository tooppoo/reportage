# Reject Combining --config with Explicit Script Arguments

- Status: Accepted
- Created: 2026-06-28T00:01:00Z

## Context

reportage supports three invocation modes:

1. `reportage <script>...` — explicit script mode
2. `reportage` — default config mode (loads `./reportage.kdl`)
3. `reportage --config <path>` — explicit config mode

The question is whether `reportage --config <path> <script>...` should be allowed.

## Decision

`reportage --config <path> <script>...` is rejected in v0. The CLI exits with code `3` and prints an error message.

## Alternatives Considered

Allowing combined mode would let users add extra scripts to a configured suite. This was considered but rejected because it creates ambiguity: which pattern-discovered files from the config are superseded by the explicit scripts? Does the config's test block still apply? These edge cases add complexity to validation and diagnostics with no clear v0 use case.

## Consequences

### Positive Consequences

- Each invocation mode has a single, unambiguous interpretation.
- Error messages are simpler: the user either configures discovery or provides files explicitly.

### Negative Consequences

- Users cannot ad-hoc supplement a configured suite with extra files in one command.

### Neutral Consequences

- The restriction may be lifted in a future version once real use cases justify it.
