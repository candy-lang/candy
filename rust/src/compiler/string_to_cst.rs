use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while_m_n},
    character::complete::{alphanumeric0, alphanumeric1, anychar, newline, space0, space1},
    combinator::{map, map_res, opt, verify},
    error::{make_error, ErrorKind, VerboseError, VerboseErrorKind},
    multi::many0,
    sequence::{delimited, tuple},
    IResult,
};
use proptest::prelude::*;

use super::cst::*;

type ParserResult<'a, T> = IResult<&'a str, T, VerboseError<&'a str>>;

pub trait StringToCst {
    fn parse_cst(&self) -> Vec<Cst>;
}
impl StringToCst for str {
    fn parse_cst(&self) -> Vec<Cst> {
        match csts(&self, 0) {
            Ok((rest, mut parsed)) => {
                if !rest.is_empty() {
                    parsed.push(Cst::Error {
                        rest: rest.to_owned(),
                        message: "Couldn't parse everything.".to_owned(),
                    });
                }
                parsed
            }
            Err(err) => vec![Cst::Error {
                rest: self.to_owned(),
                message: format!("An error occurred while parsing: {:?}", err),
            }],
        }
    }
}

fn csts(input: &str, indentation: usize) -> ParserResult<Vec<Cst>> {
    let (input, csts) = many0(tuple((
        |input| indented(input, indentation),
        |input| cst(input, indentation),
        trailing_whitespace_and_comment,
        opt(tag("\n")),
    )))(input)?;
    Ok((input, csts.into_iter().map(|it| it.1).collect()))
}

fn cst(input: &str, indentation: usize) -> ParserResult<Cst> {
    alt((
        int,
        text,
        symbol,
        |input| assignment(input, indentation),
        |input| call(input, indentation),
        // TODO: catch-all
    ))(input)

    // Err(nom::Err::Failure(VerboseError {
    //     errors: vec![(
    //         input,
    //         VerboseErrorKind::Context("Invalid parameter.".into()),
    //     )],
    // }))
}

fn int(input: &str) -> ParserResult<Cst> {
    map_res(take_while_m_n(1, 64, |c: char| c.is_digit(10)), parse_int)(input)
        .map(|(input, int)| (input, Cst::Int(Int(int))))
}
fn parse_int(input: &str) -> Result<u64, String> {
    u64::from_str_radix(input, 10).map_err(|_| "Couldn't parse int.".into())
}

fn text(input: &str) -> ParserResult<Cst> {
    delimited(tag("\""), take_while(|it| it != '\"'), tag("\""))(input)
        .map(|(input, string)| (input, Cst::Text(string.to_owned())))
}

fn symbol(input: &str) -> ParserResult<Cst> {
    tuple((verify(anychar, |it| it.is_uppercase()), alphanumeric0))(input)
        .map(|(input, (a, b))| (input, Cst::Symbol(Symbol(format!("{}{}", a, b)))))
}

fn call(input: &str, indentation: usize) -> ParserResult<Cst> {
    let (input, name) = tuple((verify(anychar, |it| it.is_lowercase()), alphanumeric0))(input)
        .map(|(input, (a, b))| (input, format!("{}{}", a, b)))?;
    let (input, _) = trailing_whitespace_and_comment(input)?;

    // TODO: handle whitespace before line break
    println!("Remaining input: {:?}", input);
    if input.chars().nth(0) == Some('\n') {
        csts(&input[1..], indentation + 1)
            .map(|(input, arguments)| (input, Cst::Call(Call { name, arguments })))
    } else {
        many0(map(
            tuple((
                |input| cst(input, indentation),
                trailing_whitespace_and_comment,
            )),
            |(it, _)| it,
        ))(input)
        .map(|(input, arguments)| (input, Cst::Call(Call { name, arguments })))
    }
}

fn assignment(input: &str, indentation: usize) -> ParserResult<Cst> {
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

    let (input, body) = if input.chars().nth(0) == Some('\n') {
        csts(input, indentation + 1)?
    } else {
        let (input, body) = cst(input, indentation)?;
        (input, vec![body])
    };
    Ok((
        input,
        Cst::Assignment(Assignment {
            name,
            parameters,
            body,
        }),
    ))
}

fn identifier(input: &str) -> ParserResult<String> {
    tuple((verify(anychar, |it| it.is_lowercase()), alphanumeric0))(input)
        .map(|(input, (a, b))| (input, format!("{}{}", a, b)))
}

fn trailing_whitespace_and_comment(input: &str) -> ParserResult<()> {
    tuple((space0, opt(tuple((tag("#"), take_while(|it| it != '\n'))))))(input)
        .map(|(input, _)| (input, ()))
}

fn indented(input: &str, indentation: usize) -> ParserResult<()> {
    let string = "  ".repeat(indentation);
    let (input, _) = tag(string.as_str())(input)?;
    Ok((input, ()))
}

// fn catchAll(input: &str) -> ParserResult<()> {

// }

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
