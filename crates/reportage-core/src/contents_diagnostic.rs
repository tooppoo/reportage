//! Bounded, escaped diagnostic rendering for a `contents_equals` byte mismatch.
//!
//! `evaluator`'s [`crate::result::ContentsMismatch`] carries only bounded facts (lengths,
//! first differing byte offset); this module turns those facts plus the full actual/expected
//! byte buffers into a bounded, escaped context string suitable for CLI stdout/stderr and the
//! `--format=json` renderer. Presentation only: comparison semantics (byte-for-byte equality)
//! live in `result::ContentsEqualsComparison::compare`, not here. See docs2/reference/semantic-diagnostics.md
//! — `contents_equals` mismatch diagnostics.
//!
//! Neither renderer may print `actual` / `expected` in full; both must go through
//! [`mismatch_context`] instead.

use crate::result::ContentsMismatch;

/// Number of lines of context to include before and after the line containing the first
/// differing byte, when line-context rendering is used.
const CONTEXT_LINES: usize = 2;

/// Byte-size cap on a single side's rendered line-context window. A larger window (a huge
/// single line, or binary-like content) falls back to a bounded byte window around the first
/// differing offset instead.
const MAX_LINE_CONTEXT_BYTES: usize = 2048;

/// Half-width, in bytes, of the fallback byte window centered on the first differing offset.
const BYTE_WINDOW_RADIUS: usize = 64;

/// Bounded, escaped rendering of both sides of a `contents_equals` mismatch, plus the
/// byte-line number (1-based, LF-delimited, CRLF not normalized) the first differing byte
/// falls on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MismatchContext {
    pub first_diff_line: usize,
    pub actual_context: String,
    pub expected_context: String,
}

/// Builds a bounded, escaped [`MismatchContext`] for `mismatch` from the full `actual` /
/// `expected` byte buffers that produced it.
///
/// `actual` and `expected` are guaranteed identical up to `mismatch.first_diff_offset` (that is
/// what makes it the *first* differing byte), so the line number is computed once and applies to
/// both sides.
pub fn mismatch_context(
    actual: &[u8],
    expected: &[u8],
    mismatch: &ContentsMismatch,
) -> MismatchContext {
    let offset = mismatch.first_diff_offset;
    let first_diff_line = count_lines_before(actual, offset);

    let (actual_window, expected_window) = match (
        line_context_window(actual, offset),
        line_context_window(expected, offset),
    ) {
        (Some(a), Some(e))
            if a.len() <= MAX_LINE_CONTEXT_BYTES && e.len() <= MAX_LINE_CONTEXT_BYTES =>
        {
            (a, e)
        }
        _ => (byte_window(actual, offset), byte_window(expected, offset)),
    };

    MismatchContext {
        first_diff_line,
        actual_context: escape(actual_window),
        expected_context: escape(expected_window),
    }
}

/// 1-based line number of the LF-delimited byte-line containing byte `offset`.
fn count_lines_before(buf: &[u8], offset: usize) -> usize {
    let bound = offset.min(buf.len());
    buf[..bound].iter().filter(|&&b| b == b'\n').count() + 1
}

/// The byte-line containing `offset`, plus up to [`CONTEXT_LINES`] lines before and after,
/// clamped to the buffer's bounds. Returns `None` if `offset` is past the end of `buf` (the
/// missing side of a length mismatch has no line to show).
fn line_context_window(buf: &[u8], offset: usize) -> Option<&[u8]> {
    if offset > buf.len() {
        return None;
    }
    let line_starts: Vec<usize> = std::iter::once(0)
        .chain(
            buf.iter()
                .enumerate()
                .filter_map(|(i, &b)| if b == b'\n' { Some(i + 1) } else { None }),
        )
        .collect();

    let target_line = line_starts
        .iter()
        .rposition(|&start| start <= offset)
        .unwrap_or(0);

    let from_line = target_line.saturating_sub(CONTEXT_LINES);
    let to_line = (target_line + CONTEXT_LINES).min(line_starts.len() - 1);

    let start = line_starts[from_line];
    let end = if to_line + 1 < line_starts.len() {
        // Exclude the trailing '\n' of the last included line for a cleaner window;
        // include up to (but not including) the start of the next line beyond the window.
        line_starts[to_line + 1]
    } else {
        buf.len()
    };
    Some(&buf[start..end])
}

