use super::{
    body::body,
    function::function,
    int::int,
    list::list,
    literal::{
        arrow, bar, closing_bracket, closing_curly_brace, closing_parenthesis, colon_equals_sign,
        dot, equals_sign, percent,
    },
    struct_::struct_,
    text::text,
    whitespace::{comment, single_line_whitespace, whitespaces_and_newlines},
    word::{identifier, symbol, word},
};
use crate::{
    cst::{CstError, CstKind, IsMultiline},
    rcst::{Rcst, SplitOuterTrailingWhitespace},
};
use tracing::instrument;

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug)]
pub struct ExpressionParsingOptions {
    pub allow_assignment: bool,
    pub allow_call: bool,
    pub allow_bar: bool,
    pub allow_function: bool,
}

#[instrument(level = "trace")]
pub fn expression(
    input: &str,
    indentation: usize,
    options: ExpressionParsingOptions,
) -> Option<(&str, Rcst)> {
    // If we start the call list with `if … else …`, the formatting looks weird.
    // Hence, we start with a single `None`.
    let (mut input, mut result) = None
        .or_else(|| int(input))
        .or_else(|| text(input, indentation))
        .or_else(|| symbol(input))
        .or_else(|| list(input, indentation))
        .or_else(|| struct_(input, indentation, options.allow_function))
        .or_else(|| {
            if options.allow_function {
                function(input, indentation)
            } else {
                None
            }
        })
        .or_else(|| identifier(input))
        .or_else(|| {
            word(input).map(|(input, word)| {
                (
                    input,
                    CstKind::Error {
                        unparsable_input: word,
                        error: CstError::UnexpectedCharacters,
                    }
                    .into(),
                )
            })
        })?;

    loop {
        fn parse_suffix<'input>(
            input: &mut &'input str,
            indentation: usize,
            result: &mut Rcst,
            parser: fn(&'input str, &Rcst, usize) -> Option<(&'input str, Rcst)>,
        ) -> bool {
            if let Some((new_input, expression)) = parser(input, result, indentation) {
                *input = new_input;
                *result = expression;
                true
            } else {
                false
            }
        }

        let mut did_make_progress = false;

        did_make_progress |= parse_suffix(
            &mut input,
            indentation,
            &mut result,
            expression_suffix_struct_access,
        );

        if options.allow_call {
            did_make_progress |=
                parse_suffix(&mut input, indentation, &mut result, expression_suffix_call);
        }
        if options.allow_bar {
            did_make_progress |=
                parse_suffix(&mut input, indentation, &mut result, expression_suffix_bar);
            did_make_progress |= parse_suffix(
                &mut input,
                indentation,
                &mut result,
                expression_suffix_match,
            );
        }

        if options.allow_assignment {
            did_make_progress |= parse_suffix(
                &mut input,
                indentation,
                &mut result,
                expression_suffix_assignment,
            );
        }

        if !did_make_progress {
            break;
        }
    }
    Some((input, result))
}

#[instrument(level = "trace")]
fn expression_suffix_struct_access<'a>(
    input: &'a str,
    current: &Rcst,
    indentation: usize,
) -> Option<(&'a str, Rcst)> {
    let (input, whitespace_after_struct) = whitespaces_and_newlines(input, indentation + 1, true);

    let (input, dot) = dot(input)?;
    let (new_input, whitespace_after_dot) = whitespaces_and_newlines(input, indentation + 1, true);
    let dot = dot.wrap_in_whitespace(whitespace_after_dot);

    let (input, key) = identifier(new_input)?;

    Some((
        input,
        CstKind::StructAccess {
            struct_: Box::new(current.clone().wrap_in_whitespace(whitespace_after_struct)),
            dot: Box::new(dot),
            key: Box::new(key),
        }
        .into(),
    ))
}

