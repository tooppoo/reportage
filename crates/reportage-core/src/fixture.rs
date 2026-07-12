//! Fixture reference resolution and materialization (#92).
//!
//! A `FixtureReference` (`@"<path>"`) names a static snapshot / fixture file
//! kept alongside a `*.repor` source file, used as expected file contents in
//! an assertion (see docs2/reference/semantics.md — Value literals, and
//! docs2/adr/20260706T170000Z_fixture-reference-value-syntax.md).
//!
//! `FixtureReference::parse` (in `model.rs`) performs lexical validation
//! (non-empty, relative, no `.` / `..` segment) at AST construction time.
//! That alone cannot prevent an escape via a symlink planted under the
//! `*.repor` directory, so this module performs the remaining checks that
//! require the filesystem and the referencing `*.repor` file's location:
//! resolving the fixture path against that file's directory, rejecting any
//! escape once both sides are canonicalized, and copying the validated
//! fixture into a runner-reserved area so assertion evaluation never reads
//! directly from the test-definition source tree.
//!
//! `evaluator::resolve_expected_contents` calls `resolve_fixture_source` /
//! `materialize_fixture` from `evaluate_file_expectation` and the `stdout` /
//! `stderr` equivalents to read a fixture's bytes during a live
//! `contents_equals` evaluation (#87). `text_equals` (#88) does not accept a
//! `FixtureReference` and so never calls into this module.

use std::io;
use std::path::{Path, PathBuf};

use crate::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticDetails};
use crate::model::FixtureReference;

/// Error resolving a [`FixtureReference`] against its `*.repor` source directory.
///
/// Distinct from [`crate::model::FixtureReferenceError`], which covers lexical
/// validation (empty, absolute, dot-segment) at parse time. This error covers
/// the filesystem-aware checks that can only run once the referencing
/// `*.repor` file's location is known.
#[derive(Debug)]
pub enum FixtureResolutionError {
    /// The resolved fixture source does not exist.
    Missing,
    /// The resolved fixture source exists but is not a regular file (e.g. a directory).
    NotARegularFile,
    /// The canonicalized fixture source lies outside the canonicalized
    /// `*.repor` directory, even though the raw path contained no `.` / `..`
    /// segment (e.g. escaped via a symlink).
    EscapesReporDirectory,
    /// An OS-level I/O error occurred while canonicalizing or inspecting the
    /// fixture source or the `*.repor` directory.
    Io(io::Error),
}

impl std::fmt::Display for FixtureResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FixtureResolutionError::Missing => write!(f, "fixture source does not exist"),
            FixtureResolutionError::NotARegularFile => {
                write!(f, "fixture source is not a regular file")
            }
            FixtureResolutionError::EscapesReporDirectory => write!(
                f,
                "fixture source resolves outside the referencing *.repor file's directory"
            ),
            FixtureResolutionError::Io(e) => write!(f, "I/O error resolving fixture source: {e}"),
        }
    }
}

impl std::error::Error for FixtureResolutionError {}

impl FixtureResolutionError {
    /// The stable, machine-readable diagnostic code for this error.
    /// See docs2/reference/diagnostics.md.
    pub const fn code(&self) -> DiagnosticCode {
        match self {
            FixtureResolutionError::Missing => DiagnosticCode::SemanticFixtureReferenceMissing,
            FixtureResolutionError::NotARegularFile => {
                DiagnosticCode::SemanticFixtureReferenceNotARegularFile
            }
            FixtureResolutionError::EscapesReporDirectory => {
                DiagnosticCode::SemanticFixtureReferenceEscapesReporDirectory
            }
            // An I/O error here is an environment failure, not a semantic
            // policy violation; it has no dedicated code of its own and is
            // reported as a missing source, the closest observable outcome.
            FixtureResolutionError::Io(_) => DiagnosticCode::SemanticFixtureReferenceMissing,
        }
    }

    /// Converts this error into the struct-based diagnostic model.
    pub fn to_diagnostic(&self) -> Diagnostic {
        Diagnostic {
            code: self.code(),
            message: self.to_string(),
            location: None,
            details: DiagnosticDetails::default(),
        }
    }
}

