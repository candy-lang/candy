use super::{
    expression::{expression, ExpressionParsingOptions},
    literal::{arrow, closing_bracket, closing_curly_brace, closing_parenthesis, colon, comma},
    utils::whitespace_indentation_score,
    whitespace::{single_line_whitespace, whitespaces_and_newlines},
};
use crate::{
    cst::{CstError, CstKind},
    rcst::{Rcst, SplitOuterTrailingWhitespace},
};
use tracing::instrument;

#[instrument(level = "trace")]
pub fn body(mut input: &str, indentation: usize) -> (&str, Vec<Rcst>) {
    let mut expressions = vec![];

    loop {
        let num_expressions_before = expressions.len();

        let (new_input, mut whitespace) = whitespaces_and_newlines(input, indentation, true);
        input = new_input;
        expressions.append(&mut whitespace);

        let mut indentation = indentation;
        if let Some((new_input, unexpected_whitespace)) = single_line_whitespace(input) {
            input = new_input;
            indentation += match &unexpected_whitespace.kind {
                CstKind::Whitespace(whitespace)
                | CstKind::Error {
                    unparsable_input: whitespace,
                    error: CstError::WeirdWhitespace,
                } => whitespace_indentation_score(whitespace) / 2,
                _ => panic!(
                    "single_line_whitespace returned something other than Whitespace or Error."
                ),
            };
            expressions.push(
                CstKind::Error {
                    unparsable_input: unexpected_whitespace.to_string(),
                    error: CstError::TooMuchWhitespace,
                }
                .into(),
            );
        }

        let parsed_expression = expression(
            input,
            indentation,
            ExpressionParsingOptions {
                allow_assignment: true,
                allow_call: true,
                allow_bar: true,
                allow_function: true,
            },
        );
        if let Some((new_input, expression)) = parsed_expression {
            input = new_input;

            let (mut whitespace, expression) = expression.split_outer_trailing_whitespace();
            expressions.push(expression);
            expressions.append(&mut whitespace);
        } else {
            let fallback = colon(new_input)
                .or_else(|| comma(new_input))
                .or_else(|| closing_parenthesis(new_input))
                .or_else(|| closing_bracket(new_input))
                .or_else(|| closing_curly_brace(new_input))
                .or_else(|| arrow(new_input));
            if let Some((new_input, cst)) = fallback {
                input = new_input;
                expressions.push(cst);
            }
        }

        let num_expressions_after = expressions.len();
        if num_expressions_before == num_expressions_after {
            break;
        }
    }
    (input, expressions)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::string_to_rcst::utils::{build_comment, build_identifier, build_space};

    #[test]
    fn test_body() {
        assert_eq!(
            body("foo # comment", 0),
            (
                "",
                vec![
                    build_identifier("foo"),
                    build_space(),
                    build_comment(" comment")
                ]
            ),
        );
    }
}
