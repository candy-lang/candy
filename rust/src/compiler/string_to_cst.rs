use std::fmt::Display;

use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while_m_n},
    character::complete::{alphanumeric0, anychar, line_ending, not_line_ending, space0},
    combinator::{fail, map, opt, success, verify},
    multi::{count, many0, many1},
    sequence::{delimited, tuple},
    IResult, Parser,
};
use nom_supreme::{error::ErrorTree, final_parser::final_parser, ParserExt};
use proptest::prelude::*;

use super::cst::*;

type ParserResult<'a, T> = IResult<&'a str, T, ErrorTree<&'a str>>;

pub trait StringToCst {
    fn parse_cst(&self) -> Vec<Cst>;
}
impl StringToCst for str {
    fn parse_cst(&self) -> Vec<Cst> {
        let parser = |input| csts0(input, 0);
        let input = format!("\n{}", self);
        let result: Result<_, ErrorTree<&str>> = final_parser(parser)(&input);
        match result {
            Ok(parsed) => parsed,
            Err(err) => vec![Cst::Error {
                rest: self.to_owned(),
                message: format!("An error occurred while parsing: {:?}", err),
            }],
        }
    }
}

fn csts1(input: &str, indentation: usize) -> ParserResult<Vec<Cst>> {
    verify(
        |input| csts0(input, indentation),
        |csts: &Vec<Cst>| !csts.is_empty(),
    )
    .context("csts1")
    .parse(input)
}
fn csts0(input: &str, indentation: usize) -> ParserResult<Vec<Cst>> {
    map(
        many0(map(
            tuple((
                many1(line_ending),
                tuple((
                    |input| indented(input, indentation),
                    opt(|input| cst(input, indentation)),
                )),
                trailing_whitespace_and_comment,
            )),
            |(_, cst, _)| cst.1,
        )),
        |csts| csts.into_iter().filter_map(|it| it).collect(),
    )
    .context("csts0")
    .parse(input)
}

fn cst(input: &str, indentation: usize) -> ParserResult<Cst> {
    alt((
        int,
        text,
        symbol,
        |input| lambda(input, indentation),
        |input| assignment(input, indentation),
        |input| call(input, indentation),
        // TODO: catch-all
    ))
    .context("cst")
    .parse(input)
}

fn int(input: &str) -> ParserResult<Cst> {
    map(take_while_m_n(1, 64, |c: char| c.is_digit(10)), |input| {
        let int = u64::from_str_radix(input, 10).expect("Couldn't parse int.");
        Cst::Int(Int(int))
    })
    .context("int")
    .parse(input)
}

fn text(input: &str) -> ParserResult<Cst> {
    map(
        delimited(tag("\""), take_while(|it| it != '\"'), tag("\"")),
        |string: &str| Cst::Text(string.to_owned()),
    )
    .context("text")
    .parse(input)
}

fn symbol(input: &str) -> ParserResult<Cst> {
    map(
        tuple((verify(anychar, |it| it.is_uppercase()), alphanumeric0)),
        |(a, b)| Cst::Symbol(Symbol(format!("{}{}", a, b))),
    )
    .context("symbol")
    .parse(input)
}
fn identifier(input: &str) -> ParserResult<String> {
    map(
        tuple((verify(anychar, |it| it.is_lowercase()), alphanumeric0)),
        |(a, b)| format!("{}{}", a, b),
    )
    .context("identifier")
    .parse(input)
}

fn lambda(input: &str, indentation: usize) -> ParserResult<Cst> {
    map(
        tuple((
            tag("{"),
            trailing_whitespace_and_comment,
            map(
                opt(tuple((
                    |input| arguments(input, indentation),
                    tag("->"),
                    trailing_whitespace_and_comment,
                ))),
                |it| it.map(|(parameters, _, _)| parameters).unwrap_or(vec![]),
            ),
            alt((
                |input| csts1(input, indentation + 1),
                map(|input| cst(input, indentation), |cst| vec![cst]),
            )),
            trailing_whitespace_and_comment,
            many0(line_ending),
            tag("}"),
        )),
        |(_, _, parameters, body, _, _, _)| Cst::Lambda(Lambda { parameters, body }),
    )
    .context("lambda")
    .parse(input)
}

