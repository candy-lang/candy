use std::{
    fmt::{self, Display, Formatter},
    sync::Arc,
};

// use crate::input::{Input, InputDb};

use super::error::CompilerError;
// use proptest::prelude::*;

// type ParserResult<'a, T> = IResult<&'a str, T, ErrorTree<&'a str>>;

// #[salsa::query_group(StringToCstStorage)]
// pub trait StringToCst: InputDb {
//     fn cst(&self, input: Input) -> Option<Arc<Vec<Cst>>>;
//     fn cst_raw(&self, input: Input) -> Option<(Arc<Vec<Cst>>, Vec<CompilerError>)>;
// }

// fn cst(db: &dyn StringToCst, input: Input) -> Option<Arc<Vec<Cst>>> {
//     todo!()
//     // db.cst_raw(input).map(|(cst, _)| cst)
// }

// fn cst_raw(db: &dyn StringToCst, input: Input) -> Option<(Arc<Vec<Cst>>, Vec<CompilerError>)> {
//     todo!()
//     //     let raw_source = db.get_input(input)?;

//     //     // TODO: handle trailing whitespace and comments properly
//     //     let source = format!("\n{}", raw_source);
//     //     let parser = map(
//     //         tuple((|input| expressions0(&source, input, 0), many0(line_ending))),
//     //         |(csts, _)| csts,
//     //     );
//     //     let result: Result<_, ErrorTree<&str>> = final_parser(parser)(&source);
//     //     Some(match result {
//     //         Ok(mut csts) => {
//     //             // TODO: remove the leading newline we inserted above
//     //             fix_offsets_csts(&mut 0, &mut csts);
//     //             let errors = extract_errors_csts(&csts);
//     //             (Arc::new(csts), errors)
//     //         }
//     //         Err(err) => (
//     //             Arc::new(vec![]),
//     //             vec![CompilerError {
//     //                 span: 0..raw_source.len(),
//     //                 message: format!("An error occurred while parsing: {:?}", err),
//     //             }],
//     //         ),
//     //     })
// }

// fn parse_cst(input: &str) -> Cst {}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum Cst {
    // EqualsSign,
    // OpeningParenthesis,
    // ClosingParenthesis,
    // OpeningCurlyBrace,
    // ClosingCurlyBrace,
    // Arrow,
    DoubleQuote,
    Identifier(String),
    Symbol(String),
    Int(u64),
    Text {
        opening_quote: Box<Cst>,
        parts: Vec<Cst>,
        closing_quote: Box<Cst>,
    },
    TextPart(String),
    Call {
        name: Box<Cst>,
        arguments: Vec<Cst>,
    },
    Whitespace(String),
    Newline, // TODO: Support different kinds of newlines.
    Comment(String),

    // Decorators.
    // LeadingWhitespace {
    //     value: String,
    //     child: Box<Cst>,
    // },
    // LeadingComment {
    //     value: String, // without #
    //     child: Box<Cst>,
    // },
    // TrailingWhitespace {
    //     child: Box<Cst>,
    //     value: String,
    // },
    // TrailingComment {
    //     child: Box<Cst>,
    //     value: String, // without #
    // },

    // Compound expressions.
    // Parenthesized {
    //     opening_parenthesis: Box<Cst>,
    //     inner: Box<Cst>,
    //     closing_parenthesis: Box<Cst>,
    // },
    // Lambda {
    //     opening_curly_brace: Box<Cst>,
    //     parameters_and_arrow: Option<(Vec<Cst>, Box<Cst>)>,
    //     body: Vec<Cst>,
    //     closing_curly_brace: Box<Cst>,
    // },
    // Call {
    //     name: Box<Cst>,
    //     arguments: Vec<Cst>,
    // },
    // Assignment {
    //     name: Box<Cst>,
    //     parameters: Vec<Cst>,
    //     equals_sign: Box<Cst>,
    //     body: Vec<Cst>,
    // },
    /// Indicates a parsing of some subtree did not succeed.
    Error {
        unparsable_input: String,
        error: CstError,
    },
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
enum CstError {
    IdentifierContainsNonAlphanumericAscii,
    SymbolContainsNonAlphanumericAscii,
    IntContainsNonDigits,
    TextDoesNotEndUntilInputEnds,
    TextNotSufficientlyIndented,
    WeirdWhitespace,
    WeirdWhitespaceInIndentation,
}

impl Display for Cst {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            // Cst::EqualsSign => "=".fmt(f),
            // Cst::OpeningParenthesis => "(".fmt(f),
            // Cst::ClosingParenthesis => ")".fmt(f),
            // Cst::OpeningCurlyBrace => "{".fmt(f),
            // Cst::ClosingCurlyBrace => "}".fmt(f),
            // Cst::Arrow => "=>".fmt(f),
            Cst::DoubleQuote => '"'.fmt(f),
            Cst::Identifier(identifier) => identifier.fmt(f),
            Cst::Symbol(symbol) => symbol.fmt(f),
            Cst::Int(int) => int.fmt(f),
            Cst::Text {
                opening_quote,
                parts,
                closing_quote,
            } => {
                opening_quote.fmt(f)?;
                for part in parts {
                    part.fmt(f)?;
                }
                closing_quote.fmt(f)
            }
            Cst::TextPart(literal) => literal.fmt(f),
            Cst::Call { name, arguments } => todo!(),
            Cst::Whitespace(whitespace) => whitespace.fmt(f),
            Cst::Newline => '\n'.fmt(f),
            Cst::Comment(comment) => {
                '#'.fmt(f)?;
                comment.fmt(f)
            }
            Cst::Error {
                unparsable_input,
                error,
            } => unparsable_input.fmt(f),
        }
    }
}

mod parse {
    // All parsers take an input and return an input that may have advanced a
    // little.
    //
    // Terminology:
    //
    // - Word: A number of characters that are not separated by whitespace or
    //   significant punctuation. Identifiers, symbols, and ints are words.
    //   Words may be invalid because they contain non-ascii or non-alphanumeric
    //   characters – for example, the word `Magic✨` is invalid.

