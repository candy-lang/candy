use crate::{
    cst::{CstError, CstKind},
    module::{Module, ModuleDb, ModuleKind, Package},
    rcst::Rcst,
    rich_ir::{RichIrBuilder, ToRichIr, TokenType},
};
use enumset::EnumSet;
use std::{str, sync::Arc};

#[salsa::query_group(StringToRcstStorage)]
pub trait StringToRcst: ModuleDb {
    fn rcst(&self, module: Module) -> RcstResult;
}

pub type RcstResult = Result<Arc<Vec<Rcst>>, ModuleError>;

fn rcst(db: &dyn StringToRcst, module: Module) -> RcstResult {
    if module.kind != ModuleKind::Code {
        return Err(ModuleError::IsNotCandy);
    }

    if let Package::Tooling(_) = &module.package {
        return Err(ModuleError::IsToolingModule);
    }
    let source = db
        .get_module_content(module)
        .ok_or(ModuleError::DoesNotExist)?;
    let source = match str::from_utf8(source.as_slice()) {
        Ok(source) => source,
        Err(_) => {
            return Err(ModuleError::InvalidUtf8);
        }
    };
    Ok(Arc::new(parse_rcst(source)))
}
#[must_use]
pub fn parse_rcst(source: &str) -> Vec<Rcst> {
    let (mut rest, mut rcsts) = parse::body(source, 0);
    if !rest.is_empty() {
        let trailing_newline = if rest.len() >= 2
                && let Some((newline_rest, newline)) = parse::newline(&rest[rest.len() - 2..])
                && newline_rest.is_empty() {
            rest = &rest[..rest.len() - 2];
            Some(newline)
        } else if let Some((_, newline)) = parse::newline(&rest[rest.len() - 1..]) {
            rest = &rest[..rest.len() - 1];
            Some(newline)
        } else {
            None
        };
        rcsts.push(
            CstKind::Error {
                unparsable_input: rest.to_string(),
                error: CstError::UnparsedRest,
            }
            .into(),
        );
        if let Some(trailing_newline) = trailing_newline {
            rcsts.push(trailing_newline);
        }
    }
    rcsts
}

#[derive(PartialEq, Eq, Debug, Clone, Copy, Hash)]
pub enum ModuleError {
    DoesNotExist,
    InvalidUtf8,
    IsNotCandy,
    IsToolingModule,
}
impl ToRichIr for ModuleError {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        let text = match self {
            ModuleError::DoesNotExist => return,
            ModuleError::InvalidUtf8 => "# Invalid UTF-8",
            ModuleError::IsNotCandy => "# Is not Candy code",
            ModuleError::IsToolingModule => "# Is a tooling module",
        };
        builder.push(text, TokenType::Comment, EnumSet::empty());
    }
}

impl CstKind<()> {
    #[must_use]
    fn wrap_in_whitespace(self, whitespace: Vec<Rcst>) -> Rcst {
        Rcst::from(self).wrap_in_whitespace(whitespace)
    }
}
impl Rcst {
    #[must_use]
    fn wrap_in_whitespace(mut self, mut whitespace: Vec<Rcst>) -> Rcst {
        if whitespace.is_empty() {
            return self;
        }

        if let CstKind::TrailingWhitespace {
            whitespace: self_whitespace,
            ..
        } = &mut self.kind
        {
            self_whitespace.append(&mut whitespace);
            self
        } else {
            CstKind::TrailingWhitespace {
                child: Box::new(self),
                whitespace,
            }
            .into()
        }
    }
}