fn call<'a>(input: &'a str, indentation: usize) -> ParserResult<Cst> {
    map(
        tuple((
            identifier,
            trailing_whitespace_and_comment,
            alt((
                |input| csts1(input, indentation + 1),
                |input| arguments(input, indentation),
            )),
        )),
        |(name, _, arguments)| Cst::Call(Call { name, arguments }),
    )
    .context("call")
    .parse(input)
}
fn call_without_arguments(input: &str) -> ParserResult<Cst> {
    map(
        tuple((identifier, trailing_whitespace_and_comment)),
        |(name, _)| {
            Cst::Call(Call {
                name,
                arguments: vec![],
            })
        },
    )
    .context("call_without_arguments")
    .parse(input)
    // let (input, name) = tuple((verify(anychar, |it| it.is_lowercase()), alphanumeric0))(input)
    //     .map(|(input, (a, b))| (input, format!("{}{}", a, b)))?;
    // let (input, _) = trailing_whitespace_and_comment(input)?;

    // Ok((
    //     input,
    //     Cst::Call(Call {
    //         name,
    //         arguments: vec![],
    //     }),
    // ))
}

fn assignment(input: &str, indentation: usize) -> ParserResult<Cst> {
    (|input| {
        let (input, left) = call(input, indentation)?;
        let (name, parameters) = match left {
            Cst::Call(Call { name, arguments }) => (name, arguments),
            _ => panic!("`call` did not return a `Cst::Call`."),
        };
        let (input, _) = tuple((
            trailing_whitespace_and_comment,
            tag("="),
            trailing_whitespace_and_comment,
        ))(input)?;

        let (input, body) = alt((
            |input| csts1(input, indentation + 1),
            map(|input| cst(input, indentation), |cst| vec![cst]),
        ))(input)?;
        Ok((
            input,
            Cst::Assignment(Assignment {
                name,
                parameters,
                body,
            }),
        ))
    })
    .context("assignment")
    .parse(input)
}

fn arguments(input: &str, indentation: usize) -> ParserResult<Vec<Cst>> {
    many0(map(
        tuple((
            alt((
                int,
                text,
                symbol,
                // TODO: only allow single-line lambdas
                |input| lambda(input, indentation),
                call_without_arguments,
                // TODO: catch-all
            )),
            trailing_whitespace_and_comment,
        )),
        |(it, _)| it,
    ))
    .context("arguments")
    .parse(input)
}

fn trailing_whitespace_and_comment(input: &str) -> ParserResult<()> {
    map(tuple((space0, opt(comment))), |_| ())
        .context("trailing_whitespace_and_comment")
        .parse(input)
}
fn comment(input: &str) -> ParserResult<()> {
    map(tuple((tag("#"), not_line_ending)), |_| ())
        .context("comment")
        .parse(input)
}

fn indented(input: &str, indentation: usize) -> ParserResult<()> {
    map(count(tag("  "), indentation), |_| ())
        .context("indented")
        .parse(input)
}

proptest! {
    #[test]
    fn test_int(value in 0u64..) {
        let string = value.to_string();
        prop_assert_eq!(int(&string).unwrap(), ("", Cst::Int(Int(value))));
    }
    #[test]
    fn test_text(value in "[\\w\\d\\s]*") {
        let stringified_text = format!("\"{}\"", value);
        prop_assert_eq!(text(&stringified_text).unwrap(), ("", Cst::Text(value)));
    }
    #[test]
    fn test_symbol(value in "[A-Z][A-Za-z0-9]*") {
        prop_assert_eq!(symbol(&value).unwrap(), ("", Cst::Symbol(Symbol(value.clone()))));
    }
    #[test]
    fn test_identifier(value in "[a-z][A-Za-z0-9]*") {
        prop_assert_eq!(identifier(&value).unwrap(), ("", value.clone()));
    }
}

