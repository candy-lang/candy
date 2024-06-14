use super::{
    lambda::lambda,
    literal::{
        closing_bracket, closing_parenthesis, colon, comma, dot, opening_bracket,
        opening_parenthesis,
    },
    text::text,
    whitespace::{whitespace, AndTrailingWhitespace, OptionAndTrailingWhitespace},
    word::{identifier, int, symbol, word},
};
use crate::{cst::CstKind, rcst::Rcst};
use replace_with::replace_with_or_abort;
use tracing::instrument;

#[instrument(level = "trace")]
pub fn expression(input: &str) -> Option<(&str, Rcst)> {
    // If we start the call list with `if … else …`, the formatting looks weird.
    // Hence, we start with a single `None`.
    let (mut input, mut result) = None
        .or_else(|| int(input))
        .or_else(|| text(input))
        .or_else(|| identifier(input))
        .or_else(|| symbol(input))
        .or_else(|| parenthesized(input))
        .or_else(|| struct_(input))
        .or_else(|| lambda(input))
        .or_else(|| {
            word(input).map(|(input, word)| {
                (
                    input,
                    CstKind::Error {
                        unparsable_input: word,
                        error: "These are unexpected characters.".to_string(),
                    }
                    .into(),
                )
            })
        })?;

    loop {
        fn parse_suffix<'input>(
            input: &mut &'input str,
            result: &mut Rcst,
            parser: fn(&'input str, &mut Rcst) -> Option<&'input str>,
        ) -> bool {
            parser(input, result).map_or(false, |new_input| {
                *input = new_input;
                true
            })
        }

        let mut did_make_progress = false;
        did_make_progress |= parse_suffix(&mut input, &mut result, expression_suffix_struct_access);
        did_make_progress |= parse_suffix(&mut input, &mut result, expression_suffix_call);
        if !did_make_progress {
            break;
        }
    }
    Some((input, result))
}

#[instrument(level = "trace")]
fn parenthesized<'a>(input: &'a str) -> Option<(&'a str, Rcst)> {
    let (input, opening_parenthesis) = opening_parenthesis(input)?.and_trailing_whitespace();

    let (input, inner) = expression(input)
        .and_trailing_whitespace()
        .unwrap_or_else(|| {
            (
                input,
                CstKind::Error {
                    unparsable_input: String::new(),
                    error: "This parenthesized expression is missing a value.".to_string(),
                }
                .into(),
            )
        });

    let (input, closing_parenthesis) = closing_parenthesis(input).unwrap_or_else(|| {
        (
            input,
            CstKind::Error {
                unparsable_input: String::new(),
                error: "This parenthesized expression is missing a closing parenthesis."
                    .to_string(),
            }
            .into(),
        )
    });

    Some((
        input,
        CstKind::Parenthesized {
            opening_parenthesis: Box::new(opening_parenthesis),
            inner: Box::new(inner),
            closing_parenthesis: Box::new(closing_parenthesis),
        }
        .into(),
    ))
}

#[instrument(level = "trace")]
fn struct_<'a>(input: &'a str) -> Option<(&'a str, Rcst)> {
    let (mut input, opening_bracket) = opening_bracket(input)?.and_trailing_whitespace();

    let mut fields = vec![];
    while let Some((new_input, field)) = struct_field(input).and_trailing_whitespace() {
        fields.push(field);
        input = new_input;
    }

    let (input, closing_bracket) = closing_bracket(input).unwrap_or_else(|| {
        (
            input,
            CstKind::Error {
                unparsable_input: String::new(),
                error: "This struct is missing a closing bracket.".to_string(),
            }
            .into(),
        )
    });

    Some((
        input,
        CstKind::Struct {
            opening_bracket: Box::new(opening_bracket),
            fields,
            closing_bracket: Box::new(closing_bracket),
        }
        .into(),
    ))
}
#[instrument(level = "trace")]
fn struct_field<'a>(input: &'a str) -> Option<(&'a str, Rcst)> {
    let (input, key) = identifier(input)?.and_trailing_whitespace();

    let (input, colon) = colon(input).and_trailing_whitespace().unwrap_or_else(|| {
        (
            input,
            CstKind::Error {
                unparsable_input: String::new(),
                error: "This struct field is missing a colon.".to_string(),
            }
            .into(),
        )
    });

    let (input, value) = expression(input)
        .and_trailing_whitespace()
        .unwrap_or_else(|| {
            (
                input,
                CstKind::Error {
                    unparsable_input: String::new(),
                    error: "This struct field is missing a value.".to_string(),
                }
                .into(),
            )
        });

    let (input, comma) = comma(input)
        .and_trailing_whitespace()
        .map_or((input, None), |(input, comma)| (input, Some(comma)));

    Some((
        input,
        CstKind::StructField {
            key: Box::new(key),
            colon: Box::new(colon),
            value: Box::new(value),
            comma: comma.map(Box::new),
        }
        .into(),
    ))
}

#[instrument(level = "trace")]
fn expression_suffix_struct_access<'a>(input: &'a str, current: &mut Rcst) -> Option<&'a str> {
    let (input, whitespace_after_struct) = whitespace(input);

    let (input, dot) = dot(input)?.and_trailing_whitespace();

    let (input, key) = identifier(input).unwrap_or_else(|| {
        (
            input,
            CstKind::Error {
                unparsable_input: String::new(),
                error: "This struct access is missing a key.".to_string(),
            }
            .into(),
        )
    });

    replace_with_or_abort(current, |current| {
        CstKind::StructAccess {
            struct_: Box::new(current.wrap_in_whitespace(whitespace_after_struct)),
            dot: Box::new(dot),
            key: Box::new(key),
        }
        .into()
    });

    Some(input)
}

#[instrument(level = "trace")]
fn expression_suffix_call<'a>(input: &'a str, current: &mut Rcst) -> Option<&'a str> {
    let (input, whitespace_after_receiver) = whitespace(input);

    let (mut input, opening_parenthesis) = opening_parenthesis(input)?.and_trailing_whitespace();

    let mut arguments = vec![];
    while let Some((new_input, argument)) = argument(input).and_trailing_whitespace() {
        arguments.push(argument);
        input = new_input;
    }

    let (input, closing_parenthesis) = closing_parenthesis(input).unwrap_or_else(|| {
        (
            input,
            CstKind::Error {
                unparsable_input: String::new(),
                error: "This call is missing a closing parenthesis.".to_string(),
            }
            .into(),
        )
    });

    replace_with_or_abort(current, |current| {
        CstKind::Call {
            receiver: Box::new(current.wrap_in_whitespace(whitespace_after_receiver)),
            opening_parenthesis: Box::new(opening_parenthesis),
            arguments,
            closing_parenthesis: Box::new(closing_parenthesis),
        }
        .into()
    });

    Some(input)
}
#[instrument(level = "trace")]
fn argument<'a>(input: &'a str) -> Option<(&'a str, Rcst)> {
    let (input, value) = expression(input)?.and_trailing_whitespace();

    let (input, comma) = comma(input)
        .and_trailing_whitespace()
        .map(|(input, comma)| (input, Some(comma)))
        .unwrap_or((input, None));

    Some((
        input,
        CstKind::CallArgument {
            value: Box::new(value),
            comma: comma.map(Box::new),
        }
        .into(),
    ))
}
