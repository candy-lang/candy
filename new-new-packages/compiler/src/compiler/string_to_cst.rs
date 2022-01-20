use super::cst::*;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while_m_n},
    character::complete::{alphanumeric0, anychar, line_ending, not_line_ending, space1},
    combinator::{map, opt, recognize, success, verify},
    multi::{count, many0},
    sequence::{delimited, tuple},
    IResult, Offset, Parser,
};
use nom_supreme::{error::ErrorTree, final_parser::final_parser, ParserExt};

type ParserResult<'a, T> = IResult<&'a str, T, ErrorTree<&'a str>>;

pub trait StringToCst {
    fn parse_cst(&self) -> Vec<Cst>;
}
impl StringToCst for str {
    fn parse_cst(&self) -> Vec<Cst> {
        let parser = |input| expressions0(self, input, 0);
        let input = format!("\n{}", self);
        let result: Result<_, ErrorTree<&str>> = final_parser(parser)(&input);
        match result {
            Ok(parsed) => parsed,
            Err(err) => vec![Cst::Error {
                offset: 0,
                unparsable_input: input.clone(),
                message: format!("An error occurred while parsing: {:?}", err),
            }],
        }
    }
}

fn expressions1<'a>(
    source: &'a str,
    input: &'a str,
    indentation: usize,
) -> ParserResult<'a, Vec<Cst>> {
    verify(
        |input| expressions0(source, input, indentation),
        |csts: &Vec<Cst>| !csts.is_empty(),
    )
    .context("expressions1")
    .parse(input)
}
fn expressions0<'a>(
    source: &'a str,
    input: &'a str,
    indentation: usize,
) -> ParserResult<'a, Vec<Cst>> {
    many0(|input| {
        leading_whitespace_and_comment_and_empty_lines(
            source,
            input,
            indentation,
            1,
            |source, input, indentation| {
                trailing_whitespace_and_comment(input, |input| {
                    leading_indentation(input, indentation, |input| {
                        expression(source, input, indentation)
                    })
                })
            },
        )
    })
    .context("expressions0")
    .parse(input)
}

fn expression<'a>(source: &'a str, input: &'a str, indentation: usize) -> ParserResult<'a, Cst> {
    alt((
        |input| int(source, input),
        |input| text(source, input),
        |input| symbol(source, input),
        |input| parenthesized(source, input, indentation),
        |input| lambda(source, input, indentation),
        |input| assignment(source, input, indentation),
        |input| call(source, input, indentation),
        // TODO: catch-all
    ))
    .context("expression")
    .parse(input)
}

// Simple characters.

fn equals_sign<'a>(source: &'a str, input: &'a str) -> ParserResult<'a, Cst> {
    parse_symbol(source, input, "equals_sign", "=", |offset| {
        Cst::EqualsSign { offset }
    })
}

fn opening_parenthesis<'a>(source: &'a str, input: &'a str) -> ParserResult<'a, Cst> {
    parse_symbol(source, input, "opening_parenthesis", "(", |offset| {
        Cst::OpeningParenthesis { offset }
    })
}

fn closing_parenthesis<'a>(source: &'a str, input: &'a str) -> ParserResult<'a, Cst> {
    parse_symbol(source, input, "closing_parenthesis", ")", |offset| {
        Cst::ClosingParenthesis { offset }
    })
}

fn opening_curly_brace<'a>(source: &'a str, input: &'a str) -> ParserResult<'a, Cst> {
    parse_symbol(source, input, "opening_curly_brace", "{", |offset| {
        Cst::OpeningCurlyBrace { offset }
    })
}

fn closing_curly_brace<'a>(source: &'a str, input: &'a str) -> ParserResult<'a, Cst> {
    parse_symbol(source, input, "closing_curly_brace", "}", |offset| {
        Cst::ClosingCurlyBrace { offset }
    })
}
fn arrow<'a>(source: &'a str, input: &'a str) -> ParserResult<'a, Cst> {
    parse_symbol(source, input, "arrow", "->", |offset| Cst::Arrow { offset })
}

