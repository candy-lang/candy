use super::rcst::{Rcst, RcstError};
use crate::module::{Module, ModuleDb, Package};
use std::sync::Arc;

#[salsa::query_group(StringToRcstStorage)]
pub trait StringToRcst: ModuleDb {
    fn rcst(&self, module: Module) -> Result<Arc<Vec<Rcst>>, InvalidModuleError>;
}

fn rcst(db: &dyn StringToRcst, module: Module) -> Result<Arc<Vec<Rcst>>, InvalidModuleError> {
    if let Package::Tooling(_) = &module.package {
        return Err(InvalidModuleError::IsToolingModule);
    }
    let source = db
        .get_module_content(module)
        .ok_or(InvalidModuleError::DoesNotExist)?;
    let source = match String::from_utf8((*source).clone()) {
        Ok(source) => source,
        Err(_) => {
            return Err(InvalidModuleError::InvalidUtf8);
        }
    };
    let (rest, mut rcsts) = parse::body(&source, 0);
    if !rest.is_empty() {
        rcsts.push(Rcst::Error {
            unparsable_input: rest.to_string(),
            error: RcstError::UnparsedRest,
        });
    }
    Ok(Arc::new(rcsts))
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum InvalidModuleError {
    DoesNotExist,
    InvalidUtf8,
    IsToolingModule,
}

impl Rcst {
    fn wrap_in_whitespace(mut self, mut whitespace: Vec<Rcst>) -> Self {
        if whitespace.is_empty() {
            return self;
        }

        if let Rcst::TrailingWhitespace {
            whitespace: self_whitespace,
            ..
        } = &mut self
        {
            self_whitespace.append(&mut whitespace);
            self
        } else {
            Rcst::TrailingWhitespace {
                child: Box::new(self),
                whitespace,
            }
        }
    }
}

fn whitespace_indentation_score(whitespace: &str) -> usize {
    whitespace
        .chars()
        .map(|c| match c {
            '\t' => 2,
            c if c.is_whitespace() => 1,
            _ => panic!("whitespace_indentation_score called with something non-whitespace"),
        })
        .sum()
}

mod parse {
    // All parsers take an input and return an input that may have advanced a
    // little.
    //
    // Note: The parser is indentation-first. Indentation is more important than
    // parentheses, brackets, etc. If some part of a definition can't be parsed,
    // all the surrounding code still has a chance to be properly parsed – even
    // mid-writing after putting the opening bracket of a struct.

    use super::{
        super::rcst::{IsMultiline, Rcst, RcstError, SplitOuterTrailingWhitespace},
        whitespace_indentation_score,
    };
    use itertools::Itertools;
    use tracing::instrument;

    static MEANINGFUL_PUNCTUATION: &str = r#"=,.:|()[]{}->'"%"#;
    static SUPPORTED_WHITESPACE: &str = " \r\n\t";

    #[instrument(level = "trace")]
    fn literal<'a>(input: &'a str, literal: &'static str) -> Option<&'a str> {
        input.strip_prefix(literal)
    }
    #[test]
    fn test_literal() {
        assert_eq!(literal("hello, world", "hello"), Some(", world"));
        assert_eq!(literal("hello, world", "hi"), None);
    }

    #[instrument(level = "trace")]
    fn equals_sign(input: &str) -> Option<(&str, Rcst)> {
        literal(input, "=").map(|it| (it, Rcst::EqualsSign))
    }
    #[instrument(level = "trace")]
    fn comma(input: &str) -> Option<(&str, Rcst)> {
        literal(input, ",").map(|it| (it, Rcst::Comma))
    }
    #[instrument(level = "trace")]
    fn dot(input: &str) -> Option<(&str, Rcst)> {
        literal(input, ".").map(|it| (it, Rcst::Dot))
    }
    #[instrument(level = "trace")]
    fn colon(input: &str) -> Option<(&str, Rcst)> {
        literal(input, ":").map(|it| (it, Rcst::Colon))
    }
    #[instrument(level = "trace")]
    fn colon_equals_sign(input: &str) -> Option<(&str, Rcst)> {
        literal(input, ":=").map(|it| (it, Rcst::ColonEqualsSign))
    }
    #[instrument(level = "trace")]
    fn bar(input: &str) -> Option<(&str, Rcst)> {
        literal(input, "|").map(|it| (it, Rcst::Bar))
    }
    #[instrument(level = "trace")]
    fn opening_bracket(input: &str) -> Option<(&str, Rcst)> {
        literal(input, "[").map(|it| (it, Rcst::OpeningBracket))
    }
    #[instrument(level = "trace")]
    fn closing_bracket(input: &str) -> Option<(&str, Rcst)> {
        literal(input, "]").map(|it| (it, Rcst::ClosingBracket))
    }
    #[instrument(level = "trace")]
    fn opening_parenthesis(input: &str) -> Option<(&str, Rcst)> {
        literal(input, "(").map(|it| (it, Rcst::OpeningParenthesis))
    }
    #[instrument(level = "trace")]
    fn closing_parenthesis(input: &str) -> Option<(&str, Rcst)> {
        literal(input, ")").map(|it| (it, Rcst::ClosingParenthesis))
    }
    #[instrument(level = "trace")]
    fn opening_curly_brace(input: &str) -> Option<(&str, Rcst)> {
        literal(input, "{").map(|it| (it, Rcst::OpeningCurlyBrace))
    }
    #[instrument(level = "trace")]
    fn closing_curly_brace(input: &str) -> Option<(&str, Rcst)> {
        literal(input, "}").map(|it| (it, Rcst::ClosingCurlyBrace))
    }
    #[instrument(level = "trace")]
    fn arrow(input: &str) -> Option<(&str, Rcst)> {
        literal(input, "->").map(|it| (it, Rcst::Arrow))
    }
    #[instrument(level = "trace")]
    fn single_quote(input: &str) -> Option<(&str, Rcst)> {
        literal(input, "'").map(|it| (it, Rcst::SingleQuote))
    }
    #[instrument(level = "trace")]
    fn double_quote(input: &str) -> Option<(&str, Rcst)> {
        literal(input, "\"").map(|it| (it, Rcst::DoubleQuote))
    }
    #[instrument(level = "trace")]
    fn percent(input: &str) -> Option<(&str, Rcst)> {
        literal(input, "%").map(|it| (it, Rcst::Percent))
    }
    #[instrument(level = "trace")]
    fn octothorpe(input: &str) -> Option<(&str, Rcst)> {
        literal(input, "#").map(|it| (it, Rcst::Octothorpe))
    }
    #[instrument(level = "trace")]
    fn newline(input: &str) -> Option<(&str, Rcst)> {
        let newlines = vec!["\n", "\r\n"];
        for newline in newlines {
            if let Some(input) = literal(input, newline) {
                return Some((input, Rcst::Newline(newline.to_string())));
            }
        }
        None
    }

    fn parse_multiple<F>(
        mut input: &str,
        parse_single: F,
        count: Option<(usize, bool)>,
    ) -> Option<(&str, Vec<Rcst>)>
    where
        F: Fn(&str) -> Option<(&str, Rcst)>,
    {
        let mut rcsts = vec![];
        while let Some((input_after_single, rcst)) = parse_single(input)
            && count.map_or(true, |(count, exact)| exact || rcsts.len() < count)
        {
            input = input_after_single;
            rcsts.push(rcst);
        }
        match count {
            Some((count, _)) if count != rcsts.len() => None,
            _ => Some((input, rcsts)),
        }
    }

    /// "Word" refers to a bunch of characters that are not separated by
    /// whitespace or significant punctuation. Identifiers, symbols, and ints
    /// are words. Words may be invalid because they contain non-ascii or
    /// non-alphanumeric characters – for example, the word `Magic🌵` is an
    /// invalid symbol.
    #[instrument(level = "trace")]
    fn word(mut input: &str) -> Option<(&str, String)> {
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

    #[instrument(level = "trace")]
    fn identifier(input: &str) -> Option<(&str, Rcst)> {
        let (input, w) = word(input)?;
        if w == "✨" {
            return Some((input, Rcst::Identifier(w)));
        }
        let next_character = w.chars().next().unwrap();
        if !next_character.is_lowercase() && next_character != '_' {
            return None;
        }
        if w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            Some((input, Rcst::Identifier(w)))
        } else {
            Some((
                input,
                Rcst::Error {
                    unparsable_input: w,
                    error: RcstError::IdentifierContainsNonAlphanumericAscii,
                },
            ))
        }
    }
    #[test]
    fn test_identifier() {
        assert_eq!(
            identifier("foo bar"),
            Some((" bar", Rcst::Identifier("foo".to_string())))
        );
        assert_eq!(
            identifier("_"),
            Some(("", Rcst::Identifier("_".to_string()))),
        );
        assert_eq!(
            identifier("_foo"),
            Some(("", Rcst::Identifier("_foo".to_string()))),
        );
        assert_eq!(identifier("Foo bar"), None);
        assert_eq!(identifier("012 bar"), None);
        assert_eq!(
            identifier("f12🔥 bar"),
            Some((
                " bar",
                Rcst::Error {
                    unparsable_input: "f12🔥".to_string(),
                    error: RcstError::IdentifierContainsNonAlphanumericAscii,
                }
            ))
        );
    }

    #[instrument(level = "trace")]
    fn symbol(input: &str) -> Option<(&str, Rcst)> {
        let (input, w) = word(input)?;
        if !w.chars().next().unwrap().is_uppercase() {
            return None;
        }
        if w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            Some((input, Rcst::Symbol(w)))
        } else {
            Some((
                input,
                Rcst::Error {
                    unparsable_input: w,
                    error: RcstError::SymbolContainsNonAlphanumericAscii,
                },
            ))
        }
    }
    #[test]
    fn test_symbol() {
        assert_eq!(
            symbol("Foo b"),
            Some((" b", Rcst::Symbol("Foo".to_string())))
        );
        assert_eq!(
            symbol("Foo_Bar"),
            Some(("", Rcst::Symbol("Foo_Bar".to_string())))
        );
        assert_eq!(symbol("foo bar"), None);
        assert_eq!(symbol("012 bar"), None);
        assert_eq!(
            symbol("F12🔥 bar"),
            Some((
                " bar",
                Rcst::Error {
                    unparsable_input: "F12🔥".to_string(),
                    error: RcstError::SymbolContainsNonAlphanumericAscii,
                }
            ))
        );
    }

    #[instrument(level = "trace")]
    fn int(input: &str) -> Option<(&str, Rcst)> {
        let (input, w) = word(input)?;
        if !w.chars().next().unwrap().is_ascii_digit() {
            return None;
        }
        if w.chars().all(|c| c.is_ascii_digit()) {
            let value = str::parse(&w).expect("Couldn't parse int.");
            Some((input, Rcst::Int { value, string: w }))
        } else {
            Some((
                input,
                Rcst::Error {
                    unparsable_input: w,
                    error: RcstError::IntContainsNonDigits,
                },
            ))
        }
    }
    #[test]
    fn test_int() {
        assert_eq!(
            int("42 "),
            Some((
                " ",
                Rcst::Int {
                    value: 42u8.into(),
                    string: "42".to_string()
                }
            ))
        );
        assert_eq!(
            int("012"),
            Some((
                "",
                Rcst::Int {
                    value: 12u8.into(),
                    string: "012".to_string()
                }
            ))
        );
        assert_eq!(
            int("123 years"),
            Some((
                " years",
                Rcst::Int {
                    value: 123u8.into(),
                    string: "123".to_string()
                }
            ))
        );
        assert_eq!(int("foo"), None);
        assert_eq!(
            int("3D"),
            Some((
                "",
                Rcst::Error {
                    unparsable_input: "3D".to_string(),
                    error: RcstError::IntContainsNonDigits,
                }
            ))
        );
    }

    #[instrument(level = "trace")]
    fn single_line_whitespace(mut input: &str) -> Option<(&str, Rcst)> {
        let mut chars = vec![];
        let mut has_error = false;
        while let Some(c) = input.chars().next() {
            const SPACE: char = ' ';
            match c {
                SPACE => {}
                c if SUPPORTED_WHITESPACE.contains(c) && c != '\n' && c != '\r' => {
                    has_error = true;
                }
                _ => break,
            }
            chars.push(c);
            input = &input[c.len_utf8()..];
        }
        let whitespace = chars.into_iter().join("");
        if has_error {
            Some((
                input,
                Rcst::Error {
                    unparsable_input: whitespace,
                    error: RcstError::WeirdWhitespace,
                },
            ))
        } else if !whitespace.is_empty() {
            Some((input, Rcst::Whitespace(whitespace)))
        } else {
            None
        }
    }
    #[test]
    fn test_single_line_whitespace() {
        assert_eq!(
            single_line_whitespace("  \nfoo"),
            Some(("\nfoo", Rcst::Whitespace("  ".to_string())))
        );
    }

    #[instrument(level = "trace")]
    fn comment(input: &str) -> Option<(&str, Rcst)> {
        let (mut input, octothorpe) = octothorpe(input)?;
        let mut comment = vec![];
        loop {
            match input.chars().next() {
                Some('\n') | Some('\r') | None => {
                    break;
                }
                Some(c) => {
                    comment.push(c);
                    input = &input[c.len_utf8()..];
                }
            }
        }
        Some((
            input,
            Rcst::Comment {
                octothorpe: Box::new(octothorpe),
                comment: comment.into_iter().join(""),
            },
        ))
    }

    #[instrument(level = "trace")]
    fn leading_indentation(mut input: &str, indentation: usize) -> Option<(&str, Rcst)> {
        let mut chars = vec![];
        let mut has_weird_whitespace = false;
        let mut indentation_score = 0;

        while indentation_score < 2 * indentation {
            let c = input.chars().next()?;
            let is_weird = match c {
                ' ' => false,
                '\n' | '\r' => return None,
                c if c.is_whitespace() => true,
                _ => return None,
            };
            chars.push(c);
            has_weird_whitespace |= is_weird;
            indentation_score += whitespace_indentation_score(&format!("{c}"));
            input = &input[c.len_utf8()..];
        }
        let whitespace = chars.into_iter().join("");
        Some((
            input,
            if has_weird_whitespace {
                Rcst::Error {
                    unparsable_input: whitespace,
                    error: RcstError::WeirdWhitespaceInIndentation,
                }
            } else {
                Rcst::Whitespace(whitespace)
            },
        ))
    }
    #[test]
    fn test_leading_indentation() {
        assert_eq!(
            leading_indentation("foo", 0),
            Some(("foo", Rcst::Whitespace("".to_string())))
        );
        assert_eq!(
            leading_indentation("  foo", 1),
            Some(("foo", Rcst::Whitespace("  ".to_string())))
        );
        assert_eq!(leading_indentation("  foo", 2), None);
    }

    /// Consumes all leading whitespace (including newlines) and optionally
    /// comments that are still within the given indentation. Won't consume a
    /// newline followed by less-indented whitespace followed by non-whitespace
    /// stuff like an expression.
    #[instrument(level = "trace")]
    fn whitespaces_and_newlines(
        mut input: &str,
        indentation: usize,
        also_comments: bool,
    ) -> (&str, Vec<Rcst>) {
        let mut parts = vec![];

        if let Some((new_input, whitespace)) = single_line_whitespace(input) {
            input = new_input;
            parts.push(whitespace);
        }

        let mut new_input = input;
        let mut new_parts = vec![];
        loop {
            let new_input_from_iteration_start = new_input;

            if also_comments {
                if let Some((new_new_input, whitespace)) = comment(new_input) {
                    new_input = new_new_input;
                    new_parts.push(whitespace);

                    input = new_input;
                    parts.append(&mut new_parts);
                }
            }

            if let Some((new_new_input, newline)) = newline(new_input) {
                input = new_input;
                parts.append(&mut new_parts);

                new_input = new_new_input;
                new_parts.push(newline);
            }

            if let Some((new_new_input, whitespace)) = leading_indentation(new_input, indentation) {
                new_input = new_new_input;
                new_parts.push(whitespace);

                input = new_input;
                parts.append(&mut new_parts);
            } else if let Some((new_new_input, whitespace)) = single_line_whitespace(new_input) {
                new_input = new_new_input;
                new_parts.push(whitespace);
            }

            if new_input == new_input_from_iteration_start {
                break;
            }
        }

        let parts = parts
            .into_iter()
            .filter(|it| {
                if let Rcst::Whitespace(ws) = it {
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
            ("foo", vec![Rcst::Newline("\n".to_string())])
        );
        assert_eq!(
            whitespaces_and_newlines("\nfoo", 1, true),
            ("\nfoo", vec![]),
        );
        assert_eq!(
            whitespaces_and_newlines("\n  foo", 1, true),
            (
                "foo",
                vec![
                    Rcst::Newline("\n".to_string()),
                    Rcst::Whitespace("  ".to_string())
                ]
            )
        );
        assert_eq!(
            whitespaces_and_newlines("\n  foo", 0, true),
            ("  foo", vec![Rcst::Newline("\n".to_string())])
        );
        assert_eq!(
            whitespaces_and_newlines(" \n  foo", 0, true),
            (
                "  foo",
                vec![
                    Rcst::Whitespace(" ".to_string()),
                    Rcst::Newline("\n".to_string())
                ]
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
                vec![Rcst::Error {
                    unparsable_input: "\t".to_string(),
                    error: RcstError::WeirdWhitespace
                }]
            )
        );
        assert_eq!(
            whitespaces_and_newlines("# hey\n  foo", 1, true),
            (
                "foo",
                vec![
                    Rcst::Comment {
                        octothorpe: Box::new(Rcst::Octothorpe),
                        comment: " hey".to_string()
                    },
                    Rcst::Newline("\n".to_string()),
                    Rcst::Whitespace("  ".to_string()),
                ],
            )
        );
        assert_eq!(
            whitespaces_and_newlines("# foo\n\n  #bar\n", 1, true),
            (
                "\n",
                vec![
                    Rcst::Comment {
                        octothorpe: Box::new(Rcst::Octothorpe),
                        comment: " foo".to_string()
                    },
                    Rcst::Newline("\n".to_string()),
                    Rcst::Newline("\n".to_string()),
                    Rcst::Whitespace("  ".to_string()),
                    Rcst::Comment {
                        octothorpe: Box::new(Rcst::Octothorpe),
                        comment: "bar".to_string()
                    }
                ]
            ),
        );
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

        let (input, mut expression) = expression(input, indentation + 1, false, true, true)
            .unwrap_or((
                input,
                Rcst::Error {
                    unparsable_input: "".to_string(),
                    error: RcstError::TextInterpolationWithoutExpression,
                },
            ));

        let (input, whitespace) = whitespaces_and_newlines(input, indentation + 1, false);
        expression = expression.wrap_in_whitespace(whitespace);

        let (input, closing_curly_braces) =
            parse_multiple(input, closing_curly_brace, Some((curly_brace_count, false))).unwrap_or(
                (
                    input,
                    vec![Rcst::Error {
                        unparsable_input: "".to_string(),
                        error: RcstError::TextInterpolationNotClosed,
                    }],
                ),
            );

        Some((
            input,
            Rcst::TextInterpolation {
                opening_curly_braces,
                expression: Box::new(expression),
                closing_curly_braces,
            },
        ))
    }

    // FIXME: It might be a good idea to ignore text interpolations in patterns
    #[instrument(level = "trace")]
    fn text(input: &str, indentation: usize) -> Option<(&str, Rcst)> {
        let (input, opening_single_quotes) = parse_multiple(input, single_quote, None)?;
        let (mut input, opening_double_quote) = double_quote(input)?;

        let push_line_to_parts = |line: &mut Vec<char>, parts: &mut Vec<Rcst>| {
            let joined_line = line.drain(..).join("");
            if !joined_line.is_empty() {
                parts.push(Rcst::TextPart(joined_line));
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
                            break Rcst::ClosingText {
                                closing_double_quote: Box::new(Rcst::DoubleQuote),
                                closing_single_quotes,
                            };
                        }
                        None => line.push('"'),
                    }
                }
                Some('{') => {
                    match text_interpolation(input, indentation, opening_single_quotes.len() + 1) {
                        Some((input_after_interpolation, interpolation)) => {
                            push_line_to_parts(&mut line, &mut parts);
                            input = input_after_interpolation;
                            parts.push(interpolation);
                        }
                        None => {
                            input = &input[1..];
                            line.push('{');
                        }
                    }
                }
                None => {
                    push_line_to_parts(&mut line, &mut parts);
                    break Rcst::Error {
                        unparsable_input: "".to_string(),
                        error: RcstError::TextNotClosed,
                    };
                }
                Some('\n') => {
                    push_line_to_parts(&mut line, &mut parts);
                    let (i, mut whitespace) =
                        whitespaces_and_newlines(input, indentation + 1, false);
                    input = i;
                    parts.append(&mut whitespace);
                    if let Some('\n') = input.chars().next() {
                        break Rcst::Error {
                            unparsable_input: "".to_string(),
                            error: RcstError::TextNotSufficientlyIndented,
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
            Rcst::Text {
                opening: Box::new(Rcst::OpeningText {
                    opening_single_quotes,
                    opening_double_quote: Box::new(opening_double_quote),
                }),
                parts,
                closing: Box::new(closing),
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
                Rcst::Text {
                    opening: Box::new(Rcst::OpeningText {
                        opening_single_quotes: vec![],
                        opening_double_quote: Box::new(Rcst::DoubleQuote),
                    }),
                    parts: vec![Rcst::TextPart("foo".to_string())],
                    closing: Box::new(Rcst::ClosingText {
                        closing_double_quote: Box::new(Rcst::DoubleQuote),
                        closing_single_quotes: vec![],
                    }),
                },
            )),
        );
        // "foo
        //   bar"2
        assert_eq!(
            text("\"foo\n  bar\"2", 0),
            Some((
                "2",
                Rcst::Text {
                    opening: Box::new(Rcst::OpeningText {
                        opening_single_quotes: vec![],
                        opening_double_quote: Box::new(Rcst::DoubleQuote),
                    }),
                    parts: vec![
                        Rcst::TextPart("foo".to_string()),
                        Rcst::Newline("\n".to_string()),
                        Rcst::Whitespace("  ".to_string()),
                        Rcst::TextPart("bar".to_string())
                    ],
                    closing: Box::new(Rcst::ClosingText {
                        closing_double_quote: Box::new(Rcst::DoubleQuote),
                        closing_single_quotes: vec![]
                    }),
                },
            )),
        );
        //   "foo
        //   bar"
        assert_eq!(
            text("\"foo\n  bar\"2", 1),
            Some((
                "\n  bar\"2",
                Rcst::Text {
                    opening: Box::new(Rcst::OpeningText {
                        opening_single_quotes: vec![],
                        opening_double_quote: Box::new(Rcst::DoubleQuote),
                    }),
                    parts: vec![Rcst::TextPart("foo".to_string()),],
                    closing: Box::new(Rcst::Error {
                        unparsable_input: "".to_string(),
                        error: RcstError::TextNotSufficientlyIndented,
                    }),
                }
            ))
        );
        assert_eq!(
            text("\"foo", 0),
            Some((
                "",
                Rcst::Text {
                    opening: Box::new(Rcst::OpeningText {
                        opening_single_quotes: vec![],
                        opening_double_quote: Box::new(Rcst::DoubleQuote)
                    }),
                    parts: vec![Rcst::TextPart("foo".to_string()),],
                    closing: Box::new(Rcst::Error {
                        unparsable_input: "".to_string(),
                        error: RcstError::TextNotClosed,
                    }),
                }
            ))
        );
        assert_eq!(
            text("''\"foo\"'bar\"'' baz", 0),
            Some((
                " baz",
                Rcst::Text {
                    opening: Box::new(Rcst::OpeningText {
                        opening_single_quotes: vec![Rcst::SingleQuote, Rcst::SingleQuote],
                        opening_double_quote: Box::new(Rcst::DoubleQuote),
                    }),
                    parts: vec![Rcst::TextPart("foo\"'bar".to_string())],
                    closing: Box::new(Rcst::ClosingText {
                        closing_double_quote: Box::new(Rcst::DoubleQuote),
                        closing_single_quotes: vec![Rcst::SingleQuote, Rcst::SingleQuote],
                    }),
                },
            )),
        );
        assert_eq!(
            text("\"foo {\"bar\"} baz\"", 0),
            Some((
                "",
                Rcst::Text {
                    opening: Box::new(Rcst::OpeningText {
                        opening_single_quotes: vec![],
                        opening_double_quote: Box::new(Rcst::DoubleQuote),
                    }),
                    parts: vec![
                        Rcst::TextPart("foo ".to_string()),
                        Rcst::TextInterpolation {
                            opening_curly_braces: vec![Rcst::OpeningCurlyBrace],
                            expression: Box::new(Rcst::Text {
                                opening: Box::new(Rcst::OpeningText {
                                    opening_single_quotes: vec![],
                                    opening_double_quote: Box::new(Rcst::DoubleQuote),
                                }),
                                parts: vec![Rcst::TextPart("bar".to_string())],
                                closing: Box::new(Rcst::ClosingText {
                                    closing_double_quote: Box::new(Rcst::DoubleQuote),
                                    closing_single_quotes: vec![],
                                }),
                            }),
                            closing_curly_braces: vec![Rcst::ClosingCurlyBrace],
                        },
                        Rcst::TextPart(" baz".to_string()),
                    ],
                    closing: Box::new(Rcst::ClosingText {
                        closing_double_quote: Box::new(Rcst::DoubleQuote),
                        closing_single_quotes: vec![],
                    }),
                },
            )),
        );
        assert_eq!(
            text("'\"foo {\"bar\"} baz\"'", 0),
            Some((
                "",
                Rcst::Text {
                    opening: Box::new(Rcst::OpeningText {
                        opening_single_quotes: vec![Rcst::SingleQuote],
                        opening_double_quote: Box::new(Rcst::DoubleQuote),
                    }),
                    parts: vec![Rcst::TextPart("foo {\"bar\"} baz".to_string())],
                    closing: Box::new(Rcst::ClosingText {
                        closing_double_quote: Box::new(Rcst::DoubleQuote),
                        closing_single_quotes: vec![Rcst::SingleQuote],
                    }),
                },
            )),
        );
        assert_eq!(
            text("\"foo {  \"bar\" } baz\"", 0),
            Some((
                "",
                Rcst::Text {
                    opening: Box::new(Rcst::OpeningText {
                        opening_single_quotes: vec![],
                        opening_double_quote: Box::new(Rcst::DoubleQuote)
                    }),
                    parts: vec![
                        Rcst::TextPart("foo ".to_string()),
                        Rcst::TextInterpolation {
                            opening_curly_braces: vec![Rcst::TrailingWhitespace {
                                child: Box::new(Rcst::OpeningCurlyBrace),
                                whitespace: vec![Rcst::Whitespace("  ".to_string())],
                            }],
                            expression: Box::new(Rcst::TrailingWhitespace {
                                child: Box::new(Rcst::Text {
                                    opening: Box::new(Rcst::OpeningText {
                                        opening_single_quotes: vec![],
                                        opening_double_quote: Box::new(Rcst::DoubleQuote),
                                    }),
                                    parts: vec![Rcst::TextPart("bar".to_string())],
                                    closing: Box::new(Rcst::ClosingText {
                                        closing_double_quote: Box::new(Rcst::DoubleQuote),
                                        closing_single_quotes: vec![],
                                    }),
                                }),
                                whitespace: vec![Rcst::Whitespace(" ".to_string())],
                            }),
                            closing_curly_braces: vec![Rcst::ClosingCurlyBrace],
                        },
                        Rcst::TextPart(" baz".to_string()),
                    ],
                    closing: Box::new(Rcst::ClosingText {
                        closing_double_quote: Box::new(Rcst::DoubleQuote),
                        closing_single_quotes: vec![],
                    }),
                },
            )),
        );
        assert_eq!(
            text(
                "\"Some text with {'\"an interpolation containing {{\"an interpolation\"}}\"'}\"",
                0,
            ),
            Some((
                "",
                Rcst::Text {
                    opening: Box::new(Rcst::OpeningText {
                        opening_single_quotes: vec![],
                        opening_double_quote: Box::new(Rcst::DoubleQuote),
                    }),
                    parts: vec![
                        Rcst::TextPart("Some text with ".to_string()),
                        Rcst::TextInterpolation {
                            opening_curly_braces: vec![Rcst::OpeningCurlyBrace],
                            expression: Box::new(Rcst::Text {
                                opening: Box::new(Rcst::OpeningText {
                                    opening_single_quotes: vec![Rcst::SingleQuote],
                                    opening_double_quote: Box::new(Rcst::DoubleQuote),
                                }),
                                parts: vec![
                                    Rcst::TextPart("an interpolation containing ".to_string()),
                                    Rcst::TextInterpolation {
                                        opening_curly_braces: vec![
                                            Rcst::OpeningCurlyBrace,
                                            Rcst::OpeningCurlyBrace,
                                        ],
                                        expression: Box::new(Rcst::Text {
                                            opening: Box::new(Rcst::OpeningText {
                                                opening_single_quotes: vec![],
                                                opening_double_quote: Box::new(Rcst::DoubleQuote),
                                            }),
                                            parts: vec![Rcst::TextPart(
                                                "an interpolation".to_string(),
                                            )],
                                            closing: Box::new(Rcst::ClosingText {
                                                closing_double_quote: Box::new(Rcst::DoubleQuote),
                                                closing_single_quotes: vec![],
                                            }),
                                        }),
                                        closing_curly_braces: vec![
                                            Rcst::ClosingCurlyBrace,
                                            Rcst::ClosingCurlyBrace,
                                        ],
                                    },
                                ],
                                closing: Box::new(Rcst::ClosingText {
                                    closing_double_quote: Box::new(Rcst::DoubleQuote),
                                    closing_single_quotes: vec![Rcst::SingleQuote],
                                })
                            }),
                            closing_curly_braces: vec![Rcst::ClosingCurlyBrace],
                        },
                    ],
                    closing: Box::new(Rcst::ClosingText {
                        closing_double_quote: Box::new(Rcst::DoubleQuote),
                        closing_single_quotes: vec![],
                    }),
                },
            )),
        );
        assert_eq!(
            text("\"{ {2} }\"", 0),
            Some((
                "",
                Rcst::Text {
                    opening: Box::new(Rcst::OpeningText {
                        opening_single_quotes: vec![],
                        opening_double_quote: Box::new(Rcst::DoubleQuote),
                    }),
                    parts: vec![Rcst::TextInterpolation {
                        opening_curly_braces: vec![Rcst::TrailingWhitespace {
                            child: Box::new(Rcst::OpeningCurlyBrace),
                            whitespace: vec![Rcst::Whitespace(" ".to_string())],
                        }],
                        expression: Box::new(Rcst::TrailingWhitespace {
                            child: Box::new(Rcst::Lambda {
                                opening_curly_brace: Box::new(Rcst::OpeningCurlyBrace),
                                parameters_and_arrow: None,
                                body: vec![Rcst::Int {
                                    value: 2u8.into(),
                                    string: "2".to_string(),
                                }],
                                closing_curly_brace: Box::new(Rcst::ClosingCurlyBrace),
                            }),
                            whitespace: vec![Rcst::Whitespace(" ".to_string())],
                        }),
                        closing_curly_braces: vec![Rcst::ClosingCurlyBrace],
                    }],
                    closing: Box::new(Rcst::ClosingText {
                        closing_double_quote: Box::new(Rcst::DoubleQuote),
                        closing_single_quotes: vec![],
                    }),
                },
            )),
        );
        assert_eq!(
            text("\"{{2}}\"", 0),
            Some((
                "",
                Rcst::Text {
                    opening: Box::new(Rcst::OpeningText {
                        opening_single_quotes: vec![],
                        opening_double_quote: Box::new(Rcst::DoubleQuote),
                    }),
                    parts: vec![
                        Rcst::TextPart("{".to_string()),
                        Rcst::TextInterpolation {
                            opening_curly_braces: vec![Rcst::OpeningCurlyBrace],
                            expression: Box::new(Rcst::Int {
                                value: 2u8.into(),
                                string: "2".to_string()
                            }),
                            closing_curly_braces: vec![Rcst::ClosingCurlyBrace],
                        },
                        Rcst::TextPart("}".to_string()),
                    ],
                    closing: Box::new(Rcst::ClosingText {
                        closing_double_quote: Box::new(Rcst::DoubleQuote),
                        closing_single_quotes: vec![],
                    }),
                },
            )),
        );
        assert_eq!(
            text("\"foo {} baz\"", 0),
            Some((
                "",
                Rcst::Text {
                    opening: Box::new(Rcst::OpeningText {
                        opening_single_quotes: vec![],
                        opening_double_quote: Box::new(Rcst::DoubleQuote),
                    }),
                    parts: vec![
                        Rcst::TextPart("foo ".to_string()),
                        Rcst::TextInterpolation {
                            opening_curly_braces: vec![Rcst::OpeningCurlyBrace],
                            expression: Box::new(Rcst::Error {
                                unparsable_input: "".to_string(),
                                error: RcstError::TextInterpolationWithoutExpression,
                            }),
                            closing_curly_braces: vec![Rcst::ClosingCurlyBrace],
                        },
                        Rcst::TextPart(" baz".to_string()),
                    ],
                    closing: Box::new(Rcst::ClosingText {
                        closing_double_quote: Box::new(Rcst::DoubleQuote),
                        closing_single_quotes: vec![],
                    }),
                },
            )),
        );
        assert_eq!(
            text("\"foo {\"bar\" baz\"", 0),
            Some((
                "",
                Rcst::Text {
                    opening: Box::new(Rcst::OpeningText {
                        opening_single_quotes: vec![],
                        opening_double_quote: Box::new(Rcst::DoubleQuote),
                    }),
                    parts: vec![
                        Rcst::TextPart("foo ".to_string()),
                        Rcst::TextInterpolation {
                            opening_curly_braces: vec![Rcst::OpeningCurlyBrace],
                            expression: Box::new(Rcst::TrailingWhitespace {
                                child: Box::new(Rcst::Text {
                                    opening: Box::new(Rcst::OpeningText {
                                        opening_single_quotes: vec![],
                                        opening_double_quote: Box::new(Rcst::DoubleQuote),
                                    }),
                                    parts: vec![Rcst::TextPart("bar".to_string())],
                                    closing: Box::new(Rcst::ClosingText {
                                        closing_double_quote: Box::new(Rcst::DoubleQuote),
                                        closing_single_quotes: vec![],
                                    }),
                                }),
                                whitespace: vec![Rcst::Whitespace(" ".to_string())],
                            }),
                            closing_curly_braces: vec![Rcst::Error {
                                unparsable_input: "".to_string(),
                                error: RcstError::TextInterpolationNotClosed,
                            }],
                        },
                        Rcst::TextPart("baz".to_string()),
                    ],
                    closing: Box::new(Rcst::ClosingText {
                        closing_double_quote: Box::new(Rcst::DoubleQuote),
                        closing_single_quotes: vec![],
                    }),
                },
            )),
        );
        assert_eq!(
            text("\"foo {\"bar\" \"a\"} baz\"", 0),
            Some((
                "a\"} baz\"",
                Rcst::Text {
                    opening: Box::new(Rcst::OpeningText {
                        opening_single_quotes: vec![],
                        opening_double_quote: Box::new(Rcst::DoubleQuote),
                    }),
                    parts: vec![
                        Rcst::TextPart("foo ".to_string()),
                        Rcst::TextInterpolation {
                            opening_curly_braces: vec![Rcst::OpeningCurlyBrace],
                            expression: Box::new(Rcst::TrailingWhitespace {
                                child: Box::new(Rcst::Text {
                                    opening: Box::new(Rcst::OpeningText {
                                        opening_single_quotes: vec![],
                                        opening_double_quote: Box::new(Rcst::DoubleQuote),
                                    }),
                                    parts: vec![Rcst::TextPart("bar".to_string())],
                                    closing: Box::new(Rcst::ClosingText {
                                        closing_double_quote: Box::new(Rcst::DoubleQuote),
                                        closing_single_quotes: vec![],
                                    })
                                }),
                                whitespace: vec![Rcst::Whitespace(" ".to_string())],
                            }),
                            closing_curly_braces: vec![Rcst::Error {
                                unparsable_input: "".to_string(),
                                error: RcstError::TextInterpolationNotClosed,
                            }],
                        },
                    ],
                    closing: Box::new(Rcst::ClosingText {
                        closing_double_quote: Box::new(Rcst::DoubleQuote),
                        closing_single_quotes: vec![],
                    }),
                },
            )),
        );
    }

    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    enum ParseType {
        Expression,
        Pattern,
    }
    impl ParseType {
        #[instrument(level = "trace")]
        fn parse(self, input: &str, indentation: usize) -> Option<(&str, Rcst)> {
            match self {
                ParseType::Expression => expression(input, indentation, false, true, true),
                ParseType::Pattern => pattern(input, indentation),
            }
        }
    }

    #[instrument(level = "trace")]
    fn expression(
        input: &str,
        indentation: usize,
        allow_assignment: bool,
        allow_call: bool,
        allow_pipe: bool,
    ) -> Option<(&str, Rcst)> {
        // If we start the call list with `if … else …`, the formatting looks
        // weird. Hence, we start with a single `None`.
        let (mut input, mut result) = None
            .or_else(|| {
                if allow_assignment {
                    assignment(input, indentation)
                } else {
                    None
                }
            })
            .or_else(|| int(input))
            .or_else(|| text(input, indentation))
            .or_else(|| symbol(input))
            .or_else(|| list(input, indentation, ParseType::Expression))
            .or_else(|| struct_(input, indentation, ParseType::Expression))
            .or_else(|| parenthesized(input, indentation))
            .or_else(|| lambda(input, indentation))
            .or_else(|| {
                if allow_call {
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
                        Rcst::Error {
                            unparsable_input: word,
                            error: RcstError::UnexpectedCharacters,
                        },
                    )
                })
            })?;

        loop {
            let mut did_make_progress = false;

            'structAccess: {
                let (new_input, whitespace_after_struct) =
                    whitespaces_and_newlines(input, indentation + 1, true);

                let Some((new_input, dot)) = dot(new_input) else { break 'structAccess; };
                let (new_input, whitespace_after_dot) =
                    whitespaces_and_newlines(new_input, indentation + 1, true);
                let dot = dot.wrap_in_whitespace(whitespace_after_dot);

                let Some((new_input, key)) = identifier(new_input) else { break 'structAccess; };

                input = new_input;
                result = Rcst::StructAccess {
                    struct_: Box::new(result.wrap_in_whitespace(whitespace_after_struct)),
                    dot: Box::new(dot),
                    key: Box::new(key),
                };
                did_make_progress = true;
            }

            if allow_pipe {
                'pipe: {
                    let (new_input, whitespace_after_receiver) =
                        whitespaces_and_newlines(input, indentation, true);

                    let Some((new_input, bar)) = bar(new_input) else { break 'pipe; };
                    let (new_input, whitespace_after_bar) =
                        whitespaces_and_newlines(new_input, indentation + 1, true);
                    let bar = bar.wrap_in_whitespace(whitespace_after_bar);

                    let indentation = if bar.is_multiline() {
                        indentation + 1
                    } else {
                        indentation
                    };
                    let (new_input, call) = expression(new_input, indentation, false, true, false)
                        .unwrap_or_else(|| {
                            let error = Rcst::Error {
                                unparsable_input: "".to_string(),
                                error: RcstError::PipeMissesCall,
                            };
                            (new_input, error)
                        });

                    input = new_input;
                    result = Rcst::Pipe {
                        receiver: Box::new(result.wrap_in_whitespace(whitespace_after_receiver)),
                        bar: Box::new(bar),
                        call: Box::new(call),
                    };
                    did_make_progress = true;
                }
                'match_: {
                    let (new_input, whitespace_after_receiver) =
                        whitespaces_and_newlines(input, indentation, true);

                    let Some((new_input, percent, cases)) = match_suffix(new_input, indentation) else { break 'match_; };

                    input = new_input;
                    result = Rcst::Match {
                        expression: Box::new(result.wrap_in_whitespace(whitespace_after_receiver)),
                        percent: Box::new(percent),
                        cases,
                    };
                    did_make_progress = true;
                }
            }

            if !did_make_progress {
                break;
            }
        }
        Some((input, result))
    }
    #[test]
    fn test_expression() {
        assert_eq!(
            expression("foo", 0, true, true, true),
            Some(("", Rcst::Identifier("foo".to_string())))
        );
        assert_eq!(
            expression("(foo Bar)", 0, false, false, true),
            Some((
                "",
                Rcst::Parenthesized {
                    opening_parenthesis: Box::new(Rcst::OpeningParenthesis),
                    inner: Box::new(Rcst::Call {
                        receiver: Box::new(Rcst::TrailingWhitespace {
                            child: Box::new(Rcst::Identifier("foo".to_string())),
                            whitespace: vec![Rcst::Whitespace(" ".to_string())]
                        }),
                        arguments: vec![Rcst::Symbol("Bar".to_string())]
                    }),
                    closing_parenthesis: Box::new(Rcst::ClosingParenthesis)
                }
            ))
        );
        // foo
        //   .bar
        assert_eq!(
            expression("foo\n  .bar", 0, true, true, true),
            Some((
                "",
                Rcst::StructAccess {
                    struct_: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Identifier("foo".to_string())),
                        whitespace: vec![
                            Rcst::Newline("\n".to_string()),
                            Rcst::Whitespace("  ".to_string()),
                        ],
                    }),
                    dot: Box::new(Rcst::Dot),
                    key: Box::new(Rcst::Identifier("bar".to_owned())),
                },
            )),
        );
        // foo
        // .bar
        assert_eq!(
            expression("foo\n.bar", 0, true, true, true),
            Some(("\n.bar", Rcst::Identifier("foo".to_string()))),
        );
        // foo
        // | bar
        assert_eq!(
            expression("foo\n| bar", 0, true, true, true),
            Some((
                "",
                Rcst::Pipe {
                    receiver: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Identifier("foo".to_string())),
                        whitespace: vec![Rcst::Newline("\n".to_string())],
                    }),
                    bar: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Bar),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }),
                    call: Box::new(Rcst::Identifier("bar".to_owned())),
                },
            )),
        );
        // foo
        // | bar baz
        assert_eq!(
            expression("foo\n| bar baz", 0, true, true, true),
            Some((
                "",
                Rcst::Pipe {
                    receiver: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Identifier("foo".to_string())),
                        whitespace: vec![Rcst::Newline("\n".to_string())],
                    }),
                    bar: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Bar),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }),
                    call: Box::new(Rcst::Call {
                        receiver: Box::new(Rcst::TrailingWhitespace {
                            child: Box::new(Rcst::Identifier("bar".to_owned())),
                            whitespace: vec![Rcst::Whitespace(" ".to_string())],
                        }),
                        arguments: vec![Rcst::Identifier("baz".to_owned())],
                    }),
                },
            )),
        );
        // foo %
        //   123 -> 123
        assert_eq!(
            expression("foo %\n  123 -> 123", 0, true, true, true),
            Some((
                "",
                Rcst::Match {
                    expression: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Identifier("foo".to_string())),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }),
                    percent: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Percent),
                        whitespace: vec![
                            Rcst::Newline("\n".to_string()),
                            Rcst::Whitespace("  ".to_string()),
                        ],
                    }),
                    cases: vec![Rcst::MatchCase {
                        pattern: Box::new(Rcst::TrailingWhitespace {
                            child: Box::new(Rcst::Int {
                                value: 123u8.into(),
                                string: "123".to_string(),
                            }),
                            whitespace: vec![Rcst::Whitespace(" ".to_string())],
                        }),
                        arrow: Box::new(Rcst::TrailingWhitespace {
                            child: Box::new(Rcst::Arrow),
                            whitespace: vec![Rcst::Whitespace(" ".to_string())],
                        }),
                        body: vec![Rcst::Int {
                            value: 123u8.into(),
                            string: "123".to_string(),
                        }],
                    }],
                },
            )),
        );
    }

    /// Multiple expressions that are occurring one after another.
    #[instrument(level = "trace")]
    fn run_of_expressions(input: &str, indentation: usize) -> Option<(&str, Vec<Rcst>)> {
        let mut expressions = vec![];
        let (mut input, expr) = expression(input, indentation, false, false, false)?;
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
            let last = expressions.pop().unwrap();
            expressions.push(last.wrap_in_whitespace(whitespace));

            let (i, expr) = match expression(i, indentation, false, has_multiline_whitespace, false)
            {
                Some(it) => it,
                None => {
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
                }
            };

            expressions.push(expr);
            input = i;
        }
        Some((input, expressions))
    }
    #[test]
    fn test_run_of_expressions() {
        assert_eq!(
            run_of_expressions("print", 0),
            Some(("", vec![Rcst::Identifier("print".to_string())])),
        );
        // foo
        //   bar
        assert_eq!(
            call("foo\n  bar", 0),
            Some((
                "",
                Rcst::Call {
                    receiver: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Identifier("foo".to_string())),
                        whitespace: vec![
                            Rcst::Newline("\n".to_string()),
                            Rcst::Whitespace("  ".to_string()),
                        ],
                    }),
                    arguments: vec![Rcst::Identifier("bar".to_string())],
                },
            )),
        );
        assert_eq!(
            run_of_expressions("(foo Bar) Baz", 0),
            Some((
                "",
                vec![
                    Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Parenthesized {
                            opening_parenthesis: Box::new(Rcst::OpeningParenthesis),
                            inner: Box::new(Rcst::Call {
                                receiver: Box::new(Rcst::TrailingWhitespace {
                                    child: Box::new(Rcst::Identifier("foo".to_string())),
                                    whitespace: vec![Rcst::Whitespace(" ".to_string())],
                                }),
                                arguments: vec![Rcst::Symbol("Bar".to_string())],
                            }),
                            closing_parenthesis: Box::new(Rcst::ClosingParenthesis),
                        }),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    },
                    Rcst::Symbol("Baz".to_string()),
                ],
            )),
        );
        assert_eq!(
            run_of_expressions("foo | bar", 0),
            Some((
                "| bar",
                vec![Rcst::TrailingWhitespace {
                    child: Box::new(Rcst::Identifier("foo".to_string())),
                    whitespace: vec![Rcst::Whitespace(" ".to_string())],
                }],
            )),
        );
    }

    #[instrument(level = "trace")]
    fn call(input: &str, indentation: usize) -> Option<(&str, Rcst)> {
        let (input, expressions) = run_of_expressions(input, indentation)?;
        if expressions.len() < 2 {
            return None;
        }

        let (whitespace, mut expressions) = expressions.split_outer_trailing_whitespace();
        let arguments = expressions.split_off(1);
        let receiver = expressions.into_iter().next().unwrap();
        Some((
            input,
            Rcst::Call {
                receiver: Box::new(receiver),
                arguments,
            }
            .wrap_in_whitespace(whitespace),
        ))
    }
    #[test]
    fn test_call() {
        assert_eq!(call("print", 0), None);
        assert_eq!(
            call("foo bar", 0),
            Some((
                "",
                Rcst::Call {
                    receiver: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Identifier("foo".to_string())),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }),
                    arguments: vec![Rcst::Identifier("bar".to_string())]
                }
            ))
        );
        assert_eq!(
            call("Foo 4 bar", 0),
            Some((
                "",
                Rcst::Call {
                    receiver: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Symbol("Foo".to_string())),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }),
                    arguments: vec![
                        Rcst::TrailingWhitespace {
                            child: Box::new(Rcst::Int {
                                value: 4u8.into(),
                                string: "4".to_string()
                            }),
                            whitespace: vec![Rcst::Whitespace(" ".to_string())],
                        },
                        Rcst::Identifier("bar".to_string())
                    ]
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
                Rcst::Call {
                    receiver: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Identifier("foo".to_string())),
                        whitespace: vec![
                            Rcst::Newline("\n".to_string()),
                            Rcst::Whitespace("  ".to_string())
                        ],
                    }),
                    arguments: vec![
                        Rcst::TrailingWhitespace {
                            child: Box::new(Rcst::Identifier("bar".to_string())),
                            whitespace: vec![
                                Rcst::Newline("\n".to_string()),
                                Rcst::Whitespace("  ".to_string())
                            ],
                        },
                        Rcst::Identifier("baz".to_string())
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
                Rcst::Call {
                    receiver: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Identifier("foo".to_string())),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }),
                    arguments: vec![
                        Rcst::TrailingWhitespace {
                            child: Box::new(Rcst::Int {
                                value: 1u8.into(),
                                string: "1".to_string()
                            }),
                            whitespace: vec![Rcst::Whitespace(" ".to_string())],
                        },
                        Rcst::TrailingWhitespace {
                            child: Box::new(Rcst::Int {
                                value: 2u8.into(),
                                string: "2".to_string()
                            }),
                            whitespace: vec![
                                Rcst::Newline("\n".to_string()),
                                Rcst::Whitespace("  ".to_string())
                            ],
                        },
                        Rcst::TrailingWhitespace {
                            child: Box::new(Rcst::Int {
                                value: 3u8.into(),
                                string: "3".to_string()
                            }),
                            whitespace: vec![
                                Rcst::Newline("\n".to_string()),
                                Rcst::Whitespace("  ".to_string())
                            ],
                        },
                        Rcst::Int {
                            value: 4u8.into(),
                            string: "4".to_string()
                        }
                    ],
                }
            ))
        );
        assert_eq!(
            call("(foo Bar) Baz\n", 0),
            Some((
                "\n",
                Rcst::Call {
                    receiver: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Parenthesized {
                            opening_parenthesis: Box::new(Rcst::OpeningParenthesis),
                            inner: Box::new(Rcst::Call {
                                receiver: Box::new(Rcst::TrailingWhitespace {
                                    child: Box::new(Rcst::Identifier("foo".to_string())),
                                    whitespace: vec![Rcst::Whitespace(" ".to_string())]
                                }),
                                arguments: vec![Rcst::Symbol("Bar".to_string())]
                            }),
                            closing_parenthesis: Box::new(Rcst::ClosingParenthesis)
                        }),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())]
                    }),
                    arguments: vec![Rcst::Symbol("Baz".to_string())]
                }
            ))
        );
        // foo T
        //
        //
        // bar = 5
        assert_eq!(
            call("foo T\n\n\nbar = 5", 0),
            Some((
                "\nbar = 5",
                Rcst::TrailingWhitespace {
                    child: Box::new(Rcst::Call {
                        receiver: Box::new(Rcst::TrailingWhitespace {
                            child: Box::new(Rcst::Identifier("foo".to_string())),
                            whitespace: vec![Rcst::Whitespace(" ".to_string())]
                        }),
                        arguments: vec![Rcst::Symbol("T".to_string())]
                    }),
                    whitespace: vec![
                        Rcst::Newline("\n".to_string()),
                        Rcst::Newline("\n".to_string())
                    ],
                }
            ))
        );
    }

    #[instrument(level = "trace")]
    fn list(input: &str, indentation: usize, parse_type: ParseType) -> Option<(&str, Rcst)> {
        let (mut input, mut opening_parenthesis) = opening_parenthesis(input)?;

        // Empty list `(,)`
        'handleEmptyList: {
            // Whitespace before comma.
            let (input, leading_whitespace) =
                whitespaces_and_newlines(input, indentation + 1, true);
            let opening_parenthesis = opening_parenthesis
                .clone()
                .wrap_in_whitespace(leading_whitespace);

            // Comma.
            let Some((input, comma)) = comma(input) else { break 'handleEmptyList; };

            // Whitespace after comma.
            let (input, trailing_whitespace) =
                whitespaces_and_newlines(input, indentation + 1, true);
            let comma = comma.wrap_in_whitespace(trailing_whitespace);

            // Closing parenthesis.
            let Some((input, closing_parenthesis)) = closing_parenthesis(input) else {
                break 'handleEmptyList;
            };

            return Some((
                input,
                Rcst::List {
                    opening_parenthesis: Box::new(opening_parenthesis),
                    items: vec![comma],
                    closing_parenthesis: Box::new(closing_parenthesis),
                },
            ));
        }

        let mut items: Vec<Rcst> = vec![];
        let mut items_indentation = indentation;
        let mut has_at_least_one_comma = false;
        loop {
            let new_input = input;

            // Whitespace before value.
            let (new_input, whitespace) =
                whitespaces_and_newlines(new_input, indentation + 1, true);
            if whitespace.is_multiline() {
                items_indentation = indentation + 1;
            }
            if items.is_empty() {
                opening_parenthesis = opening_parenthesis.wrap_in_whitespace(whitespace);
            } else {
                let last = items.pop().unwrap();
                items.push(last.wrap_in_whitespace(whitespace));
            }

            // Value.
            let (new_input, value, has_value) = match parse_type.parse(new_input, items_indentation)
            {
                Some((new_input, value)) => (new_input, value, true),
                None => (
                    new_input,
                    Rcst::Error {
                        unparsable_input: "".to_string(),
                        error: RcstError::ListItemMissesValue,
                    },
                    false,
                ),
            };

            // Whitespace between value and comma.
            let (new_input, whitespace) =
                whitespaces_and_newlines(new_input, items_indentation + 1, true);
            if whitespace.is_multiline() {
                items_indentation = indentation + 1;
            }
            let value = value.wrap_in_whitespace(whitespace);

            // Comma.
            let (new_input, comma) = match comma(new_input) {
                Some((new_input, comma)) => (new_input, Some(comma)),
                None => (new_input, None),
            };

            if !has_value && comma.is_none() {
                break;
            }
            has_at_least_one_comma |= comma.is_some();

            input = new_input;
            items.push(Rcst::ListItem {
                value: Box::new(value),
                comma: comma.map(Box::new),
            });
        }
        if !has_at_least_one_comma {
            return None;
        }

        let (new_input, whitespace) = whitespaces_and_newlines(input, indentation, true);

        let (input, closing_parenthesis) = match closing_parenthesis(new_input) {
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
                Rcst::Error {
                    unparsable_input: "".to_string(),
                    error: RcstError::ListNotClosed,
                },
            ),
        };

        Some((
            input,
            Rcst::List {
                opening_parenthesis: Box::new(opening_parenthesis),
                items,
                closing_parenthesis: Box::new(closing_parenthesis),
            },
        ))
    }
    #[test]
    fn test_list() {
        assert_eq!(list("hello", 0, ParseType::Expression), None);
        assert_eq!(list("()", 0, ParseType::Expression), None);
        assert_eq!(
            list("(,)", 0, ParseType::Expression),
            Some((
                "",
                Rcst::List {
                    opening_parenthesis: Box::new(Rcst::OpeningParenthesis),
                    items: vec![Rcst::Comma],
                    closing_parenthesis: Box::new(Rcst::ClosingParenthesis),
                },
            )),
        );
        assert_eq!(list("(foo)", 0, ParseType::Expression), None);
        assert_eq!(
            list("(foo,)", 0, ParseType::Expression),
            Some((
                "",
                Rcst::List {
                    opening_parenthesis: Box::new(Rcst::OpeningParenthesis),
                    items: vec![Rcst::ListItem {
                        value: Box::new(Rcst::Identifier("foo".to_string())),
                        comma: Some(Box::new(Rcst::Comma)),
                    }],
                    closing_parenthesis: Box::new(Rcst::ClosingParenthesis),
                },
            )),
        );
        assert_eq!(
            list("(foo,bar)", 0, ParseType::Expression),
            Some((
                "",
                Rcst::List {
                    opening_parenthesis: Box::new(Rcst::OpeningParenthesis),
                    items: vec![
                        Rcst::ListItem {
                            value: Box::new(Rcst::Identifier("foo".to_string())),
                            comma: Some(Box::new(Rcst::Comma)),
                        },
                        Rcst::ListItem {
                            value: Box::new(Rcst::Identifier("bar".to_string())),
                            comma: None,
                        },
                    ],
                    closing_parenthesis: Box::new(Rcst::ClosingParenthesis),
                },
            )),
        );
        // (
        //   foo,
        //   4,
        //   "Hi",
        // )
        assert_eq!(
            list("(\n  foo,\n  4,\n  \"Hi\",\n)", 0, ParseType::Expression),
            Some((
                "",
                Rcst::List {
                    opening_parenthesis: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::OpeningParenthesis),
                        whitespace: vec![
                            Rcst::Newline("\n".to_string()),
                            Rcst::Whitespace("  ".to_string())
                        ],
                    }),
                    items: vec![
                        Rcst::TrailingWhitespace {
                            child: Box::new(Rcst::ListItem {
                                value: Box::new(Rcst::Identifier("foo".to_string())),
                                comma: Some(Box::new(Rcst::Comma)),
                            }),
                            whitespace: vec![
                                Rcst::Newline("\n".to_string()),
                                Rcst::Whitespace("  ".to_string())
                            ],
                        },
                        Rcst::TrailingWhitespace {
                            child: Box::new(Rcst::ListItem {
                                value: Box::new(Rcst::Int {
                                    value: 4u8.into(),
                                    string: "4".to_string()
                                }),
                                comma: Some(Box::new(Rcst::Comma)),
                            }),
                            whitespace: vec![
                                Rcst::Newline("\n".to_string()),
                                Rcst::Whitespace("  ".to_string())
                            ],
                        },
                        Rcst::TrailingWhitespace {
                            child: Box::new(Rcst::ListItem {
                                value: Box::new(Rcst::Text {
                                    opening: Box::new(Rcst::OpeningText {
                                        opening_single_quotes: vec![],
                                        opening_double_quote: Box::new(Rcst::DoubleQuote),
                                    }),
                                    parts: vec![Rcst::TextPart("Hi".to_string())],
                                    closing: Box::new(Rcst::ClosingText {
                                        closing_double_quote: Box::new(Rcst::DoubleQuote),
                                        closing_single_quotes: vec![],
                                    }),
                                }),
                                comma: Some(Box::new(Rcst::Comma))
                            }),
                            whitespace: vec![Rcst::Newline("\n".to_string())]
                        }
                    ],
                    closing_parenthesis: Box::new(Rcst::ClosingParenthesis),
                },
            )),
        );
    }

    #[instrument(level = "trace")]
    fn struct_(input: &str, indentation: usize, parse_type: ParseType) -> Option<(&str, Rcst)> {
        let (mut outer_input, mut opening_bracket) = opening_bracket(input)?;

        let mut fields: Vec<Rcst> = vec![];
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

            // The key if it's explicit or the value when using a shorthand.
            let (input, key_or_value) = match parse_type.parse(input, fields_indentation) {
                Some((input, key)) => (input, Some(key)),
                None => (input, None),
            };

            // Whitespace between key/value and colon.
            let (input, key_or_value_whitespace) =
                whitespaces_and_newlines(input, fields_indentation + 1, true);
            if key_or_value_whitespace.is_multiline() {
                fields_indentation = indentation + 1;
            }

            // Colon.
            let (input, colon, has_colon) = match colon(input) {
                Some((new_input, colon)) if colon_equals_sign(input).is_none() => {
                    (new_input, colon, true)
                }
                _ => (
                    input,
                    Rcst::Error {
                        unparsable_input: "".to_string(),
                        error: RcstError::StructFieldMissesColon,
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
            let (input, value, has_value) = match parse_type.parse(input, fields_indentation + 1) {
                Some((input, value)) => (input, value, true),
                None => (
                    input,
                    Rcst::Error {
                        unparsable_input: "".to_string(),
                        error: RcstError::StructFieldMissesValue,
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

            if key_or_value.is_none() && !has_value && comma.is_none() {
                break;
            }

            let is_using_shorthand = key_or_value.is_some() && !has_colon && !has_value;
            let key_or_value = key_or_value.unwrap_or_else(|| Rcst::Error {
                unparsable_input: "".to_string(),
                error: if is_using_shorthand {
                    RcstError::StructFieldMissesValue
                } else {
                    RcstError::StructFieldMissesKey
                },
            });
            let key_or_value = key_or_value.wrap_in_whitespace(key_or_value_whitespace);

            outer_input = input;
            let comma = comma.map(Box::new);
            let field = if is_using_shorthand {
                Rcst::StructField {
                    key_and_colon: None,
                    value: Box::new(key_or_value),
                    comma,
                }
            } else {
                Rcst::StructField {
                    key_and_colon: Some(Box::new((key_or_value, colon))),
                    value: Box::new(value),
                    comma,
                }
            };
            fields.push(field);
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
                Rcst::Error {
                    unparsable_input: "".to_string(),
                    error: RcstError::StructNotClosed,
                },
            ),
        };

        Some((
            input,
            Rcst::Struct {
                opening_bracket: Box::new(opening_bracket),
                fields,
                closing_bracket: Box::new(closing_bracket),
            },
        ))
    }
    #[test]
    fn test_struct() {
        assert_eq!(struct_("hello", 0, ParseType::Expression), None);
        assert_eq!(
            struct_("[]", 0, ParseType::Expression),
            Some((
                "",
                Rcst::Struct {
                    opening_bracket: Box::new(Rcst::OpeningBracket),
                    fields: vec![],
                    closing_bracket: Box::new(Rcst::ClosingBracket),
                },
            )),
        );
        assert_eq!(
            struct_("[foo:bar]", 0, ParseType::Expression),
            Some((
                "",
                Rcst::Struct {
                    opening_bracket: Box::new(Rcst::OpeningBracket),
                    fields: vec![Rcst::StructField {
                        key_and_colon: Some(Box::new((
                            Rcst::Identifier("foo".to_string()),
                            Rcst::Colon,
                        ))),
                        value: Box::new(Rcst::Identifier("bar".to_string())),
                        comma: None,
                    }],
                    closing_bracket: Box::new(Rcst::ClosingBracket),
                },
            )),
        );
        assert_eq!(
            struct_("[foo,bar:baz]", 0, ParseType::Expression),
            Some((
                "",
                Rcst::Struct {
                    opening_bracket: Box::new(Rcst::OpeningBracket),
                    fields: vec![
                        Rcst::StructField {
                            key_and_colon: None,
                            value: Box::new(Rcst::Identifier("foo".to_string())),
                            comma: Some(Box::new(Rcst::Comma)),
                        },
                        Rcst::StructField {
                            key_and_colon: Some(Box::new((
                                Rcst::Identifier("bar".to_string()),
                                Rcst::Colon,
                            ))),
                            value: Box::new(Rcst::Identifier("baz".to_string())),
                            comma: None,
                        },
                    ],
                    closing_bracket: Box::new(Rcst::ClosingBracket),
                },
            )),
        );
        assert_eq!(
            struct_("[foo := [foo]", 0, ParseType::Pattern),
            Some((
                ":= [foo]",
                Rcst::Struct {
                    opening_bracket: Box::new(Rcst::OpeningBracket),
                    fields: vec![Rcst::StructField {
                        key_and_colon: None,
                        value: Box::new(Rcst::TrailingWhitespace {
                            child: Box::new(Rcst::Identifier("foo".to_string())),
                            whitespace: vec![Rcst::Whitespace(" ".to_string())],
                        }),
                        comma: None,
                    }],
                    closing_bracket: Box::new(Rcst::Error {
                        unparsable_input: "".to_string(),
                        error: RcstError::StructNotClosed,
                    }),
                },
            )),
        );
        // [
        //   foo: bar,
        //   4: "Hi",
        // ]
        assert_eq!(
            struct_("[\n  foo: bar,\n  4: \"Hi\",\n]", 0, ParseType::Expression),
            Some((
                "",
                Rcst::Struct {
                    opening_bracket: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::OpeningBracket),
                        whitespace: vec![
                            Rcst::Newline("\n".to_string()),
                            Rcst::Whitespace("  ".to_string()),
                        ],
                    }),
                    fields: vec![
                        Rcst::TrailingWhitespace {
                            child: Box::new(Rcst::StructField {
                                key_and_colon: Some(Box::new((
                                    Rcst::Identifier("foo".to_string()),
                                    Rcst::TrailingWhitespace {
                                        child: Box::new(Rcst::Colon),
                                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                                    },
                                ))),
                                value: Box::new(Rcst::Identifier("bar".to_string())),
                                comma: Some(Box::new(Rcst::Comma)),
                            }),
                            whitespace: vec![
                                Rcst::Newline("\n".to_string()),
                                Rcst::Whitespace("  ".to_string()),
                            ],
                        },
                        Rcst::TrailingWhitespace {
                            child: Box::new(Rcst::StructField {
                                key_and_colon: Some(Box::new((
                                    Rcst::Int {
                                        value: 4u8.into(),
                                        string: "4".to_string()
                                    },
                                    Rcst::TrailingWhitespace {
                                        child: Box::new(Rcst::Colon),
                                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                                    },
                                ))),
                                value: Box::new(Rcst::Text {
                                    opening: Box::new(Rcst::OpeningText {
                                        opening_single_quotes: vec![],
                                        opening_double_quote: Box::new(Rcst::DoubleQuote),
                                    }),
                                    parts: vec![Rcst::TextPart("Hi".to_string())],
                                    closing: Box::new(Rcst::ClosingText {
                                        closing_double_quote: Box::new(Rcst::DoubleQuote),
                                        closing_single_quotes: vec![],
                                    }),
                                }),
                                comma: Some(Box::new(Rcst::Comma)),
                            }),
                            whitespace: vec![Rcst::Newline("\n".to_string())],
                        },
                    ],
                    closing_bracket: Box::new(Rcst::ClosingBracket),
                },
            )),
        );
    }

    #[instrument(level = "trace")]
    fn parenthesized(input: &str, indentation: usize) -> Option<(&str, Rcst)> {
        let (input, opening_parenthesis) = opening_parenthesis(input)?;

        let (input, whitespace) = whitespaces_and_newlines(input, indentation + 1, true);
        let inner_indentation = if whitespace.is_multiline() {
            indentation + 1
        } else {
            indentation
        };
        let opening_parenthesis = opening_parenthesis.wrap_in_whitespace(whitespace);

        let (input, inner) = expression(input, inner_indentation, false, true, true).unwrap_or((
            input,
            Rcst::Error {
                unparsable_input: "".to_string(),
                error: RcstError::OpeningParenthesisWithoutExpression,
            },
        ));

        let (input, whitespace) = whitespaces_and_newlines(input, indentation, true);
        let inner = inner.wrap_in_whitespace(whitespace);

        let (input, closing_parenthesis) = closing_parenthesis(input).unwrap_or((
            input,
            Rcst::Error {
                unparsable_input: "".to_string(),
                error: RcstError::ParenthesisNotClosed,
            },
        ));

        Some((
            input,
            Rcst::Parenthesized {
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
                Rcst::Parenthesized {
                    opening_parenthesis: Box::new(Rcst::OpeningParenthesis),
                    inner: Box::new(Rcst::Identifier("foo".to_string())),
                    closing_parenthesis: Box::new(Rcst::ClosingParenthesis),
                }
            ))
        );
        assert_eq!(parenthesized("foo", 0), None);
        assert_eq!(
            parenthesized("(foo", 0),
            Some((
                "",
                Rcst::Parenthesized {
                    opening_parenthesis: Box::new(Rcst::OpeningParenthesis),
                    inner: Box::new(Rcst::Identifier("foo".to_string())),
                    closing_parenthesis: Box::new(Rcst::Error {
                        unparsable_input: "".to_string(),
                        error: RcstError::ParenthesisNotClosed
                    }),
                }
            ))
        );
    }

    #[instrument(level = "trace")]
    fn pattern(input: &str, indentation: usize) -> Option<(&str, Rcst)> {
        int(input)
            .or_else(|| text(input, indentation))
            .or_else(|| symbol(input))
            .or_else(|| list(input, indentation, ParseType::Pattern))
            .or_else(|| struct_(input, indentation, ParseType::Pattern))
            .or_else(|| identifier(input))
    }
    #[test]
    fn test_pattern() {
        assert_eq!(
            pattern("foo", 0),
            Some(("", Rcst::Identifier("foo".to_string())))
        );
    }

    #[instrument(level = "trace")]
    pub fn body(mut input: &str, indentation: usize) -> (&str, Vec<Rcst>) {
        let mut expressions = vec![];

        let mut number_of_expressions_in_last_iteration = -1i64;
        while number_of_expressions_in_last_iteration < expressions.len() as i64 {
            number_of_expressions_in_last_iteration = expressions.len() as i64;

            let (new_input, mut whitespace) = whitespaces_and_newlines(input, indentation, true);
            input = new_input;
            expressions.append(&mut whitespace);

            let mut indentation = indentation;
            if let Some((new_input, unexpected_whitespace)) = single_line_whitespace(input) {
                input = new_input;
                indentation += match &unexpected_whitespace {
                    Rcst::Whitespace(whitespace)
                    | Rcst::Error {
                        unparsable_input: whitespace,
                        error: RcstError::WeirdWhitespace,
                    } => whitespace_indentation_score(whitespace) / 2,
                    _ => panic!(
                        "single_line_whitespace returned something other than Whitespace or Error."
                    ),
                };
                expressions.push(Rcst::Error {
                    unparsable_input: unexpected_whitespace.to_string(),
                    error: RcstError::TooMuchWhitespace,
                });
            }

            match expression(input, indentation, true, true, true) {
                Some((new_input, expression)) => {
                    input = new_input;

                    let (whitespace, expression) = expression.split_outer_trailing_whitespace();
                    expressions.push(expression);
                    for whitespace in whitespace {
                        expressions.push(whitespace);
                    }
                }
                None => {
                    let fallback = colon(new_input)
                        .or_else(|| comma(new_input))
                        .or_else(|| closing_parenthesis(new_input))
                        .or_else(|| closing_bracket(new_input))
                        .or_else(|| closing_curly_brace(new_input))
                        .or_else(|| arrow(new_input));
                    if let Some((new_input, cst)) = fallback {
                        input = new_input;
                        expressions.push(cst);
                    }
                }
            }
        }
        (input, expressions)
    }

    #[instrument(level = "trace")]
    fn match_suffix(input: &str, indentation: usize) -> Option<(&str, Rcst, Vec<Rcst>)> {
        let (input, percent) = percent(input)?;
        let (mut input, whitespace) = whitespaces_and_newlines(input, indentation + 1, true);
        if !whitespace.is_multiline() {
            return None;
        }
        let percent = percent.wrap_in_whitespace(whitespace);

        let mut cases = vec![];
        loop {
            let Some((new_input, case)) = match_case(input, indentation + 1) else { break; };
            let (new_input, whitespace) =
                whitespaces_and_newlines(new_input, indentation + 1, true);
            input = new_input;
            let is_whitespace_multiline = whitespace.is_multiline();
            let case = case.wrap_in_whitespace(whitespace);
            cases.push(case);
            if !is_whitespace_multiline {
                break;
            }
        }
        if cases.is_empty() {
            cases.push(Rcst::Error {
                unparsable_input: input.to_string(),
                error: RcstError::MatchMissesCases,
            });
        }

        Some((input, percent, cases))
    }
    #[test]
    fn test_match_suffix() {
        assert_eq!(match_suffix("%", 0), None);
        // %
        //   1 -> 2
        // Foo
        assert_eq!(
            match_suffix("%\n  1 -> 2\nFoo", 0),
            Some((
                "\nFoo",
                Rcst::TrailingWhitespace {
                    child: Box::new(Rcst::Percent),
                    whitespace: vec![
                        Rcst::Newline("\n".to_string()),
                        Rcst::Whitespace("  ".to_string()),
                    ],
                },
                vec![Rcst::MatchCase {
                    pattern: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Int {
                            value: 1u8.into(),
                            string: "1".to_string(),
                        }),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }),
                    arrow: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Arrow),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }),
                    body: vec![Rcst::Int {
                        value: 2u8.into(),
                        string: "2".to_string(),
                    }],
                }],
            )),
        );
    }

    #[instrument(level = "trace")]
    fn match_case(input: &str, indentation: usize) -> Option<(&str, Rcst)> {
        let (input, pattern) = pattern(input, indentation)?;
        let (input, whitespace) = whitespaces_and_newlines(input, indentation, true);
        let pattern = pattern.wrap_in_whitespace(whitespace);

        let (input, arrow) = if let Some((input, arrow)) = arrow(input) {
            let (input, whitespace) = whitespaces_and_newlines(input, indentation, true);
            (input, arrow.wrap_in_whitespace(whitespace))
        } else {
            let error = Rcst::Error {
                unparsable_input: "".to_string(),
                error: RcstError::MatchCaseMissesArrow,
            };
            (input, error)
        };

        let (input, mut body) = body(input, indentation + 1);
        if body.is_empty() {
            body.push(Rcst::Error {
                unparsable_input: "".to_string(),
                error: RcstError::MatchCaseMissesBody,
            });
        }

        let case = Rcst::MatchCase {
            pattern: Box::new(pattern),
            arrow: Box::new(arrow),
            body,
        };
        Some((input, case))
    }

    #[instrument(level = "trace")]
    fn lambda(input: &str, indentation: usize) -> Option<(&str, Rcst)> {
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
                match expression(input, indentation + 1, false, false, false) {
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

        let (i, whitespace) = whitespaces_and_newlines(input, indentation + 1, true);
        if let Some((parameters, arrow)) = parameters_and_arrow {
            parameters_and_arrow = Some((parameters, arrow.wrap_in_whitespace(whitespace)));
        } else {
            opening_curly_brace = opening_curly_brace.wrap_in_whitespace(whitespace);
        }

        let (input, mut body, whitespace_before_closing_curly_brace, closing_curly_brace) = {
            let input_before_parsing_expression = i;
            let (i, body_expression) = match expression(i, indentation + 1, true, true, true) {
                Some((i, expression)) => (i, vec![expression]),
                None => (i, vec![]),
            };
            let (i, whitespace) = whitespaces_and_newlines(i, indentation + 1, true);
            if let Some((i, curly_brace)) = closing_curly_brace(i) {
                (i, body_expression, whitespace, curly_brace)
            } else {
                // There is no closing brace after a single expression. Thus,
                // we now try to parse a body of multiple expressions. We didn't
                // try this first because then the body would also have consumed
                // any trailing closing curly brace in the same line.
                // For example, for the lambda `{ 2 }`, the body parser would
                // have already consumed the `}`. The body parser works great
                // for multiline bodies, though.
                let (i, body) = body(input_before_parsing_expression, indentation + 1);
                let (i, whitespace) = whitespaces_and_newlines(i, indentation, true);
                let (i, curly_brace) = match closing_curly_brace(i) {
                    Some(it) => it,
                    None => (
                        i,
                        Rcst::Error {
                            unparsable_input: "".to_string(),
                            error: RcstError::CurlyBraceNotClosed,
                        },
                    ),
                };
                (i, body, whitespace, curly_brace)
            }
        };

        // Attach the `whitespace_before_closing_curly_brace`.
        if !body.is_empty() {
            let last = body.pop().unwrap();
            body.push(last.wrap_in_whitespace(whitespace_before_closing_curly_brace));
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
            Rcst::Lambda {
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
                Rcst::Lambda {
                    opening_curly_brace: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::OpeningCurlyBrace),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }),
                    parameters_and_arrow: None,
                    body: vec![Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Int {
                            value: 2u8.into(),
                            string: "2".to_string()
                        }),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }],
                    closing_curly_brace: Box::new(Rcst::ClosingCurlyBrace),
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
                Rcst::Lambda {
                    opening_curly_brace: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::OpeningCurlyBrace),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }),
                    parameters_and_arrow: Some((
                        vec![Rcst::TrailingWhitespace {
                            child: Box::new(Rcst::Identifier("a".to_string())),
                            whitespace: vec![Rcst::Whitespace(" ".to_string())],
                        },],
                        Box::new(Rcst::TrailingWhitespace {
                            child: Box::new(Rcst::Arrow),
                            whitespace: vec![
                                Rcst::Newline("\n".to_string()),
                                Rcst::Whitespace("  ".to_string())
                            ],
                        }),
                    )),
                    body: vec![Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Identifier("foo".to_string())),
                        whitespace: vec![Rcst::Newline("\n".to_string())],
                    }],
                    closing_curly_brace: Box::new(Rcst::ClosingCurlyBrace),
                }
            ))
        );
        // {
        // foo
        assert_eq!(
            lambda("{\nfoo", 0),
            Some((
                "foo",
                Rcst::Lambda {
                    opening_curly_brace: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::OpeningCurlyBrace),
                        whitespace: vec![Rcst::Newline("\n".to_string())],
                    }),
                    parameters_and_arrow: None,
                    body: vec![],
                    closing_curly_brace: Box::new(Rcst::Error {
                        unparsable_input: "".to_string(),
                        error: RcstError::CurlyBraceNotClosed
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
                Rcst::Lambda {
                    opening_curly_brace: Box::new(Rcst::OpeningCurlyBrace),
                    parameters_and_arrow: Some((vec![], Box::new(Rcst::Arrow))),
                    body: vec![],
                    closing_curly_brace: Box::new(Rcst::Error {
                        unparsable_input: "".to_string(),
                        error: RcstError::CurlyBraceNotClosed
                    }),
                }
            ))
        );
        // { foo
        //   bar
        // }
        assert_eq!(
            lambda("{ foo\n  bar\n}", 0),
            Some((
                "",
                Rcst::Lambda {
                    opening_curly_brace: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::OpeningCurlyBrace),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }),
                    parameters_and_arrow: None,
                    body: vec![
                        Rcst::Identifier("foo".to_string()),
                        Rcst::Newline("\n".to_string()),
                        Rcst::Whitespace("  ".to_string()),
                        Rcst::TrailingWhitespace {
                            child: Box::new(Rcst::Identifier("bar".to_string())),
                            whitespace: vec![Rcst::Newline("\n".to_string())],
                        }
                    ],
                    closing_curly_brace: Box::new(Rcst::ClosingCurlyBrace)
                }
            ))
        );
    }

    #[instrument(level = "trace")]
    fn assignment(input: &str, indentation: usize) -> Option<(&str, Rcst)> {
        let (input, mut signature) = run_of_expressions(input, indentation).or_else(|| {
            pattern(input, indentation).map(|(input, pattern)| (input, vec![pattern]))
        })?;

        let (input, whitespace) = whitespaces_and_newlines(input, indentation + 1, true);
        let last = signature.pop().unwrap();
        signature.push(last.wrap_in_whitespace(whitespace.clone()));

        let parameters = signature.split_off(1);
        let name_or_pattern = signature.into_iter().next().unwrap();

        let (input, mut assignment_sign) =
            colon_equals_sign(input).or_else(|| equals_sign(input))?;
        let original_assignment_sign = assignment_sign.clone();
        let input_after_assignment_sign = input;

        let (input, more_whitespace) = whitespaces_and_newlines(input, indentation + 1, false);
        assignment_sign = assignment_sign.wrap_in_whitespace(more_whitespace.clone());

        let is_multiline = name_or_pattern.is_multiline()
            || parameters.is_multiline()
            || whitespace.is_multiline()
            || more_whitespace.is_multiline();
        let (input, assignment_sign, body) = if is_multiline {
            let (input, body) = body(input, indentation + 1);
            if body.is_empty() {
                (
                    input_after_assignment_sign,
                    original_assignment_sign,
                    vec![],
                )
            } else {
                (input, assignment_sign, body)
            }
        } else {
            match comment(input).or_else(|| expression(input, indentation, false, true, true)) {
                Some((input, expression)) => (input, assignment_sign, vec![expression]),
                None => (
                    input_after_assignment_sign,
                    original_assignment_sign,
                    vec![],
                ),
            }
        };

        let (whitespace, (assignment_sign, body)) =
            (assignment_sign, body).split_outer_trailing_whitespace();
        Some((
            input,
            Rcst::Assignment {
                name_or_pattern: Box::new(name_or_pattern),
                parameters,
                assignment_sign: Box::new(assignment_sign),
                body,
            }
            .wrap_in_whitespace(whitespace),
        ))
    }
    #[test]
    fn test_assignment() {
        assert_eq!(
            assignment("foo = 42", 0),
            Some((
                "",
                Rcst::Assignment {
                    name_or_pattern: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Identifier("foo".to_string())),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }),
                    parameters: vec![],
                    assignment_sign: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::EqualsSign),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }),
                    body: vec![Rcst::Int {
                        value: 42u8.into(),
                        string: "42".to_string(),
                    }],
                },
            )),
        );
        assert_eq!(assignment("foo 42", 0), None);
        // foo bar =
        //   3
        // 2
        assert_eq!(
            assignment("foo bar =\n  3\n2", 0),
            Some((
                "\n2",
                Rcst::Assignment {
                    name_or_pattern: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Identifier("foo".to_string())),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }),
                    parameters: vec![Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Identifier("bar".to_string())),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }],
                    assignment_sign: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::EqualsSign),
                        whitespace: vec![
                            Rcst::Newline("\n".to_string()),
                            Rcst::Whitespace("  ".to_string())
                        ],
                    }),
                    body: vec![Rcst::Int {
                        value: 3u8.into(),
                        string: "3".to_string(),
                    }],
                },
            )),
        );
        // foo
        //   bar
        //   = 3
        assert_eq!(
            assignment("foo\n  bar\n  = 3", 0),
            Some((
                "",
                Rcst::Assignment {
                    name_or_pattern: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Identifier("foo".to_string())),
                        whitespace: vec![
                            Rcst::Newline("\n".to_string()),
                            Rcst::Whitespace("  ".to_string()),
                        ],
                    }),
                    parameters: vec![Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Identifier("bar".to_string())),
                        whitespace: vec![
                            Rcst::Newline("\n".to_string()),
                            Rcst::Whitespace("  ".to_string()),
                        ],
                    }],
                    assignment_sign: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::EqualsSign),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }),
                    body: vec![Rcst::Int {
                        value: 3u8.into(),
                        string: "3".to_string(),
                    }],
                },
            )),
        );
        assert_eq!(
            assignment("foo =\n  ", 0),
            Some((
                "\n  ",
                Rcst::Assignment {
                    name_or_pattern: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Identifier("foo".to_string())),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }),
                    parameters: vec![],
                    assignment_sign: Box::new(Rcst::EqualsSign),
                    body: vec![],
                },
            )),
        );
        assert_eq!(
            assignment("foo = # comment\n", 0),
            Some((
                "\n",
                Rcst::Assignment {
                    name_or_pattern: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Identifier("foo".to_string())),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }),
                    parameters: vec![],
                    assignment_sign: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::EqualsSign),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }),
                    body: vec![Rcst::Comment {
                        octothorpe: Box::new(Rcst::Octothorpe),
                        comment: " comment".to_string(),
                    }],
                }
            ))
        );
        // foo =
        //   # comment
        // 3
        assert_eq!(
            assignment("foo =\n  # comment\n3", 0),
            Some((
                "\n3",
                Rcst::Assignment {
                    name_or_pattern: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Identifier("foo".to_string())),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }),
                    parameters: vec![],
                    assignment_sign: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::EqualsSign),
                        whitespace: vec![
                            Rcst::Newline("\n".to_string()),
                            Rcst::Whitespace("  ".to_string()),
                        ],
                    }),
                    body: vec![Rcst::Comment {
                        octothorpe: Box::new(Rcst::Octothorpe),
                        comment: " comment".to_string(),
                    }],
                },
            )),
        );
        // foo =
        //   # comment
        //   5
        // 3
        assert_eq!(
            assignment("foo =\n  # comment\n  5\n3", 0),
            Some((
                "\n3",
                Rcst::Assignment {
                    name_or_pattern: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Identifier("foo".to_string())),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }),
                    parameters: vec![],
                    assignment_sign: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::EqualsSign),
                        whitespace: vec![
                            Rcst::Newline("\n".to_string()),
                            Rcst::Whitespace("  ".to_string()),
                        ],
                    }),
                    body: vec![
                        Rcst::Comment {
                            octothorpe: Box::new(Rcst::Octothorpe),
                            comment: " comment".to_string(),
                        },
                        Rcst::Newline("\n".to_string()),
                        Rcst::Whitespace("  ".to_string()),
                        Rcst::Int {
                            value: 5u8.into(),
                            string: "5".to_string(),
                        },
                    ],
                },
            )),
        );
        assert_eq!(
            assignment("(foo, bar) = (1, 2)", 0),
            Some((
                "",
                Rcst::Assignment {
                    name_or_pattern: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::List {
                            opening_parenthesis: Box::new(Rcst::OpeningParenthesis),
                            items: vec![
                                Rcst::TrailingWhitespace {
                                    child: Box::new(Rcst::ListItem {
                                        value: Box::new(Rcst::Identifier("foo".to_string())),
                                        comma: Some(Box::new(Rcst::Comma)),
                                    }),
                                    whitespace: vec![Rcst::Whitespace(" ".to_string())],
                                },
                                Rcst::ListItem {
                                    value: Box::new(Rcst::Identifier("bar".to_string())),
                                    comma: None,
                                },
                            ],
                            closing_parenthesis: Box::new(Rcst::ClosingParenthesis),
                        }),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }),
                    parameters: vec![],
                    assignment_sign: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::EqualsSign),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }),
                    body: vec![Rcst::List {
                        opening_parenthesis: Box::new(Rcst::OpeningParenthesis),
                        items: vec![
                            Rcst::TrailingWhitespace {
                                child: Box::new(Rcst::ListItem {
                                    value: Box::new(Rcst::Int {
                                        value: 1u8.into(),
                                        string: "1".to_string(),
                                    }),
                                    comma: Some(Box::new(Rcst::Comma)),
                                }),
                                whitespace: vec![Rcst::Whitespace(" ".to_string())],
                            },
                            Rcst::ListItem {
                                value: Box::new(Rcst::Int {
                                    value: 2u8.into(),
                                    string: "2".to_string(),
                                }),
                                comma: None,
                            },
                        ],
                        closing_parenthesis: Box::new(Rcst::ClosingParenthesis),
                    }],
                },
            )),
        );
        assert_eq!(
            assignment("[Foo: foo] = bar", 0),
            Some((
                "",
                Rcst::Assignment {
                    name_or_pattern: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Struct {
                            opening_bracket: Box::new(Rcst::OpeningBracket),
                            fields: vec![Rcst::StructField {
                                key_and_colon: Some(Box::new((
                                    Rcst::Symbol("Foo".to_string()),
                                    Rcst::TrailingWhitespace {
                                        child: Box::new(Rcst::Colon),
                                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                                    },
                                ))),
                                value: Box::new(Rcst::Identifier("foo".to_string())),
                                comma: None,
                            }],
                            closing_bracket: Box::new(Rcst::ClosingBracket),
                        }),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }),
                    parameters: vec![],
                    assignment_sign: Box::new(Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::EqualsSign),
                        whitespace: vec![Rcst::Whitespace(" ".to_string())],
                    }),
                    body: vec![Rcst::Identifier("bar".to_string())],
                },
            )),
        );
    }
}
