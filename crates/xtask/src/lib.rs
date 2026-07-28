//! Repository maintenance tasks for reportage.
//!
//! `xtask` is not published and is not part of the reportage CLI. It exists so repository
//! tooling that needs real Rust code, rather than a shell script, has a home outside the
//! shipped crates. Recipes in the repository `Justfile` are the intended entry points.

pub mod json;
pub mod output;
pub mod schema_artifacts;
