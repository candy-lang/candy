use super::{
    expression::{expression, ExpressionParsingOptions},
    literal::{closing_parenthesis, opening_parenthesis},
    whitespace::whitespaces_and_newlines,
};
use crate::{
    cst::{CstError, CstKind, IsMultiline},
    rcst::Rcst,
};
use tracing::instrument;

#[instrument(level = "trace")]
pub fn parenthesized(
    input: &str,
    indentation: usize,
    allow_function: bool,
) -> Option<(&str, Rcst)> {
    let (input, opening_parenthesis) = opening_parenthesis(input)?;

    let (input, whitespace) = whitespaces_and_newlines(input, indentation + 1, true);
    let inner_indentation = if whitespace.is_multiline() {
        indentation + 1
    } else {
        indentation
    };
    let opening_parenthesis = opening_parenthesis.wrap_in_whitespace(whitespace);

    let (input, inner) = expression(
        input,
        inner_indentation,
        ExpressionParsingOptions {
            allow_assignment: false,
            allow_call: true,
            allow_bar: true,
            allow_function,
        },
    )
    .unwrap_or((
        input,
        CstKind::Error {
            unparsable_input: String::new(),
            error: CstError::OpeningParenthesisMissesExpression,
        }
        .into(),
    ));

    let (input, whitespace) = whitespaces_and_newlines(input, indentation, true);
    let inner = inner.wrap_in_whitespace(whitespace);

    let (input, closing_parenthesis) = closing_parenthesis(input).unwrap_or((
        input,
        CstKind::Error {
            unparsable_input: String::new(),
            error: CstError::ParenthesisNotClosed,
        }
        .into(),
    ));

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

#[cfg(test)]
mod test {
    use super::*;
    use crate::string_to_rcst::utils::build_identifier;

    #[test]
    fn test_parenthesized() {
        assert_eq!(
            parenthesized("(foo)", 0, true),
            Some((
                "",
                CstKind::Parenthesized {
                    opening_parenthesis: Box::new(CstKind::OpeningParenthesis.into()),
                    inner: Box::new(build_identifier("foo")),
                    closing_parenthesis: Box::new(CstKind::ClosingParenthesis.into()),
                }
                .into(),
            )),
        );
        assert_eq!(parenthesized("foo", 0, true), None);
        assert_eq!(
            parenthesized("(foo", 0, true),
            Some((
                "",
                CstKind::Parenthesized {
                    opening_parenthesis: Box::new(CstKind::OpeningParenthesis.into()),
                    inner: Box::new(build_identifier("foo")),
                    closing_parenthesis: Box::new(
                        CstKind::Error {
                            unparsable_input: String::new(),
                            error: CstError::ParenthesisNotClosed
                        }
                        .into()
                    ),
                }
                .into(),
            )),
        );
    }
}
