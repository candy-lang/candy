use super::{
    expression::{expression, ExpressionParsingOptions},
    literal::{closing_parenthesis, comma, opening_parenthesis},
    whitespace::whitespaces_and_newlines,
};
use crate::{
    cst::{CstError, CstKind, IsMultiline},
    rcst::Rcst,
};
use tracing::instrument;

#[instrument(level = "trace")]
pub fn list(input: &str, indentation: usize) -> Option<(&str, Rcst)> {
    let (mut input_before_closing, mut opening_parenthesis) = opening_parenthesis(input)?;

    let mut items: Vec<Rcst> = vec![];
    loop {
        // Whitespace before value.
        let (input, whitespace) =
            whitespaces_and_newlines(input_before_closing, indentation + 1, true);
        let item_indentation = if whitespace.is_multiline() {
            indentation + 1
        } else {
            indentation
        };
        if items.is_empty() {
            opening_parenthesis = opening_parenthesis.wrap_in_whitespace(whitespace);
        } else {
            let last = items.pop().unwrap();
            items.push(last.wrap_in_whitespace(whitespace));
        }

        // Value.
        let (input_after_expression, value) = expression(
            input,
            item_indentation,
            ExpressionParsingOptions {
                allow_assignment: false,
                allow_call: true,
                allow_bar: true,
                allow_function: true,
            },
        )
        .map_or((input, None), |(input, expression)| {
            (input, Some(expression))
        });

        // Whitespace between value and comma.
        let (input, whitespace) =
            whitespaces_and_newlines(input_after_expression, item_indentation + 1, true);

        // Comma.
        let (input, comma) = match comma(input) {
            // It is an empty list if there is no first expression but a comma
            Some((input, comma)) if items.is_empty() && value.is_none() => {
                let (input, trailing_whitespace) =
                    whitespaces_and_newlines(input, indentation + 1, true);
                let comma = comma.wrap_in_whitespace(trailing_whitespace);

                // Closing parenthesis.
                if let Some((input, closing_parenthesis)) = closing_parenthesis(input) {
                    return Some((
                        input,
                        CstKind::List {
                            opening_parenthesis: Box::new(opening_parenthesis),
                            items: vec![comma],
                            closing_parenthesis: Box::new(closing_parenthesis),
                        }
                        .into(),
                    ));
                };

                (input, Some(comma))
            }
            Some((input, comma)) => (input, Some(comma)),
            // It is a parenthesized expression if there is no comma after the first expression
            None if items.is_empty() => {
                let (input, whitespace) =
                    whitespaces_and_newlines(input_after_expression, indentation, true);
                let (input, closing_parenthesis) =
                    closing_parenthesis(input).unwrap_or_else(|| {
                        (
                            input,
                            CstKind::Error {
                                unparsable_input: String::new(),
                                error: CstError::ParenthesisNotClosed,
                            }
                            .into(),
                        )
                    });

                return Some((
                    input,
                    CstKind::Parenthesized {
                        opening_parenthesis: Box::new(opening_parenthesis),
                        inner: Box::new(
                            value
                                .unwrap_or_else(|| {
                                    CstKind::Error {
                                        unparsable_input: String::new(),
                                        error: CstError::OpeningParenthesisMissesExpression,
                                    }
                                    .into()
                                })
                                .wrap_in_whitespace(whitespace),
                        ),
                        closing_parenthesis: Box::new(closing_parenthesis),
                    }
                    .into(),
                ));
            }
            None => (input, None),
        };

        input_before_closing = input;
        if value.is_none() && comma.is_none() {
            break;
        }
        items.push(
            CstKind::ListItem {
                value: Box::new(
                    value
                        .unwrap_or_else(|| {
                            CstKind::Error {
                                unparsable_input: String::new(),
                                error: CstError::ListItemMissesValue,
                            }
                            .into()
                        })
                        .wrap_in_whitespace(whitespace),
                ),
                comma: comma.map(Box::new),
            }
            .into(),
        );
    }

    let (input, whitespace) = whitespaces_and_newlines(input_before_closing, indentation, true);

    let (input, closing_parenthesis) = match closing_parenthesis(input) {
        Some((input, closing_parenthesis)) => {
            if items.is_empty() {
                opening_parenthesis = opening_parenthesis.wrap_in_whitespace(whitespace);
            } else {
                let last = items.pop().unwrap();
                items.push(last.wrap_in_whitespace(whitespace));
            }
            (input, closing_parenthesis)
        }
        None => (
            input,
            CstKind::Error {
                unparsable_input: String::new(),
                error: CstError::ListNotClosed,
            }
            .into(),
        ),
    };

    Some((
        input,
        CstKind::List {
            opening_parenthesis: Box::new(opening_parenthesis),
            items,
            closing_parenthesis: Box::new(closing_parenthesis),
        }
        .into(),
    ))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::string_to_rcst::utils::{build_identifier, build_simple_int, build_simple_text};

    #[test]
    fn test_parenthesized() {
        assert_eq!(
            list("(foo)", 0),
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
        assert_eq!(list("foo", 0), None);
        assert_eq!(
            list("(foo", 0),
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
        assert_eq!(
            list("()", 0),
            Some((
                "",
                CstKind::Parenthesized {
                    opening_parenthesis: Box::new(CstKind::OpeningParenthesis.into()),
                    inner: Box::new(
                        CstKind::Error {
                            unparsable_input: String::new(),
                            error: CstError::OpeningParenthesisMissesExpression,
                        }
                        .into()
                    ),
                    closing_parenthesis: Box::new(CstKind::ClosingParenthesis.into()),
                }
                .into(),
            ))
        );
    }

    #[test]
    fn test_list() {
        assert_eq!(list("hello", 0), None);
        assert_eq!(
            list("(,)", 0),
            Some((
                "",
                CstKind::List {
                    opening_parenthesis: Box::new(CstKind::OpeningParenthesis.into()),
                    items: vec![CstKind::Comma.into()],
                    closing_parenthesis: Box::new(CstKind::ClosingParenthesis.into()),
                }
                .into(),
            )),
        );
        assert_eq!(
            list("(foo,)", 0),
            Some((
                "",
                CstKind::List {
                    opening_parenthesis: Box::new(CstKind::OpeningParenthesis.into()),
                    items: vec![CstKind::ListItem {
                        value: Box::new(build_identifier("foo")),
                        comma: Some(Box::new(CstKind::Comma.into())),
                    }
                    .into()],
                    closing_parenthesis: Box::new(CstKind::ClosingParenthesis.into()),
                }
                .into(),
            )),
        );
        assert_eq!(
            list("(foo, )", 0),
            Some((
                "",
                CstKind::List {
                    opening_parenthesis: Box::new(CstKind::OpeningParenthesis.into()),
                    items: vec![CstKind::ListItem {
                        value: Box::new(build_identifier("foo")),
                        comma: Some(Box::new(CstKind::Comma.into())),
                    }
                    .with_trailing_space()],
                    closing_parenthesis: Box::new(CstKind::ClosingParenthesis.into()),
                }
                .into(),
            )),
        );
        assert_eq!(
            list("(foo,bar)", 0),
            Some((
                "",
                CstKind::List {
                    opening_parenthesis: Box::new(CstKind::OpeningParenthesis.into()),
                    items: vec![
                        CstKind::ListItem {
                            value: Box::new(build_identifier("foo")),
                            comma: Some(Box::new(CstKind::Comma.into())),
                        }
                        .into(),
                        CstKind::ListItem {
                            value: Box::new(build_identifier("bar")),
                            comma: None,
                        }
                        .into(),
                    ],
                    closing_parenthesis: Box::new(CstKind::ClosingParenthesis.into()),
                }
                .into(),
            )),
        );
        // (
        //   foo,
        //   4,
        //   "Hi",
        // )
        assert_eq!(
            list("(\n  foo,\n  4,\n  \"Hi\",\n)", 0),
            Some((
                "",
                CstKind::List {
                    opening_parenthesis: Box::new(
                        CstKind::OpeningParenthesis.with_trailing_whitespace(vec![
                            CstKind::Newline("\n".to_string()),
                            CstKind::Whitespace("  ".to_string()),
                        ]),
                    ),
                    items: vec![
                        CstKind::ListItem {
                            value: Box::new(build_identifier("foo")),
                            comma: Some(Box::new(CstKind::Comma.into())),
                        }
                        .with_trailing_whitespace(vec![
                            CstKind::Newline("\n".to_string()),
                            CstKind::Whitespace("  ".to_string())
                        ]),
                        CstKind::ListItem {
                            value: Box::new(build_simple_int(4)),
                            comma: Some(Box::new(CstKind::Comma.into())),
                        }
                        .with_trailing_whitespace(vec![
                            CstKind::Newline("\n".to_string()),
                            CstKind::Whitespace("  ".to_string())
                        ]),
                        CstKind::ListItem {
                            value: Box::new(build_simple_text("Hi")),
                            comma: Some(Box::new(CstKind::Comma.into()))
                        }
                        .with_trailing_whitespace(vec![CstKind::Newline("\n".to_string())]),
                    ],
                    closing_parenthesis: Box::new(CstKind::ClosingParenthesis.into()),
                }
                .into(),
            )),
        );
    }
}