/// Resolves `fixture` against `repor_dir` (the directory containing the
/// referencing `*.repor` file), validating containment and regular-file-ness.
///
/// `repor_dir` must be the directory itself, not the `*.repor` file's own path.
///
/// Order of checks, per docs2/adr/20260706T170000Z_fixture-reference-value-syntax.md:
/// 1. Join `fixture`'s (already lexically-validated) path onto `repor_dir`.
/// 2. Canonicalize both `repor_dir` and the candidate path.
/// 3. Verify the canonicalized candidate lies under the canonicalized `repor_dir`.
/// 4. Verify the canonicalized candidate is a regular file.
///
/// Canonicalizing before the containment check is what catches an escape via
/// a symlink that a purely lexical (`.` / `..` segment) check cannot see.
pub fn resolve_fixture_source(
    repor_dir: &Path,
    fixture: &FixtureReference,
) -> Result<PathBuf, FixtureResolutionError> {
    let candidate = repor_dir.join(fixture.as_str());

    let canonical_dir = repor_dir
        .canonicalize()
        .map_err(FixtureResolutionError::Io)?;
    let canonical_candidate = match candidate.canonicalize() {
        Ok(path) => path,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Err(FixtureResolutionError::Missing);
        }
        Err(e) => return Err(FixtureResolutionError::Io(e)),
    };

    if !canonical_candidate.starts_with(&canonical_dir) {
        return Err(FixtureResolutionError::EscapesReporDirectory);
    }

    let meta = std::fs::metadata(&canonical_candidate).map_err(FixtureResolutionError::Io)?;
    if !meta.is_file() {
        return Err(FixtureResolutionError::NotARegularFile);
    }

    Ok(canonical_candidate)
}