    use super::{Cst, CstError};
    use itertools::Itertools;

    static MEANINGFUL_PUNCTUATION: &'static str = "=>(){}[],:";

    fn literal<'a>(input: &'a str, literal: &'static str) -> Option<&'a str> {
        println!("literal({:?}, {:?})", input, literal);
        if input.starts_with(literal) {
            Some(&input[literal.len()..])
        } else {
            None
        }
    }
    #[test]
    fn test_literal() {
        assert_eq!(literal("hello, world", "hello"), Some(", world"));
        assert_eq!(literal("hello, world", "hi"), None);
    }

    // fn equals_sign(input: &str) -> Option<(&str, Cst)> {
    //     let input = literal(input, "=")?;
    //     Some((input, Cst::EqualsSign))
    // }
    // fn opening_parenthesis(input: &str) -> Option<(&str, Cst)> {
    //     let input = literal(input, "(")?;
    //     Some((input, Cst::OpeningParenthesis))
    // }
    // fn closing_parenthesis(input: &str) -> Option<(&str, Cst)> {
    //     let input = literal(input, ")")?;
    //     Some((input, Cst::ClosingParenthesis))
    // }
    // fn opening_curly_brace(input: &str) -> Option<(&str, Cst)> {
    //     let input = literal(input, "{")?;
    //     Some((input, Cst::OpeningCurlyBrace))
    // }
    // fn closing_curly_brace(input: &str) -> Option<(&str, Cst)> {
    //     let input = literal(input, "}")?;
    //     Some((input, Cst::ClosingCurlyBrace))
    // }
    // fn arrow(input: &str) -> Option<(&str, Cst)> {
    //     let input = literal(input, "=>")?;
    //     Some((input, Cst::Arrow))
    // }
    fn double_quote(input: &str) -> Option<(&str, Cst)> {
        println!("double_quote({:?})", input);
        let input = literal(input, "\"")?;
        Some((input, Cst::DoubleQuote))
    }

    fn word(mut input: &str) -> Option<(&str, String)> {
        println!("word({:?})", input);
        let mut chars = vec![];
        while let Some(c) = input.chars().next() {
            if c.is_whitespace() || MEANINGFUL_PUNCTUATION.contains(c) {
                break;
            }
            chars.push(c);
            input = &input[c.len_utf8()..];
        }
        if chars.is_empty() {
            None
        } else {
            Some((input, chars.into_iter().join("")))
        }
    }
    #[test]
    fn test_word() {
        assert_eq!(word("hello, world"), Some((", world", "hello".to_string())));
        assert_eq!(
            word("I💖Candy blub"),
            Some((" blub", "I💖Candy".to_string()))
        );
        assert_eq!(word("012🔥hi"), Some(("", "012🔥hi".to_string())));
        assert_eq!(word("foo(blub)"), Some(("(blub)", "foo".to_string())));
    }

    fn identifier(input: &str) -> Option<(&str, Cst)> {
        println!("identifier({:?})", input);
        let (input, w) = word(input)?;
        if w.chars().next().unwrap().is_lowercase() {
            if w.chars().all(|c| c.is_ascii_alphanumeric()) {
                Some((input, Cst::Identifier(w)))
            } else {
                Some((
                    input,
                    Cst::Error {
                        unparsable_input: w,
                        error: CstError::IdentifierContainsNonAlphanumericAscii,
                    },
                ))
            }
        } else {
            None
        }
    }
    #[test]
    fn test_identifier() {
        assert_eq!(
            identifier("foo bar"),
            Some((" bar", Cst::Identifier("foo".to_string())))
        );
        assert_eq!(identifier("Foo bar"), None);
        assert_eq!(identifier("012 bar"), None);
        assert_eq!(
            identifier("f12🔥 bar"),
            Some((
                " bar",
                Cst::Error {
                    unparsable_input: "f12🔥".to_string(),
                    error: CstError::IdentifierContainsNonAlphanumericAscii,
                }
            ))
        );
    }

    fn symbol(input: &str) -> Option<(&str, Cst)> {
        println!("symbol({:?})", input);
        let (input, w) = word(input)?;
        if w.chars().next().unwrap().is_uppercase() {
            if w.chars().all(|c| c.is_ascii_alphanumeric()) {
                Some((input, Cst::Symbol(w)))
            } else {
                Some((
                    input,
                    Cst::Error {
                        unparsable_input: w,
                        error: CstError::SymbolContainsNonAlphanumericAscii,
                    },
                ))
            }
        } else {
            None
        }
    }
    #[test]
    fn test_symbol() {
        assert_eq!(
            symbol("Foo b"),
            Some((" b", Cst::Symbol("Foo".to_string())))
        );
        assert_eq!(symbol("foo bar"), None);
        assert_eq!(symbol("012 bar"), None);
        assert_eq!(
            symbol("F12🔥 bar"),
            Some((
                " bar",
                Cst::Error {
                    unparsable_input: "F12🔥".to_string(),
                    error: CstError::SymbolContainsNonAlphanumericAscii,
                }
            ))
        );
    }

    fn int(input: &str) -> Option<(&str, Cst)> {
        println!("int({:?})", input);
        let (input, w) = word(input)?;
        if w.chars().next().unwrap().is_ascii_digit() {
            if w.chars().all(|c| c.is_ascii_digit()) {
                let value = u64::from_str_radix(&w, 10).expect("Couldn't parse int.");
                Some((input, Cst::Int(value)))
            } else {
                Some((
                    input,
                    Cst::Error {
                        unparsable_input: w,
                        error: CstError::IntContainsNonDigits,
                    },
                ))
            }
        } else {
            None
        }
    }
    #[test]
    fn test_int() {
        assert_eq!(int("42 "), Some((" ", Cst::Int(42))));
        assert_eq!(int("123 years"), Some((" years", Cst::Int(123))));
        assert_eq!(int("foo"), None);
        assert_eq!(
            int("3D"),
            Some((
                "",
                Cst::Error {
                    unparsable_input: "3D".to_string(),
                    error: CstError::IntContainsNonDigits,
                }
            ))
        );
    }

