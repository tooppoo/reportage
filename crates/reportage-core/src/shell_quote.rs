//! Shared POSIX shell single-quoting used anywhere a runner-generated file embeds a value
//! into a `sh` script. Kept in one place because two independently-maintained copies of
//! shell-quoting logic is a security liability: a metacharacter-handling fix to one copy that
//! is never applied to the other silently reintroduces the bug it fixed.

/// Wrap `s` in POSIX single quotes, escaping any embedded single quotes as `'\''`.
pub(crate) fn single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_string_is_single_quoted() {
        assert_eq!(single_quote("/usr/bin/true"), "'/usr/bin/true'");
    }

    #[test]
    fn string_with_spaces_is_safely_quoted() {
        assert_eq!(
            single_quote("/path with spaces/prog"),
            "'/path with spaces/prog'"
        );
    }

    #[test]
    fn string_with_single_quote_is_escaped() {
        assert_eq!(single_quote("it's"), "'it'\\''s'");
    }

    #[test]
    fn string_with_dollar_sign_is_safe() {
        assert_eq!(single_quote("$HOME"), "'$HOME'");
    }

    #[test]
    fn string_with_semicolon_is_safe() {
        assert_eq!(single_quote("arg;rm -rf /"), "'arg;rm -rf /'");
    }

    #[test]
    fn string_with_backtick_is_safe() {
        assert_eq!(single_quote("`cmd`"), "'`cmd`'");
    }
}