/// A bounded byte window centered on `offset`, used when line-context rendering would be too
/// large (a huge single line, or binary-like content with few/no `\n` bytes).
fn byte_window(buf: &[u8], offset: usize) -> &[u8] {
    let bound = offset.min(buf.len());
    let start = bound.saturating_sub(BYTE_WINDOW_RADIUS);
    let end = (bound + BYTE_WINDOW_RADIUS).min(buf.len());
    &buf[start..end]
}

/// Renders `bytes` as a display-safe string: valid UTF-8 is kept legible with only control
/// characters escaped; invalid UTF-8 falls back to a per-byte hex escape for the whole window.
/// Never emits a raw control byte (including NUL, ESC, bare CR) or an invalid UTF-8 byte
/// sequence directly. See docs2/reference/semantic-diagnostics.md.
fn escape(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.chars().map(escape_char).collect(),
        Err(_) => bytes.iter().map(|b| format!("\\x{b:02x}")).collect(),
    }
}

fn escape_char(c: char) -> String {
    match c {
        '\\' => "\\\\".to_string(),
        '\n' => "\\n".to_string(),
        '\r' => "\\r".to_string(),
        '\t' => "\\t".to_string(),
        c if (c as u32) < 0x20 || c as u32 == 0x7f => format!("\\x{:02x}", c as u32),
        c => c.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::result::ContentsEqualsComparison;

    fn mismatch(actual: &[u8], expected: &[u8]) -> ContentsMismatch {
        match ContentsEqualsComparison::compare(actual.to_vec(), expected.to_vec()).outcome {
            crate::result::ContentsEqualsOutcome::Mismatch(m) => m,
            crate::result::ContentsEqualsOutcome::Match => panic!("expected a mismatch"),
        }
    }

    #[test]
    fn reports_line_one_for_first_byte_mismatch() {
        let m = mismatch(b"a", b"b");
        let ctx = mismatch_context(b"a", b"b", &m);
        assert_eq!(ctx.first_diff_line, 1);
    }

    #[test]
    fn reports_correct_line_number_for_multiline_mismatch() {
        let actual = b"line1\nline2\nXXX\n";
        let expected = b"line1\nline2\nYYY\n";
        let m = mismatch(actual, expected);
        let ctx = mismatch_context(actual, expected, &m);
        assert_eq!(ctx.first_diff_line, 3);
    }

    #[test]
    fn context_never_contains_raw_control_bytes() {
        let actual = b"ok\x00\x1bmore";
        let expected = b"ok\x00\x1bdiff";
        let m = mismatch(actual, expected);
        let ctx = mismatch_context(actual, expected, &m);
        assert!(!ctx.actual_context.contains('\u{0}'));
        assert!(!ctx.actual_context.contains('\u{1b}'));
        assert!(ctx.actual_context.contains("\\x00"));
        assert!(ctx.actual_context.contains("\\x1b"));
    }

    #[test]
    fn invalid_utf8_falls_back_to_hex_escape() {
        let actual = vec![0xffu8, b'a'];
        let expected = vec![0xffu8, b'b'];
        let m = mismatch(&actual, &expected);
        let ctx = mismatch_context(&actual, &expected, &m);
        assert!(ctx.actual_context.contains("\\xff"));
    }

    #[test]
    fn huge_single_line_falls_back_to_bounded_byte_window() {
        let mut actual = vec![b'a'; MAX_LINE_CONTEXT_BYTES + 100];
        let mut expected = actual.clone();
        actual[MAX_LINE_CONTEXT_BYTES] = b'x';
        expected[MAX_LINE_CONTEXT_BYTES] = b'y';
        let m = mismatch(&actual, &expected);
        let ctx = mismatch_context(&actual, &expected, &m);
        assert!(ctx.actual_context.len() <= 2 * BYTE_WINDOW_RADIUS + 8);
    }
}
