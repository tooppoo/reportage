//! Corpus-wide contracts for the source-level model.
//!
//! `grammar_fixtures.rs` guards that every checked-in `.repor` fixture parses;
//! this test guards what the parser returns for that same corpus:
//! source text ownership, the case span contract, and the projection to the
//! execution model.
//! See docs/adr/20260712T090000Z_parser-returns-source-level-model.md.

use std::fs;

use reportage_core::parser::parse;

mod support;
use support::repor_corpus_paths as corpus_paths;

#[test]
fn source_file_invariants_hold_for_entire_fixture_corpus() {
    for path in corpus_paths() {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()));
        let source_file = parse(&source)
            .unwrap_or_else(|e| panic!("failed to parse fixture {}: {e}", path.display()));

        assert_eq!(
            source_file.source().as_str(),
            source,
            "{}: SourceFile must own text identical to the parser input",
            path.display()
        );

        let mut previous_end = 0usize;
        for source_case in source_file.cases() {
            let span = source_case.span();
            let display = path.display();

            assert!(
                span.start() <= span.end() && span.end() <= source.len(),
                "{display}: span {}..{} out of range",
                span.start(),
                span.end()
            );
            assert!(
                source.is_char_boundary(span.start()) && source.is_char_boundary(span.end()),
                "{display}: span {}..{} not on char boundaries",
                span.start(),
                span.end()
            );
            assert!(
                previous_end <= span.start(),
                "{display}: case spans must be in source order and non-overlapping"
            );
            previous_end = span.end();

            // The span starts at the case line's leading indentation and ends
            // with the closing brace line (its line ending included when one
            // exists in the source).
            let block = source_file.case_source(source_case);
            assert!(
                block.trim_start_matches([' ', '\t']).starts_with("case"),
                "{display}: span must start at the case keyword's line: {block:?}"
            );
            let last_line = block.trim_end_matches(['\n', '\r']).lines().last();
            assert!(
                last_line.is_some_and(|line| line.trim_start().starts_with('}')),
                "{display}: span must end on the closing brace line: {block:?}"
            );
        }
    }
}

#[test]
fn projection_preserves_case_names_and_order_for_entire_fixture_corpus() {
    for path in corpus_paths() {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read fixture {}: {e}", path.display()));
        let source_file = parse(&source)
            .unwrap_or_else(|e| panic!("failed to parse fixture {}: {e}", path.display()));

        let source_names: Vec<String> = source_file
            .cases()
            .iter()
            .map(|source_case| source_case.case().name.clone())
            .collect();

        let script = source_file.into_script();
        let script_names: Vec<String> = script.cases.iter().map(|case| case.name.clone()).collect();

        assert_eq!(
            source_names,
            script_names,
            "{}: projection must not add, drop, or reorder cases",
            path.display()
        );
    }
}
