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

pub fn parse_cst(input: &str) -> Vec<Cst> {
    let mut expressions = vec![];
    let mut outer_input = input;

    while !outer_input.is_empty() {
        let (input, mut body) = parse::body(outer_input, 0);
        expressions.append(&mut body);

        let (mut input, mut whitespace) = parse::whitespaces_and_newlines(input, 0, true);
        expressions.append(&mut whitespace);

        if let Some((new_input, unexpected_punctuation)) = parse::equals_sign(input)
            .or_else(|| parse::closing_parenthesis(input))
            .or_else(|| parse::closing_curly_brace(input))
            .or_else(|| parse::arrow(input))
        {
            input = new_input;
            expressions.push(unexpected_punctuation);
        }

        if input.len() < outer_input.len() {
            outer_input = input;
        } else {
            expressions.push(Cst::Error {
                unparsable_input: input.to_string(),
                error: CstError::UnparsedRest,
            });
            break;
        }
    }

    expressions
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum Cst {
    EqualsSign,         // =
    Comma,              // ,
    Colon,              // :
    OpeningParenthesis, // (
    ClosingParenthesis, // )
    OpeningBracket,     // [
    ClosingBracket,     // ]
    OpeningCurlyBrace,  // {
    ClosingCurlyBrace,  // }
    Arrow,              // ->
    DoubleQuote,        // "
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
    Struct {
        opening_bracket: Box<Cst>,
        fields: Vec<Cst>,
        closing_bracket: Box<Cst>,
    },
    StructField {
        key: Box<Cst>,
        colon: Box<Cst>,
        value: Box<Cst>,
        comma: Option<Box<Cst>>,
    },
    Whitespace(String),
    Newline, // TODO: Support different kinds of newlines.
    Comment(String),
    Parenthesized {
        opening_parenthesis: Box<Cst>,
        inner: Box<Cst>,
        closing_parenthesis: Box<Cst>,
    },
    Lambda {
        opening_curly_brace: Box<Cst>,
        parameters_and_arrow: Option<(Vec<Cst>, Box<Cst>)>,
        body: Vec<Cst>,
        closing_curly_brace: Box<Cst>,
    },
    Assignment {
        name: Box<Cst>,
        parameters: Vec<Cst>,
        equals_sign: Box<Cst>,
        body: Vec<Cst>,
    },
    TrailingWhitespace {
        child: Box<Cst>,
        whitespace: Vec<Cst>,
    },
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
    StructFieldMissesKey,
    StructFieldMissesColon,
    StructFieldMissesValue,
    StructNotClosed,
    WeirdWhitespace,
    WeirdWhitespaceInIndentation,
    ExpressionExpectedAfterOpeningParenthesis,
    ParenthesisNotClosed,
    TooMuchWhitespace,
    CurlyBraceNotClosed,
    UnparsedRest,
    UnexpectedPunctuation,
}

impl Display for Cst {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Cst::EqualsSign => "=".fmt(f),
            Cst::Comma => ",".fmt(f),
            Cst::Colon => ":".fmt(f),
            Cst::OpeningParenthesis => "(".fmt(f),
            Cst::ClosingParenthesis => ")".fmt(f),
            Cst::OpeningBracket => "[".fmt(f),
            Cst::ClosingBracket => "]".fmt(f),
            Cst::OpeningCurlyBrace => "{".fmt(f),
            Cst::ClosingCurlyBrace => "}".fmt(f),
            Cst::Arrow => "->".fmt(f),
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
            Cst::Call { name, arguments } => {
                name.fmt(f)?;
                for argument in arguments {
                    argument.fmt(f)?;
                }
                Ok(())
            }
            Cst::Struct {
                opening_bracket,
                fields,
                closing_bracket,
            } => {
                opening_bracket.fmt(f)?;
                for field in fields {
                    field.fmt(f)?;
                }
                closing_bracket.fmt(f)
            }
            Cst::StructField {
                key,
                colon,
                value,
                comma,
            } => {
                key.fmt(f)?;
                colon.fmt(f)?;
                value.fmt(f)?;
                if let Some(comma) = comma {
                    comma.fmt(f)?;
                }
                Ok(())
            }
            Cst::Whitespace(whitespace) => whitespace.fmt(f),
            Cst::Newline => '\n'.fmt(f),
            Cst::Comment(comment) => {
                '#'.fmt(f)?;
                comment.fmt(f)
            }
            Cst::Parenthesized {
                opening_parenthesis,
                inner,
                closing_parenthesis,
            } => {
                opening_parenthesis.fmt(f)?;
                inner.fmt(f)?;
                closing_parenthesis.fmt(f)
            }
            Cst::Lambda {
                opening_curly_brace,
                parameters_and_arrow,
                body,
                closing_curly_brace,
            } => {
                opening_curly_brace.fmt(f)?;
                if let Some((parameters, arrow)) = parameters_and_arrow {
                    for parameter in parameters {
                        parameter.fmt(f)?;
                    }
                    arrow.fmt(f)?;
                }
                for expression in body {
                    expression.fmt(f)?;
                }
                closing_curly_brace.fmt(f)
            }
            Cst::Assignment {
                name,
                parameters,
                equals_sign,
                body,
            } => {
                name.fmt(f)?;
                for parameter in parameters {
                    parameter.fmt(f)?;
                }
                equals_sign.fmt(f)?;
                for expression in body {
                    expression.fmt(f)?;
                }
                Ok(())
            }
            Cst::TrailingWhitespace { child, whitespace } => {
                child.fmt(f)?;
                for w in whitespace {
                    w.fmt(f);
                }
                Ok(())
            }
            Cst::Error {
                unparsable_input,
                error,
            } => unparsable_input.fmt(f),
        }
    }
}