fn parse_symbol<'a, F>(
    source: &'a str,
    input: &'a str,
    name: &'static str,
    symbol: &'static str,
    mut mapper: F,
) -> ParserResult<'a, Cst>
where
    F: FnMut(usize) -> Cst,
{
    map(
        |input| with_offset(source, input, tag(symbol)),
        |(offset, _)| (&mut mapper(offset)).clone(),
    )
    .context(name)
    .parse(input)
}

// Self-contained atoms of the language.

fn int<'a>(source: &'a str, input: &'a str) -> ParserResult<'a, Cst> {
    map(
        |input| {
            with_offset(
                source,
                input,
                take_while_m_n(1, 64, |c: char| c.is_digit(10)),
            )
        },
        |(offset, input)| {
            let value = u64::from_str_radix(input, 10).expect("Couldn't parse int.");
            Cst::Int {
                offset,
                value,
                source: input.to_owned(),
            }
        },
    )
    .context("int")
    .parse(input)
}

fn text<'a>(source: &'a str, input: &'a str) -> ParserResult<'a, Cst> {
    map(
        |input| {
            with_offset(
                source,
                input,
                delimited(tag("\""), take_while(|it| it != '\"'), tag("\"")),
            )
        },
        |(offset, string)| Cst::Text {
            offset,
            value: string.to_owned(),
        },
    )
    .context("text")
    .parse(input)
}

fn identifier<'a>(source: &'a str, input: &'a str) -> ParserResult<'a, Cst> {
    map(
        |input| {
            with_offset(
                source,
                input,
                recognize(tuple((
                    verify(anychar, |it| it.is_lowercase()),
                    alphanumeric0,
                ))),
            )
        },
        |(offset, value)| Cst::Identifier {
            offset,
            value: value.to_owned(),
        },
    )
    .context("identifier")
    .parse(input)
}

fn symbol<'a>(source: &'a str, input: &'a str) -> ParserResult<'a, Cst> {
    map(
        |input| {
            with_offset(
                source,
                input,
                recognize(tuple((
                    verify(anychar, |it| it.is_uppercase()),
                    alphanumeric0,
                ))),
            )
        },
        |(offset, value)| Cst::Symbol {
            offset,
            value: value.to_owned(),
        },
    )
    .context("symbol")
    .parse(input)
}

// Decorators.

