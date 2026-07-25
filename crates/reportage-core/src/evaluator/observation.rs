use std::ffi::OsString;
use std::fs::ReadDir;
use std::path::Path;

use crate::result::{DirExistsObservation, FileExistsObservation};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum FileObservation {
    Missing,
    NotRegularFile,
    Unreadable,
    Bytes(Vec<u8>),
}

#[derive(Debug)]
pub(super) enum DirObservation {
    Missing,
    NotADirectory,
    Unreadable,
    Entries(DirEntries),
}

#[derive(Debug)]
pub(super) struct DirEntries {
    entries: ReadDir,
}

impl Iterator for DirEntries {
    type Item = OsString;

    fn next(&mut self) -> Option<Self::Item> {
        self.entries
            .by_ref()
            .find_map(|entry| entry.ok().map(|entry| entry.file_name()))
    }
}

pub(super) fn observe_file_exists(workspace_root: &Path, path: &str) -> FileExistsObservation {
    match std::fs::metadata(workspace_root.join(path)) {
        Ok(meta) if meta.is_file() => FileExistsObservation::RegularFile,
        Ok(_) => FileExistsObservation::NotRegularFile,
        Err(_) => FileExistsObservation::Missing,
    }
}

/// Acquires the actual-side state and bytes of a filesystem path without applying matcher semantics.
pub(super) fn observe_file(workspace_root: &Path, path: &str) -> FileObservation {
    let resolved = workspace_root.join(path);
    let meta = match std::fs::metadata(&resolved) {
        Ok(meta) => meta,
        Err(_) => return FileObservation::Missing,
    };
    if !meta.is_file() {
        return FileObservation::NotRegularFile;
    }
    match std::fs::read(&resolved) {
        Ok(bytes) => FileObservation::Bytes(bytes),
        Err(_) => FileObservation::Unreadable,
    }
}

pub(super) fn observe_dir_exists(workspace_root: &Path, path: &str) -> DirExistsObservation {
    match std::fs::metadata(workspace_root.join(path)) {
        Ok(meta) if meta.is_dir() => DirExistsObservation::Directory,
        Ok(_) => DirExistsObservation::NotADirectory,
        Err(_) => DirExistsObservation::Missing,
    }
}

/// Acquires the actual-side state and direct-child entry names of a filesystem path without applying matcher semantics.
pub(super) fn observe_dir(workspace_root: &Path, path: &str) -> DirObservation {
    let resolved = workspace_root.join(path);
    let meta = match std::fs::metadata(&resolved) {
        Ok(meta) => meta,
        Err(_) => return DirObservation::Missing,
    };
    if !meta.is_dir() {
        return DirObservation::NotADirectory;
    }
    let entries = match std::fs::read_dir(&resolved) {
        Ok(entries) => entries,
        Err(_) => return DirObservation::Unreadable,
    };
    DirObservation::Entries(DirEntries { entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_observation_returns_raw_bytes_without_decoding() {
        let workspace = tempfile::tempdir().unwrap();
        std::fs::write(workspace.path().join("actual.bin"), [0xff, 0x00]).unwrap();

        assert_eq!(
            observe_file(workspace.path(), "actual.bin"),
            FileObservation::Bytes(vec![0xff, 0x00])
        );
    }

    #[test]
    fn directory_observation_returns_all_direct_child_names() {
        let workspace = tempfile::tempdir().unwrap();
        let subject = workspace.path().join("subject");
        std::fs::create_dir(&subject).unwrap();
        std::fs::write(subject.join("first"), []).unwrap();
        std::fs::create_dir(subject.join("second")).unwrap();

        let DirObservation::Entries(entries) = observe_dir(workspace.path(), "subject") else {
            panic!("expected direct-child entries");
        };
        let mut entries: Vec<_> = entries.collect();
        entries.sort();

        assert_eq!(
            entries,
            vec![OsString::from("first"), OsString::from("second")]
        );
    }
}
