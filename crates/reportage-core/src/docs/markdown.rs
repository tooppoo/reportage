//! Markdown renderer: serializes a [`DocumentationCatalog`] into the fixed
//! single-document Markdown contract (issue #171).
//!
//! The serialization contract (fixed by generated-document snapshots and the
//! reference documentation, docs/reference/docs-generation.md):
//!
//! - the document starts with `# <document title>` from the render options
//! - `## Contents` follows, with a nested group / file / case list linking to
//!   the explicit anchors
//! - heading levels are fixed: group `##`, file `###`, case `####`, each
//!   preceded by an explicit `<a id="...">` anchor on its own line
//! - file sections carry `Source: <source_path>`; file and case descriptions
//!   follow their heading and are omitted entirely when absent
//! - case sources are wrapped in a `reportage` fenced code block whose fence
//!   is longer than the longest backtick run in the source (minimum 3)
//! - renderer-generated blocks are separated by exactly one empty line, line
//!   endings are normalized to LF, and the document ends with exactly one LF
//!
//! Metadata (titles, group names, source paths, descriptions) is inserted
//! verbatim: no Markdown escaping, sanitization, trimming, or dedenting.
//! Input that itself contains Markdown syntax, raw HTML, or newlines may
//! therefore break the rendered layout; that is the input's responsibility,
//! and sanitizing untrusted sources belongs to the downstream publisher. The
//! explicit anchor IDs are the exception: they are renderer-generated ASCII
//! (never raw metadata inside an HTML attribute), unique by construction
//! through the 1-based Catalog structure indices, and independent of any
//! Markdown implementation's implicit slug rules. See
//! docs/adr/20260723T143711Z_markdown-documentation-format.md.
//!
//! Beyond the fence wrapper, the structural final newline before a closing
//! fence, and LF normalization, case source content is never dropped or
//! replaced.

use super::catalog::DocumentationCatalog;
use super::render::{DocumentRenderer, RenderOptions};

/// The `markdown` format: renders a catalog into one Markdown document.
pub struct MarkdownRenderer;

impl DocumentRenderer for MarkdownRenderer {
    fn render(&self, catalog: &DocumentationCatalog, options: &RenderOptions) -> String {
        // The table of contents and the section anchors are built in the same
        // pass so an anchor can never diverge from the entry linking to it.
        let mut toc_lines: Vec<String> = Vec::new();
        let mut section_blocks: Vec<String> = Vec::new();

        for (group_number, group) in numbered(&catalog.groups) {
            let group_anchor = anchor_id(&format!("group-{group_number}"), &group.name);
            toc_lines.push(toc_entry(0, &group.name, &group_anchor));
            section_blocks.push(anchored_heading("##", &group_anchor, &group.name));

            for (file_number, file) in numbered(&group.files) {
                let file_anchor =
                    anchor_id(&format!("file-{group_number}-{file_number}"), &file.title);
                toc_lines.push(toc_entry(1, &file.title, &file_anchor));
                section_blocks.push(anchored_heading("###", &file_anchor, &file.title));
                section_blocks.push(format!("Source: {}", lf(&file.source_path)));
                if let Some(description) = &file.description {
                    section_blocks.push(description_block(description));
                }

                for (case_number, case) in numbered(&file.cases) {
                    let case_anchor = anchor_id(
                        &format!("case-{group_number}-{file_number}-{case_number}"),
                        &case.title,
                    );
                    toc_lines.push(toc_entry(2, &case.title, &case_anchor));
                    section_blocks.push(anchored_heading("####", &case_anchor, &case.title));
                    if let Some(description) = &case.description {
                        section_blocks.push(description_block(description));
                    }
                    section_blocks.push(fenced_source(&case.source));
                }
            }
        }

        let mut blocks = vec![
            format!("# {}", lf(&options.document_title)),
            "## Contents".to_string(),
        ];
        if !toc_lines.is_empty() {
            blocks.push(toc_lines.join("\n"));
        }
        blocks.extend(section_blocks);
        blocks.join("\n\n") + "\n"
    }

    fn file_extension(&self) -> &'static str {
        "md"
    }
}

/// 1-based iteration: the Catalog structure indices in anchor IDs start at 1.
fn numbered<T>(items: &[T]) -> impl Iterator<Item = (usize, &T)> {
    items.iter().enumerate().map(|(i, item)| (i + 1, item))
}

/// CRLF normalized to LF; every metadata value passes through here so the
/// document carries no CRLF sequence. A lone CR is not a line ending here
/// and passes through unchanged, matching the plain format.
fn lf(value: &str) -> String {
    value.replace("\r\n", "\n")
}