    fn single_line_whitespace(mut input: &str) -> (&str, Cst) {
        println!("single_line_whitespace({:?})", input);
        let mut chars = vec![];
        let mut has_error = false;
        while let Some(c) = input.chars().next() {
            match c {
                ' ' => {
                    chars.push(' ');
                    input = &input[1..];
                }
                c if c.is_whitespace() && c != '\n' => {
                    chars.push(c);
                    has_error = true;
                    input = &input[c.len_utf8()..];
                }
                c => break,
            }
        }
        let whitespace = chars.into_iter().join("");
        if has_error {
            (
                input,
                Cst::Error {
                    unparsable_input: whitespace,
                    error: CstError::WeirdWhitespace,
                },
            )
        } else {
            (input, Cst::Whitespace(whitespace))
        }
    }

    fn comment(input: &str) -> Option<(&str, Cst)> {
        println!("comment({:?})", input);
        if !matches!(input.chars().next(), Some('#')) {
            return None;
        }
        let mut input = &input[1..];

        let mut comment = vec![];
        loop {
            match input.chars().next() {
                Some('\n') | None => {
                    break;
                }
                Some(c) => {
                    comment.push(c);
                    input = &input[c.len_utf8()..];
                }
            }
        }
        Some((input, Cst::Comment(comment.into_iter().join(""))))
    }

    fn leading_indentation(mut input: &str, indentation: usize) -> Option<(&str, Cst)> {
        println!("leading_indentation({:?}, {:?})", input, indentation);
        let mut chars = vec![];
        let mut has_error = false;
        for i in 0..(2 * indentation) {
            match input.chars().next()? {
                ' ' => {
                    chars.push(' ');
                    input = &input[1..];
                }
                c if c.is_whitespace() && c != '\n' => {
                    chars.push(c);
                    has_error = true;
                    input = &input[c.len_utf8()..];
                }
                _ => return None,
            }
        }
        let whitespace = chars.into_iter().join("");
        Some(if has_error {
            (
                input,
                Cst::Error {
                    unparsable_input: whitespace,
                    error: CstError::WeirdWhitespaceInIndentation,
                },
            )
        } else {
            (input, Cst::Whitespace(whitespace))
        })
    }
    #[test]
    fn test_leading_indentation() {
        assert_eq!(
            leading_indentation("foo", 0),
            Some(("foo", Cst::Whitespace("".to_string())))
        );
        assert_eq!(
            leading_indentation("  foo", 1),
            Some(("foo", Cst::Whitespace("  ".to_string())))
        );
        assert_eq!(leading_indentation("  foo", 2), None);
    }

    /// Consumes all leading whitespace (including newlines) and comments that
    /// are still within the given indentation. Won't consume newlines before a
    /// lower indentation.
    fn whitespaces_and_newlines(
        input: &str,
        indentation: usize,
        also_comments: bool,
    ) -> (&str, Vec<Cst>) {
        println!(
            "whitespaces_and_newlines({:?}, {:?}, {:?})",
            input, indentation, also_comments
        );
        let mut parts = vec![];
        let (input, whitespace) = single_line_whitespace(input);
        parts.push(whitespace);

        let mut input = input;
        loop {
            if also_comments {
                if let Some((i, whitespace)) = comment(input) {
                    input = i;
                    parts.push(whitespace);
                }
            }

            // We only consume newlines if there is indentation coming after.
            let mut new_input = input;
            let mut new_parts = vec![];
            while let Some('\n') = new_input.chars().next() {
                new_parts.push(Cst::Newline);
                new_input = &new_input[1..];
            }
            if new_input == input {
                break; // No newlines.
            }
            match leading_indentation(new_input, indentation) {
                Some((new_input, whitespace)) => {
                    new_parts.push(Cst::Whitespace(whitespace.to_string()));
                    parts.append(&mut new_parts);
                    input = new_input;
                }
                None => {
                    println!("Was None.");
                    break;
                }
            }
        }
        let parts = parts
            .into_iter()
            .filter(|it| {
                if let Cst::Whitespace(ws) = it {
                    !ws.is_empty()
                } else {
                    true
                }
            })
            .collect();
        (input, parts)
    }
    #[test]
    fn test_whitespaces_and_newlines() {
        assert_eq!(whitespaces_and_newlines("foo", 0, true), ("foo", vec![]));
        assert_eq!(
            whitespaces_and_newlines("\nfoo", 0, true),
            ("foo", vec![Cst::Newline])
        );
        assert_eq!(
            whitespaces_and_newlines("\n  foo", 1, true),
            ("foo", vec![Cst::Newline, Cst::Whitespace("  ".to_string())])
        );
        assert_eq!(
            whitespaces_and_newlines("\n  foo", 0, true),
            ("  foo", vec![Cst::Newline])
        );
        assert_eq!(
            whitespaces_and_newlines("\n  foo", 2, true),
            ("\n  foo", vec![])
        );
        assert_eq!(
            whitespaces_and_newlines("\tfoo", 1, true),
            (
                "foo",
                vec![Cst::Error {
                    unparsable_input: "\t".to_string(),
                    error: CstError::WeirdWhitespace
                }]
            )
        );
        assert_eq!(
            whitespaces_and_newlines("# hey\n  foo", 1, true),
            (
                "foo",
                vec![
                    Cst::Comment(" hey".to_string()),
                    Cst::Newline,
                    Cst::Whitespace("  ".to_string()),
                ],
            )
        );
    }

