use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while_m_n},
    character::complete::{alphanumeric0, alphanumeric1, anychar, newline, space0, space1},
    combinator::{map, map_res, opt, verify},
    error::{make_error, ErrorKind},
    multi::many0,
    sequence::{delimited, tuple},
    IResult,
};
use proptest::prelude::*;

use super::cst::*;

pub trait StringToCst {
    fn parse_cst(&self) -> Result<Vec<Cst>, String>;
}
impl StringToCst for str {
    fn parse_cst(&self) -> Result<Vec<Cst>, String> {
        match csts(self, 0) {
            Ok((rest, parsed)) => {
                if rest.is_empty() {
                    Ok(parsed)
                } else {
                    Err(format!(
                        "Didn't parse everything. This is still left: {}",
                        rest
                    ))
                }
            }
            Err(err) => Err(format!("An error occurred while parsing: {}", err)),
        }
    }
}

fn csts(input: &str, indentation: usize) -> IResult<&str, Vec<Cst>> {
    let (input, csts) = many0(tuple((
        |input| cst(input, indentation),
        trailing_whitespace_and_comment,
        opt(tuple((tag("\n"), |input| indented(input, indentation)))),
    )))(input)?;
    Ok((input, csts.into_iter().map(|it| it.0).collect()))
}

fn cst(input: &str, indentation: usize) -> IResult<&str, Cst> {
    alt((
        map(int, |it| Cst::Int(it)),
        map(string, |it| Cst::String(it)),
        map(symbol, |it| Cst::Symbol(it)),
        map(|input| call(input, indentation), |it| Cst::Call(it)),
        map(
            |input| assignment(input, indentation),
            |it| Cst::Assignment(it),
        ),
    ))(input)
}

fn int(input: &str) -> IResult<&str, Int> {
    map_res(take_while_m_n(1, 64, |c: char| c.is_digit(10)), parse_int)(input)
        .map(|(input, int)| (input, Int(int)))
}
fn parse_int(input: &str) -> Result<u64, String> {
    u64::from_str_radix(input, 10).map_err(|_| "Couldn't parse int.".into())
}

fn string(input: &str) -> IResult<&str, String> {
    delimited(tag("\""), take_while(|it| it != '\"'), tag("\""))(input)
        .map(|(input, string)| (input, string.to_owned()))
}

fn symbol(input: &str) -> IResult<&str, Symbol> {
    tuple((verify(anychar, |it| it.is_uppercase()), alphanumeric0))(input)
        .map(|(input, (a, b))| (input, Symbol(format!("{}{}", a, b))))
}

fn call(input: &str, indentation: usize) -> IResult<&str, Call> {
    let (input, name) = identifier(input)?;
    let (input, _) = trailing_whitespace_and_comment(input)?;

    if input.chars().nth(0) == Some('\n') {
        csts(&input[1..], indentation + 1)
            .map(|(input, arguments)| (input, Call { name, arguments }))
    } else {
        many0(map(
            tuple((
                |input| cst(input, indentation),
                trailing_whitespace_and_comment,
            )),
            |(it, _)| it,
        ))(input)
        .map(|(input, arguments)| (input, Call { name, arguments }))
    }
}

fn assignment(input: &str, indentation: usize) -> IResult<&str, Assignment> {
    let (input, left) = call(input, indentation)?;
    let (input, _) = tuple((
        trailing_whitespace_and_comment,
        tag("="),
        trailing_whitespace_and_comment,
    ))(input)?;

    let name = left.name;
    let parameters = left.arguments;
    let parameters = parameters
        .into_iter()
        .map(|parameter| {
            if let Cst::Call(Call {
                name,
                arguments: inner_parameters,
            }) = parameter
            {
                if inner_parameters.is_empty() {
                    return Ok(name);
                }
            }

            Err(nom::Err::Failure((input, ErrorKind::IsNot)))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let (input, body) = if input.chars().nth(0) == Some('\n') {
        csts(input, indentation + 1)?
    } else {
        let (input, body) = cst(input, indentation)?;
        (input, vec![body])
    };
    Ok((
        input,
        Assignment {
            name,
            parameters,
            body,
        },
    ))
}

fn identifier(input: &str) -> IResult<&str, String> {
    tuple((verify(anychar, |it| it.is_lowercase()), alphanumeric0))(input)
        .map(|(input, (a, b))| (input, format!("{}{}", a, b)))
}

fn trailing_whitespace_and_comment(input: &str) -> IResult<&str, ()> {
    tuple((space0, opt(tuple((tag("#"), take_while(|it| it != '\n'))))))(input)
        .map(|(input, _)| (input, ()))
}

fn indented(input: &str, indentation: usize) -> IResult<&str, ()> {
    let string = "  ".repeat(indentation);
    let (input, _) = tag(string.as_str())(input)?;
    Ok((input, ()))
}

proptest! {
    #[test]
    fn test_int(value in 0u64..) {
        let string = value.to_string();
        prop_assert_eq!(int(&string).unwrap(), ("", Int(value)));
    }
    #[test]
    fn test_string(value in "[\\w\\d\\s]*") {
        let stringified_string = format!("\"{}\"", value);
        prop_assert_eq!(string(&stringified_string).unwrap(), ("", value));
    }
    #[test]
    fn test_symbol(value in "[A-Z][A-Za-z0-9]*") {
        prop_assert_eq!(symbol(&value).unwrap(), ("", Symbol(value.clone())));
    }
    #[test]
    fn test_identifier(value in "[a-z][A-Za-z0-9]*") {
        prop_assert_eq!(identifier(&value).unwrap(), ("", value.clone()));
    }
}
