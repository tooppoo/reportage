# v0 Technical Selection

This document is the entry point for the current v0 technical choices.

Detailed specifications are split into:

- [Configuration](configuration.md)
- [Path Matching](path-matching.md)
- [Artifacts](artifacts.md)
- [Deferred Topics](TBD.md)

Architecture decisions are recorded under [ADR](adr/README.md).

Current accepted direction:

- Rust implementation
- KDL v2 configuration
- explicit config version under `reportage.config.version`
- project-local path-like config values with dot segments forbidden
- POSIX shell execution and PATH shims
- default artifact generation
- timeout deferred toward v0.1.x
- `--jobs` deferred toward v0.2.x