#[instrument(level = "trace")]
fn expression_suffix_call<'a>(
    mut input: &'a str,
    current: &Rcst,
    indentation: usize,
) -> Option<(&'a str, Rcst)> {
    let mut expressions = vec![current.clone()];

    let mut has_multiline_whitespace = false;
    loop {
        let (i, whitespace) = whitespaces_and_newlines(input, indentation + 1, true);
        has_multiline_whitespace |= whitespace.is_multiline();
        let indentation = if has_multiline_whitespace {
            indentation + 1
        } else {
            indentation
        };
        let last = expressions.pop().unwrap();
        expressions.push(last.wrap_in_whitespace(whitespace));

        let parsed_expression = expression(
            i,
            indentation,
            ExpressionParsingOptions {
                allow_assignment: false,
                allow_call: has_multiline_whitespace,
                allow_bar: has_multiline_whitespace,
                allow_function: true,
            },
        );
        let (i, expr) = if let Some(it) = parsed_expression {
            it
        } else {
            let fallback = closing_parenthesis(i)
                .or_else(|| closing_bracket(i))
                .or_else(|| closing_curly_brace(i))
                .or_else(|| arrow(i));
            if let Some((i, cst)) = fallback && has_multiline_whitespace {
                        (i, cst)
                    } else {
                        input = i;
                        break;
                    }
        };

        expressions.push(expr);
        input = i;
    }

    if expressions.len() < 2 {
        return None;
    }

    let (whitespace, mut expressions) = expressions.split_outer_trailing_whitespace();
    let receiver = expressions.remove(0);
    let arguments = expressions;

    Some((
        input,
        CstKind::Call {
            receiver: Box::new(receiver),
            arguments,
        }
        .wrap_in_whitespace(whitespace),
    ))
}

#[instrument(level = "trace")]
fn expression_suffix_bar<'a>(
    input: &'a str,
    current: &Rcst,
    indentation: usize,
) -> Option<(&'a str, Rcst)> {
    let (input, whitespace_after_receiver) = whitespaces_and_newlines(input, indentation, true);

    let (input, bar) = bar(input)?;
    let (input, whitespace_after_bar) = whitespaces_and_newlines(input, indentation + 1, true);
    let bar = bar.wrap_in_whitespace(whitespace_after_bar);

    let indentation = if bar.is_multiline() {
        indentation + 1
    } else {
        indentation
    };
    let (input, call) = expression(
        input,
        indentation,
        ExpressionParsingOptions {
            allow_assignment: false,
            allow_call: true,
            allow_bar: false,
            allow_function: true,
        },
    )
    .unwrap_or_else(|| {
        let error = CstKind::Error {
            unparsable_input: String::new(),
            error: CstError::BinaryBarMissesRight,
        };
        (input, error.into())
    });

    Some((
        input,
        CstKind::BinaryBar {
            left: Box::new(
                current
                    .clone()
                    .wrap_in_whitespace(whitespace_after_receiver),
            ),
            bar: Box::new(bar),
            right: Box::new(call),
        }
        .into(),
    ))
}

#[instrument(level = "trace")]
fn expression_suffix_match<'a>(
    input: &'a str,
    current: &Rcst,
    indentation: usize,
) -> Option<(&'a str, Rcst)> {
    let (input, whitespace_after_receiver) = whitespaces_and_newlines(input, indentation, true);
    let (input, percent) = percent(input)?;
    let (mut input, whitespace) = whitespaces_and_newlines(input, indentation + 1, true);
    let percent = percent.wrap_in_whitespace(whitespace);

    let mut cases = vec![];
    loop {
        let Some((new_input, case)) = match_case(input, indentation + 1) else {
            break;
        };
        let (new_input, whitespace) = whitespaces_and_newlines(new_input, indentation + 1, true);
        input = new_input;
        let is_whitespace_multiline = whitespace.is_multiline();
        let case = case.wrap_in_whitespace(whitespace);
        cases.push(case);
        if !is_whitespace_multiline {
            break;
        }
    }
    if cases.is_empty() {
        cases.push(
            CstKind::Error {
                unparsable_input: String::new(),
                error: CstError::MatchMissesCases,
            }
            .into(),
        );
    }

    Some((
        input,
        CstKind::Match {
            expression: Box::new(
                current
                    .clone()
                    .wrap_in_whitespace(whitespace_after_receiver),
            ),
            percent: Box::new(percent),
            cases,
        }
        .into(),
    ))
}