    fn text(input: &str, indentation: usize) -> Option<(&str, Cst)> {
        println!("text({:?}, {:?})", input, indentation);
        let (mut input, opening_quote) = double_quote(input)?;
        let mut line = vec![];
        let mut parts = vec![];
        let closing_quote = loop {
            match input.chars().next() {
                Some('"') => {
                    input = &input[1..];
                    parts.push(Cst::TextPart(line.drain(..).join("")));
                    break Cst::DoubleQuote;
                }
                None => {
                    parts.push(Cst::TextPart(line.drain(..).join("")));
                    break Cst::Error {
                        unparsable_input: "".to_string(),
                        error: CstError::TextDoesNotEndUntilInputEnds,
                    };
                }
                Some('\n') => {
                    parts.push(Cst::TextPart(line.drain(..).join("")));
                    let (i, mut whitespace) =
                        whitespaces_and_newlines(input, indentation + 1, false);
                    input = i;
                    parts.append(&mut whitespace);
                    if let Some('\n') = input.chars().next() {
                        break Cst::Error {
                            unparsable_input: "".to_string(),
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
            Cst::Text {
                opening_quote: Box::new(opening_quote),
                parts,
                closing_quote: Box::new(closing_quote),
            },
        ))
    }
    #[test]
    fn test_text() {
        assert_eq!(text("foo", 0), None);
        assert_eq!(
            text("\"foo\" bar", 0),
            Some((
                " bar",
                Cst::Text {
                    opening_quote: Box::new(Cst::DoubleQuote),
                    parts: vec![Cst::TextPart("foo".to_string())],
                    closing_quote: Box::new(Cst::DoubleQuote)
                }
            ))
        );
        // "foo
        //   bar"2
        assert_eq!(
            text("\"foo\n  bar\"2", 0),
            Some((
                "2",
                Cst::Text {
                    opening_quote: Box::new(Cst::DoubleQuote),
                    parts: vec![
                        Cst::TextPart("foo".to_string()),
                        Cst::Newline,
                        Cst::Whitespace("  ".to_string()),
                        Cst::TextPart("bar".to_string())
                    ],
                    closing_quote: Box::new(Cst::DoubleQuote),
                }
            ))
        );
        //   "foo
        //   bar"
        assert_eq!(
            text("\"foo\n  bar\"2", 1),
            Some((
                "\n  bar\"2",
                Cst::Text {
                    opening_quote: Box::new(Cst::DoubleQuote),
                    parts: vec![Cst::TextPart("foo".to_string()),],
                    closing_quote: Box::new(Cst::Error {
                        unparsable_input: "".to_string(),
                        error: CstError::TextNotSufficientlyIndented,
                    }),
                }
            ))
        );
        assert_eq!(
            text("\"foo", 0),
            Some((
                "",
                Cst::Text {
                    opening_quote: Box::new(Cst::DoubleQuote),
                    parts: vec![Cst::TextPart("foo".to_string()),],
                    closing_quote: Box::new(Cst::Error {
                        unparsable_input: "".to_string(),
                        error: CstError::TextDoesNotEndUntilInputEnds,
                    }),
                }
            ))
        );
    }

    fn expression(input: &str, indentation: usize) -> Option<(&str, Cst)> {
        println!("expression({:?}, {:?})", input, indentation);
        int(input).or_else(|| identifier(input))
        // let expression = int(input)
        //     .or_else(|| text(input, indentation))
        //     .or_else(|| symbol(input))
        //     // .or_else(|| parenthesized(input))
        //     // .or_else(|| lambda(input))
        //     // .or_else(|| assignment(input))
        //     .or_else(|| call(input, indentation))
        //     .or_else(|| identifier(input));
        // if let Some(result) = expression {
        //     return Some(result);
        // }
        // // TODO: Implement fallback
        // None
    }
    #[test]
    fn test_expression() {
        assert_eq!(
            text("foo", 0),
            Some(("", Cst::Identifier("foo".to_string())))
        );
    }

    fn call(input: &str, indentation: usize) -> Option<(&str, Cst)> {
        println!("call({:?}, {:?})", input, indentation);
        let (mut input, name) = expression(input, indentation)?;
        let mut arguments = vec![];
        loop {
            let (i, whitespace) = whitespaces_and_newlines(input, indentation + 1, true);
            let (i, argument) = match expression(i, indentation + 1) {
                Some(it) => it,
                None => break,
            };
            arguments.push(argument);
            input = i;
        }
        Some((
            input,
            Cst::Call {
                name: Box::new(name),
                arguments,
            },
        ))
    }
    #[test]
    fn test_call() {
        assert_eq!(
            call("foo bar", 0),
            Some((
                "",
                Cst::Call {
                    name: Box::new(Cst::Identifier("foo".to_string())),
                    arguments: vec![Cst::Identifier("bar".to_string())]
                }
            ))
        );
        // assert_eq!(
        //     call("Foo 4 bar", 0),
        //     Some(Cst::Call {
        //         name: Box::new(Cst::Symbol("Foo".to_string())),
        //         arguments: vec![Cst::Int(4), Cst::Identifier("bar".to_string())]
        //     })
        // );
        // foo
        //   bar
        //   baz
        // 2
        assert_eq!(
            call("foo\n  bar\n  baz\n2", 0),
            Some((
                "\n2",
                Cst::Call {
                    name: Box::new(Cst::Identifier("foo".to_string())),
                    arguments: vec![
                        Cst::Identifier("bar".to_string()),
                        Cst::Identifier("baz".to_string())
                    ],
                },
            ))
        );
        // foo 1 2
        //   3
        //   4
        // bar
        assert_eq!(
            call("foo 1 2\n  3\n  4\nbar", 0),
            Some((
                "\nbar",
                Cst::Call {
                    name: Box::new(Cst::Identifier("foo".to_string())),
                    arguments: vec![Cst::Int(1), Cst::Int(2), Cst::Int(3), Cst::Int(4)],
                }
            ))
        );
    }

    // fn csts(input: &str, indentation: usize) -> Cst {}
    // fn cst(input: &str, indentation: usize) -> Cst {}
}

// fn expressions1<'a>(
//     source: &'a str,
//     input: &'a str,
//     indentation: usize,
// ) -> ParserResult<'a, Vec<Cst>> {
//     verify(
//         |input| expressions0(source, input, indentation),
//         |csts: &Vec<Cst>| !csts.is_empty(),
//     )
//     .context("expressions1")
//     .parse(input)
// }
// fn expressions0<'a>(
//     source: &'a str,
//     input: &'a str,
//     indentation: usize,
// ) -> ParserResult<'a, Vec<Cst>> {
//     many0(|input| {
//         leading_whitespace_and_comment_and_empty_lines(
//             source,
//             input,
//             indentation,
//             1,
//             |source, input, indentation| {
//                 trailing_whitespace_and_comment(input, |input| {
//                     leading_indentation(input, indentation, |input| {
//                         expression(source, input, indentation)
//                     })
//                 })
//             },
//         )
//     })
//     .context("expressions0")
//     .parse(input)
// }

// fn expression<'a>(source: &'a str, input: &'a str, indentation: usize) -> ParserResult<'a, Cst> {
//     alt((
//         |input| int(source, input),
//         |input| text(source, input),
//         |input| symbol(source, input),
//         |input| parenthesized(source, input, indentation),
//         |input| lambda(source, input, indentation),
//         |input| assignment(source, input, indentation),
//         |input| call(source, input, indentation),
//         |input| identifier(source, input),
//         // TODO: catch-all
//     ))
//     .context("expression")
//     .parse(input)
// }

// // Compound expressions.

// fn parenthesized<'a>(source: &'a str, input: &'a str, indentation: usize) -> ParserResult<'a, Cst> {
//     map(
//         tuple((
//             |input| opening_parenthesis(source, input),
//             |input| expression(source, input, indentation),
//             |input| closing_parenthesis(source, input),
//         )),
//         |(opening_parenthesis, inner, closing_parenthesis)| {
//             create_cst(CstKind::Parenthesized {
//                 opening_parenthesis: Box::new(opening_parenthesis),
//                 inner: Box::new(inner),
//                 closing_parenthesis: Box::new(closing_parenthesis),
//             })
//         },
//     )
//     .context("parenthesized")
//     .parse(input)
// }

// fn lambda<'a>(source: &'a str, input: &'a str, indentation: usize) -> ParserResult<'a, Cst> {
//     map(
//         tuple((
//             |input| {
//                 trailing_whitespace_and_comment_and_empty_lines(input, |input| {
//                     opening_curly_brace(source, input)
//                 })
//             },
//             opt(tuple((
//                 |input| parameters(source, input, indentation),
//                 map(
//                     |input| trailing_whitespace_and_comment(input, |input| arrow(source, input)),
//                     |it| Box::new(it),
//                 ),
//             ))),
//             alt((
//                 |input| expressions1(source, input, indentation + 1),
//                 map(
//                     |input| {
//                         trailing_whitespace_and_comment_and_empty_lines(input, |input| {
//                             expression(source, input, indentation)
//                         })
//                     },
//                     |cst| vec![cst],
//                 ),
//                 success(vec![]),
//             )),
//             |input| {
//                 leading_whitespace_and_comment_and_empty_lines(
//                     source,
//                     input,
//                     indentation,
//                     0,
//                     |source, input, _indentation| {
//                         trailing_whitespace_and_comment(input, |input| {
//                             closing_curly_brace(source, input)
//                         })
//                     },
//                 )
//             },
//         )),
//         |(opening_curly_brace, parameters_and_arrow, body, closing_curly_brace)| {
//             create_cst(CstKind::Lambda {
//                 opening_curly_brace: Box::new(opening_curly_brace),
//                 parameters_and_arrow,
//                 body,
//                 closing_curly_brace: Box::new(closing_curly_brace),
//             })
//         },
//     )
//     .context("lambda")
//     .parse(input)
// }
// fn parameters<'a>(
//     source: &'a str,
//     input: &'a str,
//     indentation: usize,
// ) -> ParserResult<'a, Vec<Cst>> {
//     many0(|input| {
//         trailing_whitespace_and_comment_and_empty_lines(
//             input,
//             alt((
//                 |input| int(source, input),
//                 |input| text(source, input),
//                 |input| symbol(source, input),
//                 |input| parenthesized(source, input, indentation),
//                 // TODO: only allow single-line lambdas
//                 |input| lambda(source, input, indentation),
//                 |input| identifier(source, input),
//                 // TODO: catch-all
//             )),
//         )
//     })
//     .context("arguments")
//     .parse(input)
// }

// fn arguments<'a>(
//     source: &'a str,
//     input: &'a str,
//     indentation: usize,
// ) -> ParserResult<'a, Vec<Cst>> {
//     many1(|input| {
//         trailing_whitespace_and_comment(
//             input,
//             alt((
//                 |input| int(source, input),
//                 |input| text(source, input),
//                 |input| symbol(source, input),
//                 |input| parenthesized(source, input, indentation),
//                 // TODO: only allow single-line lambdas
//                 |input| lambda(source, input, indentation),
//                 |input| identifier(source, input),
//                 // TODO: catch-all
//             )),
//         )
//     })
//     .context("arguments")
//     .parse(input)
// }
// fn assignment<'a>(source: &'a str, input: &'a str, indentation: usize) -> ParserResult<'a, Cst> {
//     (|input| {
//         let (input, name, parameters) = match trailing_whitespace_and_comment(input, |input| {
//             call(source, input, indentation)
//         }) {
//             Ok((
//                 input,
//                 Cst {
//                     kind: CstKind::Call { name, arguments },
//                     ..
//                 },
//             )) => (input, name, arguments),
//             Ok(_) => panic!("`call` did not return a `CstKind::Call`."),
//             Err(_) => {
//                 let (input, name) =
//                     trailing_whitespace_and_comment(input, |input| identifier(source, input))?;
//                 (input, Box::new(name), vec![])
//             }
//         };
//         let (input, equals_sign) =
//             trailing_whitespace_and_comment(input, |input| equals_sign(source, input))?;

//         let (input, body) = alt((
//             |input| expressions1(source, input, indentation + 1),
//             map(
//                 |input| expression(source, input, indentation),
//                 |cst| vec![cst],
//             ),
//             success(vec![]),
//         ))(input)?;
//         Ok((
//             input,
//             create_cst(CstKind::Assignment {
//                 name,
//                 parameters,
//                 equals_sign: Box::new(equals_sign),
//                 body,
//             }),
//         ))
//     })
//     .context("assignment")
//     .parse(input)
// }

// proptest! {
//     #[test]
//     fn test_int(value in 0u64..) {
//         let string = value.to_string();
//         prop_assert_eq!(int(&string, &string).unwrap(), ("", create_cst(CstKind::Int{offset: 0, value: value, source: string.clone()})));
//     }
//     #[test]
//     fn test_text(value in "[\\w\\d\\s]*") {
//         let stringified_text = format!("\"{}\"", value);
//         prop_assert_eq!(text(&stringified_text, &stringified_text).unwrap(), ("", create_cst(CstKind::Text{offset: 0, value: value.clone()})));
//     }
//     #[test]
//     fn test_symbol(value in "[A-Z][A-Za-z0-9]*") {
//         prop_assert_eq!(symbol(&value, &value).unwrap(), ("", create_cst(CstKind::Symbol{ offset: 0, value: value.clone()})));
//     }
//     #[test]
//     fn test_identifier(value in "[a-z][A-Za-z0-9]*") {
//         prop_assert_eq!(identifier(&value, &value).unwrap(), ("", create_cst(CstKind::Identifier{ offset: 0, value: value.clone()})));
//     }
// }

// #[test]
// fn test_indented() {
//     fn parse(source: &str, indentation: usize) -> (&str, Cst) {
//         leading_indentation(source, indentation, |input| int(source, input)).unwrap()
//     }
//     assert_eq!(
//         parse("123", 0),
//         (
//             "",
//             create_cst(CstKind::LeadingWhitespace {
//                 value: "".to_string(),
//                 child: Box::new(create_cst(CstKind::Int {
//                     offset: 0,
//                     value: 123,
//                     source: "123".to_string()
//                 })),
//             })
//         )
//     );
//     // assert_eq!(
//     //     parse("  123", 0),
//     //     (
//     //         "  ",
//     //         CstKind::LeadingWhitespace {
//     //             value: "".to_string(),
//     //             child: Box::new(CstKind::Int { value: 123 })
//     //         }
//     //     )
//     // );
//     assert_eq!(
//         parse("  123", 1),
//         (
//             "",
//             create_cst(CstKind::LeadingWhitespace {
//                 value: "  ".to_string(),
//                 child: Box::new(create_cst(CstKind::Int {
//                     offset: 2,
//                     value: 123,
//                     source: "123".to_string()
//                 }))
//             })
//         )
//     );
//     // assert_eq!(
//     //     parse("    123", 1),
//     //     (
//     //         "  ",
//     //         CstKind::LeadingWhitespace {
//     //             value: "".to_string(),
//     //             child: Box::new(CstKind::Int { value: 123 })
//     //         }
//     //     )
//     // );
// }
// #[test]
// fn test_expressions0() {
//     fn parse(source: &str) -> (&str, Vec<Cst>) {
//         expressions0(source, source, 0).unwrap()
//     }
//     assert_eq!(parse(""), ("", vec![]));
//     assert_eq!(parse("\n"), ("\n", vec![]));
//     assert_eq!(parse("\n#abc\n"), ("\n#abc\n", vec![]));
//     assert_eq!(
//         parse("\n123"),
//         (
//             "",
//             vec![create_cst(CstKind::LeadingWhitespace {
//                 value: "\n".to_string(),
//                 child: Box::new(create_cst(CstKind::Int {
//                     offset: 1,
//                     value: 123,
//                     source: "123".to_string()
//                 }))
//             })]
//         )
//     );
//     assert_eq!(
//         parse("\nprint"),
//         (
//             "",
//             vec![create_cst(CstKind::LeadingWhitespace {
//                 value: "\n".to_string(),
//                 child: Box::new(create_cst(CstKind::Call {
//                     name: Box::new(create_cst(CstKind::Identifier {
//                         offset: 1,
//                         value: "print".to_string()
//                     })),
//                     arguments: vec![]
//                 }))
//             })]
//         )
//     );
//     assert_eq!(
//         parse("\nfoo = bar\n"),
//         (
//             "\n",
//             vec![create_cst(CstKind::LeadingWhitespace {
//                 value: "\n".to_string(),
//                 child: Box::new(create_cst(CstKind::Assignment {
//                     name: Box::new(create_cst(CstKind::TrailingWhitespace {
//                         value: " ".to_string(),
//                         child: Box::new(create_cst(CstKind::Identifier {
//                             offset: 1,
//                             value: "foo".to_string()
//                         }))
//                     })),
//                     parameters: vec![],
//                     equals_sign: Box::new(create_cst(CstKind::TrailingWhitespace {
//                         value: " ".to_string(),
//                         child: Box::new(create_cst(CstKind::EqualsSign { offset: 5 }))
//                     })),
//                     body: vec![create_cst(CstKind::Call {
//                         name: Box::new(create_cst(CstKind::Identifier {
//                             offset: 7,
//                             value: "bar".to_string()
//                         })),
//                         arguments: vec![]
//                     })]
//                 }))
//             })]
//         )
//     );
//     assert_eq!(
//         parse("\nfoo\nbar"),
//         (
//             "",
//             vec![
//                 create_cst(CstKind::LeadingWhitespace {
//                     value: "\n".to_string(),
//                     child: Box::new(create_cst(CstKind::Call {
//                         name: Box::new(create_cst(CstKind::Identifier {
//                             offset: 1,
//                             value: "foo".to_string()
//                         })),
//                         arguments: vec![],
//                     }))
//                 }),
//                 create_cst(CstKind::LeadingWhitespace {
//                     value: "\n".to_string(),
//                     child: Box::new(create_cst(CstKind::Call {
//                         name: Box::new(create_cst(CstKind::Identifier {
//                             offset: 5,
//                             value: "bar".to_string()
//                         })),
//                         arguments: vec![],
//                     }))
//                 }),
//             ]
//         )
//     );
//     assert_eq!(
//         parse("\nadd 1 2"),
//         (
//             "",
//             vec![create_cst(CstKind::LeadingWhitespace {
//                 value: "\n".to_string(),
//                 child: Box::new(create_cst(CstKind::Call {
//                     name: Box::new(create_cst(CstKind::TrailingWhitespace {
//                         value: " ".to_string(),
//                         child: Box::new(create_cst(CstKind::Identifier {
//                             offset: 1,
//                             value: "add".to_string()
//                         })),
//                     })),
//                     arguments: vec![
//                         create_cst(CstKind::TrailingWhitespace {
//                             value: " ".to_string(),
//                             child: Box::new(create_cst(CstKind::Int {
//                                 offset: 5,
//                                 value: 1,
//                                 source: "1".to_string()
//                             }))
//                         }),
//                         create_cst(CstKind::Int {
//                             offset: 7,
//                             value: 2,
//                             source: "2".to_string()
//                         })
//                     ],
//                 }))
//             })]
//         )
//     );
//     assert_eq!(
//         parse("\nfoo = bar\nadd\n  1\n  2"),
//         (
//             "",
//             vec![
//                 create_cst(CstKind::LeadingWhitespace {
//                     value: "\n".to_string(),
//                     child: Box::new(create_cst(CstKind::Assignment {
//                         name: Box::new(create_cst(CstKind::TrailingWhitespace {
//                             value: " ".to_string(),
//                             child: Box::new(create_cst(CstKind::Identifier {
//                                 offset: 1,
//                                 value: "foo".to_string()
//                             })),
//                         })),
//                         parameters: vec![],
//                         equals_sign: Box::new(create_cst(CstKind::TrailingWhitespace {
//                             value: " ".to_string(),
//                             child: Box::new(create_cst(CstKind::EqualsSign { offset: 5 }))
//                         })),
//                         body: vec![create_cst(CstKind::Call {
//                             name: Box::new(create_cst(CstKind::Identifier {
//                                 offset: 7,
//                                 value: "bar".to_string()
//                             })),
//                             arguments: vec![]
//                         })]
//                     }))
//                 }),
//                 create_cst(CstKind::LeadingWhitespace {
//                     value: "\n".to_string(),
//                     child: Box::new(create_cst(CstKind::Call {
//                         name: Box::new(create_cst(CstKind::Identifier {
//                             offset: 11,
//                             value: "add".to_string()
//                         })),
//                         arguments: vec![
//                             create_cst(CstKind::LeadingWhitespace {
//                                 value: "\n".to_string(),
//                                 child: Box::new(create_cst(CstKind::LeadingWhitespace {
//                                     value: "  ".to_string(),
//                                     child: Box::new(create_cst(CstKind::Int {
//                                         offset: 17,
//                                         value: 1,
//                                         source: "1".to_string()
//                                     }))
//                                 }))
//                             }),
//                             create_cst(CstKind::LeadingWhitespace {
//                                 value: "\n".to_string(),
//                                 child: Box::new(create_cst(CstKind::LeadingWhitespace {
//                                     value: "  ".to_string(),
//                                     child: Box::new(create_cst(CstKind::Int {
//                                         offset: 21,
//                                         value: 2,
//                                         source: "2".to_string()
//                                     }))
//                                 }))
//                             }),
//                         ],
//                     }))
//                 })
//             ]
//         )
//     );
//     assert_eq!(
//         parse("\nadd\n  2\nmyIterable"),
//         (
//             "",
//             vec![
//                 create_cst(CstKind::LeadingWhitespace {
//                     value: "\n".to_string(),
//                     child: Box::new(create_cst(CstKind::Call {
//                         name: Box::new(create_cst(CstKind::Identifier {
//                             offset: 1,
//                             value: "add".to_string()
//                         })),
//                         arguments: vec![create_cst(CstKind::LeadingWhitespace {
//                             value: "\n".to_string(),
//                             child: Box::new(create_cst(CstKind::LeadingWhitespace {
//                                 value: "  ".to_string(),
//                                 child: Box::new(create_cst(CstKind::Int {
//                                     offset: 7,
//                                     value: 2,
//                                     source: "2".to_string()
//                                 }))
//                             }))
//                         })],
//                     }))
//                 }),
//                 create_cst(CstKind::LeadingWhitespace {
//                     value: "\n".to_string(),
//                     child: Box::new(create_cst(CstKind::Call {
//                         name: Box::new(create_cst(CstKind::Identifier {
//                             offset: 9,
//                             value: "myIterable".to_string()
//                         })),
//                         arguments: vec![],
//                     }))
//                 })
//             ]
//         )
//     );
// }
// #[test]
// fn test_call() {
//     fn parse(source: &str) -> (&str, Cst) {
//         call(source, source, 0).unwrap()
//     }
//     assert_eq!(
//         parse("print"),
//         (
//             "",
//             create_cst(CstKind::Call {
//                 name: Box::new(create_cst(CstKind::Identifier {
//                     offset: 0,
//                     value: "print".to_string()
//                 })),
//                 arguments: vec![]
//             })
//         )
//     );
//     assert_eq!(
//         parse("print 123 \"foo\" Bar"),
//         (
//             "",
//             create_cst(CstKind::Call {
//                 name: Box::new(create_cst(CstKind::TrailingWhitespace {
//                     value: " ".to_string(),
//                     child: Box::new(create_cst(CstKind::Identifier {
//                         offset: 0,
//                         value: "print".to_string()
//                     }))
//                 })),
//                 arguments: vec![
//                     create_cst(CstKind::TrailingWhitespace {
//                         value: " ".to_string(),
//                         child: Box::new(create_cst(CstKind::Int {
//                             offset: 6,
//                             value: 123,
//                             source: "123".to_string()
//                         }))
//                     }),
//                     create_cst(CstKind::TrailingWhitespace {
//                         value: " ".to_string(),
//                         child: Box::new(create_cst(CstKind::Text {
//                             offset: 10,
//                             value: "foo".to_string()
//                         }))
//                     }),
//                     create_cst(CstKind::Symbol {
//                         offset: 16,
//                         value: "Bar".to_string()
//                     })
//                 ]
//             })
//         )
//     );
//     assert_eq!(
//         parse("add\n  7\nmyIterable"),
//         (
//             "\nmyIterable",
//             create_cst(CstKind::Call {
//                 name: Box::new(create_cst(CstKind::Identifier {
//                     offset: 0,
//                     value: "add".to_string()
//                 })),
//                 arguments: vec![create_cst(CstKind::LeadingWhitespace {
//                     value: "\n".to_string(),
//                     child: Box::new(create_cst(CstKind::LeadingWhitespace {
//                         value: "  ".to_string(),
//                         child: Box::new(create_cst(CstKind::Int {
//                             offset: 6,
//                             value: 7,
//                             source: "7".to_string()
//                         }))
//                     }))
//                 })]
//             })
//         )
//     );
// }
// #[test]
// fn test_lambda() {
//     fn parse(source: &str) -> (&str, Cst) {
//         lambda(source, source, 0).unwrap()
//     }

//     assert_eq!(
//         parse("{ 123 }"),
//         (
//             "",
//             create_cst(CstKind::Lambda {
//                 opening_curly_brace: Box::new(create_cst(CstKind::TrailingWhitespace {
//                     value: " ".to_string(),
//                     child: Box::new(create_cst(CstKind::OpeningCurlyBrace { offset: 0 }))
//                 })),
//                 parameters_and_arrow: None,
//                 body: vec![create_cst(CstKind::TrailingWhitespace {
//                     value: " ".to_string(),
//                     child: Box::new(create_cst(CstKind::Int {
//                         offset: 2,
//                         value: 123,
//                         source: "123".to_string()
//                     }))
//                 })],
//                 closing_curly_brace: Box::new(create_cst(CstKind::ClosingCurlyBrace { offset: 6 }))
//             }),
//         )
//     );
//     assert_eq!(
//         parse("{ n -> 5 }"),
//         (
//             "",
//             create_cst(CstKind::Lambda {
//                 opening_curly_brace: Box::new(create_cst(CstKind::TrailingWhitespace {
//                     value: " ".to_string(),
//                     child: Box::new(create_cst(CstKind::OpeningCurlyBrace { offset: 0 }))
//                 })),
//                 parameters_and_arrow: Some((
//                     vec![create_cst(CstKind::Call {
//                         name: Box::new(create_cst(CstKind::TrailingWhitespace {
//                             value: " ".to_string(),
//                             child: Box::new(create_cst(CstKind::Identifier {
//                                 offset: 2,
//                                 value: "n".to_string()
//                             }))
//                         })),
//                         arguments: vec![]
//                     })],
//                     Box::new(create_cst(CstKind::TrailingWhitespace {
//                         value: " ".to_string(),
//                         child: Box::new(create_cst(CstKind::Arrow { offset: 4 }))
//                     }))
//                 )),
//                 body: vec![create_cst(CstKind::TrailingWhitespace {
//                     value: " ".to_string(),
//                     child: Box::new(create_cst(CstKind::Int {
//                         offset: 7,
//                         value: 5,
//                         source: "5".to_string()
//                     }))
//                 })],
//                 closing_curly_brace: Box::new(create_cst(CstKind::ClosingCurlyBrace { offset: 9 }))
//             }),
//         )
//     );
//     assert_eq!(
//         parse("{ a ->\n  123\n}"),
//         (
//             "",
//             create_cst(CstKind::Lambda {
//                 opening_curly_brace: Box::new(create_cst(CstKind::TrailingWhitespace {
//                     value: " ".to_string(),
//                     child: Box::new(create_cst(CstKind::OpeningCurlyBrace { offset: 0 })),
//                 })),
//                 parameters_and_arrow: Some((
//                     vec![create_cst(CstKind::Call {
//                         name: Box::new(create_cst(CstKind::TrailingWhitespace {
//                             value: " ".to_string(),
//                             child: Box::new(create_cst(CstKind::Identifier {
//                                 offset: 2,
//                                 value: "a".to_string()
//                             }))
//                         })),
//                         arguments: vec![]
//                     })],
//                     Box::new(create_cst(CstKind::Arrow { offset: 4 }))
//                 )),
//                 body: vec![create_cst(CstKind::LeadingWhitespace {
//                     value: "\n".to_string(),
//                     child: Box::new(create_cst(CstKind::LeadingWhitespace {
//                         value: "  ".to_string(),
//                         child: Box::new(create_cst(CstKind::Int {
//                             offset: 9,
//                             value: 123,
//                             source: "123".to_string()
//                         }))
//                     }))
//                 })],
//                 closing_curly_brace: Box::new(create_cst(CstKind::LeadingWhitespace {
//                     value: "\n".to_string(),
//                     child: Box::new(create_cst(CstKind::ClosingCurlyBrace { offset: 13 }))
//                 }))
//             }),
//         )
//     );
// }
// #[test]
// fn test_leading_stuff() {
//     let source = "123";
//     assert_eq!(
//         leading_whitespace(source, |input| int(source, input)).unwrap(),
//         (
//             "",
//             create_cst(CstKind::Int {
//                 offset: 0,
//                 value: 123,
//                 source: "123".to_string()
//             })
//         )
//     );

//     let source = " 123";
//     assert_eq!(
//         leading_whitespace(source, |input| int(source, input)).unwrap(),
//         (
//             "",
//             create_cst(CstKind::LeadingWhitespace {
//                 value: " ".to_string(),
//                 child: Box::new(create_cst(CstKind::Int {
//                     offset: 1,
//                     value: 123,
//                     source: "123".to_string()
//                 }))
//             }),
//         )
//     );

//     fn parse(source: &str) -> (&str, Cst) {
//         leading_whitespace_and_comment_and_empty_lines(
//             source,
//             source,
//             0,
//             1,
//             |source, input, _indentation| int(source, input),
//         )
//         .unwrap()
//     }

//     assert_eq!(
//         parse("\n123"),
//         (
//             "",
//             create_cst(CstKind::LeadingWhitespace {
//                 value: "\n".to_string(),
//                 child: Box::new(create_cst(CstKind::Int {
//                     offset: 1,
//                     value: 123,
//                     source: "123".to_string()
//                 }),)
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
