use super::{
    body::body,
    expression::{expression, ExpressionParsingOptions},
    literal::{arrow, closing_curly_brace, opening_curly_brace},
    whitespace::whitespaces_and_newlines,
};
use crate::{
    cst::{CstError, CstKind},
    rcst::Rcst,
};
use tracing::instrument;

#[instrument(level = "trace")]
pub fn function(input: &str, indentation: usize) -> Option<(&str, Rcst)> {
    let (input, opening_curly_brace) = opening_curly_brace(input)?;
    let (input, mut opening_curly_brace, mut parameters_and_arrow) = {
        let input_without_params = input;
        let opening_curly_brace_wihout_params = opening_curly_brace.clone();

        let mut input = input;
        let mut opening_curly_brace = opening_curly_brace;
        let mut parameters: Vec<Rcst> = vec![];
        loop {
            let (i, whitespace) = whitespaces_and_newlines(input, indentation + 1, true);
            if let Some(last_parameter) = parameters.pop() {
                parameters.push(last_parameter.wrap_in_whitespace(whitespace));
            } else {
                opening_curly_brace = opening_curly_brace.wrap_in_whitespace(whitespace);
            }

            input = i;
            match expression(
                input,
                indentation + 1,
                ExpressionParsingOptions {
                    allow_assignment: false,
                    allow_call: false,
                    allow_bar: false,
                    allow_function: false,
                },
            ) {
                Some((i, parameter)) => {
                    input = i;
                    parameters.push(parameter);
                }
                None => break,
            };
        }
        match arrow(input) {
            Some((input, arrow)) => (input, opening_curly_brace, Some((parameters, arrow))),
            None => (
                input_without_params,
                opening_curly_brace_wihout_params,
                None,
            ),
        }
    };

    let (input, whitespace) = whitespaces_and_newlines(input, indentation + 1, true);
    if let Some((parameters, arrow)) = parameters_and_arrow {
        parameters_and_arrow = Some((parameters, arrow.wrap_in_whitespace(whitespace)));
    } else {
        opening_curly_brace = opening_curly_brace.wrap_in_whitespace(whitespace);
    }

    let mut body_expressions = vec![];
    let (input, mut whitespace_before_closing_curly_brace, closing_curly_brace) = {
        let input = match expression(
            input,
            indentation + 1,
            ExpressionParsingOptions {
                allow_assignment: true,
                allow_call: true,
                allow_bar: true,
                allow_function: true,
            },
        ) {
            Some((input, expression)) => {
                body_expressions.push(expression);
                input
            }
            None => input,
        };
        let (input, mut whitespace) = whitespaces_and_newlines(input, indentation + 1, true);

        if let Some((input, curly_brace)) = closing_curly_brace(input) {
            (input, whitespace, curly_brace)
        } else {
            // There is no closing brace after a single expression. Thus, we now
            // try to parse a body of multiple expressions. We didn't try this
            // first because then the body would also have consumed any trailing
            // closing curly brace in the same line.
            //
            // For example, for the function `{ 2 }`, the body parser would have
            // already consumed the `}`. The body parser works great for
            // multiline bodies, though.
            body_expressions.append(&mut whitespace);
            let (input, mut body) = body(input, indentation + 1);
            body_expressions.append(&mut body);

            let input_after_body = input;
            let (input, whitespace) = whitespaces_and_newlines(input, indentation, true);
            match closing_curly_brace(input) {
                Some((input, closing_curly_brace)) => (input, whitespace, closing_curly_brace),
                None => (
                    input_after_body,
                    vec![],
                    CstKind::Error {
                        unparsable_input: String::new(),
                        error: CstError::CurlyBraceNotClosed,
                    }
                    .into(),
                ),
            }
        }
    };

    // Attach the `whitespace_before_closing_curly_brace`.
    if !body_expressions.is_empty() {
        body_expressions.append(&mut whitespace_before_closing_curly_brace);
    } else if let Some((parameters, arrow)) = parameters_and_arrow {
        parameters_and_arrow = Some((
            parameters,
            arrow.wrap_in_whitespace(whitespace_before_closing_curly_brace),
        ));
    } else {
        opening_curly_brace =
            opening_curly_brace.wrap_in_whitespace(whitespace_before_closing_curly_brace);
    }

    Some((
        input,
        CstKind::Function {
            opening_curly_brace: Box::new(opening_curly_brace),
            parameters_and_arrow: parameters_and_arrow
                .map(|(parameters, arrow)| (parameters, Box::new(arrow))),
            body: body_expressions,
            closing_curly_brace: Box::new(closing_curly_brace),
        }
        .into(),
    ))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::string_to_rcst::utils::{
        build_comment, build_identifier, build_newline, build_simple_int, build_space,
    };

    #[test]
    fn test_function() {
        assert_eq!(function("2", 0), None);
        assert_eq!(
            function("{ 2 }", 0),
            Some((
                "",
                CstKind::Function {
                    opening_curly_brace: Box::new(CstKind::OpeningCurlyBrace.with_trailing_space()),
                    parameters_and_arrow: None,
                    body: vec![build_simple_int(2), build_space()],
                    closing_curly_brace: Box::new(CstKind::ClosingCurlyBrace.into()),
                }
                .into(),
            )),
        );
        // { a ->
        //   foo
        // }
        assert_eq!(
            function("{ a ->\n  foo\n}", 0),
            Some((
                "",
                CstKind::Function {
                    opening_curly_brace: Box::new(CstKind::OpeningCurlyBrace.with_trailing_space()),
                    parameters_and_arrow: Some((
                        vec![build_identifier("a").with_trailing_space()],
                        Box::new(CstKind::Arrow.with_trailing_whitespace(vec![
                            CstKind::Newline("\n".to_string()),
                            CstKind::Whitespace("  ".to_string())
                        ])),
                    )),
                    body: vec![build_identifier("foo"), build_newline()],
                    closing_curly_brace: Box::new(CstKind::ClosingCurlyBrace.into()),
                }
                .into(),
            )),
        );
        // {
        // foo
        assert_eq!(
            function("{\nfoo", 0),
            Some((
                "\nfoo",
                CstKind::Function {
                    opening_curly_brace: Box::new(CstKind::OpeningCurlyBrace.into()),
                    parameters_and_arrow: None,
                    body: vec![],
                    closing_curly_brace: Box::new(
                        CstKind::Error {
                            unparsable_input: String::new(),
                            error: CstError::CurlyBraceNotClosed
                        }
                        .into()
                    ),
                }
                .into(),
            )),
        );
        // {->
        // }
        assert_eq!(
            function("{->\n}", 1),
            Some((
                "\n}",
                CstKind::Function {
                    opening_curly_brace: Box::new(CstKind::OpeningCurlyBrace.into()),
                    parameters_and_arrow: Some((vec![], Box::new(CstKind::Arrow.into()))),
                    body: vec![],
                    closing_curly_brace: Box::new(
                        CstKind::Error {
                            unparsable_input: String::new(),
                            error: CstError::CurlyBraceNotClosed
                        }
                        .into()
                    ),
                }
                .into(),
            )),
        );
        // { foo
        //   bar
        // }
        assert_eq!(
            function("{ foo\n  bar\n}", 0),
            Some((
                "",
                CstKind::Function {
                    opening_curly_brace: Box::new(CstKind::OpeningCurlyBrace.with_trailing_space()),
                    parameters_and_arrow: None,
                    body: vec![
                        build_identifier("foo"),
                        build_newline(),
                        CstKind::Whitespace("  ".to_string()).into(),
                        build_identifier("bar"),
                        build_newline(),
                    ],
                    closing_curly_brace: Box::new(CstKind::ClosingCurlyBrace.into())
                }
                .into(),
            )),
        );
        // { foo # abc
        // }
        assert_eq!(
            function("{ foo # abc\n}", 0),
            Some((
                "",
                CstKind::Function {
                    opening_curly_brace: Box::new(CstKind::OpeningCurlyBrace.with_trailing_space()),
                    parameters_and_arrow: None,
                    body: vec![
                        build_identifier("foo"),
                        build_space(),
                        build_comment(" abc"),
                        build_newline(),
                    ],
                    closing_curly_brace: Box::new(CstKind::ClosingCurlyBrace.into())
                }
                .into()
            )),
        );
    }
}