#[test]
fn test_indented() {
    assert_eq!(indented("", 0).unwrap(), ("", ()));
    assert_eq!(indented("  ", 0).unwrap(), ("  ", ()));
    assert_eq!(indented("  ", 1).unwrap(), ("", ()));
    assert_eq!(indented("    ", 1).unwrap(), ("  ", ()));
}
#[test]
fn test_csts0() {
    assert_eq!(csts0("", 0).unwrap(), ("", vec![]));
    assert_eq!(csts0("\n", 0).unwrap(), ("", vec![]));
    assert_eq!(csts0("\n#abc\n", 0).unwrap(), ("", vec![]));
    assert_eq!(
        csts0("\nprint", 0).unwrap(),
        (
            "",
            vec![Cst::Call(Call {
                name: "print".to_owned(),
                arguments: vec![]
            })]
        )
    );
    assert_eq!(
        csts0("\nfoo = bar\n", 0).unwrap(),
        (
            "",
            vec![Cst::Assignment(Assignment {
                name: "foo".to_owned(),
                parameters: vec![],
                body: vec![Cst::Call(Call {
                    name: "bar".to_owned(),
                    arguments: vec![]
                })]
            })]
        )
    );
    assert_eq!(
        csts0("\nfoo\nbar", 0).unwrap(),
        (
            "",
            vec![
                Cst::Call(Call {
                    name: "foo".to_owned(),
                    arguments: vec![],
                }),
                Cst::Call(Call {
                    name: "bar".to_owned(),
                    arguments: vec![],
                })
            ]
        )
    );
    assert_eq!(
        csts0("\nadd 1 2", 0).unwrap(),
        (
            "",
            vec![Cst::Call(Call {
                name: "add".to_owned(),
                arguments: vec![Cst::Int(Int(1)), Cst::Int(Int(2))],
            })]
        )
    );
    assert_eq!(
        csts0("\nfoo = bar\nadd\n  1\n  2", 0).unwrap(),
        (
            "",
            vec![
                Cst::Assignment(Assignment {
                    name: "foo".to_owned(),
                    parameters: vec![],
                    body: vec![Cst::Call(Call {
                        name: "bar".to_owned(),
                        arguments: vec![]
                    })]
                }),
                Cst::Call(Call {
                    name: "add".to_owned(),
                    arguments: vec![Cst::Int(Int(1)), Cst::Int(Int(2))],
                })
            ]
        )
    );
    assert_eq!(
        csts0("\nadd\n  2\nmyIterable", 0).unwrap(),
        (
            "",
            vec![
                Cst::Call(Call {
                    name: "add".to_owned(),
                    arguments: vec![Cst::Int(Int(2))],
                }),
                Cst::Call(Call {
                    name: "myIterable".to_owned(),
                    arguments: vec![],
                })
            ]
        )
    );
}
#[test]
fn test_call() {
    assert_eq!(
        call("print", 0).unwrap(),
        (
            "",
            Cst::Call(Call {
                name: "print".to_owned(),
                arguments: vec![]
            })
        )
    );
    assert_eq!(
        call("print 123 \"foo\" Bar", 0).unwrap(),
        (
            "",
            Cst::Call(Call {
                name: "print".to_owned(),
                arguments: vec![
                    Cst::Int(Int(123)),
                    Cst::Text("foo".to_owned()),
                    Cst::Symbol(Symbol("Bar".to_owned()))
                ]
            })
        )
    );
    assert_eq!(
        call("add\n  7\nmyIterable", 0).unwrap(),
        (
            "myIterable",
            Cst::Call(Call {
                name: "add".to_owned(),
                arguments: vec![Cst::Int(Int(7)),]
            })
        )
    );
}
#[test]
fn test_lambda() {
    assert_eq!(
        lambda("{ 123 }", 0).unwrap(),
        (
            "",
            Cst::Lambda(Lambda {
                parameters: vec![],
                body: vec![Cst::Int(Int(123))],
            }),
        )
    );
    assert_eq!(
        lambda("{\n  123\n}", 0).unwrap(),
        (
            "",
            Cst::Lambda(Lambda {
                parameters: vec![],
                body: vec![Cst::Int(Int(123))],
            }),
        )
    );
}
#[test]
fn test_trailing_whitespace_and_comment() {
    assert_eq!(trailing_whitespace_and_comment("").unwrap(), ("", ()));
    assert_eq!(trailing_whitespace_and_comment(" \t").unwrap(), ("", ()));
    assert_eq!(
        trailing_whitespace_and_comment(" print").unwrap(),
        ("print", ())
    );
    assert_eq!(trailing_whitespace_and_comment("# abc").unwrap(), ("", ()));
    assert_eq!(
        trailing_whitespace_and_comment(" \t# abc").unwrap(),
        ("", ())
    );
}
#[test]
fn test_comment() {
    assert_eq!(comment("#").unwrap(), ("", ()));
    assert_eq!(comment("# abc").unwrap(), ("", ()));
}

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

#[derive(Debug)]
struct ParserError {
    message: String,
}
impl ParserError {
    fn new(message: String) -> Self {
        Self { message }
    }
}
impl Display for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(f)
    }
}
impl std::error::Error for ParserError {}