/// Materializes an already-[`resolve_fixture_source`]-validated fixture into
/// `reserved_dir` (a runner-reserved area), copying its bytes under its own
/// file name.
///
/// `reserved_dir` is not a contractual sandbox path: v0 only guarantees the
/// bytes are available somewhere under `reserved_dir` for assertion
/// evaluation to read, never that a script can address it directly. Callers
/// are expected to provide a fresh, single-purpose directory (e.g. a new
/// `tempfile::TempDir`) per materialization, so no destination name can
/// collide across unrelated fixtures.
///
/// `resolve_fixture_source` proved `resolved_source` was a regular file at
/// the time it ran, but `resolved_source` is a plain path, not an open file
/// handle: something could replace it with a symlink in the window between
/// that check and this call (TOCTOU). `std::fs::copy` follows symlinks, so
/// without a re-check here a swapped-in symlink would be copied from
/// wherever it now points, silently defeating `resolve_fixture_source`'s
/// containment guarantee. This function therefore re-inspects
/// `resolved_source` with [`std::fs::symlink_metadata`] (which does not
/// follow a symlink, unlike `std::fs::metadata`) immediately before copying,
/// and refuses if it is no longer a plain regular file. This narrows the
/// TOCTOU window to the gap between that re-check and the copy syscall
/// itself; it does not close it entirely, since doing so would require an
/// `O_NOFOLLOW` open and read via a file descriptor rather than `std::fs::copy`.
pub fn materialize_fixture(resolved_source: &Path, reserved_dir: &Path) -> io::Result<PathBuf> {
    let meta = std::fs::symlink_metadata(resolved_source)?;
    if !meta.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "fixture source {} changed since resolution and is no longer a regular file",
                resolved_source.display()
            ),
        ));
    }

    let file_name = resolved_source
        .file_name()
        .expect("a resolved fixture source always has a file name");
    let destination = reserved_dir.join(file_name);
    std::fs::copy(resolved_source, &destination)?;
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_fixture_directly_under_repor_dir() {
        let repor_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(repor_dir.path().join("expected.json"), b"{}").unwrap();

        let fixture = FixtureReference::parse("expected.json").unwrap();
        let resolved = resolve_fixture_source(repor_dir.path(), &fixture).unwrap();

        assert_eq!(std::fs::read(&resolved).unwrap(), b"{}");
    }

    #[test]
    fn resolves_fixture_in_a_subdirectory() {
        let repor_dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(repor_dir.path().join("snapshots")).unwrap();
        std::fs::write(repor_dir.path().join("snapshots/stdout.json"), b"hello").unwrap();

        let fixture = FixtureReference::parse("snapshots/stdout.json").unwrap();
        let resolved = resolve_fixture_source(repor_dir.path(), &fixture).unwrap();

        assert_eq!(std::fs::read(&resolved).unwrap(), b"hello");
    }

    #[test]
    fn missing_fixture_source_is_rejected() {
        let repor_dir = tempfile::TempDir::new().unwrap();
        let fixture = FixtureReference::parse("does-not-exist.json").unwrap();

        let err = resolve_fixture_source(repor_dir.path(), &fixture).unwrap_err();
        assert!(matches!(err, FixtureResolutionError::Missing));
        assert_eq!(err.code().as_str(), "semantic.fixture_reference.missing");
    }

    #[test]
    fn directory_fixture_source_is_rejected() {
        let repor_dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(repor_dir.path().join("a-dir")).unwrap();
        let fixture = FixtureReference::parse("a-dir").unwrap();

        let err = resolve_fixture_source(repor_dir.path(), &fixture).unwrap_err();
        assert!(matches!(err, FixtureResolutionError::NotARegularFile));
        assert_eq!(
            err.code().as_str(),
            "semantic.fixture_reference.not_a_regular_file"
        );
    }

    // A fixture path with no `.`/`..` segment can still escape the `*.repor`
    // directory if a symlink planted under it points outside. Lexical
    // validation alone cannot catch this; only the canonicalize-then-contain
    // check here can.
    #[test]
    #[cfg(unix)]
    fn fixture_escaping_via_symlinked_directory_is_rejected() {
        let repor_dir = tempfile::TempDir::new().unwrap();
        let outside = tempfile::TempDir::new().unwrap();
        std::fs::write(outside.path().join("secret.txt"), b"top secret").unwrap();

        std::os::unix::fs::symlink(outside.path(), repor_dir.path().join("escape")).unwrap();

        let fixture = FixtureReference::parse("escape/secret.txt").unwrap();
        let err = resolve_fixture_source(repor_dir.path(), &fixture).unwrap_err();
        assert!(matches!(err, FixtureResolutionError::EscapesReporDirectory));
        assert_eq!(
            err.code().as_str(),
            "semantic.fixture_reference.escapes_repor_directory"
        );
    }

    // A symlinked *file* (as opposed to a symlinked directory) that still
    // resolves inside the `*.repor` directory is not an escape: only the
    // final canonicalized location matters, not whether any path component
    // is itself a symlink.
    #[test]
    #[cfg(unix)]
    fn fixture_via_symlink_that_still_resolves_inside_repor_dir_is_accepted() {
        let repor_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(repor_dir.path().join("real.json"), b"real").unwrap();
        std::os::unix::fs::symlink(
            repor_dir.path().join("real.json"),
            repor_dir.path().join("linked.json"),
        )
        .unwrap();

        let fixture = FixtureReference::parse("linked.json").unwrap();
        let resolved = resolve_fixture_source(repor_dir.path(), &fixture).unwrap();
        assert_eq!(std::fs::read(&resolved).unwrap(), b"real");
    }

    #[test]
    fn materialize_fixture_copies_bytes_under_reserved_dir() {
        let repor_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(repor_dir.path().join("expected.txt"), b"expected content").unwrap();
        let fixture = FixtureReference::parse("expected.txt").unwrap();
        let resolved = resolve_fixture_source(repor_dir.path(), &fixture).unwrap();

        let reserved_dir = tempfile::TempDir::new().unwrap();
        let materialized = materialize_fixture(&resolved, reserved_dir.path()).unwrap();

        assert!(materialized.starts_with(reserved_dir.path()));
        assert_eq!(std::fs::read(&materialized).unwrap(), b"expected content");
        // The original fixture source is untouched; materialization is a copy.
        assert_eq!(
            std::fs::read(repor_dir.path().join("expected.txt")).unwrap(),
            b"expected content"
        );
    }

    // Simulates a TOCTOU swap: `resolve_fixture_source` validated this path
    // as a regular file, but by the time `materialize_fixture` runs, it has
    // been replaced with a symlink pointing outside `repor_dir`. Without a
    // re-check immediately before the copy, `std::fs::copy` would follow the
    // symlink and silently defeat the containment guarantee already proved.
    #[test]
    #[cfg(unix)]
    fn materialize_fixture_rejects_source_swapped_for_a_symlink_after_resolution() {
        let repor_dir = tempfile::TempDir::new().unwrap();
        std::fs::write(repor_dir.path().join("expected.txt"), b"expected content").unwrap();
        let fixture = FixtureReference::parse("expected.txt").unwrap();
        let resolved = resolve_fixture_source(repor_dir.path(), &fixture).unwrap();

        let outside = tempfile::TempDir::new().unwrap();
        std::fs::write(outside.path().join("secret.txt"), b"top secret").unwrap();
        std::fs::remove_file(&resolved).unwrap();
        std::os::unix::fs::symlink(outside.path().join("secret.txt"), &resolved).unwrap();

        let reserved_dir = tempfile::TempDir::new().unwrap();
        let err = materialize_fixture(&resolved, reserved_dir.path()).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        // Nothing was copied from outside the fixture directory.
        assert!(!reserved_dir.path().join("expected.txt").exists());
    }
}