#[instrument(level = "trace")]
fn expression_suffix_assignment<'a>(
    input: &'a str,
    left: &Rcst,
    indentation: usize,
) -> Option<(&'a str, Rcst)> {
    let (input, whitespace_after_left) = whitespaces_and_newlines(input, indentation, true);
    let (input, mut assignment_sign) = colon_equals_sign(input).or_else(|| equals_sign(input))?;

    // By now, it's clear that we are in an assignment, so we can do more
    // expensive operations. We also save some state in case the assignment is
    // invalid (so we can stop parsing right after the assignment sign).
    let left = left.clone().wrap_in_whitespace(whitespace_after_left);
    let just_the_assignment_sign = assignment_sign.clone();
    let input_after_assignment_sign = input;

    let (input, more_whitespace) = whitespaces_and_newlines(input, indentation + 1, true);
    assignment_sign = assignment_sign.wrap_in_whitespace(more_whitespace);

    let is_multiline = left.is_multiline() || assignment_sign.is_multiline();
    let (input, assignment_sign, body) = if is_multiline {
        let (input, body) = body(input, indentation + 1);
        if body.is_empty() {
            (
                input_after_assignment_sign,
                just_the_assignment_sign,
                vec![],
            )
        } else {
            (input, assignment_sign, body)
        }
    } else {
        let mut body = vec![];
        let mut input = input;
        if let Some((new_input, expression)) = expression(
            input,
            indentation,
            ExpressionParsingOptions {
                allow_assignment: false,
                allow_call: true,
                allow_bar: true,
                allow_function: true,
            },
        ) {
            input = new_input;
            body.push(expression);
            if let Some((new_input, whitespace)) = single_line_whitespace(input) {
                input = new_input;
                body.push(whitespace);
            }
        }
        if let Some((new_input, comment)) = comment(input) {
            input = new_input;
            body.push(comment);
        }

        if body.is_empty() {
            (
                input_after_assignment_sign,
                just_the_assignment_sign,
                vec![],
            )
        } else {
            (input, assignment_sign, body)
        }
    };

    let (whitespace, (assignment_sign, body)) =
        (assignment_sign, body).split_outer_trailing_whitespace();
    Some((
        input,
        CstKind::Assignment {
            left: Box::new(left),
            assignment_sign: Box::new(assignment_sign),
            body,
        }
        .wrap_in_whitespace(whitespace),
    ))
}