/// A description block: the value verbatim except for LF normalization and
/// dropping the single final newline a heredoc value carries, so whether the
/// metadata ends with a newline never changes block separation — the same
/// rule the plain format applies to every labeled value.
fn description_block(value: &str) -> String {
    let normalized = lf(value);
    normalized
        .strip_suffix('\n')
        .unwrap_or(&normalized)
        .to_string()
}

/// One table-of-contents line, indented two spaces per nesting depth. The
/// title lands verbatim in the link text; the link target is the generated
/// ASCII anchor.
fn toc_entry(depth: usize, title: &str, anchor: &str) -> String {
    format!("{}- [{}](#{anchor})", "  ".repeat(depth), lf(title))
}

/// One heading block: the explicit anchor immediately above the heading line,
/// so the pair always travels as a unit between empty-line block separators.
fn anchored_heading(marker: &str, anchor: &str, title: &str) -> String {
    format!("<a id=\"{anchor}\"></a>\n{marker} {}", lf(title))
}

/// The anchor ID for one structure index prefix (e.g. `file-1-2`) and its
/// display title. Uniqueness comes from the prefix alone; the slug is a
/// readability aid and is omitted when normalization leaves nothing.
fn anchor_id(index_prefix: &str, title: &str) -> String {
    match slug(title) {
        Some(slug) => format!("{index_prefix}-{slug}"),
        None => index_prefix.to_string(),
    }
}