fn leading_indentation<'a, F>(
    input: &'a str,
    indentation: usize,
    mut parser: F,
) -> ParserResult<'a, Cst>
where
    F: FnMut(&'a str) -> ParserResult<'a, Cst>,
{
    (|input| {
        let (input, indent) = recognize(count(tag("  "), indentation))(input)?;
        let (input, child) = &mut parser(input)?;
        Ok((
            *input,
            Cst::LeadingWhitespace {
                value: indent.to_owned(),
                child: Box::new(child.clone()),
            },
        ))
    })
    .context("leading_indentation")
    .parse(input)
}

fn leading_whitespace_and_comment_and_empty_lines<'a>(
    source: &'a str,
    input: &'a str,
    indentation: usize,
    min_newlines: usize,
    mut parser: fn(&'a str, &'a str, usize) -> ParserResult<'a, Cst>,
) -> ParserResult<'a, Cst> {
    let parser: Box<dyn FnMut(&'a str) -> ParserResult<Cst>> = if min_newlines > 0 {
        Box::new(move |input| with_newlines(source, input, indentation, min_newlines, parser))
    } else {
        Box::new(
            |input| match with_newlines(source, input, indentation, min_newlines, parser) {
                Ok(it) => Ok(it),
                Err(_) => parser(source, input, indentation),
            },
        )
    };
    parser
        .context("leading_whitespace_and_comment_and_empty_lines")
        .parse(input)
}
fn with_newlines<'a>(
    source: &'a str,
    input: &'a str,
    indentation: usize,
    min_newlines: usize,
    mut parser: fn(&'a str, &'a str, usize) -> ParserResult<'a, Cst>,
) -> ParserResult<'a, Cst> {
    leading_whitespace(input, |input| {
        leading_comment(
            input,
            map(
                tuple((line_ending, |input| {
                    leading_whitespace_and_comment_and_empty_lines(
                        source,
                        input,
                        indentation,
                        min_newlines - 1,
                        parser,
                    )
                })),
                |(line_break, child)| Cst::LeadingWhitespace {
                    value: line_break.to_owned(),
                    child: Box::new(child),
                },
            ),
        )
    })
}
fn leading_whitespace<'a, F>(input: &'a str, mut parser: F) -> ParserResult<'a, Cst>
where
    F: FnMut(&'a str) -> ParserResult<'a, Cst>,
{
    (|input| {
        let space_result: IResult<_, _, ErrorTree<&'a str>> = space1(input);
        let (input, result) = match space_result {
            Ok((input, space)) => {
                let (input, child) = parser(input)?;
                (
                    input,
                    Cst::TrailingWhitespace {
                        value: space.to_owned(),
                        child: Box::new(child),
                    },
                )
            }
            Err(_) => parser(input)?,
        };
        Ok((input, result))
    })
    .context("leading_whitespace")
    .parse(input)
}
fn leading_comment<'a, F>(input: &'a str, mut parser: F) -> ParserResult<'a, Cst>
where
    F: FnMut(&'a str) -> ParserResult<'a, Cst>,
{
    (|input| {
        let (input, (_, comment)) = tuple((tag("#"), not_line_ending))(input)?;
        let (input, child) = parser(input)?;
        let result = Cst::LeadingComment {
            value: comment.to_owned(),
            child: Box::new(child),
        };
        Ok((input, result))
    })
    .context("leading_comment")
    .parse(input)
}

fn trailing_whitespace_and_comment_and_empty_lines<'a, F>(
    input: &'a str,
    mut parser: F,
) -> ParserResult<'a, Cst>
where
    F: FnMut(&'a str) -> ParserResult<'a, Cst>,
{
    (|input| {
        let (mut input, mut child) = trailing_whitespace_and_comment(input, &mut parser)?;

        loop {
            let result = trailing_whitespace_and_comment(input, |input| {
                let (input, line_break) = line_ending(input)?;

                Ok((
                    input,
                    Cst::TrailingWhitespace {
                        child: Box::new(child.clone()),
                        value: line_break.to_owned(),
                    },
                ))
            });
            match result {
                Ok((remaining_input, inner_child)) => {
                    input = remaining_input;
                    child = inner_child;
                }
                Err(_) => break,
            }
        }
        Ok((input, child))
    })
    .context("trailing_whitespace_and_comment_and_empty_lines")
    .parse(input)
}
fn trailing_whitespace_and_comment<'a, F>(input: &'a str, mut parser: F) -> ParserResult<'a, Cst>
where
    F: FnMut(&'a str) -> ParserResult<'a, Cst>,
{
    (|input| trailing_comment(input, |input| trailing_whitespace(input, &mut parser)))
        .context("trailing_whitespace_and_comment")
        .parse(input)
}
fn trailing_whitespace<'a, F>(input: &'a str, mut parser: F) -> ParserResult<'a, Cst>
where
    F: FnMut(&'a str) -> ParserResult<'a, Cst>,
{
    (|input| {
        let (input, child) = parser(input)?;

        let space_result: IResult<_, _, ErrorTree<&'a str>> = space1(input);
        let (input, result) = match space_result {
            Ok((remaining, space)) => (
                remaining,
                Cst::TrailingWhitespace {
                    child: Box::new(child),
                    value: space.to_owned(),
                },
            ),
            Err(_) => (input, child),
        };
        Ok((input, result))
    })
    .context("trailing_whitespace")
    .parse(input)
}
fn trailing_comment<'a, F>(input: &'a str, mut parser: F) -> ParserResult<'a, Cst>
where
    F: FnMut(&'a str) -> ParserResult<'a, Cst>,
{
    (|input| {
        let (input, child) = parser(input)?;

        let comment_result: IResult<_, _, ErrorTree<&'a str>> =
            tuple((tag("#"), not_line_ending))(input);
        let (input, result) = match comment_result {
            Ok((remaining, (_, comment))) => (
                remaining,
                Cst::TrailingComment {
                    child: Box::new(child),
                    value: comment.to_owned(),
                },
            ),
            Err(_) => (input, child),
        };
        Ok((input, result))
    })
    .context("trailing_comment")
    .parse(input)
}

// Compound expressions.

fn parenthesized<'a>(source: &'a str, input: &'a str, indentation: usize) -> ParserResult<'a, Cst> {
    map(
        tuple((
            |input| opening_parenthesis(source, input),
            |input| expression(source, input, indentation),
            |input| closing_parenthesis(source, input),
        )),
        |(opening_parenthesis, inner, closing_parenthesis)| Cst::Parenthesized {
            opening_parenthesis: Box::new(opening_parenthesis),
            inner: Box::new(inner),
            closing_parenthesis: Box::new(closing_parenthesis),
        },
    )
    .context("parenthesized")
    .parse(input)
}

fn lambda<'a>(source: &'a str, input: &'a str, indentation: usize) -> ParserResult<'a, Cst> {
    map(
        tuple((
            |input| {
                trailing_whitespace_and_comment_and_empty_lines(input, |input| {
                    opening_curly_brace(source, input)
                })
            },
            opt(tuple((
                |input| parameters(source, input, indentation),
                map(
                    |input| {
                        trailing_whitespace_and_comment_and_empty_lines(input, |input| {
                            arrow(source, input)
                        })
                    },
                    |it| Box::new(it),
                ),
            ))),
            alt((
                |input| expressions1(source, input, indentation + 1),
                map(
                    |input| {
                        trailing_whitespace_and_comment_and_empty_lines(input, |input| {
                            expression(source, input, indentation)
                        })
                    },
                    |cst| vec![cst],
                ),
                success(vec![]),
            )),
            |input| {
                leading_whitespace_and_comment_and_empty_lines(
                    source,
                    input,
                    indentation,
                    0,
                    |source, input, indentation| {
                        trailing_whitespace_and_comment(input, |input| {
                            closing_curly_brace(source, input)
                        })
                    },
                )
            },
        )),
        |(opening_curly_brace, parameters_and_arrow, body, closing_curly_brace)| Cst::Lambda {
            opening_curly_brace: Box::new(opening_curly_brace),
            parameters_and_arrow,
            body,
            closing_curly_brace: Box::new(closing_curly_brace),
        },
    )
    .context("lambda")
    .parse(input)
}
fn parameters<'a>(
    source: &'a str,
    input: &'a str,
    indentation: usize,
) -> ParserResult<'a, Vec<Cst>> {
    many0(|input| {
        trailing_whitespace_and_comment_and_empty_lines(
            input,
            alt((
                |input| int(source, input),
                |input| text(source, input),
                |input| symbol(source, input),
                |input| parenthesized(source, input, indentation),
                // TODO: only allow single-line lambdas
                |input| lambda(source, input, indentation),
                |input| call_without_arguments(input, source),
                // TODO: catch-all
            )),
        )
    })
    .context("arguments")
    .parse(input)
}

