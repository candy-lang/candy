use itertools::Itertools;

use super::rcst::Rcst;
use crate::compiler::{
    cst::{self, CstDb},
    hir::{self, HirDb},
};
use std::sync::Arc;

#[salsa::query_group(CommentStringToRcstStorage)]
pub trait CommentStringToRcst: CstDb + HirDb {
    fn comment_rcst(&self, id: hir::Id) -> Arc<Vec<Rcst>>;
}

fn comment_rcst(db: &dyn CommentStringToRcst, id: hir::Id) -> Arc<Vec<Rcst>> {
    let comments_and_newlines = if id.is_root() {
        db.cst(id.input)
            .unwrap()
            .iter()
            .take_while(|it| {
                matches!(
                    it.kind,
                    cst::CstKind::Whitespace(_)
                        | cst::CstKind::Newline(_)
                        | cst::CstKind::Comment { .. }
                )
            })
            .cloned()
            .collect_vec()
    } else {
        let cst_id = db.hir_to_cst_id(id.clone()).unwrap();
        match db.find_cst(id.input, cst_id).kind {
            cst::CstKind::Assignment {
                box assignment_sign,
                ..
            } => match assignment_sign.kind {
                cst::CstKind::TrailingWhitespace { whitespace, .. } => whitespace.to_vec(),
                _ => vec![],
            },
            _ => panic!(
                "Tried to get the comment RCST for something other than a module or assignment."
            ),
        }
    };
    let comment_lines = comments_and_newlines
        .iter()
        .filter_map(|it| match &it.kind {
            cst::CstKind::Comment { comment, .. } => {
                Some(comment.strip_prefix(' ').unwrap_or(comment))
            }
            cst::CstKind::Newline(_) => None,
            _ => unreachable!(),
        })
        .collect_vec();

    let (rest, rcsts) = parse::blocks(comment_lines, 0).unwrap();
    assert!(rest.is_empty());
    Arc::new(rcsts)
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

    use std::collections::HashSet;

    use super::{
        super::rcst::{Rcst, RcstError},
        whitespace_indentation_score,
    };
    use itertools::Itertools;
    use url::Url;

    static SUPPORTED_WHITESPACE: &str = " \t";

    fn newline(input: Vec<&str>) -> Option<(Vec<&str>, Rcst)> {
        if let ["", remaining @ ..] = input.as_slice() && !remaining.is_empty() {
            Some((remaining.to_vec(), Rcst::Newline))
        } else {
            None
        }
    }

    fn single_line_whitespace(mut input: Vec<&str>) -> Option<(Vec<&str>, Rcst)> {
        log::trace!("single_line_whitespace({input:?})");
        let mut chars = vec![];
        let mut has_error = false;
        if let [line, remaining @ ..] = input.as_slice() {
            for c in line.chars() {
                match c {
                    ' ' => {}
                    c if SUPPORTED_WHITESPACE.contains(c) => {
                        has_error = true;
                    }
                    _ => break,
                }
                chars.push(c);
            }
            if chars.is_empty() {
                return None;
            }

            let whitespace = chars.into_iter().join("");
            input = recombine(&line[whitespace.len()..], remaining);
            let mut whitespace = Rcst::Whitespace(whitespace);
            if has_error {
                whitespace = Rcst::Error {
                    child: Some(whitespace.into()),
                    error: RcstError::WeirdWhitespace,
                };
            }
            Some((input, whitespace))
        } else {
            None
        }
    }
    #[test]
    fn test_single_line_whitespace() {
        assert_eq!(
            single_line_whitespace(vec!["  ", "foo"]),
            Some((vec!["", "foo"], Rcst::Whitespace("  ".to_string())))
        );
    }

    fn leading_indentation(mut input: Vec<&str>, indentation: usize) -> Option<(Vec<&str>, Rcst)> {
        log::trace!("leading_indentation({input:?}, {indentation:?})");
        if let [line, remaining @ ..] = input.as_slice() {
            let mut chars = vec![];
            let mut has_weird_whitespace = false;
            let mut indentation_score = 0;
            let mut line_chars = line.chars();
            while indentation_score < indentation {
                let c = line_chars.next()?;
                let is_weird = match c {
                    ' ' => false,
                    c if c.is_whitespace() => true,
                    _ => return None,
                };
                chars.push(c);
                has_weird_whitespace |= is_weird;
                indentation_score += whitespace_indentation_score(&format!("{c}"));
            }

            let whitespace = chars.into_iter().join("");
            input = recombine(&line[whitespace.len()..], remaining);
            let whitespace = Rcst::Whitespace(whitespace);
            Some((
                input,
                if has_weird_whitespace {
                    Rcst::Error {
                        child: Some(whitespace.into()),
                        error: RcstError::WeirdWhitespaceInIndentation,
                    }
                } else {
                    whitespace
                },
            ))
        } else {
            None
        }
    }
    #[test]
    fn test_leading_indentation() {
        assert_eq!(
            leading_indentation(vec!["foo"], 0),
            Some((vec!["foo"], Rcst::Whitespace("".to_string())))
        );
        assert_eq!(
            leading_indentation(vec!["  foo"], 2),
            Some((vec!["foo"], Rcst::Whitespace("  ".to_string())))
        );
        assert_eq!(leading_indentation(vec!["  foo"], 4), None);
    }

    fn newline_and_whitespace(
        mut input: Vec<&str>,
        indentation: usize,
    ) -> Option<(Vec<&str>, Vec<Rcst>)> {
        log::trace!("newline_and_whitespace({input:?}, {indentation:?})");
        let mut parts = vec![];

        if let Some((new_input, whitespace)) = single_line_whitespace(input.clone()) {
            parts.push(whitespace);
            input = new_input;
        }

        if let Some((new_input, newline)) = newline(input.clone()) {
            input = new_input;
            parts.push(newline);
        } else {
            return None;
        }

        if indentation > 0 {
            if let Some((new_input, indentation)) = leading_indentation(input.clone(), indentation)
            {
                input = new_input;
                parts.push(indentation);
            } else {
                return None;
            }
        }

        if let Some((new_input, whitespace)) = single_line_whitespace(input.clone()) {
            if indentation > 0 {
                match (parts.pop().unwrap(), whitespace) {
                    (Rcst::Whitespace(indentation), Rcst::Whitespace(whitespace)) => {
                        parts.push(Rcst::Whitespace(indentation + &whitespace));
                    }
                    _ => unreachable!(),
                }
            } else {
                parts.push(whitespace);
            }
            input = new_input;
        }

        Some((input, parts))
    }
    #[test]
    fn test_newline_and_whitespace() {
        assert_eq!(
            newline_and_whitespace(vec![" ", " a"], 0),
            Some((
                vec!["a"],
                vec![
                    Rcst::Whitespace(" ".to_string()),
                    Rcst::Newline,
                    Rcst::Whitespace(" ".to_string())
                ]
            ))
        );
        assert_eq!(newline_and_whitespace(vec!["", " a"], 2), None);
        assert_eq!(
            newline_and_whitespace(vec![" ", "  a"], 2),
            Some((
                vec!["a"],
                vec![
                    Rcst::Whitespace(" ".to_string()),
                    Rcst::Newline,
                    Rcst::Whitespace("  ".to_string())
                ]
            ))
        );
        assert_eq!(
            newline_and_whitespace(vec![" ", "   a"], 2),
            Some((
                vec!["a"],
                vec![
                    Rcst::Whitespace(" ".to_string()),
                    Rcst::Newline,
                    Rcst::Whitespace("   ".to_string())
                ]
            ))
        );
        assert_eq!(newline_and_whitespace(vec!["abc"], 2), None);
    }

    // Inline Elements
    fn escaped(escaped_char: Option<char>) -> Rcst {
        log::trace!("escaped({escaped_char:?})");

        match escaped_char {
            Some(escaped_char) if "-_[]`#\\".contains(escaped_char) => {
                Rcst::EscapedChar(Some(escaped_char))
            }
            Some(escaped_char) => Rcst::Error {
                child: Some(Rcst::EscapedChar(Some(escaped_char)).into()),
                error: RcstError::EscapeWithInvalidChar,
            },
            None => Rcst::Error {
                child: None,
                error: RcstError::EscapeWithoutChar,
            },
        }
    }
    #[test]
    fn test_escaped() {
        assert_eq!(escaped(Some('_')), Rcst::EscapedChar(Some('_')));
        assert_eq!(
            escaped(Some('a')),
            Rcst::Error {
                child: Some(Rcst::EscapedChar(Some('a')).into()),
                error: RcstError::EscapeWithInvalidChar,
            }
        );
        assert_eq!(
            escaped(None),
            Rcst::Error {
                child: None,
                error: RcstError::EscapeWithoutChar,
            }
        );
    }

    #[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
    enum InlineFormatting {
        Emphasized,
        Link,
        Code,
    }
    impl InlineFormatting {
        fn as_rcst(
            &self,
            has_opening_char: bool,
            inner_parts: Vec<Rcst>,
            has_closing_char: bool,
        ) -> Rcst {
            match self {
                InlineFormatting::Emphasized => Rcst::Emphasized {
                    has_opening_underscore: has_opening_char,
                    text: inner_parts,
                    has_closing_underscore: has_closing_char,
                },
                InlineFormatting::Link => Rcst::Link {
                    has_opening_bracket: has_opening_char,
                    text: inner_parts,
                    has_closing_bracket: has_closing_char,
                },
                InlineFormatting::Code => Rcst::InlineCode {
                    has_opening_backtick: has_opening_char,
                    code: inner_parts,
                    has_closing_backtick: has_closing_char,
                },
            }
        }
    }

    struct SingleLineInlineParser {
        top_level_parts: Vec<Rcst>,
        formattings: Vec<(bool, InlineFormatting, Vec<Rcst>)>,
        characters: Vec<char>,
    }
    impl SingleLineInlineParser {
        fn parse(
            line: &str,
            initial_state: &[InlineFormatting],
        ) -> Option<(Vec<Rcst>, Vec<InlineFormatting>)> {
            assert_eq!(
                initial_state.len(),
                initial_state.iter().collect::<HashSet<_>>().len()
            );
            let parser = SingleLineInlineParser {
                top_level_parts: vec![],
                formattings: initial_state
                    .iter()
                    .map(|&it| (false, it, vec![]))
                    .collect(),
                characters: vec![],
            };
            parser.run(line)
        }

        fn is_in_emphasized(&self) -> bool {
            self.formattings
                .iter()
                .any(|(_, it, _)| it == &InlineFormatting::Emphasized)
        }
        fn is_in_link(&self) -> bool {
            self.formattings
                .iter()
                .any(|(_, it, _)| it == &InlineFormatting::Link)
        }
        fn is_in_code(&self) -> bool {
            self.formattings
                .last()
                .map(|(_, it, _)| it == &InlineFormatting::Code)
                .unwrap_or(false)
        }

        fn push_part(&mut self, part: Rcst) {
            self.formattings
                .last_mut()
                .map(|(_, _, parts)| parts)
                .unwrap_or(&mut self.top_level_parts)
                .push(part);
        }
        fn finish_text_part(&mut self) {
            if self.characters.is_empty() {
                return;
            }

            let part = Rcst::TextPart(self.characters.drain(..).collect());
            self.push_part(part);
        }
        fn start_formatting(&mut self, formatting: InlineFormatting) {
            self.finish_text_part();
            self.formattings.push((true, formatting, vec![]));
        }
        fn end_formatting(&mut self, formatting: InlineFormatting, has_closing_char: bool) {
            self.finish_text_part();
            loop {
                let (has_opening_char, current_formatting, inner_parts) =
                    self.formattings.pop().unwrap();
                let part = current_formatting.as_rcst(
                    has_opening_char,
                    inner_parts,
                    current_formatting == formatting && has_closing_char,
                );
                self.push_part(part);
                if current_formatting == formatting {
                    break;
                }
            }
        }

        fn finish(mut self) -> Option<(Vec<Rcst>, Vec<InlineFormatting>)> {
            let remaining_formatting = self
                .formattings
                .iter()
                .map(|(_, formatting, _)| formatting)
                .copied()
                .collect_vec();
            if self.formattings.is_empty() {
                self.finish_text_part();
            } else {
                while let Some((_, formatting, _)) = self.formattings.last() {
                    self.end_formatting(*formatting, false);
                }
            }
            assert!(self.formattings.is_empty());
            assert!(self.characters.is_empty());
            if self.top_level_parts.is_empty() {
                None
            } else {
                Some((self.top_level_parts, remaining_formatting))
            }
        }

        fn run(mut self, line: &str) -> Option<(Vec<Rcst>, Vec<InlineFormatting>)> {
            let mut characters = line.chars();
            while let Some(character) = characters.next() {
                match character {
                    '_' if !self.is_in_code() => {
                        if !self.is_in_emphasized() {
                            self.start_formatting(InlineFormatting::Emphasized);
                        } else {
                            self.end_formatting(InlineFormatting::Emphasized, true);
                        }
                    }
                    '[' if !self.is_in_link() && !self.is_in_code() => {
                        self.start_formatting(InlineFormatting::Link)
                    }
                    ']' if self.is_in_link() && !self.is_in_code() => {
                        self.end_formatting(InlineFormatting::Link, true)
                    }
                    '`' => {
                        if !self.is_in_code() {
                            self.start_formatting(InlineFormatting::Code);
                        } else {
                            self.end_formatting(InlineFormatting::Code, true);
                        }
                    }
                    '\\' => {
                        self.finish_text_part();
                        self.push_part(escaped(characters.next()));
                    }
                    character => self.characters.push(character),
                }
            }
            self.finish()
        }
    }
    fn inline(
        line: &str,
        initial_formatting_state: &[InlineFormatting],
    ) -> Option<(Vec<Rcst>, Vec<InlineFormatting>)> {
        log::trace!("inline({line:?}, {initial_formatting_state:?})");
        SingleLineInlineParser::parse(line, initial_formatting_state)
    }
    #[test]
    fn test_inline() {
        assert_eq!(
            inline("abc", &[]),
            Some((vec![Rcst::TextPart("abc".to_string())], vec![]))
        );
        assert_eq!(
            inline("abc _def_ ghi", &[]),
            Some((
                vec![
                    Rcst::TextPart("abc ".to_string()),
                    Rcst::Emphasized {
                        has_opening_underscore: true,
                        text: vec![Rcst::TextPart("def".to_string())],
                        has_closing_underscore: true
                    },
                    Rcst::TextPart(" ghi".to_string())
                ],
                vec![]
            ))
        );
        assert_eq!(
            inline("abc [def] ghi", &[]),
            Some((
                vec![
                    Rcst::TextPart("abc ".to_string()),
                    Rcst::Link {
                        has_opening_bracket: true,
                        text: vec![Rcst::TextPart("def".to_string())],
                        has_closing_bracket: true
                    },
                    Rcst::TextPart(" ghi".to_string())
                ],
                vec![]
            ))
        );
        assert_eq!(
            inline("abc `def` ghi", &[]),
            Some((
                vec![
                    Rcst::TextPart("abc ".to_string()),
                    Rcst::InlineCode {
                        has_opening_backtick: true,
                        code: vec![Rcst::TextPart("def".to_string())],
                        has_closing_backtick: true
                    },
                    Rcst::TextPart(" ghi".to_string())
                ],
                vec![]
            ))
        );
        assert_eq!(
            inline("abc [_def ` ghi", &[]),
            Some((
                vec![
                    Rcst::TextPart("abc ".to_string()),
                    Rcst::Link {
                        has_opening_bracket: true,
                        text: vec![Rcst::Emphasized {
                            has_opening_underscore: true,
                            text: vec![
                                Rcst::TextPart("def ".to_string()),
                                Rcst::InlineCode {
                                    has_opening_backtick: true,
                                    code: vec![Rcst::TextPart(" ghi".to_string())],
                                    has_closing_backtick: false
                                },
                            ],
                            has_closing_underscore: false
                        },],
                        has_closing_bracket: false
                    },
                ],
                vec![
                    InlineFormatting::Link,
                    InlineFormatting::Emphasized,
                    InlineFormatting::Code
                ]
            ))
        );
        assert_eq!(
            inline(
                "abc` def]_ ghi",
                &[
                    InlineFormatting::Link,
                    InlineFormatting::Emphasized,
                    InlineFormatting::Code
                ]
            ),
            Some((
                vec![
                    Rcst::Link {
                        has_opening_bracket: false,
                        text: vec![Rcst::Emphasized {
                            has_opening_underscore: false,
                            text: vec![
                                Rcst::InlineCode {
                                    has_opening_backtick: false,
                                    code: vec![Rcst::TextPart("abc".to_string())],
                                    has_closing_backtick: true
                                },
                                Rcst::TextPart(" def".to_string()),
                            ],
                            has_closing_underscore: false
                        },],
                        has_closing_bracket: true
                    },
                    Rcst::Emphasized {
                        has_opening_underscore: true,
                        text: vec![Rcst::TextPart(" ghi".to_string()),],
                        has_closing_underscore: false
                    },
                ],
                vec![InlineFormatting::Emphasized]
            ))
        );
    }

    fn title_line(
        line: &str,
        formatting_state: Vec<InlineFormatting>,
    ) -> Option<(Rcst, Vec<InlineFormatting>)> {
        log::trace!("title_line({line:?}, {formatting_state:?})");
        if line.is_empty() {
            return None;
        }

        let octothorpe_count = line.as_bytes().iter().take_while(|&&it| it == b'#').count();
        if octothorpe_count == 0 {
            return None;
        }
        let line = &line[octothorpe_count..];

        let (text, formatting_state) =
            inline(line, formatting_state.as_slice()).unwrap_or((vec![], formatting_state));
        Some((
            Rcst::TitleLine {
                octothorpe_count,
                text,
            },
            formatting_state,
        ))
    }
    fn title(mut input: Vec<&str>) -> Option<(Vec<&str>, Rcst)> {
        log::trace!("title({input:?})");
        let mut title_lines = vec![];
        let mut formatting_state = vec![];

        if let Some((line, remaining)) = input.split_first()
                && let Some((title_line, new_formatting_state)) = title_line(line, formatting_state) {
            input = recombine("", remaining);
            title_lines.push(title_line);
            formatting_state = new_formatting_state;
        } else {
            return None;
        }

        loop {
            let Some((new_input, whitespace)) =newline_and_whitespace(input.clone(), 0) else { break };

            if let Some((line, remaining)) = new_input.split_first()
                    && let Some((new_title_line, new_formatting_state)) = title_line(line, formatting_state) {
                input = recombine("", remaining);
                let previous_line = title_lines.pop().unwrap();
                title_lines.push(previous_line.wrap_in_whitespace(whitespace));
                title_lines.push(new_title_line);
                formatting_state = new_formatting_state;
            } else {
                break;
            }
        }

        Some((input, Rcst::Title(title_lines)))
    }
    #[test]
    fn test_title() {
        assert_eq!(
            title(vec!["# Foo"]),
            Some((
                vec![""],
                Rcst::Title(vec![Rcst::TitleLine {
                    octothorpe_count: 1,
                    text: vec![Rcst::TextPart(" Foo".to_string())]
                }])
            ))
        );
        assert_eq!(
            title(vec!["##Bar"]),
            Some((
                vec![""],
                Rcst::Title(vec![Rcst::TitleLine {
                    octothorpe_count: 2,
                    text: vec![Rcst::TextPart("Bar".to_string())]
                }])
            ))
        );
        assert_eq!(
            title(vec!["# Foo", " ##Bar", "Baz"]),
            Some((
                vec!["", "Baz"],
                Rcst::Title(vec![
                    Rcst::TrailingWhitespace {
                        child: Rcst::TitleLine {
                            octothorpe_count: 1,
                            text: vec![Rcst::TextPart(" Foo".to_string())]
                        }
                        .into(),
                        whitespace: vec![Rcst::Newline, Rcst::Whitespace(" ".to_string())],
                    },
                    Rcst::TitleLine {
                        octothorpe_count: 2,
                        text: vec![Rcst::TextPart("Bar".to_string())]
                    }
                ])
            ))
        );
        assert_eq!(title(vec!["abc"]), None);
    }

    fn paragraph(input: Vec<&str>, indentation: usize) -> Option<(Vec<&str>, Rcst)> {
        log::trace!("paragraph({input:?})");

        let mut parts = vec![];
        let mut formatting_state = vec![];
        if let Some((line, remaining)) = input.split_first() {
            let (new_parts, new_formatting_state) = inline(line, formatting_state.as_slice())?;
            parts.extend(new_parts);
            formatting_state = new_formatting_state;
            input = recombine("", remaining);
        }

        loop {
            let Some((new_input, newline)) = newline(input.clone()) else { break };

            let Some((new_input, indentation)) = leading_indentation(new_input, indentation) else { break };

            // TODO: use `if let … && let …`, https://github.com/rust-lang/rust/issues/99852
            let (remaining, (mut line_text, new_formatting_state)) =
                if let Some((line, remaining)) = new_input.split_first() {
                    if let Some(result) = inline(line, formatting_state.as_slice()) {
                        (remaining, result)
                    } else {
                        break;
                    }
                } else {
                    break;
                };

            let last_text = line_text.pop().unwrap();
            line_text.push(last_text.wrap_in_whitespace(vec![newline, indentation]));
            parts.append(&mut line_text);
            input = recombine("", remaining);
            formatting_state = new_formatting_state;
        }

        if parts.is_empty() {
            None
        } else {
            Some((input, Rcst::Paragraph(parts)))
        }
    }

    fn url_line(input: Vec<&str>) -> Option<(Vec<&str>, Rcst)> {
        log::trace!("url_line({input:?})");

        if let [line, remaining @ ..] = input.as_slice() {
            if !line.starts_with("https://") && !line.starts_with("http://") {
                return None;
            }

            fn is_whitespace(character: char) -> bool {
                SUPPORTED_WHITESPACE.contains(character)
            }
            let end_index = line.find(is_whitespace).unwrap_or(line.len());

            // TODO: handle violations
            let url = Url::parse(&line[..end_index])
                .map(Rcst::UrlLine)
                .unwrap_or_else(|_| Rcst::Error {
                    child: Some(Rcst::TextPart(line.to_string()).into()),
                    error: RcstError::UrlInvalid,
                });
            Some((recombine(&line[end_index..], remaining), url))
        } else {
            None
        }
    }
    fn urls(mut input: Vec<&str>) -> Option<(Vec<&str>, Rcst)> {
        log::trace!("urls({input:?})");
        let mut urls = vec![];

        if let Some((new_input, url)) = url_line(input.clone()) {
            input = new_input;
            urls.push(url);
        } else {
            return None;
        }

        loop {
            let Some((new_input, whitespace)) = newline_and_whitespace(input.clone(), 0) else { break };

            if let Some((new_input, url)) = url_line(new_input) {
                input = new_input;
                let previous_url = urls.pop().unwrap();
                urls.push(previous_url.wrap_in_whitespace(whitespace));
                urls.push(url);
            } else {
                break;
            }
        }

        Some((input, Rcst::Urls(urls)))
    }
    #[test]
    fn test_urls() {
        assert_eq!(
            urls(vec!["https://github.com/candy-lang/candy"]),
            Some((
                vec![""],
                Rcst::Urls(vec![Rcst::UrlLine(
                    Url::parse("https://github.com/candy-lang/candy").unwrap()
                )])
            ))
        );
        assert_eq!(
            urls(vec![
                "https://github.com/candy-lang/candy ",
                " https://github.com/candy-lang"
            ]),
            Some((
                vec![""],
                Rcst::Urls(vec![
                    Rcst::TrailingWhitespace {
                        child: Rcst::UrlLine(
                            Url::parse("https://github.com/candy-lang/candy").unwrap()
                        )
                        .into(),
                        whitespace: vec![
                            Rcst::Whitespace(" ".to_string()),
                            Rcst::Newline,
                            Rcst::Whitespace(" ".to_string())
                        ],
                    },
                    Rcst::UrlLine(Url::parse("https://github.com/candy-lang").unwrap())
                ])
            ))
        );
        assert_eq!(urls(vec!["abc"]), None);
    }

    fn code_block(input: Vec<&str>) -> Option<(Vec<&str>, Rcst)> {
        log::trace!("code_block({input:?})");

        if let [line, ..] = input.as_slice() {
            const BACKTICKS: &str = "```";
            if !line.starts_with(BACKTICKS) {
                return None;
            }

            let mut code = vec![];
            let mut line = &line[BACKTICKS.len()..];
            let mut line_index = 0;
            let mut has_closing_backticks = false;
            let remaining_input = loop {
                if let Some(end_backticks_index) = line.find(BACKTICKS) {
                    if end_backticks_index > 0 {
                        code.push(Rcst::TextPart(line[..end_backticks_index].to_string()));
                    }
                    has_closing_backticks = true;
                    break recombine(
                        &line[end_backticks_index + BACKTICKS.len()..],
                        &input[line_index + 1..],
                    );
                }

                if !line.is_empty() {
                    code.push(Rcst::TextPart(line.to_string()));
                }
                line_index += 1;
                if input.len() <= line_index {
                    break vec![];
                }

                code.push(Rcst::Newline);
                line = input[line_index];
            };
            Some((
                remaining_input,
                Rcst::CodeBlock {
                    code,
                    has_closing_backticks,
                },
            ))
        } else {
            None
        }
    }
    #[test]
    fn test_code_block() {
        assert_eq!(
            code_block(vec!["```", "abc", "```"]),
            Some((
                vec![""],
                Rcst::CodeBlock {
                    code: vec![
                        Rcst::Newline,
                        Rcst::TextPart("abc".to_string()),
                        Rcst::Newline
                    ],
                    has_closing_backticks: true
                }
            ))
        );
        assert_eq!(
            code_block(vec!["```", "  abc", " ``` "]),
            Some((
                vec![" "],
                Rcst::CodeBlock {
                    code: vec![
                        Rcst::Newline,
                        Rcst::TextPart("  abc".to_string()),
                        Rcst::Newline,
                        Rcst::TextPart(" ".to_string()),
                    ],
                    has_closing_backticks: true
                }
            ))
        );
        assert_eq!(
            code_block(vec!["```", "abc", "# Foo"]),
            Some((
                vec![],
                Rcst::CodeBlock {
                    code: vec![
                        Rcst::Newline,
                        Rcst::TextPart("abc".to_string()),
                        Rcst::Newline,
                        Rcst::TextPart("# Foo".to_string()),
                    ],
                    has_closing_backticks: false
                }
            ))
        );
        assert_eq!(code_block(vec!["abc"]), None);
    }

    pub fn blocks(mut input: Vec<&str>, indentation: usize) -> Option<(Vec<&str>, Vec<Rcst>)> {
        log::trace!("blocks({input:?}, {indentation})");
        let mut blocks = vec![];

        if let Some((new_input, whitespace)) = single_line_whitespace(input.clone()) {
            input = new_input;
            blocks.push(whitespace);
        }

        loop {
            let mut has_made_progress = false;

            let block = code_block(input.clone())
                .or_else(|| {
                    if indentation == 0 {
                        title(input.clone())
                    } else {
                        None
                    }
                })
                .or_else(|| urls(input.clone()))
                // TODO: list
                .or_else(|| paragraph(input.clone(), indentation));
            if let Some((new_input, block)) = block {
                has_made_progress = true;
                blocks.push(block);
                input = new_input;
            }

            if let Some((new_input, newline)) = newline(input.clone()) {
                has_made_progress = true;
                let previous_block = blocks.pop().unwrap();
                blocks.push(previous_block.wrap_in_whitespace(vec![newline]));
                input = new_input;
            }

            if !has_made_progress {
                break;
            }
        }

        assert!(indentation > 0 || input.is_empty());

        if blocks.is_empty() {
            None
        } else {
            Some((input, blocks))
        }
    }

    fn recombine<'a>(first_line: &'a str, remaining_lines: &[&'a str]) -> Vec<&'a str> {
        let mut lines = vec![first_line];
        lines.extend_from_slice(remaining_lines);
        lines
    }
}
