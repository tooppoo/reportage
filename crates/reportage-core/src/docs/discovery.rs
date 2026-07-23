//! Documentation-specific source discovery: glob input resolution, eligibility
//! policy, and the display path contract.
//!
//! This is deliberately separate from `suite::discover_files`: documentation
//! discovery adds policies (the `.repor` extension requirement, symlink
//! rejection, per-pattern eligible-match validation, traversal error
//! propagation, display path normalization) that must not leak into the run
//! pipeline's discovery rules. See
//! docs/adr/20260723T070556Z_documentation-generation-command.md.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

/// One selected source: the path used for filesystem access and the
/// renderer-ready display path.
///
/// `display_path` follows the display path contract: relative to the working
/// directory, lexically normalized (no `.` / `..`), `/`-separated, UTF-8.
/// It is the identity used for deduplication, ordering, and every path shown
/// in the Documentation Catalog and the generated document. `load_path` is
/// only ever used to open the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredSource {
    pub load_path: PathBuf,
    pub display_path: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DiscoveryError {
    /// The pattern list itself is empty. The CLI already rejects this as a
    /// usage error; this variant keeps the library API total.
    NoPatterns,
    /// The pattern is not valid glob syntax.
    InvalidPattern { pattern: String, message: String },
    /// The pattern is an absolute path; v0 accepts only working-directory-relative patterns.
    AbsolutePattern { pattern: String },
    /// After lexical normalization the pattern can escape the working directory.
    PatternEscapesWorkingDirectory { pattern: String },
    /// The pattern matched no eligible `.repor` regular file.
    NoEligibleSource { pattern: String },
    /// A matched source path is not valid UTF-8, so it cannot be converted to
    /// a display path losslessly. The path itself is deliberately not echoed:
    /// a lossy rendering would misidentify the file.
    NonUtf8SourcePath { pattern: String },
    /// An OS-level I/O error occurred while walking the filesystem. Generation
    /// fails instead of producing a partial document.
    Traversal { pattern: String, message: String },
}

impl std::fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiscoveryError::NoPatterns => {
                write!(f, "at least one source pattern is required")
            }
            DiscoveryError::InvalidPattern { pattern, message } => {
                write!(f, "invalid glob pattern '{pattern}': {message}")
            }
            DiscoveryError::AbsolutePattern { pattern } => {
                write!(
                    f,
                    "pattern '{pattern}' is absolute; patterns must be relative to the current working directory"
                )
            }
            DiscoveryError::PatternEscapesWorkingDirectory { pattern } => {
                write!(
                    f,
                    "pattern '{pattern}' can escape the current working directory; patterns must stay inside it"
                )
            }
            DiscoveryError::NoEligibleSource { pattern } => {
                write!(
                    f,
                    "pattern '{pattern}' matched no eligible source; each pattern must match at least one regular '.repor' file (directories, symlinks, and other files are not eligible)"
                )
            }
            DiscoveryError::NonUtf8SourcePath { pattern } => {
                write!(
                    f,
                    "pattern '{pattern}' matched a path that is not valid UTF-8; non-UTF-8 source paths are not supported"
                )
            }
            DiscoveryError::Traversal { pattern, message } => {
                write!(
                    f,
                    "I/O error while resolving pattern '{pattern}': {message}"
                )
            }
        }
    }
}

impl std::error::Error for DiscoveryError {}

/// Resolves glob `patterns` against `base_dir` into a deduplicated,
/// deterministically ordered source list.
///
/// Each pattern must select at least one eligible source. Duplicate selections
/// (across patterns, or through different lexical routes to the same file) are
/// collapsed on the normalized display path, and the result is ordered by
/// `String` comparison of display paths, so the outcome never depends on glob
/// expansion order or filesystem enumeration order.
pub fn resolve_patterns(
    base_dir: &Path,
    patterns: &[String],
) -> Result<Vec<DiscoveredSource>, DiscoveryError> {
    if patterns.is_empty() {
        return Err(DiscoveryError::NoPatterns);
    }

    let mut selected: BTreeMap<String, PathBuf> = BTreeMap::new();

    for pattern in patterns {
        validate_pattern(pattern)?;

        let full_pattern = base_dir.join(pattern);
        let full_pattern_str =
            full_pattern
                .to_str()
                .ok_or_else(|| DiscoveryError::NonUtf8SourcePath {
                    pattern: pattern.clone(),
                })?;

        let entries = glob::glob(full_pattern_str).map_err(|e| DiscoveryError::InvalidPattern {
            pattern: pattern.clone(),
            message: e.to_string(),
        })?;

        let mut eligible_matches = 0usize;
        for entry in entries {
            let path = entry.map_err(|e| DiscoveryError::Traversal {
                pattern: pattern.clone(),
                message: e.to_string(),
            })?;

            let relative = path.strip_prefix(base_dir).unwrap_or(&path).to_path_buf();
            if relative.to_str().is_none() {
                return Err(DiscoveryError::NonUtf8SourcePath {
                    pattern: pattern.clone(),
                });
            }

            if !is_eligible(base_dir, &relative) {
                continue;
            }

            let display_path = normalize_display_path(&relative).ok_or_else(|| {
                DiscoveryError::PatternEscapesWorkingDirectory {
                    pattern: pattern.clone(),
                }
            })?;

            eligible_matches += 1;
            selected.entry(display_path).or_insert(path);
        }

        if eligible_matches == 0 {
            return Err(DiscoveryError::NoEligibleSource {
                pattern: pattern.clone(),
            });
        }
    }

    Ok(selected
        .into_iter()
        .map(|(display_path, load_path)| DiscoveredSource {
            load_path,
            display_path,
        })
        .collect())
}

