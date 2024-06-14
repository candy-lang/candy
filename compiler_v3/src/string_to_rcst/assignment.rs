use super::{
    expression::expression,
    literal::{
        arrow, closing_parenthesis, colon, colon_equals_sign, comma, equals_sign,
        opening_parenthesis,
    },
    whitespace::{AndTrailingWhitespace, OptionAndTrailingWhitespace},
    word::identifier,
};
use crate::{cst::CstKind, rcst::Rcst};
use tracing::instrument;

#[instrument(level = "trace")]
pub fn assignment(input: &str) -> Option<(&str, Rcst)> {
    let (input, let_keyword) = let_(input)?.and_trailing_whitespace();

    let (input, name) = identifier(input)
        .unwrap_or_else(|| {
            (
                input,
                CstKind::Error {
                    unparsable_input: String::new(),
                    error: "Assignment is missing a name.".to_string(),
                }
                .into(),
            )
        })
        .and_trailing_whitespace();

    let (input, kind) = function_assignment(input)
        .unwrap_or_else(|| value_assignment(input))
        .and_trailing_whitespace();

    let (input, assignment_sign) = equals_sign(input)
        .or_else(|| colon_equals_sign(input))
        .unwrap_or_else(|| {
            (
                input,
                CstKind::Error {
                    unparsable_input: String::new(),
                    error: "Assignment is missing an assignment sign (`=` or `:=`).".to_string(),
                }
                .into(),
            )
        })
        .and_trailing_whitespace();

    let (input, body) = expression(input).unwrap_or_else(|| {
        (
            input,
            CstKind::Error {
                unparsable_input: String::new(),
                error: "Assignment is missing a body.".to_string(),
            }
            .into(),
        )
    });

    Some((
        input,
        CstKind::Assignment {
            let_keyword: Box::new(let_keyword),
            name: Box::new(name),
            kind: Box::new(kind),
            assignment_sign: Box::new(assignment_sign),
            body: Box::new(body),
        }
        .into(),
    ))
}

#[instrument(level = "trace")]
fn let_(input: &str) -> Option<(&str, Rcst)> {
    input
        .strip_prefix("let")
        .take_if(|it| {
            !matches!(
                it.chars().next(),
                Some('A'..='Z' | 'a'..='z' | '0'..='9' | '_')
            )
        })
        .map(|it| (it, CstKind::Let.into()))
}

#[instrument(level = "trace")]
fn function_assignment(input: &str) -> Option<(&str, Rcst)> {
    let (input, opening_parenthesis) = opening_parenthesis(input)?.and_trailing_whitespace();

    let (input, parameters) = parameters(input);

    let (input, closing_parenthesis) = closing_parenthesis(input)
        .unwrap_or_else(|| {
            (
                input,
                CstKind::Error {
                    unparsable_input: String::new(),
                    error: "Function assignment is missing a closing parenthesis.".to_string(),
                }
                .into(),
            )
        })
        .and_trailing_whitespace();

    let (input, arrow) = arrow(input)
        .unwrap_or_else(|| {
            (
                input,
                CstKind::Error {
                    unparsable_input: String::new(),
                    error: "Function assignment is missing an arrow.".to_string(),
                }
                .into(),
            )
        })
        .and_trailing_whitespace();

    let (input, return_type) = expression(input).unwrap_or_else(|| {
        (
            input,
            CstKind::Error {
                unparsable_input: String::new(),
                error: "Function assignment is missing a return type.".to_string(),
            }
            .into(),
        )
    });

    Some((
        input,
        CstKind::AssignmentFunction {
            opening_parenthesis: Box::new(opening_parenthesis),
            parameters,
            closing_parenthesis: Box::new(closing_parenthesis),
            arrow: Box::new(arrow),
            return_type: Box::new(return_type),
        }
        .into(),
    ))
}

#[instrument(level = "trace")]
fn value_assignment(input: &str) -> (&str, Rcst) {
    let (input, colon_and_type) =
        colon(input)
            .and_trailing_whitespace()
            .map_or((input, None), |(input, colon)| {
                let (input, type_) = expression(input).unwrap_or_else(|| {
                    (
                        input,
                        CstKind::Error {
                            unparsable_input: String::new(),
                            error: "Value assignment is missing a type.".to_string(),
                        }
                        .into(),
                    )
                });
                (input, Some((colon, type_)))
            });

    (
        input,
        CstKind::AssignmentValue {
            colon_and_type: colon_and_type.map(Box::new),
        }
        .into(),
    )
}

#[instrument(level = "trace")]
pub fn parameters(mut input: &str) -> (&str, Vec<Rcst>) {
    let mut parameters = vec![];
    while let Some((new_input, parameter)) = parameter(input).and_trailing_whitespace() {
        input = new_input;
        parameters.push(parameter);
    }
    (input, parameters)
}

#[instrument(level = "trace")]
fn parameter(input: &str) -> Option<(&str, Rcst)> {
    let (input, name) = identifier(input)?.and_trailing_whitespace();

    let (input, colon_and_type) =
        colon(input)
            .and_trailing_whitespace()
            .map_or((input, None), |(input, colon)| {
                let (input, type_) = expression(input).unwrap_or_else(|| {
                    (
                        input,
                        CstKind::Error {
                            unparsable_input: String::new(),
                            error: "Parameter is missing a type.".to_string(),
                        }
                        .into(),
                    )
                });
                (input, Some(Box::new((colon, type_))))
            });

    let (input, comma) = comma(input)
        .map(AndTrailingWhitespace::and_trailing_whitespace)
        .map_or((input, None), |(input, comma)| (input, Some(comma)));

    Some((
        input,
        CstKind::Parameter {
            name: Box::new(name),
            colon_and_type,
            comma: comma.map(Box::new),
        }
        .into(),
    ))
}
