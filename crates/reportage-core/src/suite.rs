use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::{
    parser,
    result::{FileError, FileErrorKind, ValidatedFile},
};

#[derive(Debug)]
pub enum DiscoveryError {
    InvalidGlobPattern(String),
    EmptyPattern(String),
}

impl std::fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiscoveryError::InvalidGlobPattern(msg) => {
                write!(f, "invalid glob pattern: {msg}")
            }
            DiscoveryError::EmptyPattern(pattern) => {
                write!(
                    f,
                    "pattern '{pattern}' matched no files; each tests.path pattern must match at least one file"
                )
            }
        }
    }
}

impl std::error::Error for DiscoveryError {}

/// Expands glob patterns relative to `base_dir`, deduplicates, and returns a lexicographically sorted list of matched file paths.
///
/// Returns `DiscoveryError::EmptyPattern` if any pattern matches no files.
pub fn discover_files(
    base_dir: &Path,
    patterns: &[String],
) -> Result<Vec<PathBuf>, DiscoveryError> {
    let mut matched: BTreeSet<PathBuf> = BTreeSet::new();

    for pattern in patterns {
        let full_pattern = base_dir.join(pattern);
        let full_pattern_str = full_pattern.to_string_lossy();

        let entries = glob::glob(&full_pattern_str)
            .map_err(|e| DiscoveryError::InvalidGlobPattern(format!("{pattern}: {e}")))?;

        let mut count = 0usize;
        for entry in entries {
            match entry {
                Ok(path) if path.is_file() => {
                    matched.insert(path);
                    count += 1;
                }
                Ok(_) => {}
                Err(_) => {}
            }
        }

        if count == 0 {
            return Err(DiscoveryError::EmptyPattern(pattern.clone()));
        }
    }

    Ok(matched.into_iter().collect())
}

/// Reads and parses all `paths`. Returns validated files and any file-level errors.
///
/// All files are read and parsed before any `$` actions execute.
/// Errors from multiple files are collected in a single pass.
pub fn load_and_validate(paths: &[PathBuf]) -> (Vec<ValidatedFile>, Vec<FileError>) {
    let mut validated = Vec::new();
    let mut errors = Vec::new();

    for path in paths {
        match std::fs::read_to_string(path) {
            Err(e) => {
                errors.push(FileError {
                    source_path: path.clone(),
                    kind: FileErrorKind::ReadError(e.to_string()),
                });
            }
            Ok(source) => match parser::parse(&source) {
                Err(e) => {
                    errors.push(FileError {
                        source_path: path.clone(),
                        kind: FileErrorKind::ParseError {
                            diagnostic_code: e.code(),
                            message: e.to_string(),
                            location: e.to_diagnostic().location,
                        },
                    });
                }
                Ok(source_file) => {
                    // Execution only needs the projected Script; the source-level
                    // model (source text, case spans) is dropped here until a
                    // documentation-oriented consumer needs it past this point.
                    validated.push(ValidatedFile {
                        source_path: path.clone(),
                        script: source_file.into_script(),
                    });
                }
            },
        }
    }

    (validated, errors)
}
