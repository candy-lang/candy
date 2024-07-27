use super::parser::Parser;
use crate::ast::{AstError, AstIdentifier, AstInt, AstResult, AstString, AstSymbol};
use tracing::instrument;

#[instrument(level = "trace")]
pub fn identifier(parser: Parser) -> Option<(Parser, AstIdentifier)> {
    let (parser, w) = word(parser)?;
    if &*w.string == "let" {
        return None;
    }

    let next_character = w.string.chars().next().unwrap();
    if !next_character.is_lowercase() && next_character != '_' {
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

    Some((parser, AstIdentifier { identifier }))
}

#[instrument(level = "trace")]
pub fn symbol(parser: Parser) -> Option<(Parser, AstSymbol)> {
    let (parser, w) = word(parser)?;
    if !w.string.chars().next().unwrap().is_uppercase() {
        return None;
    }

    let symbol = if w
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
                error: "This symbol contains non-alphanumeric ASCII characters.".to_string(),
            },
        )
    };

    Some((parser, AstSymbol { symbol }))
}

#[instrument(level = "trace")]
pub fn int(parser: Parser) -> Option<(Parser, AstInt)> {
    let (parser, string) = word(parser)?;
    if !string.string.chars().next().unwrap().is_ascii_digit() {
        return None;
    }

    let value = if string.string.chars().all(|c| c.is_ascii_digit()) {
        AstResult::ok(str::parse(&string.string).expect("Couldn't parse int."))
    } else {
        AstResult::error(
            None,
            AstError {
                unparsable_input: string.clone(),
                error: "This integer contains characters that are not digits.".to_string(),
            },
        )
    };
    Some((parser, AstInt { value, string }))
}

const MEANINGFUL_PUNCTUATION: &str = r#"=,.:|()[]{}->#""#;

/// "Word" refers to a bunch of characters that are not separated by whitespace
/// or significant punctuation. Identifiers, symbols, and ints are words. Words
/// may be invalid because they contain non-ascii or non-alphanumeric characters
///  – for example, the word `Magic🌵` is an invalid symbol.
#[instrument(level = "trace")]
pub fn word(parser: Parser) -> Option<(Parser, AstString)> {
    parser.consume_while(|c| !c.is_whitespace() && !MEANINGFUL_PUNCTUATION.contains(c))
}
