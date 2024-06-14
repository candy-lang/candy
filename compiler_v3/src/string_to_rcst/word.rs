use crate::{cst::CstKind, rcst::Rcst};
use tracing::instrument;

#[instrument(level = "trace")]
pub fn identifier(input: &str) -> Option<(&str, Rcst)> {
    let (input, w) = word(input)?;
    if input == "let" {
        return None;
    }

    let next_character = w.chars().next().unwrap();
    if !next_character.is_lowercase() && next_character != '_' {
        return None;
    }

    Some((
        input,
        if w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            CstKind::Identifier(w).into()
        } else {
            CstKind::Error {
                unparsable_input: w,
                error: "This identifier contains non-alphanumeric ASCII characters.".to_string(),
            }
            .into()
        },
    ))
}

#[instrument(level = "trace")]
pub fn symbol(input: &str) -> Option<(&str, Rcst)> {
    let (input, w) = word(input)?;
    if !w.chars().next().unwrap().is_uppercase() {
        return None;
    }

    Some((
        input,
        if w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            CstKind::Symbol(w).into()
        } else {
            CstKind::Error {
                unparsable_input: w,
                error: "This symbol contains non-alphanumeric ASCII characters.".to_string(),
            }
            .into()
        },
    ))
}

#[instrument(level = "trace")]
pub fn int(input: &str) -> Option<(&str, Rcst)> {
    let (input, string) = word(input)?;
    if !string.chars().next().unwrap().is_ascii_digit() {
        return None;
    }

    let rcst = if string.chars().all(|c| c.is_ascii_digit()) {
        // Decimal
        let value = str::parse(&string).expect("Couldn't parse int.");
        CstKind::Int { value, string }.into()
    } else {
        CstKind::Error {
            unparsable_input: string,
            error: "This integer contains characters that are not digits.".to_string(),
        }
        .into()
    };
    Some((input, rcst))
}

const MEANINGFUL_PUNCTUATION: &str = r#"=,.:|()[]{}->#""#;

/// "Word" refers to a bunch of characters that are not separated by whitespace
/// or significant punctuation. Identifiers, symbols, and ints are words. Words
/// may be invalid because they contain non-ascii or non-alphanumeric characters
///  – for example, the word `Magic🌵` is an invalid symbol.
#[instrument(level = "trace")]
pub fn word(mut input: &str) -> Option<(&str, String)> {
    let mut string = String::new();
    while let Some(c) = input.chars().next() {
        if c.is_whitespace() || MEANINGFUL_PUNCTUATION.contains(c) {
            break;
        }
        string.push(c);
        input = &input[c.len_utf8()..];
    }
    if string.is_empty() {
        None
    } else {
        Some((input, string))
    }
}