/// Rejects absolute patterns and patterns that can escape the working
/// directory after lexical normalization.
///
/// The depth simulation counts `**` as zero components because it may match
/// zero directories: `**/../x` therefore escapes even though every other
/// wildcard guarantees at least one matched component. Wildcards never match
/// a literal `..` (directory enumeration never yields dot entries), so only
/// literal `..` components can move upward.
fn validate_pattern(pattern: &str) -> Result<(), DiscoveryError> {
    let path = Path::new(pattern);
    if path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, Component::RootDir | Component::Prefix(_)))
    {
        return Err(DiscoveryError::AbsolutePattern {
            pattern: pattern.to_string(),
        });
    }

    let mut min_depth: i64 = 0;
    for component in pattern.split('/') {
        match component {
            "" | "." | "**" => {}
            ".." => {
                min_depth -= 1;
                if min_depth < 0 {
                    return Err(DiscoveryError::PatternEscapesWorkingDirectory {
                        pattern: pattern.to_string(),
                    });
                }
            }
            _ => min_depth += 1,
        }
    }
    Ok(())
}

/// Documentation eligibility: a regular file with the `.repor` extension,
/// reached without any symlink component between `base_dir` and the file.
///
/// The symlink walk checks every accumulated prefix of the pre-normalization
/// relative path: `sym/../x.repor` must be judged on `sym` (a symlink) before
/// lexical normalization collapses it away.
fn is_eligible(base_dir: &Path, relative: &Path) -> bool {
    if relative.extension().and_then(|e| e.to_str()) != Some("repor") {
        return false;
    }

    let mut current = base_dir.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => return false,
            Ok(_) => {}
            Err(_) => return false,
        }
    }

    std::fs::symlink_metadata(base_dir.join(relative))
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
}