#[instrument(level = "trace")]
fn match_case(input: &str, indentation: usize) -> Option<(&str, Rcst)> {
    let (input, pattern) = expression(
        input,
        indentation,
        ExpressionParsingOptions {
            allow_assignment: false,
            allow_call: true,
            allow_bar: true,
            allow_function: true,
        },
    )?;
    let (input, whitespace) = whitespaces_and_newlines(input, indentation, true);
    let pattern = pattern.wrap_in_whitespace(whitespace);

    let (input, arrow) = if let Some((input, arrow)) = arrow(input) {
        let (input, whitespace) = whitespaces_and_newlines(input, indentation, true);
        (input, arrow.wrap_in_whitespace(whitespace))
    } else {
        let error = CstKind::Error {
            unparsable_input: String::new(),
            error: CstError::MatchCaseMissesArrow,
        };
        (input, error.into())
    };

    let (input, mut body) = body(input, indentation + 1);
    if body.is_empty() {
        body.push(
            CstKind::Error {
                unparsable_input: String::new(),
                error: CstError::MatchCaseMissesBody,
            }
            .into(),
        );
    }

    let case = CstKind::MatchCase {
        pattern: Box::new(pattern),
        arrow: Box::new(arrow),
        body,
    };
    Some((input, case.into()))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::string_to_rcst::utils::{
        build_comment, build_identifier, build_newline, build_simple_int, build_space, build_symbol,
    };

    #[test]
    fn test_expression() {
        assert_eq!(
            expression(
                "foo",
                0,
                ExpressionParsingOptions {
                    allow_assignment: true,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some(("", build_identifier("foo")))
        );
        assert_eq!(
            expression(
                "(foo Bar)",
                0,
                ExpressionParsingOptions {
                    allow_assignment: false,
                    allow_call: false,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "",
                CstKind::Parenthesized {
                    opening_parenthesis: Box::new(CstKind::OpeningParenthesis.into()),
                    inner: Box::new(
                        CstKind::Call {
                            receiver: Box::new(build_identifier("foo").with_trailing_space()),
                            arguments: vec![build_symbol("Bar")],
                        }
                        .into()
                    ),
                    closing_parenthesis: Box::new(CstKind::ClosingParenthesis.into())
                }
                .into(),
            )),
        );
        // foo
        //   .bar
        assert_eq!(
            expression(
                "foo\n  .bar",
                0,
                ExpressionParsingOptions {
                    allow_assignment: true,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "",
                CstKind::StructAccess {
                    struct_: Box::new(build_identifier("foo").with_trailing_whitespace(vec![
                        CstKind::Newline("\n".to_string()),
                        CstKind::Whitespace("  ".to_string()),
                    ])),
                    dot: Box::new(CstKind::Dot.into()),
                    key: Box::new(build_identifier("bar")),
                }
                .into(),
            )),
        );
        // foo
        // .bar
        assert_eq!(
            expression(
                "foo\n.bar",
                0,
                ExpressionParsingOptions {
                    allow_assignment: true,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some(("\n.bar", build_identifier("foo"))),
        );
        // foo
        // | bar
        assert_eq!(
            expression(
                "foo\n| bar",
                0,
                ExpressionParsingOptions {
                    allow_assignment: true,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "",
                CstKind::BinaryBar {
                    left: Box::new(
                        build_identifier("foo")
                            .with_trailing_whitespace(vec![CstKind::Newline("\n".to_string())]),
                    ),
                    bar: Box::new(CstKind::Bar.with_trailing_space()),
                    right: Box::new(build_identifier("bar")),
                }
                .into(),
            )),
        );
        // foo
        // | bar baz
        assert_eq!(
            expression(
                "foo\n| bar baz",
                0,
                ExpressionParsingOptions {
                    allow_assignment: true,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "",
                CstKind::BinaryBar {
                    left: Box::new(
                        build_identifier("foo")
                            .with_trailing_whitespace(vec![CstKind::Newline("\n".to_string())]),
                    ),
                    bar: Box::new(CstKind::Bar.with_trailing_space()),
                    right: Box::new(
                        CstKind::Call {
                            receiver: Box::new(build_identifier("bar").with_trailing_space()),
                            arguments: vec![build_identifier("baz")],
                        }
                        .into(),
                    ),
                }
                .into(),
            )),
        );
        // foo %
        //   123 -> 123
        assert_eq!(
            expression(
                "foo %\n  123 -> 123",
                0,
                ExpressionParsingOptions {
                    allow_assignment: true,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "",
                CstKind::Match {
                    expression: Box::new(build_identifier("foo").with_trailing_space()),
                    percent: Box::new(CstKind::Percent.with_trailing_whitespace(vec![
                        CstKind::Newline("\n".to_string()),
                        CstKind::Whitespace("  ".to_string()),
                    ])),
                    cases: vec![CstKind::MatchCase {
                        pattern: Box::new(build_simple_int(123).with_trailing_space()),
                        arrow: Box::new(CstKind::Arrow.with_trailing_space()),
                        body: vec![build_simple_int(123)],
                    }
                    .into()],
                }
                .into(),
            )),
        );
        assert_eq!(
            expression(
                "(0, foo) | (foo, 0)",
                0,
                ExpressionParsingOptions {
                    allow_assignment: false,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "",
                CstKind::BinaryBar {
                    left: Box::new(
                        CstKind::List {
                            opening_parenthesis: Box::new(CstKind::OpeningParenthesis.into()),
                            items: vec![
                                CstKind::ListItem {
                                    value: Box::new(build_simple_int(0)),
                                    comma: Some(Box::new(CstKind::Comma.into())),
                                }
                                .with_trailing_space(),
                                CstKind::ListItem {
                                    value: Box::new(build_identifier("foo")),
                                    comma: None,
                                }
                                .into(),
                            ],
                            closing_parenthesis: Box::new(CstKind::ClosingParenthesis.into()),
                        }
                        .with_trailing_space(),
                    ),
                    bar: Box::new(CstKind::Bar.with_trailing_space()),
                    right: Box::new(
                        CstKind::List {
                            opening_parenthesis: Box::new(CstKind::OpeningParenthesis.into()),
                            items: vec![
                                CstKind::ListItem {
                                    value: Box::new(build_identifier("foo")),
                                    comma: Some(Box::new(CstKind::Comma.into())),
                                }
                                .with_trailing_space(),
                                CstKind::ListItem {
                                    value: Box::new(build_simple_int(0)),
                                    comma: None,
                                }
                                .into(),
                            ],
                            closing_parenthesis: Box::new(CstKind::ClosingParenthesis.into()),
                        }
                        .into(),
                    ),
                }
                .into(),
            )),
        );
        assert_eq!(
            expression(
                "foo bar",
                0,
                ExpressionParsingOptions {
                    allow_assignment: false,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "",
                CstKind::Call {
                    receiver: Box::new(build_identifier("foo").with_trailing_space()),
                    arguments: vec![build_identifier("bar")],
                }
                .into(),
            ))
        );
        assert_eq!(
            expression(
                "Foo 4 bar",
                0,
                ExpressionParsingOptions {
                    allow_assignment: false,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "",
                CstKind::Call {
                    receiver: Box::new(build_symbol("Foo").with_trailing_space()),
                    arguments: vec![
                        build_simple_int(4).with_trailing_space(),
                        build_identifier("bar"),
                    ],
                }
                .into(),
            )),
        );
        // foo
        //   bar
        //   baz
        // 2
        assert_eq!(
            expression(
                "foo\n  bar\n  baz\n2",
                0,
                ExpressionParsingOptions {
                    allow_assignment: false,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "\n2",
                CstKind::Call {
                    receiver: Box::new(build_identifier("foo").with_trailing_whitespace(vec![
                        CstKind::Newline("\n".to_string()),
                        CstKind::Whitespace("  ".to_string()),
                    ])),
                    arguments: vec![
                        build_identifier("bar").with_trailing_whitespace(vec![
                            CstKind::Newline("\n".to_string()),
                            CstKind::Whitespace("  ".to_string()),
                        ]),
                        build_identifier("baz"),
                    ],
                }
                .into(),
            )),
        );
        // foo 1 2
        //   3
        //   4
        // bar
        assert_eq!(
            expression(
                "foo 1 2\n  3\n  4\nbar",
                0,
                ExpressionParsingOptions {
                    allow_assignment: false,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "\nbar",
                CstKind::Call {
                    receiver: Box::new(build_identifier("foo").with_trailing_space()),
                    arguments: vec![
                        build_simple_int(1).with_trailing_space(),
                        build_simple_int(2).with_trailing_whitespace(vec![
                            CstKind::Newline("\n".to_string()),
                            CstKind::Whitespace("  ".to_string()),
                        ]),
                        build_simple_int(3).with_trailing_whitespace(vec![
                            CstKind::Newline("\n".to_string()),
                            CstKind::Whitespace("  ".to_string()),
                        ]),
                        build_simple_int(4),
                    ],
                }
                .into(),
            )),
        );
        // foo
        //   bar | baz
        assert_eq!(
            expression(
                "foo\n  bar | baz",
                0,
                ExpressionParsingOptions {
                    allow_assignment: true,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "",
                CstKind::Call {
                    receiver: Box::new(build_identifier("foo").with_trailing_whitespace(vec![
                        CstKind::Newline("\n".to_string()),
                        CstKind::Whitespace("  ".to_string()),
                    ])),
                    arguments: vec![CstKind::BinaryBar {
                        left: Box::new(build_identifier("bar").with_trailing_space()),
                        bar: Box::new(CstKind::Bar.with_trailing_space()),
                        right: Box::new(build_identifier("baz")),
                    }
                    .into()],
                }
                .into(),
            )),
        );
        assert_eq!(
            expression(
                "(foo Bar) Baz\n",
                0,
                ExpressionParsingOptions {
                    allow_assignment: false,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "\n",
                CstKind::Call {
                    receiver: Box::new(
                        CstKind::Parenthesized {
                            opening_parenthesis: Box::new(CstKind::OpeningParenthesis.into()),
                            inner: Box::new(
                                CstKind::Call {
                                    receiver: Box::new(
                                        build_identifier("foo").with_trailing_space(),
                                    ),
                                    arguments: vec![build_symbol("Bar")],
                                }
                                .into()
                            ),
                            closing_parenthesis: Box::new(CstKind::ClosingParenthesis.into()),
                        }
                        .with_trailing_space(),
                    ),
                    arguments: vec![build_symbol("Baz")]
                }
                .into(),
            )),
        );
        // foo T
        //
        //
        // bar = 5
        assert_eq!(
            expression(
                "foo T\n\n\nbar = 5",
                0,
                ExpressionParsingOptions {
                    allow_assignment: false,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "\n\n\nbar = 5",
                CstKind::Call {
                    receiver: Box::new(build_identifier("foo").with_trailing_space()),
                    arguments: vec![build_symbol("T")],
                }
                .into(),
            )),
        );
        assert_eq!(
            expression(
                "foo = 42",
                0,
                ExpressionParsingOptions {
                    allow_assignment: true,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "",
                CstKind::Assignment {
                    left: Box::new(build_identifier("foo").with_trailing_space()),
                    assignment_sign: Box::new(CstKind::EqualsSign.with_trailing_space()),
                    body: vec![build_simple_int(42)],
                }
                .into(),
            )),
        );
        assert_eq!(
            expression(
                "foo =\n  bar\n\nbaz",
                0,
                ExpressionParsingOptions {
                    allow_assignment: true,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "\n\nbaz",
                CstKind::Assignment {
                    left: Box::new(build_identifier("foo").with_trailing_space()),
                    assignment_sign: Box::new(CstKind::EqualsSign.with_trailing_whitespace(vec![
                        CstKind::Newline("\n".to_string()),
                        CstKind::Whitespace("  ".to_string())
                    ])),
                    body: vec![build_identifier("bar")],
                }
                .into(),
            )),
        );
        assert_eq!(
            expression(
                "foo 42",
                0,
                ExpressionParsingOptions {
                    allow_assignment: true,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "",
                CstKind::Call {
                    receiver: Box::new(build_identifier("foo").with_trailing_space()),
                    arguments: vec![build_simple_int(42)],
                }
                .into(),
            )),
        );
        assert_eq!(
            expression(
                "foo %",
                0,
                ExpressionParsingOptions {
                    allow_assignment: false,
                    allow_call: false,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "",
                CstKind::Match {
                    expression: Box::new(build_identifier("foo").with_trailing_space()),
                    percent: Box::new(CstKind::Percent.into()),
                    cases: vec![CstKind::Error {
                        unparsable_input: String::new(),
                        error: CstError::MatchMissesCases,
                    }
                    .into()],
                }
                .into(),
            )),
        );
        assert_eq!(
            expression(
                "foo %\n",
                0,
                ExpressionParsingOptions {
                    allow_assignment: false,
                    allow_call: false,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "\n",
                CstKind::Match {
                    expression: Box::new(build_identifier("foo").with_trailing_space()),
                    percent: Box::new(CstKind::Percent.into()),
                    cases: vec![CstKind::Error {
                        unparsable_input: String::new(),
                        error: CstError::MatchMissesCases,
                    }
                    .into()],
                }
                .into(),
            )),
        );
        // foo %
        //   1 -> 2
        // Foo
        assert_eq!(
            expression(
                "foo %\n  1 -> 2\nFoo",
                0,
                ExpressionParsingOptions {
                    allow_assignment: false,
                    allow_call: false,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "\nFoo",
                CstKind::Match {
                    expression: Box::new(build_identifier("foo").with_trailing_space()),
                    percent: Box::new(CstKind::Percent.with_trailing_whitespace(vec![
                        CstKind::Newline("\n".to_string()),
                        CstKind::Whitespace("  ".to_string()),
                    ])),
                    cases: vec![CstKind::MatchCase {
                        pattern: Box::new(build_simple_int(1).with_trailing_space()),
                        arrow: Box::new(CstKind::Arrow.with_trailing_space()),
                        body: vec![build_simple_int(2)],
                    }
                    .into()],
                }
                .into(),
            )),
        );
        // foo bar =
        //   3
        // 2
        assert_eq!(
            expression(
                "foo bar =\n  3\n2",
                0,
                ExpressionParsingOptions {
                    allow_assignment: true,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "\n2",
                CstKind::Assignment {
                    left: Box::new(
                        CstKind::Call {
                            receiver: Box::new(build_identifier("foo").with_trailing_space()),
                            arguments: vec![build_identifier("bar")],
                        }
                        .with_trailing_space(),
                    ),
                    assignment_sign: Box::new(CstKind::EqualsSign.with_trailing_whitespace(vec![
                        CstKind::Newline("\n".to_string()),
                        CstKind::Whitespace("  ".to_string())
                    ])),
                    body: vec![build_simple_int(3)],
                }
                .into(),
            )),
        );
        // main := { environment ->
        //   input
        // }
        assert_eq!(
            expression(
                "main := { environment ->\n  input\n}",
                0,
                ExpressionParsingOptions {
                    allow_assignment: true,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "",
                CstKind::Assignment {
                    left: Box::new(build_identifier("main").with_trailing_space()),
                    assignment_sign: Box::new(CstKind::ColonEqualsSign.with_trailing_space()),
                    body: vec![CstKind::Function {
                        opening_curly_brace: Box::new(
                            CstKind::OpeningCurlyBrace.with_trailing_space()
                        ),
                        parameters_and_arrow: Some((
                            vec![build_identifier("environment").with_trailing_space()],
                            Box::new(CstKind::Arrow.with_trailing_whitespace(vec![
                                CstKind::Newline("\n".to_string()),
                                CstKind::Whitespace("  ".to_string()),
                            ])),
                        )),
                        body: vec![build_identifier("input"), build_newline()],
                        closing_curly_brace: Box::new(CstKind::ClosingCurlyBrace.into()),
                    }
                    .into()],
                }
                .into(),
            )),
        );
        // foo
        //   bar
        //   = 3
        assert_eq!(
            expression(
                "foo\n  bar\n  = 3",
                0,
                ExpressionParsingOptions {
                    allow_assignment: true,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "",
                CstKind::Assignment {
                    left: Box::new(
                        CstKind::Call {
                            receiver: Box::new(build_identifier("foo").with_trailing_whitespace(
                                vec![
                                    CstKind::Newline("\n".to_string()),
                                    CstKind::Whitespace("  ".to_string()),
                                ]
                            )),
                            arguments: vec![build_identifier("bar")],
                        }
                        .with_trailing_whitespace(vec![
                            CstKind::Newline("\n".to_string()),
                            CstKind::Whitespace("  ".to_string()),
                        ])
                    ),
                    assignment_sign: Box::new(CstKind::EqualsSign.with_trailing_space()),
                    body: vec![build_simple_int(3)],
                }
                .into(),
            )),
        );
        assert_eq!(
            expression(
                "foo =\n  ",
                0,
                ExpressionParsingOptions {
                    allow_assignment: true,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "\n  ",
                CstKind::Assignment {
                    left: Box::new(build_identifier("foo").with_trailing_space()),
                    assignment_sign: Box::new(CstKind::EqualsSign.into()),
                    body: vec![],
                }
                .into(),
            )),
        );
        assert_eq!(
            expression(
                "foo = # comment\n",
                0,
                ExpressionParsingOptions {
                    allow_assignment: true,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "\n",
                CstKind::Assignment {
                    left: Box::new(build_identifier("foo").with_trailing_space()),
                    assignment_sign: Box::new(CstKind::EqualsSign.with_trailing_space()),
                    body: vec![build_comment(" comment")],
                }
                .into(),
            )),
        );
        assert_eq!(
            expression(
                "foo = bar # comment\n",
                0,
                ExpressionParsingOptions {
                    allow_assignment: true,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "\n",
                CstKind::Assignment {
                    left: Box::new(build_identifier("foo").with_trailing_space()),
                    assignment_sign: Box::new(CstKind::EqualsSign.with_trailing_space()),
                    body: vec![
                        build_identifier("bar"),
                        build_space(),
                        build_comment(" comment"),
                    ],
                }
                .into(),
            )),
        );
        // foo =
        //   # comment
        // 3
        assert_eq!(
            expression(
                "foo =\n  # comment\n3",
                0,
                ExpressionParsingOptions {
                    allow_assignment: true,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "\n3",
                CstKind::Assignment {
                    left: Box::new(build_identifier("foo").with_trailing_space()),
                    assignment_sign: Box::new(CstKind::EqualsSign.with_trailing_whitespace(vec![
                        CstKind::Newline("\n".to_string()),
                        CstKind::Whitespace("  ".to_string()),
                    ])),
                    body: vec![build_comment(" comment")],
                }
                .into(),
            )),
        );
        // foo =
        //   # comment
        //   5
        // 3
        assert_eq!(
            expression(
                "foo =\n  # comment\n  5\n3",
                0,
                ExpressionParsingOptions {
                    allow_assignment: true,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "\n3",
                CstKind::Assignment {
                    left: Box::new(build_identifier("foo").with_trailing_space()),
                    assignment_sign: Box::new(CstKind::EqualsSign.with_trailing_whitespace(vec![
                        CstKind::Newline("\n".to_string()),
                        CstKind::Whitespace("  ".to_string()),
                    ])),
                    body: vec![
                        build_comment(" comment"),
                        build_newline(),
                        CstKind::Whitespace("  ".to_string()).into(),
                        build_simple_int(5),
                    ],
                }
                .into(),
            )),
        );
        assert_eq!(
            expression(
                "(foo, bar) = (1, 2)",
                0,
                ExpressionParsingOptions {
                    allow_assignment: true,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "",
                CstKind::Assignment {
                    left: Box::new(
                        CstKind::List {
                            opening_parenthesis: Box::new(CstKind::OpeningParenthesis.into()),
                            items: vec![
                                CstKind::ListItem {
                                    value: Box::new(build_identifier("foo")),
                                    comma: Some(Box::new(CstKind::Comma.into())),
                                }
                                .with_trailing_space(),
                                CstKind::ListItem {
                                    value: Box::new(build_identifier("bar")),
                                    comma: None,
                                }
                                .into(),
                            ],
                            closing_parenthesis: Box::new(CstKind::ClosingParenthesis.into()),
                        }
                        .with_trailing_space()
                    ),
                    assignment_sign: Box::new(CstKind::EqualsSign.with_trailing_space()),
                    body: vec![CstKind::List {
                        opening_parenthesis: Box::new(CstKind::OpeningParenthesis.into()),
                        items: vec![
                            CstKind::ListItem {
                                value: Box::new(build_simple_int(1)),
                                comma: Some(Box::new(CstKind::Comma.into())),
                            }
                            .with_trailing_space(),
                            CstKind::ListItem {
                                value: Box::new(build_simple_int(2)),
                                comma: None,
                            }
                            .into(),
                        ],
                        closing_parenthesis: Box::new(CstKind::ClosingParenthesis.into()),
                    }
                    .into()],
                }
                .into(),
            )),
        );
        assert_eq!(
            expression(
                "[Foo: foo] = bar",
                0,
                ExpressionParsingOptions {
                    allow_assignment: true,
                    allow_call: true,
                    allow_bar: true,
                    allow_function: true
                }
            ),
            Some((
                "",
                CstKind::Assignment {
                    left: Box::new(
                        CstKind::Struct {
                            opening_bracket: Box::new(CstKind::OpeningBracket.into()),
                            fields: vec![CstKind::StructField {
                                key_and_colon: Some(Box::new((
                                    build_symbol("Foo"),
                                    CstKind::Colon.with_trailing_space(),
                                ))),
                                value: Box::new(build_identifier("foo")),
                                comma: None,
                            }
                            .into()],
                            closing_bracket: Box::new(CstKind::ClosingBracket.into()),
                        }
                        .with_trailing_space(),
                    ),
                    assignment_sign: Box::new(CstKind::EqualsSign.with_trailing_space()),
                    body: vec![build_identifier("bar")],
                }
                .into(),
            )),
        );
    }
}