#[must_use]
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

    use crate::{
        cst::{CstError, CstKind, IsMultiline},
        rcst::{Rcst, SplitOuterTrailingWhitespace},
    };
    use itertools::Itertools;
    use tracing::instrument;

    use super::whitespace_indentation_score;

    static MEANINGFUL_PUNCTUATION: &str = r#"=,.:|()[]{}->'"%#"#;
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
        literal(input, "=").map(|it| (it, CstKind::EqualsSign.into()))
    }
    #[instrument(level = "trace")]
    fn comma(input: &str) -> Option<(&str, Rcst)> {
        literal(input, ",").map(|it| (it, CstKind::Comma.into()))
    }
    #[instrument(level = "trace")]
    fn dot(input: &str) -> Option<(&str, Rcst)> {
        literal(input, ".").map(|it| (it, CstKind::Dot.into()))
    }
    #[instrument(level = "trace")]
    fn colon(input: &str) -> Option<(&str, Rcst)> {
        literal(input, ":").map(|it| (it, CstKind::Colon.into()))
    }
    #[instrument(level = "trace")]
    fn colon_equals_sign(input: &str) -> Option<(&str, Rcst)> {
        literal(input, ":=").map(|it| (it, CstKind::ColonEqualsSign.into()))
    }
    #[instrument(level = "trace")]
    fn bar(input: &str) -> Option<(&str, Rcst)> {
        literal(input, "|").map(|it| (it, CstKind::Bar.into()))
    }
    #[instrument(level = "trace")]
    fn opening_bracket(input: &str) -> Option<(&str, Rcst)> {
        literal(input, "[").map(|it| (it, CstKind::OpeningBracket.into()))
    }
    #[instrument(level = "trace")]
    fn closing_bracket(input: &str) -> Option<(&str, Rcst)> {
        literal(input, "]").map(|it| (it, CstKind::ClosingBracket.into()))
    }
    #[instrument(level = "trace")]
    fn opening_parenthesis(input: &str) -> Option<(&str, Rcst)> {
        literal(input, "(").map(|it| (it, CstKind::OpeningParenthesis.into()))
    }
    #[instrument(level = "trace")]
    fn closing_parenthesis(input: &str) -> Option<(&str, Rcst)> {
        literal(input, ")").map(|it| (it, CstKind::ClosingParenthesis.into()))
    }
    #[instrument(level = "trace")]
    fn opening_curly_brace(input: &str) -> Option<(&str, Rcst)> {
        literal(input, "{").map(|it| (it, CstKind::OpeningCurlyBrace.into()))
    }
    #[instrument(level = "trace")]
    fn closing_curly_brace(input: &str) -> Option<(&str, Rcst)> {
        literal(input, "}").map(|it| (it, CstKind::ClosingCurlyBrace.into()))
    }
    #[instrument(level = "trace")]
    fn arrow(input: &str) -> Option<(&str, Rcst)> {
        literal(input, "->").map(|it| (it, CstKind::Arrow.into()))
    }
    #[instrument(level = "trace")]
    fn single_quote(input: &str) -> Option<(&str, Rcst)> {
        literal(input, "'").map(|it| (it, CstKind::SingleQuote.into()))
    }
    #[instrument(level = "trace")]
    fn double_quote(input: &str) -> Option<(&str, Rcst)> {
        literal(input, "\"").map(|it| (it, CstKind::DoubleQuote.into()))
    }
    #[instrument(level = "trace")]
    fn percent(input: &str) -> Option<(&str, Rcst)> {
        literal(input, "%").map(|it| (it, CstKind::Percent.into()))
    }
    #[instrument(level = "trace")]
    fn octothorpe(input: &str) -> Option<(&str, Rcst)> {
        literal(input, "#").map(|it| (it, CstKind::Octothorpe.into()))
    }
    #[instrument(level = "trace")]
    pub(super) fn newline(input: &str) -> Option<(&str, Rcst)> {
        let newlines = vec!["\n", "\r\n"];
        for newline in newlines {
            if let Some(input) = literal(input, newline) {
                return Some((input, CstKind::Newline(newline.to_string()).into()));
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
        assert_eq!(word("foo#abc"), Some(("#abc", "foo".to_string())));
    }

    #[instrument(level = "trace")]
    fn identifier(input: &str) -> Option<(&str, Rcst)> {
        let (input, w) = word(input)?;
        if w == "✨" {
            return Some((input, CstKind::Identifier(w).into()));
        }
        let next_character = w.chars().next().unwrap();
        if !next_character.is_lowercase() && next_character != '_' {
            return None;
        }
        if w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            Some((input, CstKind::Identifier(w).into()))
        } else {
            Some((
                input,
                CstKind::Error {
                    unparsable_input: w,
                    error: CstError::IdentifierContainsNonAlphanumericAscii,
                }
                .into(),
            ))
        }
    }
    #[test]
    fn test_identifier() {
        assert_eq!(
            identifier("foo bar"),
            Some((" bar", build_identifier("foo")))
        );
        assert_eq!(identifier("_"), Some(("", build_identifier("_"))));
        assert_eq!(identifier("_foo"), Some(("", build_identifier("_foo"))));
        assert_eq!(identifier("Foo bar"), None);
        assert_eq!(identifier("012 bar"), None);
        assert_eq!(
            identifier("f12🔥 bar"),
            Some((
                " bar",
                CstKind::Error {
                    unparsable_input: "f12🔥".to_string(),
                    error: CstError::IdentifierContainsNonAlphanumericAscii,
                }
                .into(),
            )),
        );
    }

    #[instrument(level = "trace")]
    fn symbol(input: &str) -> Option<(&str, Rcst)> {
        let (input, w) = word(input)?;
        if !w.chars().next().unwrap().is_uppercase() {
            return None;
        }
        if w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            Some((input, CstKind::Symbol(w).into()))
        } else {
            Some((
                input,
                CstKind::Error {
                    unparsable_input: w,
                    error: CstError::SymbolContainsNonAlphanumericAscii,
                }
                .into(),
            ))
        }
    }
    #[test]
    fn test_symbol() {
        assert_eq!(symbol("Foo b"), Some((" b", build_symbol("Foo"))));
        assert_eq!(symbol("Foo_Bar"), Some(("", build_symbol("Foo_Bar"))));
        assert_eq!(symbol("foo bar"), None);
        assert_eq!(symbol("012 bar"), None);
        assert_eq!(
            symbol("F12🔥 bar"),
            Some((
                " bar",
                CstKind::Error {
                    unparsable_input: "F12🔥".to_string(),
                    error: CstError::SymbolContainsNonAlphanumericAscii,
                }
                .into()
            )),
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
            Some((input, CstKind::Int { value, string: w }.into()))
        } else {
            Some((
                input,
                CstKind::Error {
                    unparsable_input: w,
                    error: CstError::IntContainsNonDigits,
                }
                .into(),
            ))
        }
    }
    #[test]
    fn test_int() {
        assert_eq!(int("42 "), Some((" ", build_simple_int(42))));
        assert_eq!(
            int("012"),
            Some((
                "",
                CstKind::Int {
                    value: 12u8.into(),
                    string: "012".to_string()
                }
                .into(),
            )),
        );
        assert_eq!(int("123 years"), Some((" years", build_simple_int(123))));
        assert_eq!(int("foo"), None);
        assert_eq!(
            int("3D"),
            Some((
                "",
                CstKind::Error {
                    unparsable_input: "3D".to_string(),
                    error: CstError::IntContainsNonDigits,
                }
                .into(),
            )),
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
                CstKind::Error {
                    unparsable_input: whitespace,
                    error: CstError::WeirdWhitespace,
                }
                .into(),
            ))
        } else if !whitespace.is_empty() {
            Some((input, CstKind::Whitespace(whitespace).into()))
        } else {
            None
        }
    }
    #[test]
    fn test_single_line_whitespace() {
        assert_eq!(
            single_line_whitespace("  \nfoo"),
            Some(("\nfoo", CstKind::Whitespace("  ".to_string()).into())),
        );
    }

    #[instrument(level = "trace")]
    fn comment(input: &str) -> Option<(&str, Rcst)> {
        let (mut input, octothorpe) = octothorpe(input)?;
        let mut comment = vec![];
        loop {
            match input.chars().next() {
                Some('\n' | '\r') | None => break,
                Some(c) => {
                    comment.push(c);
                    input = &input[c.len_utf8()..];
                }
            }
        }
        Some((
            input,
            CstKind::Comment {
                octothorpe: Box::new(octothorpe),
                comment: comment.into_iter().join(""),
            }
            .into(),
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
                CstKind::Error {
                    unparsable_input: whitespace,
                    error: CstError::WeirdWhitespaceInIndentation,
                }
                .into()
            } else {
                CstKind::Whitespace(whitespace).into()
            },
        ))
    }
    #[test]
    fn test_leading_indentation() {
        assert_eq!(
            leading_indentation("foo", 0),
            Some(("foo", CstKind::Whitespace(String::new()).into())),
        );
        assert_eq!(
            leading_indentation("  foo", 1),
            Some(("foo", CstKind::Whitespace("  ".to_string()).into())),
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
        let mut is_sufficiently_indented = true;
        loop {
            let new_input_from_iteration_start = new_input;

            if also_comments
                && is_sufficiently_indented
                && let Some((new_new_input, whitespace)) = comment(new_input)
            {
                new_input = new_new_input;
                new_parts.push(whitespace);

                input = new_input;
                parts.append(&mut new_parts);
            }

            if let Some((new_new_input, newline)) = newline(new_input) {
                new_input = new_new_input;
                new_parts.push(newline);
                is_sufficiently_indented = false;
            }

            if let Some((new_new_input, whitespace)) = leading_indentation(new_input, indentation) {
                new_input = new_new_input;
                new_parts.push(whitespace);

                input = new_input;
                parts.append(&mut new_parts);
                is_sufficiently_indented = true;
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
                if let CstKind::Whitespace(ws) = &it.kind {
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
            ("foo", vec![build_newline()]),
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
                    build_newline(),
                    CstKind::Whitespace("  ".to_string()).into(),
                ],
            ),
        );
        assert_eq!(
            whitespaces_and_newlines("\n  foo", 0, true),
            ("  foo", vec![build_newline()]),
        );
        assert_eq!(
            whitespaces_and_newlines(" \n  foo", 0, true),
            ("  foo", vec![build_space(), build_newline()]),
        );
        assert_eq!(
            whitespaces_and_newlines("\n  foo", 2, true),
            ("\n  foo", vec![]),
        );
        assert_eq!(
            whitespaces_and_newlines("\tfoo", 1, true),
            (
                "foo",
                vec![CstKind::Error {
                    unparsable_input: "\t".to_string(),
                    error: CstError::WeirdWhitespace,
                }
                .into()],
            ),
        );
        assert_eq!(
            whitespaces_and_newlines("# hey\n  foo", 1, true),
            (
                "foo",
                vec![
                    build_comment(" hey"),
                    build_newline(),
                    CstKind::Whitespace("  ".to_string()).into(),
                ],
            )
        );
        assert_eq!(
            whitespaces_and_newlines("# foo\n\n  #bar\n", 1, true),
            (
                "\n",
                vec![
                    build_comment(" foo"),
                    build_newline(),
                    build_newline(),
                    CstKind::Whitespace("  ".to_string()).into(),
                    build_comment("bar"),
                ],
            ),
        );
        assert_eq!(
            whitespaces_and_newlines(" # abc\n", 1, true),
            ("\n", vec![build_space(), build_comment(" abc")]),
        );
        assert_eq!(
            whitespaces_and_newlines("\n# abc\n", 1, true),
            ("\n# abc\n", vec![]),
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
                CstKind::Error {
                    unparsable_input: String::new(),
                    error: CstError::TextInterpolationMissesExpression,
                }
                .into(),
            ));

        let (input, whitespace) = whitespaces_and_newlines(input, indentation + 1, false);
        expression = expression.wrap_in_whitespace(whitespace);

        let (input, closing_curly_braces) =
            parse_multiple(input, closing_curly_brace, Some((curly_brace_count, false))).unwrap_or(
                (
                    input,
                    vec![CstKind::Error {
                        unparsable_input: String::new(),
                        error: CstError::TextInterpolationNotClosed,
                    }
                    .into()],
                ),
            );

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

    // TODO: It might be a good idea to ignore text interpolations in patterns
    #[instrument(level = "trace")]
    fn text(input: &str, indentation: usize) -> Option<(&str, Rcst)> {
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
                    let (i, mut whitespace) =
                        whitespaces_and_newlines(input, indentation + 1, false);
                    input = i;
                    parts.append(&mut whitespace);
                    if let Some('\n') = input.chars().next() {
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

    #[instrument(level = "trace")]
    fn expression(
        input: &str,
        indentation: usize,
        allow_assignment: bool,
        allow_call: bool,
        allow_bar: bool,
    ) -> Option<(&str, Rcst)> {
        // If we start the call list with `if … else …`, the formatting looks
        // weird. Hence, we start with a single `None`.
        let (mut input, mut result) = None
            .or_else(|| int(input))
            .or_else(|| text(input, indentation))
            .or_else(|| symbol(input))
            .or_else(|| list(input, indentation))
            .or_else(|| struct_(input, indentation))
            .or_else(|| parenthesized(input, indentation))
            .or_else(|| function(input, indentation))
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
            let mut did_make_progress = false;

            #[allow(clippy::items_after_statements)]
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

            did_make_progress |= parse_suffix(
                &mut input,
                indentation,
                &mut result,
                expression_suffix_struct_access,
            );

            if allow_call {
                did_make_progress |=
                    parse_suffix(&mut input, indentation, &mut result, expression_suffix_call);
            }
            if allow_bar {
                did_make_progress |=
                    parse_suffix(&mut input, indentation, &mut result, expression_suffix_bar);
                did_make_progress |= parse_suffix(
                    &mut input,
                    indentation,
                    &mut result,
                    expression_suffix_match,
                );
            }

            if allow_assignment {
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
        let (input, whitespace_after_struct) =
            whitespaces_and_newlines(input, indentation + 1, true);

        let (input, dot) = dot(input)?;
        let (new_input, whitespace_after_dot) =
            whitespaces_and_newlines(input, indentation + 1, true);
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

            let argument = expression(
                i,
                indentation,
                false,
                has_multiline_whitespace,
                has_multiline_whitespace,
            );
            let (i, expr) = if let Some(it) = argument {
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
        let (input, call) =
            expression(input, indentation, false, true, false).unwrap_or_else(|| {
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
        let (input, mut assignment_sign) =
            colon_equals_sign(input).or_else(|| equals_sign(input))?;

        // By now, it's clear that we are in an assignment, so we can do more
        // expensive operations. We also save some state in case the assignment
        // is invalid (so we can stop parsing right after the assignment sign).
        let left = left.clone().wrap_in_whitespace(whitespace_after_left);
        let just_the_assignment_sign = assignment_sign.clone();
        let input_after_assignment_sign = input;

        let (input, more_whitespace) = whitespaces_and_newlines(input, indentation + 1, false);
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
            if let Some((new_input, expression)) = expression(input, indentation, false, true, true)
            {
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
    #[test]
    fn test_expression() {
        assert_eq!(
            expression("foo", 0, true, true, true),
            Some(("", build_identifier("foo")))
        );
        assert_eq!(
            expression("(foo Bar)", 0, false, false, true),
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
            expression("foo\n  .bar", 0, true, true, true),
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
            expression("foo\n.bar", 0, true, true, true),
            Some(("\n.bar", build_identifier("foo"))),
        );
        // foo
        // | bar
        assert_eq!(
            expression("foo\n| bar", 0, true, true, true),
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
            expression("foo\n| bar baz", 0, true, true, true),
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
            expression("foo %\n  123 -> 123", 0, true, true, true),
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
            expression("(0, foo) | (foo, 0)", 0, false, true, true),
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
            expression("foo bar", 0, false, true, true),
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
            expression("Foo 4 bar", 0, false, true, true),
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
            expression("foo\n  bar\n  baz\n2", 0, false, true, true),
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
            expression("foo 1 2\n  3\n  4\nbar", 0, false, true, true),
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
            expression("foo\n  bar | baz", 0, true, true, true),
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
            expression("(foo Bar) Baz\n", 0, false, true, true),
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
            expression("foo T\n\n\nbar = 5", 0, false, true, true),
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
            expression("foo = 42", 0, true, true, true),
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
            expression("foo =\n  bar\n\nbaz", 0, true, true, true),
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
            expression("foo 42", 0, true, true, true),
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
            expression("foo %", 0, false, false, true),
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
            expression("foo %\n", 0, false, false, true),
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
            expression("foo %\n  1 -> 2\nFoo", 0, false, false, true),
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
            expression("foo bar =\n  3\n2", 0, true, true, true),
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
            expression("main := { environment ->\n  input\n}", 0, true, true, true),
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
            expression("foo\n  bar\n  = 3", 0, true, true, true),
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
            expression("foo =\n  ", 0, true, true, true),
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
            expression("foo = # comment\n", 0, true, true, true),
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
            expression("foo = bar # comment\n", 0, true, true, true),
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
            expression("foo =\n  # comment\n3", 0, true, true, true),
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
            expression("foo =\n  # comment\n  5\n3", 0, true, true, true),
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
            expression("(foo, bar) = (1, 2)", 0, true, true, true),
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
            expression("[Foo: foo] = bar", 0, true, true, true),
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

    #[instrument(level = "trace")]
    fn list(input: &str, indentation: usize) -> Option<(&str, Rcst)> {
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
            let Some((input, comma)) = comma(input) else {
                break 'handleEmptyList;
            };

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
                CstKind::List {
                    opening_parenthesis: Box::new(opening_parenthesis),
                    items: vec![comma],
                    closing_parenthesis: Box::new(closing_parenthesis),
                }
                .into(),
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
            input = new_input;

            // Value.
            let (new_input, value, has_value) =
                match expression(new_input, items_indentation, false, true, true) {
                    Some((new_input, value)) => (new_input, value, true),
                    None => (
                        new_input,
                        CstKind::Error {
                            unparsable_input: String::new(),
                            error: CstError::ListItemMissesValue,
                        }
                        .into(),
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
            items.push(
                CstKind::ListItem {
                    value: Box::new(value),
                    comma: comma.map(Box::new),
                }
                .into(),
            );
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
                CstKind::Error {
                    unparsable_input: String::new(),
                    error: CstError::ListNotClosed,
                }
                .into(),
            ),
        };

        Some((
            input,
            CstKind::List {
                opening_parenthesis: Box::new(opening_parenthesis),
                items,
                closing_parenthesis: Box::new(closing_parenthesis),
            }
            .into(),
        ))
    }
    #[test]
    fn test_list() {
        assert_eq!(list("hello", 0), None);
        assert_eq!(list("()", 0), None);
        assert_eq!(
            list("(,)", 0),
            Some((
                "",
                CstKind::List {
                    opening_parenthesis: Box::new(CstKind::OpeningParenthesis.into()),
                    items: vec![CstKind::Comma.into()],
                    closing_parenthesis: Box::new(CstKind::ClosingParenthesis.into()),
                }
                .into(),
            )),
        );
        assert_eq!(list("(foo)", 0), None);
        assert_eq!(
            list("(foo,)", 0),
            Some((
                "",
                CstKind::List {
                    opening_parenthesis: Box::new(CstKind::OpeningParenthesis.into()),
                    items: vec![CstKind::ListItem {
                        value: Box::new(build_identifier("foo")),
                        comma: Some(Box::new(CstKind::Comma.into())),
                    }
                    .into()],
                    closing_parenthesis: Box::new(CstKind::ClosingParenthesis.into()),
                }
                .into(),
            )),
        );
        assert_eq!(
            list("(foo, )", 0),
            Some((
                "",
                CstKind::List {
                    opening_parenthesis: Box::new(CstKind::OpeningParenthesis.into()),
                    items: vec![CstKind::ListItem {
                        value: Box::new(build_identifier("foo")),
                        comma: Some(Box::new(CstKind::Comma.into())),
                    }
                    .with_trailing_space()],
                    closing_parenthesis: Box::new(CstKind::ClosingParenthesis.into()),
                }
                .into(),
            )),
        );
        assert_eq!(
            list("(foo,bar)", 0),
            Some((
                "",
                CstKind::List {
                    opening_parenthesis: Box::new(CstKind::OpeningParenthesis.into()),
                    items: vec![
                        CstKind::ListItem {
                            value: Box::new(build_identifier("foo")),
                            comma: Some(Box::new(CstKind::Comma.into())),
                        }
                        .into(),
                        CstKind::ListItem {
                            value: Box::new(build_identifier("bar")),
                            comma: None,
                        }
                        .into(),
                    ],
                    closing_parenthesis: Box::new(CstKind::ClosingParenthesis.into()),
                }
                .into(),
            )),
        );
        // (
        //   foo,
        //   4,
        //   "Hi",
        // )
        assert_eq!(
            list("(\n  foo,\n  4,\n  \"Hi\",\n)", 0),
            Some((
                "",
                CstKind::List {
                    opening_parenthesis: Box::new(
                        CstKind::OpeningParenthesis.with_trailing_whitespace(vec![
                            CstKind::Newline("\n".to_string()),
                            CstKind::Whitespace("  ".to_string()),
                        ]),
                    ),
                    items: vec![
                        CstKind::ListItem {
                            value: Box::new(build_identifier("foo")),
                            comma: Some(Box::new(CstKind::Comma.into())),
                        }
                        .with_trailing_whitespace(vec![
                            CstKind::Newline("\n".to_string()),
                            CstKind::Whitespace("  ".to_string())
                        ]),
                        CstKind::ListItem {
                            value: Box::new(build_simple_int(4)),
                            comma: Some(Box::new(CstKind::Comma.into())),
                        }
                        .with_trailing_whitespace(vec![
                            CstKind::Newline("\n".to_string()),
                            CstKind::Whitespace("  ".to_string())
                        ]),
                        CstKind::ListItem {
                            value: Box::new(build_simple_text("Hi")),
                            comma: Some(Box::new(CstKind::Comma.into()))
                        }
                        .with_trailing_whitespace(vec![CstKind::Newline("\n".to_string())]),
                    ],
                    closing_parenthesis: Box::new(CstKind::ClosingParenthesis.into()),
                }
                .into(),
            )),
        );
    }

    #[instrument(level = "trace")]
    fn struct_(input: &str, indentation: usize) -> Option<(&str, Rcst)> {
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
            outer_input = input;

            // The key if it's explicit or the value when using a shorthand.
            let (input, key_or_value) =
                match expression(input, fields_indentation, false, true, true) {
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
                    CstKind::Error {
                        unparsable_input: String::new(),
                        error: CstError::StructFieldMissesColon,
                    }
                    .into(),
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
            let (input, value, has_value) =
                match expression(input, fields_indentation + 1, false, true, true) {
                    Some((input, value)) => (input, value, true),
                    None => (
                        input,
                        CstKind::Error {
                            unparsable_input: String::new(),
                            error: CstError::StructFieldMissesValue,
                        }
                        .into(),
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
            let key_or_value = key_or_value.unwrap_or_else(|| {
                CstKind::Error {
                    unparsable_input: String::new(),
                    error: if is_using_shorthand {
                        CstError::StructFieldMissesValue
                    } else {
                        CstError::StructFieldMissesKey
                    },
                }
                .into()
            });
            let key_or_value = key_or_value.wrap_in_whitespace(key_or_value_whitespace);

            outer_input = input;
            let comma = comma.map(Box::new);
            let field = if is_using_shorthand {
                CstKind::StructField {
                    key_and_colon: None,
                    value: Box::new(key_or_value),
                    comma,
                }
            } else {
                CstKind::StructField {
                    key_and_colon: Some(Box::new((key_or_value, colon))),
                    value: Box::new(value),
                    comma,
                }
            };
            fields.push(field.into());
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
                CstKind::Error {
                    unparsable_input: String::new(),
                    error: CstError::StructNotClosed,
                }
                .into(),
            ),
        };

        Some((
            input,
            CstKind::Struct {
                opening_bracket: Box::new(opening_bracket),
                fields,
                closing_bracket: Box::new(closing_bracket),
            }
            .into(),
        ))
    }
    #[test]
    fn test_struct() {
        assert_eq!(struct_("hello", 0), None);
        assert_eq!(
            struct_("[]", 0),
            Some((
                "",
                CstKind::Struct {
                    opening_bracket: Box::new(CstKind::OpeningBracket.into()),
                    fields: vec![],
                    closing_bracket: Box::new(CstKind::ClosingBracket.into()),
                }
                .into(),
            )),
        );
        assert_eq!(
            struct_("[ ]", 0),
            Some((
                "",
                CstKind::Struct {
                    opening_bracket: Box::new(CstKind::OpeningBracket.with_trailing_space()),
                    fields: vec![],
                    closing_bracket: Box::new(CstKind::ClosingBracket.into()),
                }
                .into(),
            )),
        );
        assert_eq!(
            struct_("[foo:bar]", 0),
            Some((
                "",
                CstKind::Struct {
                    opening_bracket: Box::new(CstKind::OpeningBracket.into()),
                    fields: vec![CstKind::StructField {
                        key_and_colon: Some(Box::new((
                            build_identifier("foo"),
                            CstKind::Colon.into(),
                        ))),
                        value: Box::new(build_identifier("bar")),
                        comma: None,
                    }
                    .into()],
                    closing_bracket: Box::new(CstKind::ClosingBracket.into()),
                }
                .into(),
            )),
        );
        assert_eq!(
            struct_("[foo,bar:baz]", 0),
            Some((
                "",
                CstKind::Struct {
                    opening_bracket: Box::new(CstKind::OpeningBracket.into()),
                    fields: vec![
                        CstKind::StructField {
                            key_and_colon: None,
                            value: Box::new(build_identifier("foo")),
                            comma: Some(Box::new(CstKind::Comma.into())),
                        }
                        .into(),
                        CstKind::StructField {
                            key_and_colon: Some(Box::new((
                                build_identifier("bar"),
                                CstKind::Colon.into(),
                            ))),
                            value: Box::new(build_identifier("baz")),
                            comma: None,
                        }
                        .into(),
                    ],
                    closing_bracket: Box::new(CstKind::ClosingBracket.into()),
                }
                .into(),
            )),
        );
        assert_eq!(
            struct_("[foo := [foo]", 0),
            Some((
                ":= [foo]",
                CstKind::Struct {
                    opening_bracket: Box::new(CstKind::OpeningBracket.into()),
                    fields: vec![CstKind::StructField {
                        key_and_colon: None,
                        value: Box::new(build_identifier("foo").with_trailing_space()),
                        comma: None,
                    }
                    .into()],
                    closing_bracket: Box::new(
                        CstKind::Error {
                            unparsable_input: String::new(),
                            error: CstError::StructNotClosed,
                        }
                        .into()
                    ),
                }
                .into(),
            )),
        );
        // [
        //   foo: bar,
        //   4: "Hi",
        // ]
        assert_eq!(
            struct_("[\n  foo: bar,\n  4: \"Hi\",\n]", 0),
            Some((
                "",
                CstKind::Struct {
                    opening_bracket: Box::new(CstKind::OpeningBracket.with_trailing_whitespace(
                        vec![
                            CstKind::Newline("\n".to_string()),
                            CstKind::Whitespace("  ".to_string()),
                        ],
                    )),
                    fields: vec![
                        CstKind::StructField {
                            key_and_colon: Some(Box::new((
                                build_identifier("foo"),
                                CstKind::Colon.with_trailing_space(),
                            ))),
                            value: Box::new(build_identifier("bar")),
                            comma: Some(Box::new(CstKind::Comma.into())),
                        }
                        .with_trailing_whitespace(vec![
                            CstKind::Newline("\n".to_string()),
                            CstKind::Whitespace("  ".to_string()),
                        ]),
                        CstKind::StructField {
                            key_and_colon: Some(Box::new((
                                build_simple_int(4),
                                CstKind::Colon.with_trailing_space(),
                            ))),
                            value: Box::new(build_simple_text("Hi")),
                            comma: Some(Box::new(CstKind::Comma.into())),
                        }
                        .with_trailing_whitespace(vec![CstKind::Newline("\n".to_string())]),
                    ],
                    closing_bracket: Box::new(CstKind::ClosingBracket.into()),
                }
                .into(),
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
            CstKind::Error {
                unparsable_input: String::new(),
                error: CstError::OpeningParenthesisMissesExpression,
            }
            .into(),
        ));

        let (input, whitespace) = whitespaces_and_newlines(input, indentation, true);
        let inner = inner.wrap_in_whitespace(whitespace);

        let (input, closing_parenthesis) = closing_parenthesis(input).unwrap_or((
            input,
            CstKind::Error {
                unparsable_input: String::new(),
                error: CstError::ParenthesisNotClosed,
            }
            .into(),
        ));

        Some((
            input,
            CstKind::Parenthesized {
                opening_parenthesis: Box::new(opening_parenthesis),
                inner: Box::new(inner),
                closing_parenthesis: Box::new(closing_parenthesis),
            }
            .into(),
        ))
    }
    #[test]
    fn test_parenthesized() {
        assert_eq!(
            parenthesized("(foo)", 0),
            Some((
                "",
                CstKind::Parenthesized {
                    opening_parenthesis: Box::new(CstKind::OpeningParenthesis.into()),
                    inner: Box::new(build_identifier("foo")),
                    closing_parenthesis: Box::new(CstKind::ClosingParenthesis.into()),
                }
                .into(),
            )),
        );
        assert_eq!(parenthesized("foo", 0), None);
        assert_eq!(
            parenthesized("(foo", 0),
            Some((
                "",
                CstKind::Parenthesized {
                    opening_parenthesis: Box::new(CstKind::OpeningParenthesis.into()),
                    inner: Box::new(build_identifier("foo")),
                    closing_parenthesis: Box::new(
                        CstKind::Error {
                            unparsable_input: String::new(),
                            error: CstError::ParenthesisNotClosed
                        }
                        .into()
                    ),
                }
                .into(),
            )),
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
                indentation += match &unexpected_whitespace.kind {
                    CstKind::Whitespace(whitespace)
                    | CstKind::Error {
                        unparsable_input: whitespace,
                        error: CstError::WeirdWhitespace,
                    } => whitespace_indentation_score(whitespace) / 2,
                    _ => panic!(
                        "single_line_whitespace returned something other than Whitespace or Error."
                    ),
                };
                expressions.push(
                    CstKind::Error {
                        unparsable_input: unexpected_whitespace.to_string(),
                        error: CstError::TooMuchWhitespace,
                    }
                    .into(),
                );
            }

            if let Some((new_input, expression)) = expression(input, indentation, true, true, true)
            {
                input = new_input;

                let (mut whitespace, expression) = expression.split_outer_trailing_whitespace();
                expressions.push(expression);
                expressions.append(&mut whitespace);
            } else {
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
        (input, expressions)
    }
    #[test]
    fn test_body() {
        assert_eq!(
            body("foo # comment", 0),
            (
                "",
                vec![
                    build_identifier("foo"),
                    build_space(),
                    build_comment(" comment")
                ]
            ),
        );
    }

    #[instrument(level = "trace")]
    fn match_case(input: &str, indentation: usize) -> Option<(&str, Rcst)> {
        let (input, pattern) = expression(input, indentation, false, true, true)?;
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

    #[instrument(level = "trace")]
    fn function(input: &str, indentation: usize) -> Option<(&str, Rcst)> {
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

        let (input, whitespace) = whitespaces_and_newlines(input, indentation + 1, true);
        if let Some((parameters, arrow)) = parameters_and_arrow {
            parameters_and_arrow = Some((parameters, arrow.wrap_in_whitespace(whitespace)));
        } else {
            opening_curly_brace = opening_curly_brace.wrap_in_whitespace(whitespace);
        }

        let (input, mut body, mut whitespace_before_closing_curly_brace, closing_curly_brace) = {
            let input_before_parsing_expression = input;
            let (input, body_expression) =
                match expression(input, indentation + 1, true, true, true) {
                    Some((input, expression)) => (input, vec![expression]),
                    None => (input, vec![]),
                };
            let (input, whitespace) = whitespaces_and_newlines(input, indentation + 1, true);
            if let Some((input, curly_brace)) = closing_curly_brace(input) {
                (input, body_expression, whitespace, curly_brace)
            } else {
                // There is no closing brace after a single expression. Thus,
                // we now try to parse a body of multiple expressions. We didn't
                // try this first because then the body would also have consumed
                // any trailing closing curly brace in the same line.
                // For example, for the function `{ 2 }`, the body parser would
                // have already consumed the `}`. The body parser works great
                // for multiline bodies, though.
                let (input, body) = body(input_before_parsing_expression, indentation + 1);
                let input_after_body = input;
                let (input, whitespace) = whitespaces_and_newlines(input, indentation, true);
                match closing_curly_brace(input) {
                    Some((input, closing_curly_brace)) => {
                        (input, body, whitespace, closing_curly_brace)
                    }
                    None => (
                        input_after_body,
                        body,
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
        if !body.is_empty() {
            body.append(&mut whitespace_before_closing_curly_brace);
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
                body,
                closing_curly_brace: Box::new(closing_curly_brace),
            }
            .into(),
        ))
    }
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

    #[cfg(test)]
    fn build_comment(value: impl AsRef<str>) -> Rcst {
        CstKind::Comment {
            octothorpe: Box::new(CstKind::Octothorpe.into()),
            comment: value.as_ref().to_string(),
        }
        .into()
    }
    #[cfg(test)]
    fn build_identifier(value: impl AsRef<str>) -> Rcst {
        CstKind::Identifier(value.as_ref().to_string()).into()
    }
    #[cfg(test)]
    fn build_symbol(value: impl AsRef<str>) -> Rcst {
        CstKind::Symbol(value.as_ref().to_string()).into()
    }
    #[cfg(test)]
    fn build_simple_int(value: usize) -> Rcst {
        CstKind::Int {
            value: value.into(),
            string: value.to_string(),
        }
        .into()
    }
    #[cfg(test)]
    fn build_simple_text(value: impl AsRef<str>) -> Rcst {
        CstKind::Text {
            opening: Box::new(
                CstKind::OpeningText {
                    opening_single_quotes: vec![],
                    opening_double_quote: Box::new(CstKind::DoubleQuote.into()),
                }
                .into(),
            ),
            parts: vec![CstKind::TextPart(value.as_ref().to_string()).into()],
            closing: Box::new(
                CstKind::ClosingText {
                    closing_double_quote: Box::new(CstKind::DoubleQuote.into()),
                    closing_single_quotes: vec![],
                }
                .into(),
            ),
        }
        .into()
    }
    #[cfg(test)]
    fn build_space() -> Rcst {
        CstKind::Whitespace(" ".to_string()).into()
    }
    #[cfg(test)]
    fn build_newline() -> Rcst {
        CstKind::Newline("\n".to_string()).into()
    }

    #[cfg(test)]
    impl Rcst {
        fn with_trailing_space(self) -> Rcst {
            self.with_trailing_whitespace(vec![CstKind::Whitespace(" ".to_string())])
        }
        fn with_trailing_whitespace(self, trailing_whitespace: Vec<CstKind<()>>) -> Rcst {
            CstKind::TrailingWhitespace {
                child: Box::new(self),
                whitespace: trailing_whitespace.into_iter().map(Into::into).collect(),
            }
            .into()
        }
    }
    #[cfg(test)]
    impl CstKind<()> {
        fn with_trailing_space(self) -> Rcst {
            Rcst::from(self).with_trailing_space()
        }
        fn with_trailing_whitespace(self, trailing_whitespace: Vec<CstKind<()>>) -> Rcst {
            Rcst::from(self).with_trailing_whitespace(trailing_whitespace)
        }
    }
}