/// Lexically normalizes a matched relative path into the display form:
/// `.` removed, `..` resolved against preceding components, `/` separators.
///
/// Returns `None` when a `..` would climb past the working directory; pattern
/// validation makes that unreachable for paths produced by glob matching, but
/// the contract is enforced here as well rather than assumed.
fn normalize_display_path(relative: &Path) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str()?.to_string()),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, "").unwrap();
    }

    #[test]
    fn resolves_literal_relative_path() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("a.repor"));

        let sources = resolve_patterns(dir.path(), &["a.repor".to_string()]).unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].display_path, "a.repor");
        assert_eq!(sources[0].load_path, dir.path().join("a.repor"));
    }

    #[test]
    fn supports_star_question_class_and_recursive_globs() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("ab.repor"));
        touch(&dir.path().join("nested/deep/c.repor"));

        for pattern in ["*.repor", "a?.repor", "[a]b.repor", "**/*.repor"] {
            let sources = resolve_patterns(dir.path(), &[pattern.to_string()]).unwrap();
            assert!(
                sources.iter().any(|s| s.display_path == "ab.repor"),
                "pattern {pattern} must match ab.repor"
            );
        }

        let sources = resolve_patterns(dir.path(), &["**/*.repor".to_string()]).unwrap();
        assert_eq!(
            sources
                .iter()
                .map(|s| s.display_path.as_str())
                .collect::<Vec<_>>(),
            vec!["ab.repor", "nested/deep/c.repor"]
        );
    }

    #[test]
    fn rejects_absolute_pattern() {
        let dir = tempfile::tempdir().unwrap();
        let err = resolve_patterns(dir.path(), &["/etc/*.repor".to_string()]).unwrap_err();
        assert!(matches!(err, DiscoveryError::AbsolutePattern { .. }));
    }

    #[test]
    fn rejects_pattern_escaping_the_working_directory() {
        let dir = tempfile::tempdir().unwrap();
        for pattern in ["../a.repor", "sub/../../a.repor", "**/../a.repor"] {
            let err = resolve_patterns(dir.path(), &[pattern.to_string()]).unwrap_err();
            assert!(
                matches!(err, DiscoveryError::PatternEscapesWorkingDirectory { .. }),
                "pattern {pattern} must be rejected"
            );
        }
    }

    #[test]
    fn allows_parent_components_that_stay_inside() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("a.repor"));
        std::fs::create_dir(dir.path().join("sub")).unwrap();

        let sources = resolve_patterns(dir.path(), &["sub/../a.repor".to_string()]).unwrap();
        assert_eq!(sources[0].display_path, "a.repor");
    }

    #[test]
    fn invalid_glob_syntax_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let err = resolve_patterns(dir.path(), &["[".to_string()]).unwrap_err();
        assert!(matches!(err, DiscoveryError::InvalidPattern { .. }));
    }

    #[test]
    fn pattern_matching_nothing_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let err = resolve_patterns(dir.path(), &["missing/*.repor".to_string()]).unwrap_err();
        assert!(matches!(err, DiscoveryError::NoEligibleSource { .. }));
    }

    #[test]
    fn non_repor_files_and_directories_are_not_eligible() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("notes.txt"));
        std::fs::create_dir_all(dir.path().join("casedir.repor")).unwrap();

        let err = resolve_patterns(dir.path(), &["*".to_string()]).unwrap_err();
        assert!(matches!(err, DiscoveryError::NoEligibleSource { .. }));
    }

    #[test]
    fn directories_are_not_recursed_implicitly() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("sub/a.repor"));

        // The directory itself matches `*` but is not an eligible source, and
        // nothing recurses into it without an explicit `**`.
        let err = resolve_patterns(dir.path(), &["*".to_string()]).unwrap_err();
        assert!(matches!(err, DiscoveryError::NoEligibleSource { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_file_is_not_eligible() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("real.repor"));
        std::os::unix::fs::symlink(dir.path().join("real.repor"), dir.path().join("link.repor"))
            .unwrap();

        let err = resolve_patterns(dir.path(), &["link.repor".to_string()]).unwrap_err();
        assert!(matches!(err, DiscoveryError::NoEligibleSource { .. }));

        // A wildcard selecting both keeps only the real file.
        let sources = resolve_patterns(dir.path(), &["*.repor".to_string()]).unwrap();
        assert_eq!(
            sources
                .iter()
                .map(|s| s.display_path.as_str())
                .collect::<Vec<_>>(),
            vec!["real.repor"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_directory_component_is_not_eligible() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("real/a.repor"));
        std::os::unix::fs::symlink(dir.path().join("real"), dir.path().join("linkdir")).unwrap();

        let err = resolve_patterns(dir.path(), &["linkdir/*.repor".to_string()]).unwrap_err();
        assert!(matches!(err, DiscoveryError::NoEligibleSource { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_component_before_parent_dir_is_not_eligible() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("a.repor"));
        std::os::unix::fs::symlink(dir.path().join("elsewhere"), dir.path().join("sym")).unwrap();

        // `sym/../a.repor` lexically normalizes to `a.repor`, but the route
        // goes through a symlink and must not be accepted on that route.
        let err = resolve_patterns(dir.path(), &["sym/../a.repor".to_string()]).unwrap_err();
        assert!(matches!(err, DiscoveryError::NoEligibleSource { .. }));
    }

    #[test]
    fn duplicate_selections_collapse_on_display_path() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("a.repor"));
        std::fs::create_dir(dir.path().join("sub")).unwrap();

        let sources = resolve_patterns(
            dir.path(),
            &[
                "a.repor".to_string(),
                "*.repor".to_string(),
                "sub/../a.repor".to_string(),
            ],
        )
        .unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].display_path, "a.repor");
    }

    #[test]
    fn ordering_is_deterministic_and_pattern_order_independent() {
        let dir = tempfile::tempdir().unwrap();
        touch(&dir.path().join("b.repor"));
        touch(&dir.path().join("a.repor"));
        touch(&dir.path().join("Z.repor"));

        let forward = resolve_patterns(
            dir.path(),
            &[
                "b.repor".to_string(),
                "a.repor".to_string(),
                "Z.repor".to_string(),
            ],
        )
        .unwrap();
        let reverse = resolve_patterns(
            dir.path(),
            &[
                "Z.repor".to_string(),
                "a.repor".to_string(),
                "b.repor".to_string(),
            ],
        )
        .unwrap();

        assert_eq!(forward, reverse);
        // Case-sensitive, locale-independent String ordering: uppercase sorts first.
        assert_eq!(
            forward
                .iter()
                .map(|s| s.display_path.as_str())
                .collect::<Vec<_>>(),
            vec!["Z.repor", "a.repor", "b.repor"]
        );
    }

    #[test]
    fn empty_pattern_list_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let err = resolve_patterns(dir.path(), &[]).unwrap_err();
        assert_eq!(err, DiscoveryError::NoPatterns);
    }
}
