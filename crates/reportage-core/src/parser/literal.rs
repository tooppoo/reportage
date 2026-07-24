use super::{ParseError, Rule};
use crate::model::{
    FileContentsReference, FixtureReference, RequiredLiteralKind, ValueLiteralKind, WorkspacePath,
};

fn extract_string_inner(quoted: pest::iterators::Pair<Rule>) -> String {
    // quoted_string = { "\"" ~ string_inner ~ "\"" }
    let raw = quoted
        .into_inner()
        .next()
        .expect("quoted_string must have string_inner")
        .as_str();
    unescape_string(raw)
}

/// Unescapes a raw `string_inner` match into its AST value.
///
/// The grammar's `string_char` rule only accepts `\\`, `\"`, `\n`, and `\t` as escape sequences, so every `\` in `raw` is guaranteed to be followed by one of those four characters.
fn unescape_string(raw: &str) -> String {
    let mut result = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            result.push(c);
            continue;
        }
        match chars.next() {
            Some('\\') => result.push('\\'),
            Some('"') => result.push('"'),
            Some('n') => result.push('\n'),
            Some('t') => result.push('\t'),
            other => {
                unreachable!("grammar guarantees only \\\\, \\\", \\n, \\t escapes, got {other:?}")
            }
        }
    }
    result
}

/// A parsed `value_literal`: its surface kind, its unescaped inner value,
/// and enough source context to build an actionable kind-mismatch diagnostic.
struct ValueLiteral {
    kind: ValueLiteralKind,
    /// The unescaped inner string value.
    value: String,
    /// The inner quoted string exactly as written in source, including its
    /// surrounding quotes (e.g. `"out.txt"`), used to render suggestions.
    quoted_source: String,
    line: usize,
}

/// The literal kind an argument position requires, together with which
/// surface forms its grammar actually accepts — the extra bit
/// [`RequiredLiteralKind`] alone doesn't carry. A kind mismatch's suggested
/// replacement must only point at forms the position's grammar would accept,
/// or the suggestion would steer the author into the very `parse.syntax`
/// error the semantic diagnostic exists to avoid.
#[derive(Clone, Copy)]
enum RequiredKind {
    /// The position requires a `<"...">` workspace path literal.
    WorkspacePath,
    /// The position requires a TextValue and its grammar accepts both the
    /// string literal and heredoc literal forms (a `write` step's content,
    /// `file contains` expected text).
    TextValueStringOrHeredoc,
    /// The position requires a TextValue but its grammar only wires up the
    /// string literal form (`stdout contains` / `stderr contains` expected
    /// text), so the suggestion must not mention a heredoc literal.
    TextValueStringOnly,
    /// The position requires a plain `"..."` string literal
    /// (`dir contains` entry name).
    StringLiteral,
    /// The position requires a `FileContentsReference`: a `<"...">`
    /// workspace path literal or an `@"..."` fixture reference literal
    /// (a `contents_equals` expected value). See #92.
    FileContentsReference,
}

impl RequiredKind {
    /// The user-facing requirement this maps to in the diagnostic contract.
    fn required_literal_kind(self) -> RequiredLiteralKind {
        match self {
            RequiredKind::WorkspacePath => RequiredLiteralKind::WorkspacePath,
            RequiredKind::TextValueStringOrHeredoc | RequiredKind::TextValueStringOnly => {
                RequiredLiteralKind::TextValue
            }
            RequiredKind::StringLiteral => RequiredLiteralKind::StringLiteral,
            RequiredKind::FileContentsReference => RequiredLiteralKind::FileContentsReference,
        }
    }
}

impl ValueLiteral {
    /// The literal exactly as written in source, e.g. `"out.txt"`,
    /// `<"out.txt">`, or `@"out.txt"`.
    fn rendered(&self) -> String {
        match self.kind {
            ValueLiteralKind::StringLiteral => self.quoted_source.clone(),
            ValueLiteralKind::WorkspacePath => format!("<{}>", self.quoted_source),
            ValueLiteralKind::FixtureReference => format!("@{}", self.quoted_source),
        }
    }