trait IsMultiline {
    fn is_multiline(&self) -> bool;
}

impl IsMultiline for Cst {
    fn is_multiline(&self) -> bool {
        log::info!("Is {:?} multiline?", self);
        match self {
            Cst::EqualsSign => false,
            Cst::Comma => false,
            Cst::Colon => false,
            Cst::OpeningParenthesis => false,
            Cst::ClosingParenthesis => false,
            Cst::OpeningBracket => false,
            Cst::ClosingBracket => false,
            Cst::OpeningCurlyBrace => false,
            Cst::ClosingCurlyBrace => false,
            Cst::Arrow => false,
            Cst::DoubleQuote => false,
            Cst::Identifier(_) => false,
            Cst::Symbol(_) => false,
            Cst::Int(_) => false,
            Cst::Text {
                opening_quote,
                parts,
                closing_quote,
            } => {
                opening_quote.is_multiline() || parts.is_multiline() || closing_quote.is_multiline()
            }
            Cst::TextPart(_) => false,
            Cst::Call { name, arguments } => name.is_multiline() || arguments.is_multiline(),
            Cst::Struct {
                opening_bracket,
                fields,
                closing_bracket,
            } => {
                opening_bracket.is_multiline()
                    || fields.iter().any(|field| field.is_multiline())
                    || closing_bracket.is_multiline()
            }
            Cst::StructField {
                key,
                colon,
                value,
                comma,
            } => {
                key.is_multiline()
                    || colon.is_multiline()
                    || value.is_multiline()
                    || comma
                        .as_ref()
                        .map(|comma| comma.is_multiline())
                        .unwrap_or(false)
            }
            Cst::Whitespace(whitespace) => whitespace.is_multiline(),
            Cst::Newline => true,
            Cst::Comment(_) => false,
            Cst::Parenthesized {
                opening_parenthesis,
                inner,
                closing_parenthesis,
            } => {
                opening_parenthesis.is_multiline()
                    || inner.is_multiline()
                    || closing_parenthesis.is_multiline()
            }
            Cst::Lambda {
                opening_curly_brace,
                parameters_and_arrow,
                body,
                closing_curly_brace,
            } => {
                opening_curly_brace.is_multiline()
                    || parameters_and_arrow
                        .as_ref()
                        .map(|(parameters, arrow)| {
                            parameters.is_multiline() || arrow.is_multiline()
                        })
                        .unwrap_or(false)
                    || body.is_multiline()
                    || closing_curly_brace.is_multiline()
            }
            Cst::Assignment {
                name,
                parameters,
                equals_sign,
                body,
            } => {
                name.is_multiline()
                    || parameters.is_multiline()
                    || equals_sign.is_multiline()
                    || body.is_multiline()
            }
            Cst::TrailingWhitespace { child, whitespace } => {
                log::info!("Is child multiline?");
                let c = child.is_multiline();
                log::info!("Is whitespace multiline?");
                let w = whitespace.is_multiline();
                log::info!("Combining");
                c || w
            }
            Cst::Error {
                unparsable_input,
                error,
            } => unparsable_input.is_multiline(),
        }
    }
}

impl IsMultiline for str {
    fn is_multiline(&self) -> bool {
        self.contains('\n')
    }
}

impl IsMultiline for Vec<Cst> {
    fn is_multiline(&self) -> bool {
        self.iter().any(|cst| cst.is_multiline())
    }
}

impl<T: IsMultiline> IsMultiline for Option<T> {
    fn is_multiline(&self) -> bool {
        match self {
            Some(it) => it.is_multiline(),
            None => false,
        }
    }
}

