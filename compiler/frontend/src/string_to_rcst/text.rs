use super::{
    expression::{expression, ExpressionParsingOptions},
    literal::{closing_curly_brace, double_quote, newline, opening_curly_brace, single_quote},
    utils::parse_multiple,
    whitespace::whitespaces_and_newlines,
};
use crate::{
    cst::{CstError, CstKind},
    rcst::Rcst,
};
use itertools::Itertools;
use tracing::instrument;

// TODO: It might be a good idea to ignore text interpolations in patterns
#[instrument(level = "trace")]
pub fn text(input: &str, indentation: usize) -> Option<(&str, Rcst)> {
    let (input, opening_single_quotes) = parse_multiple(input, single_quote, None)?;
    let (mut input, opening_double_quote) = double_quote(input)?;

    let push_line_to_parts = |line: &mut Vec<char>, parts: &mut Vec<Rcst>| {
        let joined_line = line.drain(..).join("");
        if !joined_line.is_empty() {
            parts.push(CstKind::TextPart(joined_line).into());
        }
    };

    let mut line = vec![];
    let mut parts = vec![];
    let closing = loop {
        match input.chars().next() {
            Some('"') => {
                input = &input[1..];
                match parse_multiple(
                    input,
                    single_quote,
                    Some((opening_single_quotes.len(), false)),
                ) {
                    Some((input_after_single_quotes, closing_single_quotes)) => {
                        input = input_after_single_quotes;
                        push_line_to_parts(&mut line, &mut parts);
                        break CstKind::ClosingText {
                            closing_double_quote: Box::new(CstKind::DoubleQuote.into()),
                            closing_single_quotes,
                        };
                    }
                    None => line.push('"'),
                }
            }
            Some('{') => {
                if let Some((input_after_interpolation, interpolation)) =
                    text_interpolation(input, indentation, opening_single_quotes.len() + 1)
                {
                    push_line_to_parts(&mut line, &mut parts);
                    input = input_after_interpolation;
                    parts.push(interpolation);
                } else {
                    input = &input[1..];
                    line.push('{');
                }
            }
            None => {
                push_line_to_parts(&mut line, &mut parts);
                break CstKind::Error {
                    unparsable_input: String::new(),
                    error: CstError::TextNotClosed,
                };
            }
            Some('\n') => {
                push_line_to_parts(&mut line, &mut parts);
                let (i, mut whitespace) = whitespaces_and_newlines(input, indentation + 1, false);
                input = i;
                parts.append(&mut whitespace);
                if input.starts_with('\n') {
                    break CstKind::Error {
                        unparsable_input: String::new(),
                        error: CstError::TextNotSufficientlyIndented,
                    };
                }
            }
            Some(c) => {
                input = &input[c.len_utf8()..];
                line.push(c);
            }
        }
    };
    Some((
        input,
        CstKind::Text {
            opening: Box::new(
                CstKind::OpeningText {
                    opening_single_quotes,
                    opening_double_quote: Box::new(opening_double_quote),
                }
                .into(),
            ),
            parts,
            closing: Box::new(closing.into()),
        }
        .into(),
    ))
}

#[instrument(level = "trace")]
fn text_interpolation(
    input: &str,
    indentation: usize,
    curly_brace_count: usize,
) -> Option<(&str, Rcst)> {
    let (input, mut opening_curly_braces) =
        parse_multiple(input, opening_curly_brace, Some((curly_brace_count, true)))?;

    let (input, whitespace) = whitespaces_and_newlines(input, indentation + 1, false);
    let last = opening_curly_braces.pop().unwrap();
    opening_curly_braces.push(last.wrap_in_whitespace(whitespace));

    let (input, mut expression) = expression(
        input,
        indentation + 1,
        ExpressionParsingOptions {
            allow_assignment: false,
            allow_call: true,
            allow_bar: true,
            allow_function: true,
        },
    )
    .unwrap_or((
        input,
        CstKind::Error {
            unparsable_input: String::new(),
            error: CstError::TextInterpolationMissesExpression,
        }
        .into(),
    ));

    let (input, whitespace) = whitespaces_and_newlines(input, indentation + 1, false);
    expression = expression.wrap_in_whitespace(whitespace);

    let (input, closing_curly_braces) =
        parse_multiple(input, closing_curly_brace, Some((curly_brace_count, false))).unwrap_or((
            input,
            vec![CstKind::Error {
                unparsable_input: String::new(),
                error: CstError::TextInterpolationNotClosed,
            }
            .into()],
        ));

    Some((
        input,
        CstKind::TextInterpolation {
            opening_curly_braces,
            expression: Box::new(expression),
            closing_curly_braces,
        }
        .into(),
    ))
}