fn call<'a>(source: &'a str, input: &'a str, indentation: usize) -> ParserResult<'a, Cst> {
    map(
        tuple((
            |input| trailing_whitespace_and_comment(input, |input| identifier(source, input)),
            alt((
                |input| expressions1(source, input, indentation + 1),
                |input| arguments(source, input, indentation),
            )),
        )),
        |(name, arguments)| Cst::Call {
            name: Box::new(name),
            arguments,
        },
    )
    .context("call")
    .parse(input)
}
fn call_without_arguments<'a>(source: &'a str, input: &'a str) -> ParserResult<'a, Cst> {
    map(
        |input| trailing_whitespace_and_comment(input, |input| identifier(source, input)),
        |name| Cst::Call {
            name: Box::new(name),
            arguments: vec![],
        },
    )
    .context("call_without_arguments")
    .parse(input)
}
fn arguments<'a>(
    source: &'a str,
    input: &'a str,
    indentation: usize,
) -> ParserResult<'a, Vec<Cst>> {
    many0(|input| {
        trailing_whitespace_and_comment(
            input,
            alt((
                |input| int(source, input),
                |input| text(source, input),
                |input| symbol(source, input),
                |input| parenthesized(source, input, indentation),
                // TODO: only allow single-line lambdas
                |input| lambda(source, input, indentation),
                |input| call_without_arguments(input, source),
                // TODO: catch-all
            )),
        )
    })
    .context("arguments")
    .parse(input)
}
fn assignment<'a>(source: &'a str, input: &'a str, indentation: usize) -> ParserResult<'a, Cst> {
    (|input| {
        let (input, left) =
            trailing_whitespace_and_comment(input, |input| call(source, input, indentation))?;
        let (name, parameters) = match left {
            Cst::Call { name, arguments } => (name, arguments),
            _ => panic!("`call` did not return a `Cst::Call`."),
        };
        let (input, equals_sign) =
            trailing_whitespace_and_comment(input, |input| equals_sign(source, input))?;

        let (input, body) = alt((
            |input| expressions1(source, input, indentation + 1),
            map(
                |input| expression(source, input, indentation),
                |cst| vec![cst],
            ),
            success(vec![]),
        ))(input)?;
        Ok((
            input,
            Cst::Assignment {
                name,
                parameters,
                equals_sign: Box::new(equals_sign),
                body,
            },
        ))
    })
    .context("assignment")
    .parse(input)
}

