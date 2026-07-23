//! Documentation source loading boundary: reads and parses selected sources
//! while keeping the unconsumed source-level model.
//!
//! Unlike `suite::load_and_validate`, this loader never projects a
//! [`SourceFile`] into the execution `Script`: the Documentation Catalog needs
//! the source text, case spans, and documentation metadata that
//! `SourceFile::into_script` drops. See
//! docs/adr/20260723T070556Z_documentation-generation-command.md.

use std::path::PathBuf;

use crate::parser;
use crate::source::SourceFile;

use super::discovery::DiscoveredSource;

/// One loaded source: the filesystem access path, the display path, and the
/// unconsumed source-level model.
#[derive(Debug)]
pub struct LoadedSourceFile {
    pub load_path: PathBuf,
    pub display_path: String,
    pub source: SourceFile,
}

/// A read or parse failure for one selected source, reported under its
/// display path.
#[derive(Debug, PartialEq, Eq)]
pub struct SourceLoadError {
    pub display_path: String,
    pub detail: String,
}

impl std::fmt::Display for SourceLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.display_path, self.detail)
    }
}

/// Reads and parses every selected source, in the given (display path) order.
///
/// All sources are processed so that every failing file is reported at once,
/// but a single error already fails the whole generation: no Catalog is built
/// and no document is written from a partially loaded source set.
pub fn load_sources(
    sources: Vec<DiscoveredSource>,
) -> Result<Vec<LoadedSourceFile>, Vec<SourceLoadError>> {
    let mut loaded = Vec::with_capacity(sources.len());
    let mut errors = Vec::new();

    for source in sources {
        match std::fs::read_to_string(&source.load_path) {
            Err(e) => errors.push(SourceLoadError {
                display_path: source.display_path,
                detail: format!("cannot read source: {e}"),
            }),
            Ok(text) => match parser::parse(&text) {
                Err(e) => errors.push(SourceLoadError {
                    display_path: source.display_path,
                    detail: e.to_string(),
                }),
                Ok(source_file) => loaded.push(LoadedSourceFile {
                    load_path: source.load_path,
                    display_path: source.display_path,
                    source: source_file,
                }),
            },
        }
    }

    if errors.is_empty() {
        Ok(loaded)
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn discovered(dir: &Path, name: &str, contents: &str) -> DiscoveredSource {
        let path = dir.join(name);
        std::fs::write(&path, contents).unwrap();
        DiscoveredSource {
            load_path: path,
            display_path: name.to_string(),
        }
    }

    const VALID_CASE: &str = "case \"ok\" {\n  $ true\n  assert {\n    exit 0\n  }\n}\n";

    #[test]
    fn loads_sources_keeping_the_unconsumed_source_model() {
        let dir = tempfile::tempdir().unwrap();
        let sources = vec![discovered(dir.path(), "a.repor", VALID_CASE)];

        let loaded = load_sources(sources).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].display_path, "a.repor");
        // The source-level model is intact: the case span still slices the
        // original text.
        let case = &loaded[0].source.cases()[0];
        assert!(
            loaded[0]
                .source
                .case_source(case)
                .starts_with("case \"ok\"")
        );
    }

    #[test]
    fn a_single_parse_error_fails_the_whole_load() {
        let dir = tempfile::tempdir().unwrap();
        let sources = vec![
            discovered(dir.path(), "a.repor", VALID_CASE),
            discovered(dir.path(), "b.repor", "case \"broken\" {\n"),
        ];

        let errors = load_sources(sources).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].display_path, "b.repor");
        assert!(errors[0].detail.contains("parse error"));
    }

    #[test]
    fn read_and_parse_errors_are_collected_in_input_order() {
        let dir = tempfile::tempdir().unwrap();
        let missing = DiscoveredSource {
            load_path: dir.path().join("missing.repor"),
            display_path: "missing.repor".to_string(),
        };
        let broken = discovered(dir.path(), "z-broken.repor", "case {");

        let errors = load_sources(vec![missing, broken]).unwrap_err();
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0].display_path, "missing.repor");
        assert!(errors[0].detail.starts_with("cannot read source:"));
        assert_eq!(errors[1].display_path, "z-broken.repor");
    }
}
