use super::utils::MEANINGFUL_PUNCTUATION;
use crate::{
    cst::{CstError, CstKind},
    rcst::Rcst,
};
use itertools::Itertools;
use tracing::instrument;

/// "Word" refers to a bunch of characters that are not separated by whitespace
/// or significant punctuation. Identifiers, symbols, and ints are words. Words
/// may be invalid because they contain non-ascii or non-alphanumeric characters
/// – for example, the word `Magic🌵` is an invalid symbol.
#[instrument(level = "trace")]
pub fn word(mut input: &str) -> Option<(&str, String)> {
    let mut chars = vec![];
    while let Some(c) = input.chars().next() {
        if c.is_whitespace() || MEANINGFUL_PUNCTUATION.contains(c) {
            break;
        }
        chars.push(c);
        input = &input[c.len_utf8()..];
    }
    if chars.is_empty() {
        None
    } else {
        Some((input, chars.into_iter().join("")))
    }
}

#[instrument(level = "trace")]
pub fn identifier(input: &str) -> Option<(&str, Rcst)> {
    let (input, w) = word(input)?;
    if w == "✨" {
        return Some((input, CstKind::Identifier(w).into()));
    }
    let next_character = w.chars().next().unwrap();
    if !next_character.is_lowercase() && next_character != '_' {
        return None;
    }
    if w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        Some((input, CstKind::Identifier(w).into()))
    } else {
        Some((
            input,
            CstKind::Error {
                unparsable_input: w,
                error: CstError::IdentifierContainsNonAlphanumericAscii,
            }
            .into(),
        ))
    }
}

#[instrument(level = "trace")]
pub fn symbol(input: &str) -> Option<(&str, Rcst)> {
    let (input, w) = word(input)?;
    if !w.chars().next().unwrap().is_uppercase() {
        return None;
    }
    if w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        Some((input, CstKind::Symbol(w).into()))
    } else {
        Some((
            input,
            CstKind::Error {
                unparsable_input: w,
                error: CstError::SymbolContainsNonAlphanumericAscii,
            }
            .into(),
        ))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::string_to_rcst::utils::{build_identifier, build_symbol};

    #[test]
    fn test_word() {
        assert_eq!(word("hello, world"), Some((", world", "hello".to_string())));
        assert_eq!(
            word("I💖Candy blub"),
            Some((" blub", "I💖Candy".to_string()))
        );
        assert_eq!(word("012🔥hi"), Some(("", "012🔥hi".to_string())));
        assert_eq!(word("foo(blub)"), Some(("(blub)", "foo".to_string())));
        assert_eq!(word("foo#abc"), Some(("#abc", "foo".to_string())));
    }

    #[test]
    fn test_identifier() {
        assert_eq!(
            identifier("foo bar"),
            Some((" bar", build_identifier("foo")))
        );
        assert_eq!(identifier("_"), Some(("", build_identifier("_"))));
        assert_eq!(identifier("_foo"), Some(("", build_identifier("_foo"))));
        assert_eq!(identifier("Foo bar"), None);
        assert_eq!(identifier("012 bar"), None);
        assert_eq!(
            identifier("f12🔥 bar"),
            Some((
                " bar",
                CstKind::Error {
                    unparsable_input: "f12🔥".to_string(),
                    error: CstError::IdentifierContainsNonAlphanumericAscii,
                }
                .into(),
            )),
        );
    }

    #[test]
    fn test_symbol() {
        assert_eq!(symbol("Foo b"), Some((" b", build_symbol("Foo"))));
        assert_eq!(symbol("Foo_Bar"), Some(("", build_symbol("Foo_Bar"))));
        assert_eq!(symbol("foo bar"), None);
        assert_eq!(symbol("012 bar"), None);
        assert_eq!(
            symbol("F12🔥 bar"),
            Some((
                " bar",
                CstKind::Error {
                    unparsable_input: "F12🔥".to_string(),
                    error: CstError::SymbolContainsNonAlphanumericAscii,
                }
                .into()
            )),
        );
    }
}