#[instrument(level = "trace")]
fn text_part(mut input: &str, single_quotes_count: usize) -> Option<(&str, Rcst)> {
    let mut text_part = vec![];
    loop {
        let next_char = input.chars().next();
        // TODO Optimize this somehow
        if next_char.is_none()
            || newline(input).is_some()
            || parse_multiple(
                input,
                opening_curly_brace,
                Some((single_quotes_count + 1, true)),
            )
            .is_some()
            || double_quote(input)
                .and_then(|(input_after_double_quote, _)| {
                    parse_multiple(
                        input_after_double_quote,
                        single_quote,
                        Some((single_quotes_count, false)),
                    )
                })
                .is_some()
        {
            let text_part = text_part.iter().join("");
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

#[instrument(level = "trace")]
fn convert_whitespace_into_text_newlines(whitespace: Vec<Rcst>) -> Vec<Rcst> {
    let mut last_newline: Option<Rcst> = None;
    let mut whitespace_after_last_newline: Vec<Rcst> = vec![];
    let mut parts: Vec<Rcst> = vec![];
    for whitespace in whitespace
        .iter()
        .chain(std::iter::once(&CstKind::Newline("\n".to_string()).into()))
    {
        if let CstKind::Newline(newline) = whitespace.kind.clone() {
            if let Some(last_newline) = last_newline {
                parts.push(last_newline.wrap_in_whitespace(whitespace_after_last_newline));
                whitespace_after_last_newline = vec![];
            }
            last_newline = Some(CstKind::TextNewline(newline).into());
        } else {
            whitespace_after_last_newline.push(whitespace.clone());
        }
    }
    parts
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::string_to_rcst::utils::{
        build_identifier, build_newline, build_simple_int, build_simple_text,
    };

    #[test]
    fn test_text() {
        assert_eq!(text("foo", 0), None);
        assert_eq!(
            text(r#""foo" bar"#, 0),
            Some((" bar", build_simple_text("foo"))),
        );
        // "foo
        //   bar"2
        assert_eq!(
            text("\"foo\n  bar\"2", 0),
            Some((
                "2",
                CstKind::Text {
                    opening: Box::new(
                        CstKind::OpeningText {
                            opening_single_quotes: vec![],
                            opening_double_quote: Box::new(CstKind::DoubleQuote.into()),
                        }
                        .into(),
                    ),
                    parts: vec![
                        CstKind::TextPart("foo".to_string()).into(),
                        build_newline(),
                        CstKind::Whitespace("  ".to_string()).into(),
                        CstKind::TextPart("bar".to_string()).into(),
                    ],
                    closing: Box::new(
                        CstKind::ClosingText {
                            closing_double_quote: Box::new(CstKind::DoubleQuote.into()),
                            closing_single_quotes: vec![]
                        }
                        .into(),
                    ),
                }
                .into(),
            )),
        );
        //   "foo
        //   bar"
        assert_eq!(
            text("\"foo\n  bar\"2", 1),
            Some((
                "\n  bar\"2",
                CstKind::Text {
                    opening: Box::new(
                        CstKind::OpeningText {
                            opening_single_quotes: vec![],
                            opening_double_quote: Box::new(CstKind::DoubleQuote.into()),
                        }
                        .into(),
                    ),
                    parts: vec![CstKind::TextPart("foo".to_string()).into()],
                    closing: Box::new(
                        CstKind::Error {
                            unparsable_input: String::new(),
                            error: CstError::TextNotSufficientlyIndented,
                        }
                        .into(),
                    ),
                }
                .into(),
            )),
        );
        assert_eq!(
            text("\"foo", 0),
            Some((
                "",
                CstKind::Text {
                    opening: Box::new(
                        CstKind::OpeningText {
                            opening_single_quotes: vec![],
                            opening_double_quote: Box::new(CstKind::DoubleQuote.into())
                        }
                        .into(),
                    ),
                    parts: vec![CstKind::TextPart("foo".to_string()).into()],
                    closing: Box::new(
                        CstKind::Error {
                            unparsable_input: String::new(),
                            error: CstError::TextNotClosed,
                        }
                        .into(),
                    ),
                }
                .into(),
            )),
        );
        assert_eq!(
            text("''\"foo\"'bar\"'' baz", 0),
            Some((
                " baz",
                CstKind::Text {
                    opening: Box::new(
                        CstKind::OpeningText {
                            opening_single_quotes: vec![
                                CstKind::SingleQuote.into(),
                                CstKind::SingleQuote.into(),
                            ],
                            opening_double_quote: Box::new(CstKind::DoubleQuote.into()),
                        }
                        .into(),
                    ),
                    parts: vec![CstKind::TextPart("foo\"'bar".to_string()).into()],
                    closing: Box::new(
                        CstKind::ClosingText {
                            closing_double_quote: Box::new(CstKind::DoubleQuote.into()),
                            closing_single_quotes: vec![
                                CstKind::SingleQuote.into(),
                                CstKind::SingleQuote.into(),
                            ],
                        }
                        .into(),
                    ),
                }
                .into(),
            )),
        );
        assert_eq!(
            text("\"foo {\"bar\"} baz\"", 0),
            Some((
                "",
                CstKind::Text {
                    opening: Box::new(
                        CstKind::OpeningText {
                            opening_single_quotes: vec![],
                            opening_double_quote: Box::new(CstKind::DoubleQuote.into()),
                        }
                        .into(),
                    ),
                    parts: vec![
                        CstKind::TextPart("foo ".to_string()).into(),
                        CstKind::TextInterpolation {
                            opening_curly_braces: vec![CstKind::OpeningCurlyBrace.into()],
                            expression: Box::new(build_simple_text("bar")),
                            closing_curly_braces: vec![CstKind::ClosingCurlyBrace.into()],
                        }
                        .into(),
                        CstKind::TextPart(" baz".to_string()).into(),
                    ],
                    closing: Box::new(
                        CstKind::ClosingText {
                            closing_double_quote: Box::new(CstKind::DoubleQuote.into()),
                            closing_single_quotes: vec![],
                        }
                        .into(),
                    ),
                }
                .into(),
            )),
        );
        assert_eq!(
            text("'\"foo {\"bar\"} baz\"'", 0),
            Some((
                "",
                CstKind::Text {
                    opening: Box::new(
                        CstKind::OpeningText {
                            opening_single_quotes: vec![CstKind::SingleQuote.into()],
                            opening_double_quote: Box::new(CstKind::DoubleQuote.into()),
                        }
                        .into(),
                    ),
                    parts: vec![CstKind::TextPart("foo {\"bar\"} baz".to_string()).into()],
                    closing: Box::new(
                        CstKind::ClosingText {
                            closing_double_quote: Box::new(CstKind::DoubleQuote.into()),
                            closing_single_quotes: vec![CstKind::SingleQuote.into()],
                        }
                        .into(),
                    ),
                }
                .into(),
            )),
        );
        assert_eq!(
            text("\"foo {  \"bar\" } baz\"", 0),
            Some((
                "",
                CstKind::Text {
                    opening: Box::new(
                        CstKind::OpeningText {
                            opening_single_quotes: vec![],
                            opening_double_quote: Box::new(CstKind::DoubleQuote.into())
                        }
                        .into(),
                    ),
                    parts: vec![
                        CstKind::TextPart("foo ".to_string()).into(),
                        CstKind::TextInterpolation {
                            opening_curly_braces: vec![CstKind::OpeningCurlyBrace
                                .with_trailing_whitespace(vec![CstKind::Whitespace(
                                    "  ".to_string(),
                                )])],
                            expression: Box::new(build_simple_text("bar").with_trailing_space()),
                            closing_curly_braces: vec![CstKind::ClosingCurlyBrace.into()],
                        }
                        .into(),
                        CstKind::TextPart(" baz".to_string()).into(),
                    ],
                    closing: Box::new(
                        CstKind::ClosingText {
                            closing_double_quote: Box::new(CstKind::DoubleQuote.into()),
                            closing_single_quotes: vec![],
                        }
                        .into(),
                    ),
                }
                .into(),
            )),
        );
        assert_eq!(
            text(
                "\"Some text with {'\"an interpolation containing {{\"an interpolation\"}}\"'}\"",
                0,
            ),
            Some((
                "",
                CstKind::Text {
                    opening: Box::new(
                        CstKind::OpeningText {
                            opening_single_quotes: vec![],
                            opening_double_quote: Box::new(CstKind::DoubleQuote.into()),
                        }
                        .into(),
                    ),
                    parts: vec![
                        CstKind::TextPart("Some text with ".to_string()).into(),
                        CstKind::TextInterpolation {
                            opening_curly_braces: vec![CstKind::OpeningCurlyBrace.into()],
                            expression: Box::new(
                                CstKind::Text {
                                    opening:
                                        Box::new(
                                            CstKind::OpeningText {
                                                opening_single_quotes: vec![
                                                    CstKind::SingleQuote.into()
                                                ],
                                                opening_double_quote: Box::new(
                                                    CstKind::DoubleQuote.into()
                                                ),
                                            }
                                            .into(),
                                        ),
                                    parts: vec![
                                        CstKind::TextPart(
                                            "an interpolation containing ".to_string(),
                                        )
                                        .into(),
                                        CstKind::TextInterpolation {
                                            opening_curly_braces: vec![
                                                CstKind::OpeningCurlyBrace.into(),
                                                CstKind::OpeningCurlyBrace.into(),
                                            ],
                                            expression: Box::new(build_simple_text(
                                                "an interpolation"
                                            )),
                                            closing_curly_braces: vec![
                                                CstKind::ClosingCurlyBrace.into(),
                                                CstKind::ClosingCurlyBrace.into(),
                                            ],
                                        }
                                        .into(),
                                    ],
                                    closing:
                                        Box::new(
                                            CstKind::ClosingText {
                                                closing_double_quote: Box::new(
                                                    CstKind::DoubleQuote.into()
                                                ),
                                                closing_single_quotes: vec![
                                                    CstKind::SingleQuote.into()
                                                ],
                                            }
                                            .into()
                                        ),
                                }
                                .into(),
                            ),
                            closing_curly_braces: vec![CstKind::ClosingCurlyBrace.into()],
                        }
                        .into(),
                    ],
                    closing: Box::new(
                        CstKind::ClosingText {
                            closing_double_quote: Box::new(CstKind::DoubleQuote.into()),
                            closing_single_quotes: vec![],
                        }
                        .into(),
                    ),
                }
                .into(),
            )),
        );
        assert_eq!(
            text("\"{ {2} }\"", 0),
            Some((
                "",
                CstKind::Text {
                    opening: Box::new(
                        CstKind::OpeningText {
                            opening_single_quotes: vec![],
                            opening_double_quote: Box::new(CstKind::DoubleQuote.into()),
                        }
                        .into(),
                    ),
                    parts: vec![
                        CstKind::TextInterpolation {
                            opening_curly_braces: vec![
                                CstKind::OpeningCurlyBrace.with_trailing_space()
                            ],
                            expression: Box::new(
                                CstKind::Function {
                                    opening_curly_brace: Box::new(
                                        CstKind::OpeningCurlyBrace.into()
                                    ),
                                    parameters_and_arrow: None,
                                    body: vec![build_simple_int(2)],
                                    closing_curly_brace: Box::new(
                                        CstKind::ClosingCurlyBrace.into()
                                    ),
                                }
                                .with_trailing_space(),
                            ),
                            closing_curly_braces: vec![CstKind::ClosingCurlyBrace.into()],
                        }
                        .into(),
                    ],
                    closing: Box::new(
                        CstKind::ClosingText {
                            closing_double_quote: Box::new(CstKind::DoubleQuote.into()),
                            closing_single_quotes: vec![],
                        }
                        .into(),
                    ),
                }
                .into(),
            )),
        );
        assert_eq!(
            text("\"{{2}}\"", 0),
            Some((
                "",
                CstKind::Text {
                    opening: Box::new(
                        CstKind::OpeningText {
                            opening_single_quotes: vec![],
                            opening_double_quote: Box::new(CstKind::DoubleQuote.into()),
                        }
                        .into(),
                    ),
                    parts: vec![
                        CstKind::TextPart("{".to_string()).into(),
                        CstKind::TextInterpolation {
                            opening_curly_braces: vec![CstKind::OpeningCurlyBrace.into()],
                            expression: Box::new(build_simple_int(2)),
                            closing_curly_braces: vec![CstKind::ClosingCurlyBrace.into()],
                        }
                        .into(),
                        CstKind::TextPart("}".to_string()).into(),
                    ],
                    closing: Box::new(
                        CstKind::ClosingText {
                            closing_double_quote: Box::new(CstKind::DoubleQuote.into()),
                            closing_single_quotes: vec![],
                        }
                        .into(),
                    ),
                }
                .into(),
            )),
        );
        assert_eq!(
            text("\"foo {} baz\"", 0),
            Some((
                "",
                CstKind::Text {
                    opening: Box::new(
                        CstKind::OpeningText {
                            opening_single_quotes: vec![],
                            opening_double_quote: Box::new(CstKind::DoubleQuote.into()),
                        }
                        .into(),
                    ),
                    parts: vec![
                        CstKind::TextPart("foo ".to_string()).into(),
                        CstKind::TextInterpolation {
                            opening_curly_braces: vec![CstKind::OpeningCurlyBrace.into()],
                            expression: Box::new(
                                CstKind::Error {
                                    unparsable_input: String::new(),
                                    error: CstError::TextInterpolationMissesExpression,
                                }
                                .into(),
                            ),
                            closing_curly_braces: vec![CstKind::ClosingCurlyBrace.into()],
                        }
                        .into(),
                        CstKind::TextPart(" baz".to_string()).into(),
                    ],
                    closing: Box::new(
                        CstKind::ClosingText {
                            closing_double_quote: Box::new(CstKind::DoubleQuote.into()),
                            closing_single_quotes: vec![],
                        }
                        .into(),
                    ),
                }
                .into(),
            )),
        );
        assert_eq!(
            text("\"foo {\"bar\" baz\"", 0),
            Some((
                "",
                CstKind::Text {
                    opening: Box::new(
                        CstKind::OpeningText {
                            opening_single_quotes: vec![],
                            opening_double_quote: Box::new(CstKind::DoubleQuote.into()),
                        }
                        .into(),
                    ),
                    parts: vec![
                        CstKind::TextPart("foo ".to_string()).into(),
                        CstKind::TextInterpolation {
                            opening_curly_braces: vec![CstKind::OpeningCurlyBrace.into()],
                            expression: Box::new(
                                CstKind::Call {
                                    receiver: Box::new(
                                        build_simple_text("bar").with_trailing_space(),
                                    ),
                                    arguments: vec![
                                        build_identifier("baz"),
                                        CstKind::Text {
                                            opening: Box::new(
                                                CstKind::OpeningText {
                                                    opening_single_quotes: vec![],
                                                    opening_double_quote: Box::new(
                                                        CstKind::DoubleQuote.into()
                                                    ),
                                                }
                                                .into(),
                                            ),
                                            parts: vec![],
                                            closing: Box::new(
                                                CstKind::Error {
                                                    unparsable_input: String::new(),
                                                    error: CstError::TextNotClosed,
                                                }
                                                .into()
                                            )
                                        }
                                        .into()
                                    ],
                                }
                                .into(),
                            ),
                            closing_curly_braces: vec![CstKind::Error {
                                unparsable_input: String::new(),
                                error: CstError::TextInterpolationNotClosed,
                            }
                            .into()],
                        }
                        .into(),
                    ],
                    closing: Box::new(
                        CstKind::Error {
                            unparsable_input: String::new(),
                            error: CstError::TextNotClosed,
                        }
                        .into(),
                    ),
                }
                .into(),
            )),
        );
        assert_eq!(
            text("\"foo {\"bar\" \"a\"} baz\"", 0),
            Some((
                "",
                CstKind::Text {
                    opening: Box::new(
                        CstKind::OpeningText {
                            opening_single_quotes: vec![],
                            opening_double_quote: Box::new(CstKind::DoubleQuote.into()),
                        }
                        .into(),
                    ),
                    parts: vec![
                        CstKind::TextPart("foo ".to_string()).into(),
                        CstKind::TextInterpolation {
                            opening_curly_braces: vec![CstKind::OpeningCurlyBrace.into()],
                            expression: Box::new(
                                CstKind::Call {
                                    receiver: Box::new(
                                        build_simple_text("bar").with_trailing_space(),
                                    ),
                                    arguments: vec![build_simple_text("a")],
                                }
                                .into(),
                            ),
                            closing_curly_braces: vec![CstKind::ClosingCurlyBrace.into()],
                        }
                        .into(),
                        CstKind::TextPart(" baz".to_string()).into(),
                    ],
                    closing: Box::new(
                        CstKind::ClosingText {
                            closing_double_quote: Box::new(CstKind::DoubleQuote.into()),
                            closing_single_quotes: vec![],
                        }
                        .into(),
                    ),
                }
                .into(),
            )),
        );
    }
}
