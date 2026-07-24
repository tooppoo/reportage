use std::path::PathBuf;

use crate::result::ActionResult;

mod execution;
mod expectation;
mod observation;

pub use execution::evaluate;
pub use expectation::{ExpectedContentsError, evaluate_expectation_at_checkpoint};

#[cfg(test)]
mod tests;

/// Observable evidence available at a point in case execution.
///
/// A checkpoint is an evidence context, not a full filesystem snapshot.
/// The initial checkpoint has workspace state but no last action result.
///
/// See docs/reference/semantics.md — Checkpoint.
pub struct Checkpoint {
    pub workspace: WorkspaceState,
    pub last_action: Option<ActionResult>,
    /// Directory containing the `*.repor` file this case was loaded from, used to resolve a
    /// `contents_equals` expected `FixtureReference` (`@"<path>"`) relative to it. See
    /// `fixture::resolve_fixture_source`.
    pub repor_dir: PathBuf,
}

impl Checkpoint {
    /// The initial checkpoint: workspace state present, no last action result.
    pub fn initial(workspace_root: PathBuf, repor_dir: PathBuf) -> Self {
        Self {
            workspace: WorkspaceState {
                root: workspace_root,
            },
            last_action: None,
            repor_dir,
        }
    }

    /// An action-updated checkpoint after `$ ...` completes.
    pub fn after_action(action: ActionResult, workspace_root: PathBuf, repor_dir: PathBuf) -> Self {
        Self {
            workspace: WorkspaceState {
                root: workspace_root,
            },
            last_action: Some(action),
            repor_dir,
        }
    }
}

/// Observable workspace state: the concrete case's isolated workspace root.
///
/// File and directory expectations, and `write` steps, resolve paths
/// relative to `root`. See docs/reference/semantics.md — Workspace lifecycle.
pub struct WorkspaceState {
    pub root: PathBuf,
}
