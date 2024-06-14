use super::{
    expression::expression,
    literal::{closing_curly_brace, double_quote, opening_curly_brace},
    whitespace::AndTrailingWhitespace,
};
use crate::{cst::CstKind, rcst::Rcst};
use tracing::instrument;

// TODO: It might be a good idea to ignore text interpolations in patterns
#[instrument(level = "trace")]
pub fn text(input: &str) -> Option<(&str, Rcst)> {
    let (mut input, opening_double_quote) = double_quote(input)?;

    let mut parts = vec![];
    let (input, closing_double_quote) = loop {
        let (new_input, part) = match input.chars().next() {
            Some('"') => break double_quote(input).unwrap(),
            Some('\r' | '\n') | None => {
                break (
                    input,
                    CstKind::Error {
                        unparsable_input: String::new(),
                        error: "This text isn't closed.".to_string(),
                    }
                    .into(),
                )
            }
            Some('{') => text_interpolation(input).unwrap(),
            _ => text_part(input).unwrap(),
        };
        input = new_input;
        parts.push(part);
    };

    Some((
        input,
        CstKind::Text {
            opening_double_quote: Box::new(opening_double_quote),
            parts,
            closing_double_quote: Box::new(closing_double_quote),
        }
        .into(),
    ))
}

#[instrument(level = "trace")]
fn text_interpolation(input: &str) -> Option<(&str, Rcst)> {
    let (input, opening_curly_brace) = opening_curly_brace(input)?.and_trailing_whitespace();

    let (input, expression) = expression(input)
        .unwrap_or((
            input,
            CstKind::Error {
                unparsable_input: String::new(),
                error: "Here's a start of a text interpolation without an expression after it."
                    .to_string(),
            }
            .into(),
        ))
        .and_trailing_whitespace();

    let (input, closing_curly_brace) = closing_curly_brace(input).unwrap_or((
        input,
        CstKind::Error {
            unparsable_input: String::new(),
            error: "This text interpolation isn't closed.".to_string(),
        }
        .into(),
    ));

    Some((
        input,
        CstKind::TextInterpolation {
            opening_curly_brace: Box::new(opening_curly_brace),
            expression: Box::new(expression),
            closing_curly_brace: Box::new(closing_curly_brace),
        }
        .into(),
    ))
}

#[instrument(level = "trace")]
fn text_part(mut input: &str) -> Option<(&str, Rcst)> {
    let mut text_part = String::new();
    loop {
        let next_char = input.chars().next();
        if matches!(next_char, None | Some('{' | '"' | '\r' | '\n')) {
            break if text_part.is_empty() {
                None
            } else {
                Some((input, CstKind::TextPart(text_part).into()))
            };
        }

        let next_char = next_char.unwrap();
        input = &input[next_char.len_utf8()..];
        text_part.push(next_char);
    }
}
