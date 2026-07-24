#[test]
fn write_step_empty_block_content_is_empty_string() {
    let src = "case \"x\" {\n  write <\"empty.txt\"> ```\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
    let script = parse_script(src).unwrap();
    let step = write_file_step(&script);
    assert_eq!(step.content.to_text_value().as_str(), "");
}

#[test]
fn write_step_blank_line_is_preserved_as_empty_line_after_dedent() {
    let src = "case \"x\" {\n  write <\"a.txt\"> ```\n    first\n\n    third\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
    let script = parse_script(src).unwrap();
    let step = write_file_step(&script);
    assert_eq!(step.content.to_text_value().as_str(), "first\n\nthird\n");
}

#[test]
fn write_step_whitespace_only_line_is_dedented_to_empty_line() {
    // The blank line has trailing spaces shallower than the closing fence's indent;
    // it must still be exempt from the shallow-indent check and dedent to empty.
    let src = "case \"x\" {\n  write <\"a.txt\"> ```\n    first\n  \n    third\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
    let script = parse_script(src).unwrap();
    let step = write_file_step(&script);
    assert_eq!(step.content.to_text_value().as_str(), "first\n\nthird\n");
}

#[test]
fn write_step_tab_indent_is_treated_as_literal_prefix_not_width() {
    // Closing fence indented with a tab; body lines must match that exact
    // tab character as a string prefix, not a width-equivalent number of spaces.
    let src = "case \"x\" {\n  write <\"a.txt\"> ```\n\thello\n\t```\n  $ true\n  assert { exit 0 }\n}\n";
    let script = parse_script(src).unwrap();
    let step = write_file_step(&script);
    assert_eq!(step.content.to_text_value().as_str(), "hello\n");
}

#[test]
fn write_step_crlf_line_endings_are_preserved() {
    let src = "case \"x\" {\r\n  write <\"a.txt\"> ```\r\n    hello\r\n    ```\r\n  $ true\r\n  assert { exit 0 }\r\n}\r\n";
    let script = parse_script(src).unwrap();
    let step = write_file_step(&script);
    assert_eq!(step.content.to_text_value().as_str(), "hello\r\n");
}

#[test]
fn write_step_content_preserves_variable_looking_text_literally() {
    let src = "case \"x\" {\n  write <\"a.txt\"> ```\n    ${ENTRY_KIND}\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
    let script = parse_script(src).unwrap();
    let step = write_file_step(&script);
    assert_eq!(step.content.to_text_value().as_str(), "${ENTRY_KIND}\n");
}

#[test]
fn write_step_closing_fence_longer_than_opening_is_accepted() {
    let src = "case \"x\" {\n  write <\"a.txt\"> ```\n    hello\n    ````\n  $ true\n  assert { exit 0 }\n}\n";
    let script = parse_script(src).unwrap();
    let step = write_file_step(&script);
    assert_eq!(step.content.to_text_value().as_str(), "hello\n");
}

#[test]
fn write_step_longer_opening_fence_allows_embedded_triple_backticks() {
    let src = "case \"x\" {\n  write <\"a.md\"> ````\n    ```ts\n    console.log(1)\n    ```\n    ````\n  $ true\n  assert { exit 0 }\n}\n";
    let script = parse_script(src).unwrap();
    let step = write_file_step(&script);
    assert_eq!(
        step.content.to_text_value().as_str(),
        "```ts\nconsole.log(1)\n```\n"
    );
}

#[test]
fn write_step_shallow_indent_is_rejected() {
    // "mid" is indented less than the closing fence's 4 spaces.
    let src = "case \"x\" {\n  write <\"a.txt\"> ```\n    first\n  mid\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
    let err = parse_script(src).unwrap_err();
    assert!(matches!(err, ParseError::ShallowHeredocIndent { .. }));
    assert_eq!(err.code().as_str(), "parse.heredoc_literal.shallow_indent");
}

#[test]
fn write_step_unterminated_fence_is_a_syntax_error() {
    let src =
        "case \"x\" {\n  write <\"a.txt\"> ```\n    hello\n  $ true\n  assert { exit 0 }\n}\n";
    let err = parse_script(src).unwrap_err();
    assert!(matches!(err, ParseError::Syntax { .. }));
}

#[test]
fn write_step_opening_fence_inline_comment_is_rejected() {
    let src = "case \"x\" {\n  write <\"a.txt\"> ``` # comment\n    hello\n    ```\n  $ true\n  assert { exit 0 }\n}\n";
    let err = parse_script(src).unwrap_err();
    assert!(matches!(err, ParseError::Syntax { .. }));
}

// Known limitation (documented in docs/reference/semantics.md and the ADR): a
// `write` step missing its own closing fence does not always produce a
// syntax error. The grammar scans forward for the next line shaped like
// a valid closing fence, which here belongs to what the author intended
// as a *separate* `write <"b.txt">` step. That step's opening line is
// silently absorbed as literal content of `a.txt`, and `b.txt`'s write
// step disappears from the AST entirely — this test pins that exact
// behavior so a future grammar change cannot silently alter it further
// without a test failure calling it out.
#[test]
fn missing_closing_fence_silently_absorbs_a_later_write_step_as_content() {
    let src = concat!(
        "case \"x\" {\n",
        "  write <\"a.txt\"> ```\n",
        "    first\n",
        "    write <\"b.txt\"> ```\n",
        "    second\n",
        "    ```\n",
        "  $ true\n",
        "  assert { exit 0 }\n",
        "}\n",
    );
    let script = parse_script(src).unwrap();

    // Only 3 steps: the intended `write <"b.txt">` step never materializes.
    assert_eq!(script.cases[0].steps.len(), 3);

    let step = write_file_step(&script);
    assert_eq!(step.path.as_str(), "a.txt");
    assert_eq!(
        step.content.to_text_value().as_str(),
        "first\nwrite <\"b.txt\"> ```\nsecond\n"
    );

    assert!(matches!(script.cases[0].steps[1], Step::Action(_)));
    assert!(matches!(script.cases[0].steps[2], Step::AssertionBlock(_)));
}

