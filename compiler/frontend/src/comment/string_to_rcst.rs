use super::rcst::Rcst;
use crate::{
    cst::{self, CstDb},
    hir::{self, HirDb},
};
use itertools::Itertools;
use std::sync::Arc;

#[salsa::query_group(CommentStringToRcstStorage)]
pub trait CommentStringToRcst: CstDb + HirDb {
    fn comment_rcst(&self, id: hir::Id) -> Arc<Vec<Rcst>>;
}

fn comment_rcst(db: &dyn CommentStringToRcst, id: hir::Id) -> Arc<Vec<Rcst>> {
    let comments_and_newlines = if id.is_root() {
        db.cst(id.module)
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
        let cst_id = db.hir_to_cst_id(&id).unwrap();
        match db.find_cst(id.module, cst_id).kind {
            cst::CstKind::Assignment {
                box assignment_sign,
                ..
            } => match assignment_sign.kind {
                cst::CstKind::TrailingWhitespace { whitespace, .. } => whitespace,
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

    let (remaining, rcsts) = parse::blocks(comment_lines, 0).unwrap();
    assert!(remaining.is_empty());
    Arc::new(rcsts)
}

impl Rcst {
    fn wrap_in_whitespace(mut self, mut whitespace: Vec<Self>) -> Self {
        if whitespace.is_empty() {
            return self;
        }

        if let Self::TrailingWhitespace {
            whitespace: self_whitespace,
            ..
        } = &mut self
        {
            self_whitespace.append(&mut whitespace);
            self
        } else {
            Self::TrailingWhitespace {
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

    use super::{
        super::rcst::{Rcst, RcstError, RcstListItemMarker},
        whitespace_indentation_score,
    };
    use itertools::Itertools;
    use rustc_hash::FxHashSet;
    use tracing::instrument;
    use url::Url;

    static SUPPORTED_WHITESPACE: &str = " \t";

    #[instrument]
    fn newline(input: Vec<&str>) -> Option<(Vec<&str>, Rcst)> {
        if let ["", remaining @ ..] = input.as_slice() && !remaining.is_empty() {
            Some((remaining.to_vec(), Rcst::Newline))
        } else {
            None
        }
    }

    #[instrument]
    fn single_line_whitespace(input: Vec<&str>) -> Option<(Vec<&str>, Rcst)> {
        if let [line, remaining @ ..] = input.as_slice() {
            let (line, whitespace) = single_line_whitespace_raw(line)?;
            let input = recombine(line, remaining);
            Some((input, whitespace))
        } else {
            None
        }
    }
    #[instrument]
    fn single_line_whitespace_raw(mut line: &str) -> Option<(&str, Rcst)> {
        let mut chars = vec![];
        let mut has_error = false;
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
        line = &line[whitespace.len()..];

        let mut whitespace = Rcst::Whitespace(whitespace);
        if has_error {
            whitespace = Rcst::Error {
                child: Some(whitespace.into()),
                error: RcstError::WeirdWhitespace,
            };
        }
        Some((line, whitespace))
    }
    #[test]
    fn test_single_line_whitespace() {
        assert_eq!(
            single_line_whitespace(vec!["  ", "foo"]),
            Some((vec!["", "foo"], Rcst::Whitespace("  ".to_string())))
        );
    }

    #[instrument]
    fn leading_indentation(mut input: Vec<&str>, indentation: usize) -> Option<(Vec<&str>, Rcst)> {
        assert!(indentation > 0);

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
            leading_indentation(vec!["  foo"], 2),
            Some((vec!["foo"], Rcst::Whitespace("  ".to_string())))
        );
        assert_eq!(leading_indentation(vec!["  foo"], 4), None);
    }

    #[instrument]
    fn whitespaces_and_newlines(
        mut input: Vec<&str>,
        indentation: usize,
    ) -> Option<(Vec<&str>, Vec<Rcst>)> {
        let mut parts = vec![];

        if let Some((new_input, whitespace)) = single_line_whitespace(input.clone()) {
            input = new_input;
            parts.push(whitespace);
        }

        let mut new_input = input.clone();
        let mut new_parts = vec![];
        loop {
            let new_input_from_iteration_start = new_input.clone();
            let mut has_proper_indentation = false;

            if let Some((new_new_input, newline)) = newline(new_input.clone()) {
                new_input = new_new_input;
                new_parts.push(newline);

                if indentation == 0 {
                    has_proper_indentation = true;
                } else if let Some((new_new_input, whitespace)) =
                    leading_indentation(new_input.clone(), indentation)
                {
                    has_proper_indentation = true;
                    new_parts.push(whitespace);
                    new_input = new_new_input;
                }
            }

            if let Some((new_new_input, whitespace)) = single_line_whitespace(new_input.clone()) {
                new_parts.push(whitespace);
                new_input = new_new_input;
            }

            if new_input == new_input_from_iteration_start {
                break;
            }
            if has_proper_indentation {
                input = new_input.clone();
                parts.append(&mut new_parts);
            }
        }

        if parts.is_empty() {
            None
        } else {
            Some((input, parts))
        }
    }
    #[test]
    fn test_whitespaces_and_newlines() {
        assert_eq!(
            whitespaces_and_newlines(vec![" ", " a"], 0),
            Some((
                vec!["a"],
                vec![
                    Rcst::Whitespace(" ".to_string()),
                    Rcst::Newline,
                    Rcst::Whitespace(" ".to_string())
                ],
            )),
        );
        assert_eq!(whitespaces_and_newlines(vec!["", " a"], 2), None);
        assert_eq!(
            whitespaces_and_newlines(vec![" ", "  a"], 2),
            Some((
                vec!["a"],
                vec![
                    Rcst::Whitespace(" ".to_string()),
                    Rcst::Newline,
                    Rcst::Whitespace("  ".to_string())
                ],
            )),
        );
        assert_eq!(
            whitespaces_and_newlines(vec![" ", "   a"], 2),
            Some((
                vec!["a"],
                vec![
                    Rcst::Whitespace(" ".to_string()),
                    Rcst::Newline,
                    Rcst::Whitespace("  ".to_string()),
                    Rcst::Whitespace(" ".to_string()),
                ],
            )),
        );
        assert_eq!(whitespaces_and_newlines(vec!["abc"], 2), None);
        assert_eq!(
            whitespaces_and_newlines(vec!["", ""], 0),
            Some((vec![""], vec![Rcst::Newline]))
        );
    }

    // Inline Elements
    #[instrument]
    fn escaped(escaped_char: Option<char>) -> Rcst {
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
                error: RcstError::EscapeMissesChar,
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
                error: RcstError::EscapeMissesChar,
            }
        );
    }

    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    enum InlineFormatting {
        Emphasized,
        Link,
        Code,
    }
    impl InlineFormatting {
        fn as_rcst(
            self,
            has_opening_char: bool,
            inner_parts: Vec<Rcst>,
            has_closing_char: bool,
        ) -> Rcst {
            match self {
                Self::Emphasized => Rcst::Emphasized {
                    has_opening_underscore: has_opening_char,
                    text: inner_parts,
                    has_closing_underscore: has_closing_char,
                },
                Self::Link => Rcst::Link {
                    has_opening_bracket: has_opening_char,
                    text: inner_parts,
                    has_closing_bracket: has_closing_char,
                },
                Self::Code => Rcst::InlineCode {
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
                initial_state.iter().collect::<FxHashSet<_>>().len()
            );
            let parser = Self {
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
                .map_or(false, |(_, it, _)| it == &InlineFormatting::Code)
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
                        if self.is_in_emphasized() {
                            self.end_formatting(InlineFormatting::Emphasized, true);
                        } else {
                            self.start_formatting(InlineFormatting::Emphasized);
                        }
                    }
                    '[' if !self.is_in_link() && !self.is_in_code() => {
                        self.start_formatting(InlineFormatting::Link);
                    }
                    ']' if self.is_in_link() && !self.is_in_code() => {
                        self.end_formatting(InlineFormatting::Link, true);
                    }
                    '`' => {
                        if self.is_in_code() {
                            self.end_formatting(InlineFormatting::Code, true);
                        } else {
                            self.start_formatting(InlineFormatting::Code);
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
    #[instrument]
    fn inline(
        line: &str,
        initial_formatting_state: &[InlineFormatting],
    ) -> Option<(Vec<Rcst>, Vec<InlineFormatting>)> {
        SingleLineInlineParser::parse(line, initial_formatting_state)
    }
    #[test]
    fn test_inline() {
        assert_eq!(
            inline("abc", &[]),
            Some((vec![Rcst::TextPart("abc".to_string())], vec![])),
        );
        assert_eq!(
            inline("abc _def_ ghi", &[]),
            Some((
                vec![
                    Rcst::TextPart("abc ".to_string()),
                    Rcst::Emphasized {
                        has_opening_underscore: true,
                        text: vec![Rcst::TextPart("def".to_string())],
                        has_closing_underscore: true,
                    },
                    Rcst::TextPart(" ghi".to_string()),
                ],
                vec![],
            )),
        );
        assert_eq!(
            inline("abc [def] ghi", &[]),
            Some((
                vec![
                    Rcst::TextPart("abc ".to_string()),
                    Rcst::Link {
                        has_opening_bracket: true,
                        text: vec![Rcst::TextPart("def".to_string())],
                        has_closing_bracket: true,
                    },
                    Rcst::TextPart(" ghi".to_string()),
                ],
                vec![],
            )),
        );
        assert_eq!(
            inline("abc `def` ghi", &[]),
            Some((
                vec![
                    Rcst::TextPart("abc ".to_string()),
                    Rcst::InlineCode {
                        has_opening_backtick: true,
                        code: vec![Rcst::TextPart("def".to_string())],
                        has_closing_backtick: true,
                    },
                    Rcst::TextPart(" ghi".to_string()),
                ],
                vec![]
            )),
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
                                    has_closing_backtick: false,
                                },
                            ],
                            has_closing_underscore: false,
                        }],
                        has_closing_bracket: false,
                    },
                ],
                vec![
                    InlineFormatting::Link,
                    InlineFormatting::Emphasized,
                    InlineFormatting::Code,
                ],
            )),
        );
        assert_eq!(
            inline(
                "abc` def]_ ghi",
                &[
                    InlineFormatting::Link,
                    InlineFormatting::Emphasized,
                    InlineFormatting::Code,
                ],
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
                                    has_closing_backtick: true,
                                },
                                Rcst::TextPart(" def".to_string()),
                            ],
                            has_closing_underscore: false,
                        }],
                        has_closing_bracket: true,
                    },
                    Rcst::Emphasized {
                        has_opening_underscore: true,
                        text: vec![Rcst::TextPart(" ghi".to_string())],
                        has_closing_underscore: false,
                    },
                ],
                vec![InlineFormatting::Emphasized],
            )),
        );
    }

    #[instrument]
    fn title_line(
        line: &str,
        formatting_state: Vec<InlineFormatting>,
    ) -> Option<(Rcst, Vec<InlineFormatting>)> {
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
    #[instrument]
    fn title(mut input: Vec<&str>) -> Option<(Vec<&str>, Rcst)> {
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
            let Some((new_input, whitespace)) = whitespaces_and_newlines(input.clone(), 0) else {
                break;
            };

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
                    text: vec![Rcst::TextPart(" Foo".to_string())],
                }]),
            )),
        );
        assert_eq!(
            title(vec!["##Bar"]),
            Some((
                vec![""],
                Rcst::Title(vec![Rcst::TitleLine {
                    octothorpe_count: 2,
                    text: vec![Rcst::TextPart("Bar".to_string())],
                }]),
            )),
        );
        assert_eq!(
            title(vec!["# Foo", " ##Bar", "Baz"]),
            Some((
                vec!["", "Baz"],
                Rcst::Title(vec![
                    Rcst::TrailingWhitespace {
                        child: Rcst::TitleLine {
                            octothorpe_count: 1,
                            text: vec![Rcst::TextPart(" Foo".to_string())],
                        }
                        .into(),
                        whitespace: vec![Rcst::Newline, Rcst::Whitespace(" ".to_string())],
                    },
                    Rcst::TitleLine {
                        octothorpe_count: 2,
                        text: vec![Rcst::TextPart("Bar".to_string())],
                    },
                ]),
            )),
        );
        assert_eq!(title(vec!["abc"]), None);
    }

    #[instrument]
    fn paragraph(mut input: Vec<&str>, indentation: usize) -> Option<(Vec<&str>, Rcst)> {
        let mut parts = vec![];
        let mut formatting_state = vec![];
        if let Some((line, remaining)) = input.split_first() {
            let (mut new_parts, new_formatting_state) = inline(line, formatting_state.as_slice())?;
            parts.append(&mut new_parts);
            formatting_state = new_formatting_state;
            input = recombine("", remaining);
        }

        loop {
            let Some((mut new_input, newline)) = newline(input.clone()) else {
                break;
            };
            let mut whitespace = vec![newline];

            if indentation > 0 {
                let Some((new_new_input, indentation)) =
                    leading_indentation(new_input, indentation)
                else {
                    break;
                };
                new_input = new_new_input;
                whitespace.push(indentation);
            };

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

            if let Some(part) = parts.pop() {
                parts.push(part.wrap_in_whitespace(whitespace));
            } else {
                parts.append(&mut whitespace);
            }
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
    #[test]
    fn test_paragraph() {
        assert_eq!(
            paragraph(vec!["item 1 item item item", "item item "], 0),
            Some((
                vec![""],
                Rcst::Paragraph(vec![
                    Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::TextPart("item 1 item item item".to_string())),
                        whitespace: vec![Rcst::Newline]
                    },
                    Rcst::TextPart("item item ".to_string()),
                ]),
            ))
        );
        assert_eq!(
            paragraph(vec!["item 1 item item item", "  item item ", ""], 2),
            Some((
                vec!["", ""],
                Rcst::Paragraph(vec![
                    Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::TextPart("item 1 item item item".to_string())),
                        whitespace: vec![Rcst::Newline, Rcst::Whitespace("  ".to_string())]
                    },
                    Rcst::TextPart("item item ".to_string()),
                ]),
            ))
        );
        assert_eq!(
            paragraph(vec!["item 2a", "    item item"], 4),
            Some((
                vec![""],
                Rcst::Paragraph(vec![
                    Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::TextPart("item 2a".to_string())),
                        whitespace: vec![Rcst::Newline, Rcst::Whitespace("    ".to_string())]
                    },
                    Rcst::TextPart("item item".to_string()),
                ]),
            ))
        );
    }

    #[instrument]
    fn url_line(input: Vec<&str>) -> Option<(Vec<&str>, Rcst)> {
        fn is_whitespace(character: char) -> bool {
            SUPPORTED_WHITESPACE.contains(character)
        }

        if let [line, remaining @ ..] = input.as_slice() {
            if !line.starts_with("https://") && !line.starts_with("http://") {
                return None;
            }

            let end_index = line.find(is_whitespace).unwrap_or(line.len());

            // TODO: handle violations
            let url = Url::parse(&line[..end_index]).map_or_else(
                |_| Rcst::Error {
                    child: Some(Rcst::TextPart((*line).to_string()).into()),
                    error: RcstError::UrlInvalid,
                },
                Rcst::UrlLine,
            );
            Some((recombine(&line[end_index..], remaining), url))
        } else {
            None
        }
    }
    #[instrument]
    fn urls(mut input: Vec<&str>) -> Option<(Vec<&str>, Rcst)> {
        let mut urls = vec![];

        if let Some((new_input, url)) = url_line(input.clone()) {
            input = new_input;
            urls.push(url);
        } else {
            return None;
        }

        loop {
            let Some((new_input, whitespace)) = whitespaces_and_newlines(input.clone(), 0) else {
                break;
            };

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
                    Url::parse("https://github.com/candy-lang/candy").unwrap(),
                )]),
            )),
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
                            Url::parse("https://github.com/candy-lang/candy").unwrap(),
                        )
                        .into(),
                        whitespace: vec![
                            Rcst::Whitespace(" ".to_string()),
                            Rcst::Newline,
                            Rcst::Whitespace(" ".to_string())
                        ],
                    },
                    Rcst::UrlLine(Url::parse("https://github.com/candy-lang").unwrap()),
                ]),
            )),
        );
        assert_eq!(urls(vec!["abc"]), None);
    }

    #[instrument]
    fn code_block(input: Vec<&str>) -> Option<(Vec<&str>, Rcst)> {
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
                        Rcst::Newline,
                    ],
                    has_closing_backticks: true,
                },
            )),
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
                    has_closing_backticks: true,
                },
            )),
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
                    has_closing_backticks: false,
                },
            )),
        );
        assert_eq!(code_block(vec!["abc"]), None);
    }

    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
    enum ListType {
        Unordered,
        Ordered,
    }
    #[instrument]
    fn unordered_list_item_marker(line: &str) -> Option<(&str, RcstListItemMarker, usize)> {
        line.strip_prefix('-').map(|line| {
            let (line, has_trailing_space) = line
                .strip_prefix(' ')
                .map_or((line, false), |line| (line, true));
            (
                line,
                RcstListItemMarker::Unordered { has_trailing_space },
                1 + usize::from(has_trailing_space),
            )
        })
    }
    #[instrument]
    fn ordered_list_item_marker(mut line: &str) -> Option<(&str, RcstListItemMarker, usize)> {
        let number = line
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>();
        if number.is_empty() {
            return None;
        }
        line = &line[number.len()..];
        let mut number = Rcst::TextPart(number);

        if let Some((new_line, whitespace)) = single_line_whitespace_raw(line) {
            line = new_line;
            number = Rcst::TrailingWhitespace {
                child: number.into(),
                whitespace: vec![whitespace],
            };
        }

        if line.starts_with('.') {
            line = &line[1..];
        } else {
            return None;
        }

        let has_trailing_space = if line.starts_with(' ') {
            line = &line[1..];
            true
        } else {
            false
        };

        let extra_indentation = format!("{}", number).len() + 1 + usize::from(has_trailing_space);

        Some((
            line,
            RcstListItemMarker::Ordered {
                number: Box::new(number),
                has_trailing_space,
            },
            extra_indentation,
        ))
    }
    #[instrument]
    fn list_item(
        mut input: Vec<&str>,
        mut indentation: usize,
        list_type: Option<ListType>,
    ) -> Option<(Vec<&str>, Rcst, ListType)> {
        let Some((line, remaining)) = input.split_first() else {
            return None;
        };
        let allows_unordered = list_type.map_or(true, |it| it == ListType::Unordered);
        let allows_ordered = list_type.map_or(true, |it| it == ListType::Ordered);
        // TODO: move the `allow_…` before the match checks when Rust's MIR no longer breaks
        let ((line, marker, extra_indentation), list_type) =
            if let Some(marker) = unordered_list_item_marker(line) && allows_unordered  {
                (marker, ListType::Unordered)
            } else if let Some(marker) = ordered_list_item_marker(line) && allows_ordered  {
                (marker, ListType::Ordered)
            } else {
                return None;
            };
        input = recombine(line, remaining);
        indentation += extra_indentation;

        let (input, content) = blocks(input.clone(), indentation).unwrap_or((input, vec![]));
        Some((input, Rcst::ListItem { marker, content }, list_type))
    }
    #[instrument]
    fn list(input: Vec<&str>, indentation: usize) -> Option<(Vec<&str>, Rcst)> {
        let mut list_items = vec![];

        let Some((mut input, first_item, list_type)) = list_item(input, indentation, None) else {
            return None;
        };
        list_items.push(first_item);

        loop {
            let Some((new_input, whitespace)) =
                whitespaces_and_newlines(input.clone(), indentation)
            else {
                break;
            };

            let Some((new_input, new_list_item, new_list_type)) =
                list_item(new_input, indentation, Some(list_type))
            else {
                break;
            };
            assert_eq!(list_type, new_list_type);
            input = new_input;
            let previous_item = list_items.pop().unwrap();
            list_items.push(previous_item.wrap_in_whitespace(whitespace));
            list_items.push(new_list_item);
        }

        Some((input, Rcst::List(list_items)))
    }
    #[test]
    fn test_list() {
        assert_eq!(
            list(vec!["- Foo"], 0),
            Some((
                vec![""],
                Rcst::List(vec![Rcst::ListItem {
                    marker: RcstListItemMarker::Unordered {
                        has_trailing_space: true
                    },
                    content: vec![Rcst::Paragraph(vec![Rcst::TextPart("Foo".to_string())])]
                }]),
            )),
        );
        assert_eq!(
            list(vec!["-Bar"], 0),
            Some((
                vec![""],
                Rcst::List(vec![Rcst::ListItem {
                    marker: RcstListItemMarker::Unordered {
                        has_trailing_space: false
                    },
                    content: vec![Rcst::Paragraph(vec![Rcst::TextPart("Bar".to_string())])]
                }]),
            )),
        );
        assert_eq!(
            list(vec!["0.Foo"], 0),
            Some((
                vec![""],
                Rcst::List(vec![Rcst::ListItem {
                    marker: RcstListItemMarker::Ordered {
                        number: Box::new(Rcst::TextPart("0".to_string())),
                        has_trailing_space: false,
                    },
                    content: vec![Rcst::Paragraph(vec![Rcst::TextPart("Foo".to_string())])]
                }]),
            )),
        );
        assert_eq!(
            list(vec!["0 .  Foo"], 0),
            Some((
                vec![""],
                Rcst::List(vec![Rcst::ListItem {
                    marker: RcstListItemMarker::Ordered {
                        number: Box::new(Rcst::TrailingWhitespace {
                            child: Box::new(Rcst::TextPart("0".to_string())),
                            whitespace: vec![Rcst::Whitespace(" ".to_string())]
                        }),
                        has_trailing_space: true,
                    },
                    content: vec![
                        Rcst::Whitespace(" ".to_string()),
                        Rcst::Paragraph(vec![Rcst::TextPart("Foo".to_string())]),
                    ],
                }]),
            )),
        );
        assert_eq!(
            list(vec!["- Foo", ""], 0),
            Some((
                vec!["", ""],
                Rcst::List(vec![Rcst::ListItem {
                    marker: RcstListItemMarker::Unordered {
                        has_trailing_space: true
                    },
                    content: vec![Rcst::Paragraph(vec![Rcst::TextPart("Foo".to_string())])]
                }]),
            )),
        );
        assert_eq!(
            list(vec!["- Foo", "- Bar"], 0),
            Some((
                vec![""],
                Rcst::List(vec![
                    Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::ListItem {
                            marker: RcstListItemMarker::Unordered {
                                has_trailing_space: true
                            },
                            content: vec![Rcst::Paragraph(vec![Rcst::TextPart("Foo".to_string())])]
                        }),
                        whitespace: vec![Rcst::Newline]
                    },
                    Rcst::ListItem {
                        marker: RcstListItemMarker::Unordered {
                            has_trailing_space: true
                        },
                        content: vec![Rcst::Paragraph(vec![Rcst::TextPart("Bar".to_string())])]
                    },
                ]),
            )),
        );
        assert_eq!(
            list(vec!["0. Foo", "1. Bar"], 0),
            Some((
                vec![""],
                Rcst::List(vec![
                    Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::ListItem {
                            marker: RcstListItemMarker::Ordered {
                                number: Box::new(Rcst::TextPart("0".to_string())),
                                has_trailing_space: true,
                            },
                            content: vec![Rcst::Paragraph(vec![Rcst::TextPart("Foo".to_string())])]
                        }),
                        whitespace: vec![Rcst::Newline]
                    },
                    Rcst::ListItem {
                        marker: RcstListItemMarker::Ordered {
                            number: Box::new(Rcst::TextPart("1".to_string())),
                            has_trailing_space: true,
                        },
                        content: vec![Rcst::Paragraph(vec![Rcst::TextPart("Bar".to_string())])]
                    },
                ]),
            )),
        );
        assert_eq!(
            list(vec!["- item 1", "", "  - item 1a"], 0,),
            Some((
                vec![""],
                Rcst::List(vec![Rcst::ListItem {
                    marker: RcstListItemMarker::Unordered {
                        has_trailing_space: true,
                    },
                    content: vec![
                        Rcst::TrailingWhitespace {
                            child: Box::new(Rcst::Paragraph(vec![Rcst::TextPart(
                                "item 1".to_string(),
                            )])),
                            whitespace: vec![
                                Rcst::Newline,
                                Rcst::Newline,
                                Rcst::Whitespace("  ".to_string()),
                            ],
                        },
                        Rcst::List(vec![Rcst::ListItem {
                            marker: RcstListItemMarker::Unordered {
                                has_trailing_space: true,
                            },
                            content: vec![Rcst::Paragraph(vec![Rcst::TextPart(
                                "item 1a".to_string()
                            )])],
                        }]),
                    ],
                }]),
            )),
        );
        assert_eq!(
            list(
                vec![
                    "- item 1 item item item",
                    "  item item ",
                    "- item 2",
                    "",
                    "  - item 2a",
                    "    item item",
                    "  -item 2b",
                    "",
                    "- item 3"
                ],
                0,
            ),
            Some((
                vec![""],
                Rcst::List(vec![
                    Rcst::TrailingWhitespace {
                        whitespace: vec![Rcst::Newline],
                        child: Box::new(Rcst::ListItem {
                            marker: RcstListItemMarker::Unordered {
                                has_trailing_space: true,
                            },
                            content: vec![Rcst::Paragraph(vec![
                                Rcst::TrailingWhitespace {
                                    child: Box::new(Rcst::TextPart(
                                        "item 1 item item item".to_string(),
                                    )),
                                    whitespace: vec![
                                        Rcst::Newline,
                                        Rcst::Whitespace("  ".to_string())
                                    ]
                                },
                                Rcst::TextPart("item item ".to_string()),
                            ])],
                        }),
                    },
                    Rcst::TrailingWhitespace {
                        whitespace: vec![Rcst::Newline, Rcst::Newline],
                        child: Box::new(Rcst::ListItem {
                            marker: RcstListItemMarker::Unordered {
                                has_trailing_space: true,
                            },
                            content: vec![
                                Rcst::TrailingWhitespace {
                                    child: Box::new(Rcst::Paragraph(vec![Rcst::TextPart(
                                        "item 2".to_string(),
                                    )])),
                                    whitespace: vec![
                                        Rcst::Newline,
                                        Rcst::Newline,
                                        Rcst::Whitespace("  ".to_string()),
                                    ],
                                },
                                Rcst::List(vec![
                                    Rcst::TrailingWhitespace {
                                        child: Box::new(Rcst::ListItem {
                                            marker: RcstListItemMarker::Unordered {
                                                has_trailing_space: true,
                                            },
                                            content: vec![Rcst::Paragraph(vec![
                                                Rcst::TrailingWhitespace {
                                                    child: Box::new(Rcst::TextPart(
                                                        "item 2a".to_string()
                                                    )),
                                                    whitespace: vec![
                                                        Rcst::Newline,
                                                        Rcst::Whitespace("    ".to_string()),
                                                    ]
                                                },
                                                Rcst::TextPart("item item".to_string()),
                                            ]),],
                                        }),
                                        whitespace: vec![
                                            Rcst::Newline,
                                            Rcst::Whitespace("  ".to_string()),
                                        ],
                                    },
                                    Rcst::ListItem {
                                        marker: RcstListItemMarker::Unordered {
                                            has_trailing_space: false
                                        },
                                        content: vec![Rcst::Paragraph(vec![Rcst::TextPart(
                                            "item 2b".to_string()
                                        )])],
                                    },
                                ]),
                            ],
                        }),
                    },
                    Rcst::ListItem {
                        marker: RcstListItemMarker::Unordered {
                            has_trailing_space: true
                        },
                        content: vec![Rcst::Paragraph(vec![Rcst::TextPart("item 3".to_string())])]
                    },
                ]),
            )),
        );
        assert_eq!(list(vec!["abc"], 0), None);
    }

    #[instrument]
    pub fn blocks(mut input: Vec<&str>, indentation: usize) -> Option<(Vec<&str>, Vec<Rcst>)> {
        let mut blocks: Vec<Rcst> = vec![];

        loop {
            let (new_input, whitespace) = if let Some((new_input, whitespace)) =
                whitespaces_and_newlines(input.clone(), indentation)
            {
                (new_input, Some(whitespace))
            } else {
                (input.clone(), None)
            };

            let block = code_block(new_input.clone())
                .or_else(|| {
                    if indentation == 0 {
                        title(new_input.clone())
                    } else {
                        None
                    }
                })
                .or_else(|| {
                    if indentation == 0 {
                        urls(new_input.clone())
                    } else {
                        None
                    }
                })
                .or_else(|| list(new_input.clone(), indentation))
                .or_else(|| paragraph(new_input.clone(), indentation));
            if let Some((new_input, block)) = block {
                if let Some(mut whitespace) = whitespace {
                    if let Some(previous) = blocks.pop() {
                        blocks.push(previous.wrap_in_whitespace(whitespace));
                    } else {
                        blocks.append(&mut whitespace);
                    }
                }

                blocks.push(block);
                input = new_input;
            } else {
                break;
            }
        }

        if indentation == 0 {
            if let Some((new_input, mut whitespace)) =
                whitespaces_and_newlines(input.clone(), indentation).or_else(|| {
                    single_line_whitespace(input.clone())
                        .map(|(new_input, whitespace)| (new_input, vec![whitespace]))
                })
            {
                if let Some(previous) = blocks.pop() {
                    blocks.push(previous.wrap_in_whitespace(whitespace));
                } else {
                    blocks.append(&mut whitespace);
                }
                input = new_input;
            }

            assert!(input == vec![""]);
            input = vec![];
        }

        if blocks.is_empty() {
            None
        } else {
            Some((input, blocks))
        }
    }
    #[test]
    fn test_blocks() {
        assert_eq!(
            blocks(vec!["item 1 item item item", "item item "], 0),
            Some((
                vec![],
                vec![Rcst::Paragraph(vec![
                    Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::TextPart("item 1 item item item".to_string())),
                        whitespace: vec![Rcst::Newline]
                    },
                    Rcst::TextPart("item item ".to_string()),
                ])],
            )),
        );
        assert_eq!(
            blocks(vec!["item 1 item item item", "  item item ", ""], 2),
            Some((
                vec!["", ""],
                vec![Rcst::Paragraph(vec![
                    Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::TextPart("item 1 item item item".to_string())),
                        whitespace: vec![Rcst::Newline, Rcst::Whitespace("  ".to_string())]
                    },
                    Rcst::TextPart("item item ".to_string()),
                ])],
            )),
        );
        assert_eq!(
            blocks(vec!["foo", "", "  bar"], 2),
            Some((
                vec![""],
                vec![
                    Rcst::TrailingWhitespace {
                        child: Box::new(Rcst::Paragraph(vec![Rcst::TextPart("foo".to_string())])),
                        whitespace: vec![
                            Rcst::Newline,
                            Rcst::Newline,
                            Rcst::Whitespace("  ".to_string()),
                        ]
                    },
                    Rcst::Paragraph(vec![Rcst::TextPart("bar".to_string()),])
                ],
            )),
        );
    }

    fn recombine<'a>(first_line: &'a str, remaining_lines: &[&'a str]) -> Vec<&'a str> {
        let mut lines = vec![first_line];
        lines.extend_from_slice(remaining_lines);
        lines
    }
}
