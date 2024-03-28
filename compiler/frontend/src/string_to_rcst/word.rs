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
    use crate::string_to_rcst::utils::assert_rich_ir_snapshot;

    #[test]
    fn test_word() {
        assert_rich_ir_snapshot!(word("hello, world"), @r###"
        Remaining input: ", world"
        Parsed: hello
        "###);
        assert_rich_ir_snapshot!(word("I💖Candy blub"), @r###"
        Remaining input: " blub"
        Parsed: I💖Candy
        "###);
        assert_rich_ir_snapshot!(word("012🔥hi"), @r###"
        Remaining input: ""
        Parsed: 012🔥hi
        "###);
        assert_rich_ir_snapshot!(word("foo(blub)"), @r###"
        Remaining input: "(blub)"
        Parsed: foo
        "###);
        assert_rich_ir_snapshot!(word("foo#abc"), @r###"
        Remaining input: "#abc"
        Parsed: foo
        "###);
    }

    #[test]
    fn test_identifier() {
        assert_rich_ir_snapshot!(identifier("foo bar"), @r###"
        Remaining input: " bar"
        Parsed: Identifier "foo"
        "###);
        assert_rich_ir_snapshot!(identifier("_"), @r###"
        Remaining input: ""
        Parsed: Identifier "_"
        "###);
        assert_rich_ir_snapshot!(identifier("_foo"), @r###"
        Remaining input: ""
        Parsed: Identifier "_foo"
        "###);
        assert_rich_ir_snapshot!(identifier("Foo bar"), @"Nothing was parsed");
        assert_rich_ir_snapshot!(identifier("012 bar"), @"Nothing was parsed");
        assert_rich_ir_snapshot!(identifier("f12🔥 bar"), @r###"
        Remaining input: " bar"
        Parsed: Error:
          unparsable_input: "f12🔥"
          error: IdentifierContainsNonAlphanumericAscii
        "###);
    }

    #[test]
    fn test_symbol() {
        assert_rich_ir_snapshot!(symbol("Foo b"), @r###"
        Remaining input: " b"
        Parsed: Symbol "Foo"
        "###);
        assert_rich_ir_snapshot!(symbol("Foo_Bar"), @r###"
        Remaining input: ""
        Parsed: Symbol "Foo_Bar"
        "###);
        assert_rich_ir_snapshot!(symbol("foo bar"), @"Nothing was parsed");
        assert_rich_ir_snapshot!(symbol("012 bar"), @"Nothing was parsed");
        assert_rich_ir_snapshot!(symbol("F12🔥 bar"), @r###"
        Remaining input: " bar"
        Parsed: Error:
          unparsable_input: "F12🔥"
          error: SymbolContainsNonAlphanumericAscii
        "###);
    }
}
