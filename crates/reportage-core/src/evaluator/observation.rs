use std::path::Path;

use crate::result::{
    ContentsEqualsComparison, ContentsEqualsObservation, DirContainsObservation,
    DirExistsObservation, FileContentObservation, FileExistsObservation,
};

pub(super) fn observe_file_exists(workspace_root: &Path, path: &str) -> FileExistsObservation {
    match std::fs::metadata(workspace_root.join(path)) {
        Ok(meta) if meta.is_file() => FileExistsObservation::RegularFile,
        Ok(_) => FileExistsObservation::NotRegularFile,
        Err(_) => FileExistsObservation::Missing,
    }
}

/// Observes the actual side of a `file <"path"> contents_equals <expected>` expectation and, if
/// `path` is a readable regular file, compares its bytes against `expected` byte-for-byte.
///
/// `expected` has already been resolved successfully by the time this runs (see
/// `resolve_expected_contents`): a missing / non-regular / unreadable *actual* `path` is the
/// subject under test failing to produce the expected output, so — unlike an unresolvable
/// expected value — it is always an assertion failure, never a test-definition error.
pub(super) fn observe_file_contents_equals(
    workspace_root: &Path,
    path: &str,
    expected: Vec<u8>,
) -> ContentsEqualsObservation {
    let resolved = workspace_root.join(path);
    let meta = match std::fs::metadata(&resolved) {
        Ok(meta) => meta,
        Err(_) => return ContentsEqualsObservation::ActualMissing,
    };
    if !meta.is_file() {
        return ContentsEqualsObservation::ActualNotRegularFile;
    }
    let actual = match std::fs::read(&resolved) {
        Ok(bytes) => bytes,
        Err(_) => return ContentsEqualsObservation::ActualUnreadable,
    };
    ContentsEqualsObservation::Compared(ContentsEqualsComparison::compare(actual, expected))
}

/// Observes whether `path`, resolved against `workspace_root`, is a readable UTF-8 regular file containing `expected` as a plain substring.
///
/// Per docs/reference/semantic-diagnostics.md, missing / non-regular-file / unreadable / non-UTF-8 content are all "the `contains` precondition is unmet" — a single failure category distinct from "the file exists and is readable, but does not contain the expected substring".
pub(super) fn observe_file_contains(
    workspace_root: &Path,
    path: &str,
    expected: &str,
) -> FileContentObservation {
    let resolved = workspace_root.join(path);
    let meta = match std::fs::metadata(&resolved) {
        Ok(meta) => meta,
        Err(_) => return FileContentObservation::Missing,
    };
    if !meta.is_file() {
        return FileContentObservation::NotRegularFile;
    }
    let bytes = match std::fs::read(&resolved) {
        Ok(bytes) => bytes,
        Err(_) => return FileContentObservation::Unreadable,
    };
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => return FileContentObservation::NotUtf8,
    };
    if text.contains(expected) {
        FileContentObservation::Found
    } else {
        FileContentObservation::NotFound
    }
}

/// Evaluates a `dir <"path"> ...` expectation against the real filesystem.
///
/// The subject path policy (relative, no `.`/`..` segments, non-empty), and for `contains` the
/// entry name policy, are checked earlier, in `evaluate_case`, before this function runs.
///
/// `exp.path` is resolved relative to `workspace_root`, the current concrete case's isolated
/// workspace, exactly like `file` assertion paths. See docs/reference/semantics.md.
pub(super) fn observe_dir_exists(workspace_root: &Path, path: &str) -> DirExistsObservation {
    match std::fs::metadata(workspace_root.join(path)) {
        Ok(meta) if meta.is_dir() => DirExistsObservation::Directory,
        Ok(_) => DirExistsObservation::NotADirectory,
        Err(_) => DirExistsObservation::Missing,
    }
}

/// Observes whether `path`, resolved against `workspace_root`, is a directory containing an
/// entry named `entry_name` directly under it.
///
/// Never recurses, never glob-matches, and never inspects file content: `entry_name` is compared
/// against each direct child's raw entry name for an exact match, regardless of that entry's file
/// type. See docs/reference/semantics.md.
pub(super) fn observe_dir_contains(
    workspace_root: &Path,
    path: &str,
    entry_name: &str,
) -> DirContainsObservation {
    let resolved = workspace_root.join(path);
    let meta = match std::fs::metadata(&resolved) {
        Ok(meta) => meta,
        Err(_) => return DirContainsObservation::SubjectMissing,
    };
    if !meta.is_dir() {
        return DirContainsObservation::SubjectNotADirectory;
    }
    let entries = match std::fs::read_dir(&resolved) {
        Ok(entries) => entries,
        Err(_) => return DirContainsObservation::SubjectUnreadable,
    };
    let found = entries
        .filter_map(Result::ok)
        .any(|entry| entry.file_name() == std::ffi::OsStr::new(entry_name));
    if found {
        DirContainsObservation::Found
    } else {
        DirContainsObservation::EntryMissing
    }
}
