//! The Markdown building blocks every Markdown format shares: explicit
//! anchors, table-of-contents entries, anchored headings, description blocks,
//! and safe code fences.
//!
//! These are structural decisions of the Markdown format, not of any one
//! projection: the anchor slug normalization and the fence-length rule are
//! documented, user-facing contracts (docs/reference/docs-generation.md), and
//! a second Markdown renderer that re-derived either would let the two drift
//! while both claimed to follow the same rule. See
//! docs/adr/20260723T143711Z_markdown-documentation-format.md.
//!
//! Metadata reaches these helpers verbatim except for LF normalization; only
//! anchor IDs and fences are renderer-generated, which is what keeps raw
//! metadata out of HTML attributes and out of fence-terminating positions.

use super::render::lf;

/// 1-based iteration: the Catalog structure indices in anchor IDs start at 1.
pub(super) fn numbered<T>(items: &[T]) -> impl Iterator<Item = (usize, &T)> {
    items.iter().enumerate().map(|(i, item)| (i + 1, item))
}

/// A description block: the value verbatim except for LF normalization and
/// dropping the single final newline a heredoc value carries, so whether the
/// metadata ends with a newline never changes block separation — the same
/// rule the plain format applies to every labeled value.
pub(super) fn description_block(value: &str) -> String {
    let normalized = lf(value);
    normalized
        .strip_suffix('\n')
        .unwrap_or(&normalized)
        .to_string()
}

/// One table-of-contents line, indented two spaces per nesting depth. The
/// title lands verbatim in the link text; the link target is the generated
/// ASCII anchor.
pub(super) fn toc_entry(depth: usize, title: &str, anchor: &str) -> String {
    format!("{}- [{}](#{anchor})", "  ".repeat(depth), lf(title))
}

/// One heading block: the explicit anchor immediately above the heading line,
/// so the pair always travels as a unit between empty-line block separators.
pub(super) fn anchored_heading(marker: &str, anchor: &str, title: &str) -> String {
    format!("<a id=\"{anchor}\"></a>\n{marker} {}", lf(title))
}

/// The anchor ID for one structure index prefix (e.g. `file-1-2`) and its
/// display title. Uniqueness comes from the prefix alone; the slug is a
/// readability aid and is omitted when normalization leaves nothing.
pub(super) fn anchor_id(index_prefix: &str, title: &str) -> String {
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

/// A body wrapped in a fenced code block with the given info string.
///
/// The fence must be computed on the exact body before LF normalization (the
/// contract's stated stage); normalization cannot change backtick runs, so the
/// result is the same either way. A body without a final newline gets one
/// structural LF so the closing fence sits on its own line; a body with one
/// gets nothing extra, so no blank line appears before the fence.
pub(super) fn fenced(language: &str, body: &str) -> String {
    let fence = "`".repeat(fence_length(body));
    let mut content = lf(body);
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    format!("{fence}{language}\n{content}{fence}")
}

/// One longer than the longest backtick run in the body, and at least 3, so a
/// body containing fences cannot terminate the block early.
fn fence_length(body: &str) -> usize {
    (longest_backtick_run(body) + 1).max(3)
}

fn longest_backtick_run(body: &str) -> usize {
    let mut longest = 0;
    let mut current = 0;
    for c in body.chars() {
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

    /// The slug is a readability aid over a prefix that is already unique, so
    /// every non-ASCII-alphanumeric run collapses and an empty result drops
    /// the slug entirely rather than producing a bare trailing `-`.
    #[test]
    fn slug_normalization_is_fixed() {
        assert_eq!(slug("File Assertions"), Some("file-assertions".to_string()));
        assert_eq!(slug("A+B=C 2"), Some("a-b-c-2".to_string()));
        assert_eq!(slug("--Hello,  World!--"), Some("hello-world".to_string()));
        assert_eq!(slug("日本語タイトル"), None);
        assert_eq!(slug(""), None);
        assert_eq!(slug("日本語 mixed 語"), Some("mixed".to_string()));
    }

    /// Uniqueness comes from the index prefix alone, so a title with no ASCII
    /// slug drops the suffix entirely instead of producing a trailing `-`.
    #[test]
    fn an_anchor_id_appends_the_slug_only_when_one_remains() {
        assert_eq!(anchor_id("file-1-2", "Title"), "file-1-2-title");
        assert_eq!(anchor_id("file-1-2", "日本語"), "file-1-2");
    }

    /// The fence grows past the longest run inside the body, so a body that
    /// itself contains a fence cannot close the block early.
    #[test]
    fn fence_is_longer_than_the_longest_backtick_run_and_at_least_three() {
        assert_eq!(fence_length("no backticks"), 3);
        assert_eq!(fence_length("a `` b"), 3);
        assert_eq!(fence_length("a ``` b"), 4);
        assert_eq!(fence_length("a `````` b"), 7);

        assert_eq!(fenced("text", "plain\n"), "```text\nplain\n```");
        assert_eq!(fenced("text", "a ``` b\n"), "````text\na ``` b\n````");
    }

    /// The contract allows computing the fence before or after LF
    /// normalization because backtick runs cannot change: fixed here.
    #[test]
    fn fence_length_is_identical_before_and_after_lf_normalization() {
        let body = "case \"t\" {\r\n  $ echo '````'\r\n}\r\n";
        assert_eq!(fence_length(body), fence_length(&lf(body)));
    }

    /// A body without a final newline gets one structural LF so the closing
    /// fence sits on its own line; a body with one gets no extra blank line.
    #[test]
    fn a_fence_closes_on_its_own_line_without_adding_a_blank_one() {
        assert_eq!(fenced("", "no newline"), "```\nno newline\n```");
        assert_eq!(fenced("", "with newline\n"), "```\nwith newline\n```");
        assert_eq!(fenced("", ""), "```\n```");
    }

    #[test]
    fn headings_and_toc_entries_carry_the_title_verbatim() {
        assert_eq!(
            anchored_heading("###", "file-1-1-title", "Raw *title*"),
            "<a id=\"file-1-1-title\"></a>\n### Raw *title*"
        );
        assert_eq!(
            toc_entry(2, "Raw *title*", "case-1-1-1"),
            "    - [Raw *title*](#case-1-1-1)"
        );
    }

    /// A metadata value's own trailing newline must not change block
    /// separation, so it is dropped exactly once.
    #[test]
    fn a_description_drops_one_trailing_newline_and_normalizes_crlf() {
        assert_eq!(description_block("one\r\ntwo\r\n"), "one\ntwo");
        assert_eq!(description_block("one\n\n"), "one\n");
    }
}