pub fn with_offset<'a, O, F>(
    source: &'a str,
    input: &'a str,
    mut parser: F,
) -> ParserResult<'a, (usize, O)>
where
    F: FnMut(&'a str) -> ParserResult<O>,
{
    (move |input: &'a str| match parser.parse(input) {
        Ok((remaining, result)) => {
            let offset = source.offset(&input);
            Ok((remaining, (offset, result)))
        }
        Err(e) => Err(e),
    })
    .context("with_offset")
    .parse(input)
}

// proptest! {
//     #[test]
//     fn test_int(value in 0u64..) {
//         let string = value.to_string();
//         prop_assert_eq!(int(&string).unwrap(), ("", Cst::Int(Int(value))));
//     }
//     #[test]
//     fn test_text(value in "[\\w\\d\\s]*") {
//         let stringified_text = format!("\"{}\"", value);
//         prop_assert_eq!(text(&stringified_text).unwrap(), ("", Cst::Text(Text(value))));
//     }
//     #[test]
//     fn test_symbol(value in "[A-Z][A-Za-z0-9]*") {
//         prop_assert_eq!(symbol(&value).unwrap(), ("", Cst::Symbol(Symbol(value.clone()))));
//     }
//     #[test]
//     fn test_identifier(value in "[a-z][A-Za-z0-9]*") {
//         prop_assert_eq!(identifier(&value).unwrap(), ("", value.clone()));
//     }
// }

