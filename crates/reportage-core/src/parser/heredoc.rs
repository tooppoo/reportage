/// Parses a `heredoc_literal` pair into its dedented `String` content.
/// Shared by `write_step_heredoc` and `file_exp_heredoc` — the fence and
/// dedent rules are identical regardless of which construct the heredoc
/// literal appears in.
fn parse_heredoc_literal(pair: pest::iterators::Pair<Rule>) -> Result<String, ParseError> {
    // heredoc_literal = { PUSH(opening_fence) ~ ws* ~ nl ~ heredoc_body ~ closing_fence_line ~ DROP }
    let mut inner = pair.into_inner();

    let _opening_fence = inner
        .next()
        .expect("heredoc_literal must have an opening_fence (pushed onto the pest match stack)");

    let body_pair = inner
        .next()
        .expect("heredoc_literal must have heredoc_body");
    let body_start_line = body_pair.line_col().0;
    let body_text = body_pair.as_str();

    let closing_pair = inner
        .next()
        .expect("heredoc_literal must have closing_fence_line");
    // closing_fence_line = { closing_fence_indent ~ PEEK ~ "`"* ~ ws* ~ (nl | EOI) }
    let indent = closing_pair
        .into_inner()
        .next()
        .expect("closing_fence_line must have closing_fence_indent")
        .as_str();

    dedent_heredoc_body(body_text, indent, body_start_line)
}

/// Dedents a heredoc literal body against its closing fence's indentation.
///
/// Every non-blank line must start with `indent` as a literal string prefix
/// (no tab/space width normalization); that prefix is stripped. Blank and
/// whitespace-only lines are exempt from the prefix check and are dedented
/// to a genuinely empty line instead. Line endings (LF or CRLF) are
/// preserved exactly as they appeared in the source.
///
/// `body_start_line` is the source line number of `body`'s first line, used
/// to report the correct line for a shallow-indentation error.
fn dedent_heredoc_body(
    body: &str,
    indent: &str,
    body_start_line: usize,
) -> Result<String, ParseError> {
    let mut result = String::with_capacity(body.len());
    for (i, (content, ending)) in split_lines_keep_ending(body).into_iter().enumerate() {
        let is_blank = content.chars().all(|c| c == ' ' || c == '\t');
        if is_blank {
            result.push_str(ending);
            continue;
        }
        match content.strip_prefix(indent) {
            Some(stripped) => {
                result.push_str(stripped);
                result.push_str(ending);
            }
            None => {
                return Err(ParseError::ShallowHeredocIndent {
                    line: body_start_line + i,
                });
            }
        }
    }
    Ok(result)
}

/// Splits `s` into `(line_content, line_ending)` pairs without normalizing
/// line endings. `line_ending` is `"\n"`, `"\r\n"`, or `""` for a trailing
/// line with no terminator (not produced by the grammar, which requires
/// every heredoc body line to end in an actual newline, but handled here
/// defensively).
fn split_lines_keep_ending(s: &str) -> Vec<(&str, &str)> {
    let mut result = Vec::new();
    let mut rest = s;
    while !rest.is_empty() {
        match rest.find('\n') {
            Some(idx) => {
                let line = &rest[..idx];
                match line.strip_suffix('\r') {
                    Some(stripped) => result.push((stripped, "\r\n")),
                    None => result.push((line, "\n")),
                }
                rest = &rest[idx + 1..];
            }
            None => {
                result.push((rest, ""));
                rest = "";
            }
        }
    }
    result
}