impl<A: IsMultiline, B: IsMultiline> IsMultiline for (A, B) {
    fn is_multiline(&self) -> bool {
        self.0.is_multiline() || self.1.is_multiline()
    }
}

impl Cst {
    fn wrap_in_whitespace(mut self, mut whitespace: Vec<Cst>) -> Self {
        if !whitespace.is_empty() {
            if let Cst::TrailingWhitespace {
                child,
                whitespace: self_whitespace,
            } = &mut self
            {
                self_whitespace.append(&mut whitespace);
                self
            } else {
                Cst::TrailingWhitespace {
                    child: Box::new(self),
                    whitespace,
                }
            }
        } else {
            self
        }
    }
}

mod parse {
    // All parsers take an input and return an input that may have advanced a
    // little.
    //
    // Note: The parser is indentation-first. Indentation is more important than
    // parentheses, brackets, etc. If some part of a definition can't be parsed,
    // all the surrounding code still has a chance to be properly parsed – even
    // mid-writing after putting the opening bracket of a struct.

    use crate::compiler::string_to_cst::IsMultiline;

    use super::{Cst, CstError};
    use itertools::Itertools;

    static MEANINGFUL_PUNCTUATION: &'static str = "=:,(){}[]->";

    fn literal<'a>(input: &'a str, literal: &'static str) -> Option<&'a str> {
        log::info!("literal({:?}, {:?})", input, literal);
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

    pub fn equals_sign(input: &str) -> Option<(&str, Cst)> {
        let input = literal(input, "=")?;
        Some((input, Cst::EqualsSign))
    }
    pub fn comma(input: &str) -> Option<(&str, Cst)> {
        let input = literal(input, ",")?;
        Some((input, Cst::Comma))
    }
    pub fn colon(input: &str) -> Option<(&str, Cst)> {
        let input = literal(input, ":")?;
        Some((input, Cst::Colon))
    }
    fn opening_bracket(input: &str) -> Option<(&str, Cst)> {
        let input = literal(input, "[")?;
        Some((input, Cst::OpeningBracket))
    }
    pub fn closing_bracket(input: &str) -> Option<(&str, Cst)> {
        let input = literal(input, "]")?;
        Some((input, Cst::ClosingBracket))
    }
    fn opening_parenthesis(input: &str) -> Option<(&str, Cst)> {
        let input = literal(input, "(")?;
        Some((input, Cst::OpeningParenthesis))
    }
    pub fn closing_parenthesis(input: &str) -> Option<(&str, Cst)> {
        let input = literal(input, ")")?;
        Some((input, Cst::ClosingParenthesis))
    }
    fn opening_curly_brace(input: &str) -> Option<(&str, Cst)> {
        let input = literal(input, "{")?;
        Some((input, Cst::OpeningCurlyBrace))
    }
    pub fn closing_curly_brace(input: &str) -> Option<(&str, Cst)> {
        let input = literal(input, "}")?;
        Some((input, Cst::ClosingCurlyBrace))
    }
    pub fn arrow(input: &str) -> Option<(&str, Cst)> {
        let input = literal(input, "->")?;
        Some((input, Cst::Arrow))
    }
    fn double_quote(input: &str) -> Option<(&str, Cst)> {
        log::info!("double_quote({:?})", input);
        let input = literal(input, "\"")?;
        Some((input, Cst::DoubleQuote))
    }

    /// "Word" refers to a number of characters that are not separated by
    /// whitespace or significant punctuation. Identifiers, symbols, and ints
    /// are words. Words may be invalid because they contain non-ascii or
    /// non-alphanumeric characters – for example, the word `Magic✨` is an
    /// invalid identifier or symbol.
    fn word(mut input: &str) -> Option<(&str, String)> {
        log::info!("word({:?})", input);
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
        log::info!("identifier({:?})", input);
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
        log::info!("symbol({:?})", input);
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
        log::info!("int({:?})", input);
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
        log::info!("single_line_whitespace({:?})", input);
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
        log::info!("comment({:?})", input);
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
        log::info!("leading_indentation({:?}, {:?})", input, indentation);
        let mut chars = vec![];
        let mut has_weird_whitespace = false;
        let mut indent_in_spaces = 0;

        while indent_in_spaces < 2 * indentation {
            let c = input.chars().next()?;
            let (is_weird, indent_bonus) = match c {
                ' ' => (false, 1),
                '\t' => (true, 2),
                c if c.is_whitespace() => (true, 1),
                _ => return None,
            };
            chars.push(c);
            has_weird_whitespace |= is_weird;
            indent_in_spaces += indent_bonus;
            input = &input[c.len_utf8()..];
        }
        let whitespace = chars.into_iter().join("");
        Some(if has_weird_whitespace {
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
    /// lower or higher indentation.
    pub fn whitespaces_and_newlines(
        input: &str,
        indentation: usize,
        also_comments: bool,
    ) -> (&str, Vec<Cst>) {
        log::info!(
            "whitespaces_and_newlines({:?}, {:?}, {:?})",
            input,
            indentation,
            also_comments
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

            // We only consume newlines if there is sufficient indentation
            // coming after.
            let mut new_input = input;
            let mut new_parts = vec![];
            while let Some('\n') = new_input.chars().next() {
                new_parts.push(Cst::Newline);
                new_input = &new_input[1..];
            }
            if new_input == input {
                break; // No newlines.
            }
            log::warn!("Indentation: {} Input: {:?}", indentation, input);
            match leading_indentation(new_input, indentation) {
                Some((new_input, whitespace)) => {
                    new_parts.push(Cst::Whitespace(whitespace.to_string()));
                    parts.append(&mut new_parts);
                    input = new_input;
                }
                None => {
                    log::info!("Was None.");
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
            whitespaces_and_newlines(" \n  foo", 0, true),
            (
                "  foo",
                vec![Cst::Whitespace(" ".to_string()), Cst::Newline]
            )
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
        log::info!("text({:?}, {:?})", input, indentation);
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

    fn expression(
        input: &str,
        indentation: usize,
        allow_call_and_assignment: bool,
    ) -> Option<(&str, Cst)> {
        log::info!(
            "expression({:?}, {:?}, {:?})",
            input,
            indentation,
            allow_call_and_assignment
        );
        int(input)
            .or_else(|| text(input, indentation))
            .or_else(|| symbol(input))
            .or_else(|| struct_(input, indentation))
            .or_else(|| parenthesized(input, indentation))
            .or_else(|| lambda(input, indentation))
            .or_else(|| {
                if allow_call_and_assignment {
                    assignment(input, indentation)
                } else {
                    None
                }
            })
            .or_else(|| {
                if allow_call_and_assignment {
                    call(input, indentation)
                } else {
                    None
                }
            })
            .or_else(|| identifier(input))
            .or_else(|| {
                word(input).map(|(input, word)| {
                    (
                        input,
                        Cst::Error {
                            unparsable_input: word,
                            error: CstError::UnexpectedPunctuation,
                        },
                    )
                })
            })
    }
    #[test]
    fn test_expression() {
        assert_eq!(
            text("foo", 0),
            Some(("", Cst::Identifier("foo".to_string())))
        );
    }

    /// Multiple expressions that are occurring one after another.
    fn run_of_expressions(input: &str, indentation: usize) -> Option<(&str, Vec<Cst>)> {
        log::info!("run_of_expressions({:?}, {:?})", input, indentation);
        let mut expressions = vec![];
        let (mut input, expr) = expression(input, indentation, false)?;
        expressions.push(expr);

        let mut has_multiline_whitespace = false;
        loop {
            let (i, whitespace) = whitespaces_and_newlines(input, indentation + 1, true);
            has_multiline_whitespace |= whitespace.is_multiline();
            let indentation = if has_multiline_whitespace {
                indentation + 1
            } else {
                indentation
            };

            let (i, expr) = match expression(i, indentation, has_multiline_whitespace) {
                Some(it) => it,
                None => {
                    let fallback = closing_parenthesis(i)
                        .or_else(|| closing_bracket(i))
                        .or_else(|| closing_curly_brace(i))
                        .or_else(|| arrow(i));
                    if let Some((i, cst)) = fallback {
                        (i, cst)
                    } else {
                        break;
                    }
                }
            };

            let last = expressions.pop().unwrap();
            expressions.push(last.wrap_in_whitespace(whitespace));

            expressions.push(expr);
            input = i;
        }
        Some((input, expressions))
    }

    fn call(input: &str, indentation: usize) -> Option<(&str, Cst)> {
        log::info!("call({:?}, {:?})", input, indentation);
        let (input, mut expressions) = run_of_expressions(input, indentation)?;
        if expressions.len() < 2 {
            return None;
        }
        let arguments = expressions.split_off(1);
        let name = expressions.into_iter().next().unwrap();
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
        assert_eq!(call("print", 0), None);
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
        assert_eq!(
            call("Foo 4 bar", 0),
            Some((
                "",
                Cst::Call {
                    name: Box::new(Cst::Symbol("Foo".to_string())),
                    arguments: vec![Cst::Int(4), Cst::Identifier("bar".to_string())]
                }
            ))
        );
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

    fn struct_(input: &str, indentation: usize) -> Option<(&str, Cst)> {
        log::info!("struct({:?}, {:?})", input, indentation);

        let (mut outer_input, mut opening_bracket) = opening_bracket(input)?;

        let mut fields: Vec<Cst> = vec![];
        let mut fields_indentation = indentation;
        loop {
            let input = outer_input;

            // Whitespace before key.
            let (input, whitespace) = whitespaces_and_newlines(input, indentation + 1, true);
            if whitespace.is_multiline() {
                fields_indentation = indentation + 1;
            }
            if fields.is_empty() {
                opening_bracket = opening_bracket.wrap_in_whitespace(whitespace);
            } else {
                let last = fields.pop().unwrap();
                fields.push(last.wrap_in_whitespace(whitespace));
            }

            // The key itself.
            let (input, key, has_key) = match expression(input, fields_indentation, true) {
                Some((input, key)) => (input, key, true),
                None => (
                    input,
                    Cst::Error {
                        unparsable_input: "".to_string(),
                        error: CstError::StructFieldMissesKey,
                    },
                    false,
                ),
            };

            // Whitespace between key and colon.
            let (input, whitespace) = whitespaces_and_newlines(input, fields_indentation + 1, true);
            if whitespace.is_multiline() {
                fields_indentation = indentation + 1;
            }
            let key = key.wrap_in_whitespace(whitespace);

            // Colon.
            let (input, colon, has_colon) = match colon(input) {
                Some((input, colon)) => (input, colon, true),
                None => (
                    input,
                    Cst::Error {
                        unparsable_input: "".to_string(),
                        error: CstError::StructFieldMissesColon,
                    },
                    false,
                ),
            };

            // Whitespace between colon and value.
            let (input, whitespace) = whitespaces_and_newlines(input, fields_indentation + 1, true);
            if whitespace.is_multiline() {
                fields_indentation = indentation + 1;
            }
            let colon = colon.wrap_in_whitespace(whitespace);

            // Value.
            let (input, value, has_value) = match expression(input, fields_indentation + 1, true) {
                Some((input, value)) => (input, value, true),
                None => (
                    input,
                    Cst::Error {
                        unparsable_input: "".to_string(),
                        error: CstError::StructFieldMissesValue,
                    },
                    false,
                ),
            };

            // Whitespace between value and comma.
            let (input, whitespace) = whitespaces_and_newlines(input, fields_indentation + 1, true);
            if whitespace.is_multiline() {
                fields_indentation = indentation + 1;
            }
            let value = value.wrap_in_whitespace(whitespace);

            // Comma.
            let (input, comma) = match comma(input) {
                Some((input, comma)) => (input, Some(comma)),
                None => (input, None),
            };

            if !has_key && !has_colon && !has_value && comma.is_none() {
                break;
            }

            outer_input = input;
            fields.push(Cst::StructField {
                key: Box::new(key),
                colon: Box::new(colon),
                value: Box::new(value),
                comma: comma.map(|it| Box::new(it)),
            });
        }
        let input = outer_input;

        let (new_input, whitespace) = whitespaces_and_newlines(input, indentation, true);

        let (input, closing_bracket) = match closing_bracket(new_input) {
            Some((input, closing_bracket)) => {
                if fields.is_empty() {
                    opening_bracket = opening_bracket.wrap_in_whitespace(whitespace);
                } else {
                    let last = fields.pop().unwrap();
                    fields.push(last.wrap_in_whitespace(whitespace));
                }
                (input, closing_bracket)
            }
            None => (
                input,
                Cst::Error {
                    unparsable_input: "".to_string(),
                    error: CstError::StructNotClosed,
                },
            ),
        };

        Some((
            input,
            Cst::Struct {
                opening_bracket: Box::new(opening_bracket),
                fields,
                closing_bracket: Box::new(closing_bracket),
            },
        ))
    }
    #[test]
    fn test_struct() {
        assert_eq!(struct_("hello", 0), None);
        assert_eq!(
            struct_("[]", 0),
            Some((
                "",
                Cst::Struct {
                    opening_bracket: Box::new(Cst::OpeningBracket),
                    fields: vec![],
                    closing_bracket: Box::new(Cst::ClosingBracket),
                }
            ))
        );
        assert_eq!(
            struct_("[foo:bar]", 0),
            Some((
                "",
                Cst::Struct {
                    opening_bracket: Box::new(Cst::OpeningBracket),
                    fields: vec![Cst::StructField {
                        key: Box::new(Cst::Identifier("foo".to_string())),
                        colon: Box::new(Cst::Colon),
                        value: Box::new(Cst::Identifier("bar".to_string())),
                        comma: None,
                    },],
                    closing_bracket: Box::new(Cst::ClosingBracket),
                }
            ))
        );
        // [
        //   foo: bar,
        //   4: "Hi",
        // ]
        assert_eq!(
            struct_("[\n  foo: bar,\n  4: \"Hi\",\n]", 0),
            Some((
                "",
                Cst::Struct {
                    opening_bracket: Box::new(Cst::TrailingWhitespace {
                        child: Box::new(Cst::OpeningBracket),
                        whitespace: vec![Cst::Newline, Cst::Whitespace("  ".to_string())],
                    }),
                    fields: vec![
                        Cst::TrailingWhitespace {
                            child: Box::new(Cst::StructField {
                                key: Box::new(Cst::Identifier("foo".to_string())),
                                colon: Box::new(Cst::TrailingWhitespace {
                                    child: Box::new(Cst::Colon),
                                    whitespace: vec![Cst::Whitespace(" ".to_string())],
                                }),
                                value: Box::new(Cst::Identifier("bar".to_string())),
                                comma: Some(Box::new(Cst::Comma)),
                            }),
                            whitespace: vec![Cst::Newline, Cst::Whitespace("  ".to_string())]
                        },
                        Cst::TrailingWhitespace {
                            child: Box::new(Cst::StructField {
                                key: Box::new(Cst::Int(4)),
                                colon: Box::new(Cst::TrailingWhitespace {
                                    child: Box::new(Cst::Colon),
                                    whitespace: vec![Cst::Whitespace(" ".to_string())],
                                }),
                                value: Box::new(Cst::Text {
                                    opening_quote: Box::new(Cst::DoubleQuote),
                                    parts: vec![Cst::TextPart("Hi".to_string())],
                                    closing_quote: Box::new(Cst::DoubleQuote),
                                }),
                                comma: Some(Box::new(Cst::Comma))
                            }),
                            whitespace: vec![Cst::Newline]
                        }
                    ],
                    closing_bracket: Box::new(Cst::ClosingBracket),
                }
            ))
        );
    }

    fn parenthesized(input: &str, indentation: usize) -> Option<(&str, Cst)> {
        log::info!("parenthesized({:?}, {:?})", input, indentation);

        let (input, opening_parenthesis) = opening_parenthesis(input)?;

        let (input, whitespace) = whitespaces_and_newlines(input, indentation + 1, true);
        let inner_indentation = if whitespace.is_multiline() {
            indentation + 1
        } else {
            indentation
        };
        let opening_parenthesis = opening_parenthesis.wrap_in_whitespace(whitespace);

        log::info!("Parenthesized inners indented by {}", inner_indentation);
        let (input, inner) = expression(input, inner_indentation, true).unwrap_or((
            input,
            Cst::Error {
                unparsable_input: "".to_string(),
                error: CstError::ExpressionExpectedAfterOpeningParenthesis,
            },
        ));

        let (input, whitespace) = whitespaces_and_newlines(input, indentation, true);
        let inner = inner.wrap_in_whitespace(whitespace);

        let (input, closing_parenthesis) = closing_parenthesis(input).unwrap_or((
            input,
            Cst::Error {
                unparsable_input: "".to_string(),
                error: CstError::ParenthesisNotClosed,
            },
        ));

        Some((
            input,
            Cst::Parenthesized {
                opening_parenthesis: Box::new(opening_parenthesis),
                inner: Box::new(inner),
                closing_parenthesis: Box::new(closing_parenthesis),
            },
        ))
    }
    #[test]
    fn test_parenthesized() {
        assert_eq!(
            parenthesized("(foo)", 0),
            Some((
                "",
                Cst::Parenthesized {
                    opening_parenthesis: Box::new(Cst::OpeningParenthesis),
                    inner: Box::new(Cst::Identifier("foo".to_string())),
                    closing_parenthesis: Box::new(Cst::ClosingParenthesis),
                }
            ))
        );
        assert_eq!(parenthesized("foo", 0), None);
        assert_eq!(
            parenthesized("(foo", 0),
            Some((
                "",
                Cst::Parenthesized {
                    opening_parenthesis: Box::new(Cst::OpeningParenthesis),
                    inner: Box::new(Cst::Identifier("foo".to_string())),
                    closing_parenthesis: Box::new(Cst::Error {
                        unparsable_input: "".to_string(),
                        error: CstError::ParenthesisNotClosed
                    }),
                }
            ))
        );
    }

    pub fn body(mut input: &str, indentation: usize) -> (&str, Vec<Cst>) {
        log::warn!("body({:?}, {:?})", input, indentation);
        let mut expressions = vec![];
        loop {
            let mut new_expressions = vec![];
            let mut new_input = input;

            let (new_new_input, mut whitespace) =
                whitespaces_and_newlines(new_input, indentation, true);
            new_expressions.append(&mut whitespace);
            new_input = new_new_input;

            let (mut new_input, unexpected_whitespace) = single_line_whitespace(new_input);
            let mut indentation = indentation;
            if let Cst::Whitespace(whitespace) = &unexpected_whitespace {
                if !whitespace.is_empty() {
                    indentation += whitespace.len() / 2; // TODO
                    new_expressions.push(Cst::Error {
                        unparsable_input: whitespace.to_string(),
                        error: CstError::TooMuchWhitespace,
                    });
                }
            } else {
                new_expressions.push(unexpected_whitespace);
            }

            match expression(new_input, indentation, true) {
                Some((new_new_input, expression)) => {
                    new_input = new_new_input;
                    new_expressions.push(expression);
                }
                None => {
                    let fallback = colon(new_input)
                        .or_else(|| comma(new_input))
                        .or_else(|| closing_parenthesis(new_input))
                        .or_else(|| closing_bracket(new_input))
                        .or_else(|| closing_curly_brace(new_input))
                        .or_else(|| arrow(new_input));
                    if let Some((i, cst)) = fallback {
                        new_input = i;
                        new_expressions.push(cst);
                    } else {
                        break (input, expressions);
                    }
                }
            }
            input = new_input;
            expressions.append(&mut new_expressions);
        }
    }

    fn lambda(input: &str, indentation: usize) -> Option<(&str, Cst)> {
        log::info!("lambda({:?}, {:?})", input, indentation);
        let (input, mut opening_curly_brace) = opening_curly_brace(input)?;
        let (mut input, mut parameters_and_arrow) = {
            let input_without_params = input;
            let mut input = input;
            let mut parameters = vec![];
            loop {
                let (i, whitespace) = whitespaces_and_newlines(input, indentation + 1, true);
                if parameters.is_empty() {
                    opening_curly_brace = opening_curly_brace.wrap_in_whitespace(whitespace);
                }

                input = i;
                match expression(input, indentation + 1, false) {
                    Some((i, parameter)) => {
                        input = i;
                        parameters.push(parameter);
                    }
                    None => break,
                };
            }
            match arrow(input) {
                Some((input, arrow)) => (input, Some((parameters, arrow))),
                None => (input_without_params, None),
            }
        };

        let (i, whitespace) = whitespaces_and_newlines(input, indentation + 1, true);
        if let Some((parameters, arrow)) = parameters_and_arrow {
            parameters_and_arrow = Some((parameters, arrow.wrap_in_whitespace(whitespace)));
        } else {
            opening_curly_brace = opening_curly_brace.wrap_in_whitespace(whitespace);
        }

        let (i, mut body) = body(i, indentation + 1);
        if !body.is_empty() {
            input = i;
        }

        let (i, whitespace) = whitespaces_and_newlines(i, indentation, true);
        if !body.is_empty() {
            let last = body.pop().unwrap();
            body.push(last.wrap_in_whitespace(whitespace));
        } else if let Some((parameters, arrow)) = parameters_and_arrow {
            parameters_and_arrow = Some((parameters, arrow.wrap_in_whitespace(whitespace)));
        } else {
            opening_curly_brace = opening_curly_brace.wrap_in_whitespace(whitespace);
        }

        let closing_curly_brace = match closing_curly_brace(i) {
            Some((i, closing_curly_brace)) => {
                input = i;
                closing_curly_brace
            }
            None => Cst::Error {
                unparsable_input: "".to_string(),
                error: CstError::CurlyBraceNotClosed,
            },
        };

        Some((
            input,
            Cst::Lambda {
                opening_curly_brace: Box::new(opening_curly_brace),
                parameters_and_arrow: parameters_and_arrow
                    .map(|(parameters, arrow)| (parameters, Box::new(arrow))),
                body,
                closing_curly_brace: Box::new(closing_curly_brace),
            },
        ))
    }
    #[test]
    fn test_lambda() {
        assert_eq!(lambda("2", 0), None);
        assert_eq!(
            lambda("{ 2 }", 0),
            Some((
                "",
                Cst::Lambda {
                    opening_curly_brace: Box::new(Cst::OpeningCurlyBrace),
                    parameters_and_arrow: None,
                    body: vec![Cst::Int(2)],
                    closing_curly_brace: Box::new(Cst::ClosingCurlyBrace),
                }
            ))
        );
        // { a ->
        //   foo
        // }
        assert_eq!(
            lambda("{ a ->\n  foo\n}", 0),
            Some((
                "",
                Cst::Lambda {
                    opening_curly_brace: Box::new(Cst::OpeningCurlyBrace),
                    parameters_and_arrow: Some((
                        vec![Cst::Identifier("a".to_string())],
                        Box::new(Cst::Arrow)
                    )),
                    body: vec![Cst::Identifier("foo".to_string())],
                    closing_curly_brace: Box::new(Cst::ClosingCurlyBrace),
                }
            ))
        );
        // {
        // foo
        assert_eq!(
            lambda("{\nfoo", 0),
            Some((
                "\nfoo",
                Cst::Lambda {
                    opening_curly_brace: Box::new(Cst::OpeningCurlyBrace),
                    parameters_and_arrow: None,
                    body: vec![],
                    closing_curly_brace: Box::new(Cst::Error {
                        unparsable_input: "".to_string(),
                        error: CstError::CurlyBraceNotClosed
                    }),
                }
            ))
        );
        // {->
        // }
        assert_eq!(
            lambda("{->\n}", 1),
            Some((
                "\n}",
                Cst::Lambda {
                    opening_curly_brace: Box::new(Cst::OpeningCurlyBrace),
                    parameters_and_arrow: Some((vec![], Box::new(Cst::Arrow))),
                    body: vec![],
                    closing_curly_brace: Box::new(Cst::Error {
                        unparsable_input: "".to_string(),
                        error: CstError::CurlyBraceNotClosed
                    }),
                }
            ))
        );
    }

    fn assignment(input: &str, indentation: usize) -> Option<(&str, Cst)> {
        log::info!("assignment({:?}, {:?})", input, indentation);
        let (input, mut signature) = run_of_expressions(input, indentation)?;
        log::info!("Signature is parsed.");
        if signature.is_empty() {
            return None;
        }

        log::info!("Removing whitespace before = on {:?}.", input);
        let (input, whitespace) = whitespaces_and_newlines(input, indentation + 1, true);
        let last = signature.pop().unwrap();
        signature.push(last.wrap_in_whitespace(whitespace.clone()));

        let parameters = signature.split_off(1);
        let name = signature.into_iter().next().unwrap();

        log::info!("Trying to parse equals on {:?}.", input);
        let (input, mut equals_sign) = equals_sign(input)?;
        let input_after_equals_sign = input;

        let (input, more_whitespace) = whitespaces_and_newlines(input, indentation, true);
        equals_sign = equals_sign.wrap_in_whitespace(more_whitespace.clone());

        log::info!("Checking if it's multiline");
        let is_multiline = name.is_multiline()
            || parameters.is_multiline()
            || whitespace.is_multiline()
            || more_whitespace.is_multiline();
        log::info!("Is it multiline? {} Now, parsing body", is_multiline);
        let (input, body) = if is_multiline {
            let (input, whitespace) = leading_indentation(input, 1)?;
            equals_sign = equals_sign.wrap_in_whitespace(vec![whitespace]);

            let (input, body) = body(input, indentation + 1);
            if body.is_empty() {
                (input_after_equals_sign, body)
            } else {
                (input, body)
            }
        } else {
            match expression(input, indentation, true) {
                Some((input, expression)) => (input, vec![expression]),
                None => (input_after_equals_sign, vec![]),
            }
        };

        Some((
            input,
            Cst::Assignment {
                name: Box::new(name),
                parameters,
                equals_sign: Box::new(equals_sign),
                body,
            },
        ))
    }
    #[test]
    fn test_assignment() {
        assert_eq!(
            assignment("foo = 42", 0),
            Some((
                "",
                Cst::Assignment {
                    name: Box::new(Cst::Identifier("foo".to_string())),
                    parameters: vec![],
                    equals_sign: Box::new(Cst::EqualsSign),
                    body: vec![Cst::Int(42)],
                }
            ))
        );
        assert_eq!(assignment("foo 42", 0), None);
        // foo bar =
        //   3
        // 2
        assert_eq!(
            assignment("foo bar =\n  3\n2", 0),
            Some((
                "\n2",
                Cst::Assignment {
                    name: Box::new(Cst::Identifier("foo".to_string())),
                    parameters: vec![Cst::Identifier("bar".to_string())],
                    equals_sign: Box::new(Cst::EqualsSign),
                    body: vec![Cst::Int(3)],
                }
            ))
        );
        // foo
        //   bar
        //   = 3
        assert_eq!(
            assignment("foo bar\n  = 3", 0),
            Some((
                "",
                Cst::Assignment {
                    name: Box::new(Cst::Identifier("foo".to_string())),
                    parameters: vec![Cst::Identifier("bar".to_string())],
                    equals_sign: Box::new(Cst::EqualsSign),
                    body: vec![Cst::Int(3)],
                }
            ))
        );
    }
}
