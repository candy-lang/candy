use super::{literal::KEYWORDS, parser::Parser};
use crate::ast::{AstError, AstResult, AstString};
use tracing::instrument;

#[instrument(level = "trace")]
pub fn raw_identifier(parser: Parser) -> Option<(Parser, AstResult<AstString>)> {
    let (parser, w) = word(parser)?;
    if KEYWORDS.iter().any(|&it| it == &*w.string) {
        return None;
    }

    let next_character = w.string.chars().next().unwrap();
    if next_character.is_ascii_digit() {
        return None;
    }

    let identifier = if w
        .string
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        AstResult::ok(w)
    } else {
        AstResult::error(
            w.clone(),
            AstError {
                unparsable_input: w,
                error: "This identifier contains non-alphanumeric ASCII characters.".to_string(),
            },
        )
    };

    Some((parser, identifier))
}

const MEANINGFUL_PUNCTUATION: &str = r#"=,.:|()[]{}->#""#;

/// "Word" refers to a bunch of characters that are not separated by whitespace
/// or significant punctuation. Identifiers, symbols, and ints are words. Words
/// may be invalid because they contain non-ascii or non-alphanumeric characters
///  – for example, the word `Magic🌵` is an invalid symbol.
#[instrument(level = "trace")]
pub fn word(parser: Parser) -> Option<(Parser, AstString)> {
    parser.consume_while_not_empty(|c| !c.is_whitespace() && !MEANINGFUL_PUNCTUATION.contains(c))
}