    /// Checks this literal against the kind `position` requires, returning
    /// the unescaped inner value on a match and an actionable
    /// `LiteralKindMismatch` (semantic.literal.kind_mismatch) otherwise.
    fn expect_kind(
        self,
        expected: RequiredKind,
        position: &'static str,
    ) -> Result<String, ParseError> {
        let matches = match expected {
            RequiredKind::WorkspacePath => self.kind == ValueLiteralKind::WorkspacePath,
            // TextValue's other form, the heredoc literal, is a distinct
            // grammar rule and never reaches this check.
            RequiredKind::TextValueStringOrHeredoc
            | RequiredKind::TextValueStringOnly
            | RequiredKind::StringLiteral => self.kind == ValueLiteralKind::StringLiteral,
            RequiredKind::FileContentsReference => {
                matches!(
                    self.kind,
                    ValueLiteralKind::WorkspacePath | ValueLiteralKind::FixtureReference
                )
            }
        };
        if matches {
            return Ok(self.value);
        }

        let suggestion = match expected {
            RequiredKind::WorkspacePath => format!("<{}>", self.quoted_source),
            RequiredKind::TextValueStringOrHeredoc => {
                format!(
                    "a string literal or heredoc literal (e.g. {})",
                    self.quoted_source
                )
            }
            RequiredKind::TextValueStringOnly | RequiredKind::StringLiteral => {
                self.quoted_source.clone()
            }
            RequiredKind::FileContentsReference => {
                format!(
                    "a workspace path literal or fixture reference literal (e.g. <{0}> or @{0})",
                    self.quoted_source
                )
            }
        };
        Err(ParseError::LiteralKindMismatch {
            line: self.line,
            position,
            expected: expected.required_literal_kind(),
            actual: self.kind,
            source: self.rendered(),
            suggestion,
        })
    }
}

/// Parses a `value_literal` pair into its kind, unescaped value, and source
/// rendering. Infallible: which kinds a position accepts is checked
/// separately via [`ValueLiteral::expect_kind`].
fn parse_value_literal(pair: pest::iterators::Pair<Rule>) -> ValueLiteral {
    // value_literal = { workspace_path_literal | fixture_reference_literal | quoted_string }
    debug_assert_eq!(pair.as_rule(), Rule::value_literal);
    let line = pair.line_col().0;
    let variant = pair
        .into_inner()
        .next()
        .expect("value_literal must have a variant");

    let (kind, quoted) = match variant.as_rule() {
        Rule::quoted_string => (ValueLiteralKind::StringLiteral, variant),
        Rule::workspace_path_literal | Rule::fixture_reference_literal => {
            let kind = if variant.as_rule() == Rule::workspace_path_literal {
                ValueLiteralKind::WorkspacePath
            } else {
                ValueLiteralKind::FixtureReference
            };
            let quoted = variant
                .into_inner()
                .next()
                .expect("path/fixture literal must wrap a quoted_string");
            (kind, quoted)
        }
        rule => unreachable!("unexpected rule in value_literal: {rule:?}"),
    };

    let quoted_source = quoted.as_str().to_string();
    ValueLiteral {
        kind,
        value: extract_string_inner(quoted),
        quoted_source,
        line,
    }
}

/// Parses a `value_literal` pair into a [`FileContentsReference`] (a
/// `contents_equals` expected value): a `<"...">` workspace path literal or
/// an `@"..."` fixture reference literal, each validated against its own
/// lexical policy at construction time. Any other literal kind (a plain
/// `"..."` string literal) is rejected as a `LiteralKindMismatch` via
/// [`ValueLiteral::expect_kind`]. See #92 and
/// docs/adr/20260706T170000Z_fixture-reference-value-syntax.md.
fn parse_file_contents_reference(
    literal_pair: pest::iterators::Pair<Rule>,
    position: &'static str,
) -> Result<FileContentsReference, ParseError> {
    let literal = parse_value_literal(literal_pair);
    let kind = literal.kind;
    let line = literal.line;
    let raw = literal.expect_kind(RequiredKind::FileContentsReference, position)?;

    match kind {
        ValueLiteralKind::WorkspacePath => {
            let path =
                WorkspacePath::parse(&raw).map_err(|reason| ParseError::InvalidWorkspacePath {
                    line,
                    raw,
                    reason,
                    position,
                })?;
            Ok(FileContentsReference::Workspace(path))
        }
        ValueLiteralKind::FixtureReference => {
            let fixture = FixtureReference::parse(&raw)
                .map_err(|reason| ParseError::InvalidFixtureReference { line, raw, reason })?;
            Ok(FileContentsReference::Fixture(fixture))
        }
        ValueLiteralKind::StringLiteral => {
            unreachable!("expect_kind already rejected StringLiteral for FileContentsReference")
        }
    }
}