// #[test]
// fn test_indented() {
//     assert_eq!(indented("", 0).unwrap(), ("", ()));
//     assert_eq!(indented("  ", 0).unwrap(), ("  ", ()));
//     assert_eq!(indented("  ", 1).unwrap(), ("", ()));
//     assert_eq!(indented("    ", 1).unwrap(), ("  ", ()));
// }
// #[test]
// fn test_expressions0() {
//     assert_eq!(expressions0("", 0).unwrap(), ("", vec![]));
//     assert_eq!(expressions0("\n", 0).unwrap(), ("", vec![]));
//     assert_eq!(expressions0("\n#abc\n", 0).unwrap(), ("", vec![]));
//     assert_eq!(
//         expressions0("\nprint", 0).unwrap(),
//         (
//             "",
//             vec![Cst::Call(Call {
//                 name: "print".to_owned(),
//                 arguments: vec![]
//             })]
//         )
//     );
//     assert_eq!(
//         expressions0("\nfoo = bar\n", 0).unwrap(),
//         (
//             "",
//             vec![Cst::Assignment(Assignment {
//                 name: "foo".to_owned(),
//                 parameters: vec![],
//                 body: vec![Cst::Call(Call {
//                     name: "bar".to_owned(),
//                     arguments: vec![]
//                 })]
//             })]
//         )
//     );
//     assert_eq!(
//         expressions0("\nfoo\nbar", 0).unwrap(),
//         (
//             "",
//             vec![
//                 Cst::Call(Call {
//                     name: "foo".to_owned(),
//                     arguments: vec![],
//                 }),
//                 Cst::Call(Call {
//                     name: "bar".to_owned(),
//                     arguments: vec![],
//                 })
//             ]
//         )
//     );
//     assert_eq!(
//         expressions0("\nadd 1 2", 0).unwrap(),
//         (
//             "",
//             vec![Cst::Call(Call {
//                 name: "add".to_owned(),
//                 arguments: vec![Cst::Int(Int(1)), Cst::Int(Int(2))],
//             })]
//         )
//     );
//     assert_eq!(
//         expressions0("\nfoo = bar\nadd\n  1\n  2", 0).unwrap(),
//         (
//             "",
//             vec![
//                 Cst::Assignment(Assignment {
//                     name: "foo".to_owned(),
//                     parameters: vec![],
//                     body: vec![Cst::Call(Call {
//                         name: "bar".to_owned(),
//                         arguments: vec![]
//                     })]
//                 }),
//                 Cst::Call(Call {
//                     name: "add".to_owned(),
//                     arguments: vec![Cst::Int(Int(1)), Cst::Int(Int(2))],
//                 })
//             ]
//         )
//     );
//     assert_eq!(
//         expressions0("\nadd\n  2\nmyIterable", 0).unwrap(),
//         (
//             "",
//             vec![
//                 Cst::Call(Call {
//                     name: "add".to_owned(),
//                     arguments: vec![Cst::Int(Int(2))],
//                 }),
//                 Cst::Call(Call {
//                     name: "myIterable".to_owned(),
//                     arguments: vec![],
//                 })
//             ]
//         )
//     );
// }
// #[test]
// fn test_call() {
//     assert_eq!(
//         call("print", 0).unwrap(),
//         (
//             "",
//             Cst::Call(Call {
//                 name: "print".to_owned(),
//                 arguments: vec![]
//             })
//         )
//     );
//     assert_eq!(
//         call("print 123 \"foo\" Bar", 0).unwrap(),
//         (
//             "",
//             Cst::Call(Call {
//                 name: "print".to_owned(),
//                 arguments: vec![
//                     Cst::Int(Int(123)),
//                     Cst::Text(Text("foo".to_owned())),
//                     Cst::Symbol(Symbol("Bar".to_owned()))
//                 ]
//             })
//         )
//     );
//     assert_eq!(
//         call("add\n  7\nmyIterable", 0).unwrap(),
//         (
//             "myIterable",
//             Cst::Call(Call {
//                 name: "add".to_owned(),
//                 arguments: vec![Cst::Int(Int(7)),]
//             })
//         )
//     );
// }
// #[test]
// fn test_lambda() {
//     assert_eq!(
//         lambda("{ 123 }", 0).unwrap(),
//         (
//             "",
//             Cst::Lambda(Lambda {
//                 parameters: vec![],
//                 body: vec![Cst::Int(Int(123))],
//             }),
//         )
//     );
//     assert_eq!(
//         lambda("{\n  123\n}", 0).unwrap(),
//         (
//             "",
//             Cst::Lambda(Lambda {
//                 parameters: vec![],
//                 body: vec![Cst::Int(Int(123))],
//             }),
//         )
//     );
// }
// #[test]
// fn test_trailing_whitespace_and_comment() {
//     assert_eq!(trailing_whitespace_and_comment("").unwrap(), ("", ()));
//     assert_eq!(trailing_whitespace_and_comment(" \t").unwrap(), ("", ()));
//     assert_eq!(
//         trailing_whitespace_and_comment(" print").unwrap(),
//         ("print", ())
//     );
//     assert_eq!(trailing_whitespace_and_comment("# abc").unwrap(), ("", ()));
//     assert_eq!(
//         trailing_whitespace_and_comment(" \t# abc").unwrap(),
//         ("", ())
//     );
// }
// #[test]
// fn test_comment() {
//     assert_eq!(comment("#").unwrap(), ("", ()));
//     assert_eq!(comment("# abc").unwrap(), ("", ()));
// }

// trait ParserCandyExt<'a, O> {
//     fn map<R, F, G>(&mut self, f: F) -> Box<dyn FnMut(&'a str) -> ParserResult<'a, R>>
//     where
//         F: FnMut(O) -> R;
// }
// impl<'a, P, O> ParserCandyExt<'a, O> for P
// where
//     P: Parser<&'a str, O, ErrorTree<&'a str>>,
// {
//     fn map<R, F, G>(&mut self, f: F) -> impl FnMut(&'a str) -> ParserResult<'a, R>
//     where
//         F: FnMut(O) -> R,
//     {
//         todo!()
//     }
// }
