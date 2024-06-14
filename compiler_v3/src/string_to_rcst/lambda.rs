use super::{
    assignment::{assignment, parameters},
    expression::expression,
    literal::{arrow, closing_curly_brace, opening_curly_brace},
    whitespace::{whitespace, AndTrailingWhitespace},
};
use crate::{cst::CstKind, rcst::Rcst};
use tracing::instrument;

#[instrument(level = "trace")]
pub fn lambda(input: &str) -> Option<(&str, Rcst)> {
    let (input, opening_curly_brace) = opening_curly_brace(input)?.and_trailing_whitespace();

    let (input_with_parameters, parameters) = parameters(input);
    let (input, parameters_and_arrow) = if parameters.is_empty() {
        (input, None)
    } else {
        arrow(input_with_parameters).map_or((input, None), |(input, arrow)| {
            (input, Some((parameters, Box::new(arrow))))
        })
    };

    let (input, body) = body(input);

    let (input, closing_curly_brace) = closing_curly_brace(input).unwrap_or_else(|| {
        (
            input,
            CstKind::Error {
                unparsable_input: String::new(),
                error: "This lambda is missing a closing curly brace.".to_string(),
            }
            .into(),
        )
    });

    Some((
        input,
        CstKind::Lambda {
            opening_curly_brace: Box::new(opening_curly_brace),
            parameters_and_arrow,
            body,
            closing_curly_brace: Box::new(closing_curly_brace),
        }
        .into(),
    ))
}

#[instrument(level = "trace")]
pub fn body(mut input: &str) -> (&str, Vec<Rcst>) {
    let mut rcsts = vec![];
    while !input.is_empty() {
        let mut made_progress = false;

        if let Some((new_input, assignment)) = assignment(input) {
            input = new_input;
            rcsts.push(assignment);
            made_progress = true;
        }

        if let Some((new_input, expression)) = expression(input) {
            input = new_input;
            rcsts.push(expression);
            made_progress = true;
        }

        let (new_input, mut whitespace) = whitespace(input);
        if !whitespace.is_empty() {
            input = new_input;
            rcsts.append(&mut whitespace);
            made_progress = true;
        }

        if !made_progress {
            break;
        }
    }
    (input, rcsts)
}