/// The fixed slug normalization: ASCII alphanumerics are kept (letters
/// lowercased), every other run of characters collapses into one `-`, and
/// leading/trailing `-` are stripped. `None` when nothing remains.
fn slug(title: &str) -> Option<String> {
    let mut out = String::new();
    let mut separate = false;
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            if separate && !out.is_empty() {
                out.push('-');
            }
            separate = false;
            out.push(c.to_ascii_lowercase());
        } else {
            separate = true;
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

/// The case source wrapped in a `reportage` fenced code block.
///
/// The fence must be computed on the exact Catalog source (the contract's
/// stated stage); LF normalization cannot change backtick runs, so the result
/// is the same either way. A source without a final newline gets one
/// structural LF so the closing fence sits on its own line; a source with one
/// gets nothing extra, so no blank line appears before the fence.
fn fenced_source(source: &str) -> String {
    let fence = "`".repeat(fence_length(source));
    let mut body = lf(source);
    if !body.is_empty() && !body.ends_with('\n') {
        body.push('\n');
    }
    format!("{fence}reportage\n{body}{fence}")
}

/// One longer than the longest backtick run in the source, and at least 3.
fn fence_length(source: &str) -> usize {
    (longest_backtick_run(source) + 1).max(3)
}

fn longest_backtick_run(source: &str) -> usize {
    let mut longest = 0;
    let mut current = 0;
    for c in source.chars() {
        if c == '`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    longest
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docs::catalog::{
        DocumentationCatalog, DocumentationGroup, DocumentedCase, DocumentedFile,
    };

    fn render(catalog: &DocumentationCatalog) -> String {
        MarkdownRenderer.render(catalog, &RenderOptions::default())
    }

    fn file(title: &str, cases: Vec<DocumentedCase>) -> DocumentedFile {
        DocumentedFile {
            title: title.to_string(),
            description: None,
            source_path: format!("{title}.repor"),
            cases,
        }
    }

    fn case(title: &str, source: &str) -> DocumentedCase {
        DocumentedCase {
            title: title.to_string(),
            description: None,
            source: source.to_string(),
        }
    }

    fn single_group(files: Vec<DocumentedFile>) -> DocumentationCatalog {
        DocumentationCatalog {
            groups: vec![DocumentationGroup {
                name: "Filesystem".to_string(),
                files,
            }],
        }
    }

    fn representative_catalog(case_source: &str) -> DocumentationCatalog {
        single_group(vec![DocumentedFile {
            title: "File assertions".to_string(),
            description: Some("About files.".to_string()),
            source_path: "examples/file-assertions.repor".to_string(),
            cases: vec![DocumentedCase {
                title: "File creation".to_string(),
                description: Some("Creates a file.".to_string()),
                source: case_source.to_string(),
            }],
        }])
    }

    #[test]
    fn renders_the_fixed_document_structure() {
        let output = render(&representative_catalog("case \"x\" {\n  $ true\n}\n"));

        assert_eq!(
            output,
            "# Reportage Documentation\n\
             \n\
             ## Contents\n\
             \n\
             - [Filesystem](#group-1-filesystem)\n\
             \x20\x20- [File assertions](#file-1-1-file-assertions)\n\
             \x20\x20\x20\x20- [File creation](#case-1-1-1-file-creation)\n\
             \n\
             <a id=\"group-1-filesystem\"></a>\n\
             ## Filesystem\n\
             \n\
             <a id=\"file-1-1-file-assertions\"></a>\n\
             ### File assertions\n\
             \n\
             Source: examples/file-assertions.repor\n\
             \n\
             About files.\n\
             \n\
             <a id=\"case-1-1-1-file-creation\"></a>\n\
             #### File creation\n\
             \n\
             Creates a file.\n\
             \n\
             ```reportage\n\
             case \"x\" {\n\
             \x20\x20$ true\n\
             }\n\
             ```\n"
        );
    }

    /// `--title` reaches the markdown format through the render options,
    /// verbatim, and never leaks into anchor IDs.
    #[test]
    fn the_document_title_option_is_used_verbatim_and_stays_out_of_anchors() {
        let catalog = representative_catalog("case \"x\" {\n  $ true\n}\n");
        let options = RenderOptions {
            document_title: "**Project** <em>docs</em>".to_string(),
        };

        let output = MarkdownRenderer.render(&catalog, &options);
        assert!(output.starts_with("# **Project** <em>docs</em>\n\n## Contents\n"));
        assert_eq!(
            collect_anchor_ids(&output),
            collect_anchor_ids(&render(&catalog)),
            "anchor IDs must not depend on the document title"
        );
    }

    /// The documented empty-title promise: an empty `--title` is neither
    /// rejected nor replaced by the default; the heading line becomes `# `.
    #[test]
    fn an_empty_document_title_is_rendered_verbatim() {
        let catalog = representative_catalog("case \"x\" {\n  $ true\n}\n");
        let options = RenderOptions {
            document_title: String::new(),
        };

        let output = MarkdownRenderer.render(&catalog, &options);
        assert!(output.starts_with("# \n\n## Contents\n"));
        assert!(!output.contains("Reportage Documentation"));
    }

    #[test]
    fn slug_normalization_is_fixed() {
        assert_eq!(slug("File Assertions"), Some("file-assertions".to_string()));
        assert_eq!(slug("A+B=C 2"), Some("a-b-c-2".to_string()));
        assert_eq!(slug("--Hello,  World!--"), Some("hello-world".to_string()));
        assert_eq!(slug("日本語タイトル"), None);
        assert_eq!(slug(""), None);
        assert_eq!(slug("日本語 mixed 語"), Some("mixed".to_string()));
    }

    fn collect_anchor_ids(output: &str) -> Vec<String> {
        output
            .lines()
            .filter_map(|line| {
                line.strip_prefix("<a id=\"")
                    .and_then(|rest| rest.strip_suffix("\"></a>"))
                    .map(str::to_string)
            })
            .collect()
    }

    /// Duplicate titles, titles that normalize to the same slug, and titles
    /// without any ASCII slug all stay collision-free: uniqueness comes from
    /// the structure indices alone.
    #[test]
    fn anchors_stay_unique_for_duplicate_and_slugless_titles() {
        let source = "case \"x\" {\n  $ true\n}\n";
        let catalog = single_group(vec![
            file(
                "Dup Title",
                vec![case("same", source), case("same", source)],
            ),
            file("Dup Title", vec![]),
            file("dup-title", vec![]),
            file("日本語", vec![]),
        ]);

        let output = render(&catalog);
        let anchors = collect_anchor_ids(&output);
        let unique: std::collections::BTreeSet<_> = anchors.iter().collect();
        assert_eq!(
            anchors.len(),
            unique.len(),
            "anchors must be unique: {anchors:?}"
        );
        assert_eq!(
            anchors,
            vec![
                "group-1-filesystem",
                "file-1-1-dup-title",
                "case-1-1-1-same",
                "case-1-1-2-same",
                "file-1-2-dup-title",
                "file-1-3-dup-title",
                "file-1-4",
            ]
        );
        for anchor in &anchors {
            assert!(
                anchor
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "anchor {anchor:?} must be ASCII lowercase, digits, and hyphens only"
            );
        }
    }

    /// Metadata is mapped verbatim: Markdown syntax and raw HTML in titles
    /// and descriptions are not escaped or sanitized.
    #[test]
    fn metadata_is_not_escaped_or_sanitized() {
        let mut catalog = representative_catalog("case \"x\" {\n  $ true\n}\n");
        catalog.groups[0].name = "## Not a heading".to_string();
        catalog.groups[0].files[0].title = "**bold** <script>x</script>".to_string();
        catalog.groups[0].files[0].description =
            Some("Line one.\n\n<em>raw html</em> and [link](x).".to_string());

        let output = render(&catalog);
        assert!(output.contains("## ## Not a heading"));
        assert!(output.contains("### **bold** <script>x</script>"));
        assert!(output.contains("- [**bold** <script>x</script>](#file-1-1-bold-script-x-script)"));
        assert!(output.contains("\n\nLine one.\n\n<em>raw html</em> and [link](x).\n\n"));
    }

    #[test]
    fn fence_is_longer_than_the_longest_backtick_run_and_at_least_three() {
        assert_eq!(fence_length("no backticks"), 3);
        assert_eq!(fence_length("a `` b"), 3);
        assert_eq!(fence_length("a ``` b"), 4);
        assert_eq!(fence_length("a `````` b"), 7);

        let source = "case \"t\" {\n  $ echo '```'\n}\n";
        let output = render(&single_group(vec![file("f", vec![case("c", source)])]));
        assert!(output.contains("````reportage\ncase \"t\" {\n  $ echo '```'\n}\n````"));
    }

    /// The contract allows computing the fence before or after LF
    /// normalization because backtick runs cannot change: fixed here.
    #[test]
    fn fence_length_is_identical_before_and_after_lf_normalization() {
        let source = "case \"t\" {\r\n  $ echo '````'\r\n}\r\n";
        assert_eq!(fence_length(source), fence_length(&lf(source)));
    }

    #[test]
    fn crlf_sources_and_metadata_are_normalized_to_lf() {
        let mut catalog = representative_catalog(
            "case \"x\" {\r\n  $ true\r\n\r\n  assert {\r\n    exit 0\r\n  }\r\n}\r\n",
        );
        catalog.groups[0].files[0].description = Some("line one\r\nline two".to_string());

        let output = render(&catalog);
        assert!(!output.contains('\r'));
        assert!(output.contains(
            "```reportage\ncase \"x\" {\n  $ true\n\n  assert {\n    exit 0\n  }\n}\n```"
        ));
        assert!(output.contains("line one\nline two"));
    }

    /// A source without a final newline gets one structural LF before the
    /// closing fence; a source with one gets nothing extra. The two render to
    /// the same fenced block, and no blank line appears before the closing
    /// fence.
    #[test]
    fn closing_fence_placement_is_independent_of_the_final_newline() {
        let with_newline = render(&single_group(vec![file(
            "f",
            vec![case("c", "case \"x\" {\n  $ true\n}\n")],
        )]));
        let without_newline = render(&single_group(vec![file(
            "f",
            vec![case("c", "case \"x\" {\n  $ true\n}")],
        )]));

        assert_eq!(with_newline, without_newline);
        assert!(with_newline.contains("}\n```\n"));
        assert!(!with_newline.contains("}\n\n```"));
    }

    /// Interior whitespace, blank lines, and comments in the source are
    /// reproduced without loss inside the fence.
    #[test]
    fn source_whitespace_and_comments_are_preserved() {
        let source = "case \"x\" {\n  # a comment\n\n  $ true   \n} # trailing";
        let output = render(&single_group(vec![file("f", vec![case("c", source)])]));

        assert!(output.contains(
            "```reportage\ncase \"x\" {\n  # a comment\n\n  $ true   \n} # trailing\n```"
        ));
    }

    /// A zero-case file still gets its TOC entry, heading, and source path,
    /// and produces no case heading or fence.
    #[test]
    fn zero_case_files_render_without_case_sections() {
        let mut caseless = file("Case-less", vec![]);
        caseless.description = Some("Still documented.".to_string());
        let output = render(&single_group(vec![caseless]));

        assert!(output.contains("  - [Case-less](#file-1-1-case-less)"));
        assert!(output.contains("<a id=\"file-1-1-case-less\"></a>\n### Case-less"));
        assert!(output.contains("Source: Case-less.repor"));
        assert!(output.contains("Still documented."));
        assert!(!output.contains("####"));
        assert!(!output.contains("```"));
    }

    /// A description's final newline (a heredoc value always carries one)
    /// never changes block separation.
    #[test]
    fn description_final_newline_does_not_change_block_separation() {
        let build = |description: &str| {
            let mut catalog = representative_catalog("case \"x\" {\n  $ true\n}\n");
            catalog.groups[0].files[0].description = Some(description.to_string());
            render(&catalog)
        };

        assert_eq!(
            build("Multi paragraph.\n\nSecond paragraph.\n"),
            build("Multi paragraph.\n\nSecond paragraph.")
        );
        assert!(!build("With final newline.\n").contains("\n\n\n"));
    }

    /// Absent descriptions produce no empty paragraph: blocks stay separated
    /// by exactly one empty line and the document ends with exactly one LF.
    #[test]
    fn block_separation_and_document_tail_are_exact() {
        let output = render(&single_group(vec![
            file("a", vec![case("c1", "case \"c1\" {\n  $ true\n}\n")]),
            file("b", vec![]),
        ]));

        assert!(!output.contains("\n\n\n"));
        assert!(output.ends_with('\n'));
        assert!(!output.ends_with("\n\n"));
    }
}
