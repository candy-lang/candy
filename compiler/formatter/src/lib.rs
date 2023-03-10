#![feature(anonymous_lifetime_in_impl_trait)]

use candy_frontend::{
    cst::{Cst, CstData, CstError, CstKind, Id, IsMultiline},
    id::{CountableId, IdGenerator},
    position::Offset,
};
use existing_whitespace::ExistingWhitespace;
use extension_trait::extension_trait;
use itertools::Itertools;
use last_line_width::LastLineWidth;
use std::ops::Range;
use traversal::dft_pre;

mod existing_whitespace;
mod last_line_width;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TextEdit {
    pub range: Range<Offset>,
    pub new_text: String,
}

pub const MAX_LINE_LENGTH: usize = 100;

#[extension_trait]
pub impl<C: AsRef<[Cst]>> Formatter for C {
    fn format_to_string(&self) -> String {
        self.format().iter().join("")
    }
    fn format_to_edits(&self) -> Vec<TextEdit> {
        todo!()
    }
    fn format(&self) -> Vec<Cst> {
        let id_generator = IdGenerator::start_at(largest_id(self.as_ref()).to_usize() + 1);
        let mut state = FormatterState { id_generator };
        state.format_csts(self.as_ref().iter(), 0)
        // TODO: fix spans
    }
}

fn largest_id(csts: &[Cst]) -> Id {
    csts.iter()
        .map(|it| {
            dft_pre(it, |it| it.kind.children().into_iter())
                .map(|(_, it)| it.data.id)
                .max()
                .unwrap()
        })
        .max()
        .unwrap()
}

struct FormatterState {
    id_generator: IdGenerator<Id>,
}
impl FormatterState {
    fn format_csts(&mut self, csts: impl AsRef<[Cst]>, indentation_level: usize) -> Vec<Cst> {
        let mut result = vec![];

        let mut saw_non_whitespace = false;
        let csts = csts.as_ref();
        let mut index = 0;
        'outer: while index < csts.len() {
            let cst = &csts[index];

            if let CstKind::Newline(_) = cst.kind {
                if saw_non_whitespace {
                    // Remove leading newlines.
                    result.push(cst.to_owned());
                }
                index += 1;

                if csts[index..].iter().all(|it| {
                    matches!(
                        it.kind,
                        CstKind::Whitespace(_)
                            | CstKind::Error {
                                error: CstError::TooMuchWhitespace,
                                ..
                            }
                            | CstKind::Newline(_),
                    )
                }) {
                    // Remove trailing newlines and whitespace.
                    break 'outer;
                }

                continue;
            }

            // Indentation
            let (mut cst, indentation_id) = if let CstKind::Whitespace(_)
            | CstKind::Error {
                error: CstError::TooMuchWhitespace,
                ..
            } = &cst.kind
            {
                index += 1;
                (csts.get(index), Some(cst.data.id))
            } else {
                (Some(cst), None)
            };

            // Remove more whitespaces before an actual expression or comment.
            let not_whitespace = loop {
                let Some(next) = cst else {
                    // Remove whitespace at the end of the file.
                    break 'outer;
                };

                match next.kind {
                    CstKind::Whitespace(_)
                    | CstKind::Error {
                        error: CstError::TooMuchWhitespace,
                        ..
                    } => {
                        // Remove multiple sequential whitespaces.
                        index += 1;
                        cst = csts.get(index);
                    }
                    CstKind::Newline(_) => {
                        // Remove indentation when it is followed by a newline.
                        continue 'outer;
                    }
                    _ => break next,
                }
            };

            if indentation_level > 0 {
                result.push(Cst {
                    data: CstData {
                        id: indentation_id.unwrap_or_else(|| self.id_generator.generate()),
                        span: Range::default(),
                    },
                    kind: CstKind::Whitespace("  ".repeat(indentation_level)),
                });
            }

            result.push(self.format_cst(not_whitespace, indentation_level));
            index += 1;
            saw_non_whitespace = true;

            loop {
                let Some(next) = csts.get(index) else { break; };

                match next.kind {
                    CstKind::Whitespace(_)
                    | CstKind::Error {
                        error: CstError::TooMuchWhitespace,
                        ..
                    } => {
                        // Remove whitespace after an expression or comment.
                        index += 1;
                    }
                    CstKind::Newline(_) => break,
                    _ => {
                        // Another expression without a newline in between.
                        result.push(Cst {
                            data: CstData {
                                id: self.id_generator.generate(),
                                span: Range::default(),
                            },
                            kind: CstKind::Newline("\n".to_string()),
                        });

                        result.push(self.format_cst(next, indentation_level));
                        index += 1;
                    }
                }
            }
        }
        result
    }

    fn format_cst(&mut self, cst: &Cst, indentation_level: usize) -> Cst {
        let new_kind = match &cst.kind {
            CstKind::EqualsSign
            | CstKind::Comma
            | CstKind::Dot
            | CstKind::Colon
            | CstKind::ColonEqualsSign
            | CstKind::Bar
            | CstKind::OpeningParenthesis
            | CstKind::ClosingParenthesis
            | CstKind::OpeningBracket
            | CstKind::ClosingBracket
            | CstKind::OpeningCurlyBrace
            | CstKind::ClosingCurlyBrace
            | CstKind::Arrow
            | CstKind::SingleQuote
            | CstKind::DoubleQuote
            | CstKind::Percent
            | CstKind::Octothorpe
            | CstKind::Whitespace(_)
            | CstKind::Newline(_)
            | CstKind::Comment { .. } => return cst.to_owned(),
            CstKind::TrailingWhitespace { child, whitespace } => todo!(),
            CstKind::Identifier(_) | CstKind::Symbol(_) => return cst.to_owned(),
            CstKind::Int { value, string } => return cst.to_owned(), // TODO
            CstKind::OpeningText {
                opening_single_quotes,
                opening_double_quote,
            } => todo!(),
            CstKind::ClosingText {
                closing_double_quote,
                closing_single_quotes,
            } => todo!(),
            CstKind::Text {
                opening,
                parts,
                closing,
            } => todo!(),
            CstKind::TextPart(_) => todo!(),
            CstKind::TextInterpolation {
                opening_curly_braces,
                expression,
                closing_curly_braces,
            } => todo!(),
            CstKind::BinaryBar { left, bar, right } => todo!(),
            CstKind::Parenthesized {
                opening_parenthesis,
                inner,
                closing_parenthesis,
            } => todo!(),
            CstKind::Call {
                receiver,
                arguments,
            } => {
                let (receiver, receiver_whitespace) = receiver.split_trailing_whitespace();
                let receiver = self.format_cst(receiver, indentation_level);

                let mut arguments = arguments
                    .iter()
                    .map(|argument| {
                        let (argument, argument_whitespace) = argument.split_trailing_whitespace();
                        let argument = self.format_cst(argument, indentation_level + 1);
                        (argument, argument_whitespace)
                    })
                    .collect_vec();

                let indentation_level = if arguments.iter().all(|(it, _)| it.is_singleline())
                    && arguments
                        .iter()
                        .map(|(it, _)| 1 + it.last_line_width())
                        .sum::<usize>()
                        + receiver.last_line_width()
                        <= MAX_LINE_LENGTH
                {
                    None
                } else {
                    Some(indentation_level + 1)
                };

                let receiver = receiver_whitespace.into_trailing(
                    &mut self.id_generator,
                    receiver,
                    indentation_level,
                );

                let last_argument = arguments.pop().unwrap().0;
                let mut arguments = arguments
                    .into_iter()
                    .map(|(argument, argument_whitespace)| {
                        argument_whitespace.into_trailing(
                            &mut self.id_generator,
                            argument,
                            indentation_level,
                        )
                    })
                    .collect_vec();
                arguments.push(last_argument);

                CstKind::Call {
                    receiver: Box::new(receiver),
                    arguments,
                }
            }
            CstKind::List {
                opening_parenthesis,
                items,
                closing_parenthesis,
            } => todo!(),
            CstKind::ListItem { value, comma } => todo!(),
            CstKind::Struct {
                opening_bracket,
                fields,
                closing_bracket,
            } => todo!(),
            CstKind::StructField {
                key_and_colon,
                value,
                comma,
            } => todo!(),
            CstKind::StructAccess { struct_, dot, key } => todo!(),
            CstKind::Match {
                expression,
                percent,
                cases,
            } => todo!(),
            CstKind::MatchCase {
                pattern,
                arrow,
                body,
            } => todo!(),
            CstKind::Lambda {
                opening_curly_brace,
                parameters_and_arrow,
                body,
                closing_curly_brace,
            } => todo!(),
            CstKind::Assignment {
                left,
                assignment_sign,
                body,
            } => todo!(),
            CstKind::Error { .. } => return cst.to_owned(),
        };
        Cst {
            data: cst.data.clone(),
            kind: new_kind,
        }
    }
}

#[extension_trait]
impl FormatCstExtension for Cst {
    fn split_trailing_whitespace(&self) -> (&Cst, ExistingWhitespace) {
        match &self.kind {
            CstKind::TrailingWhitespace { child, whitespace } => (
                child,
                ExistingWhitespace::Some {
                    id: self.data.id,
                    trailing_whitespace: whitespace,
                },
            ),
            _ => (self, ExistingWhitespace::None),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::Formatter;
    use candy_frontend::{rcst_to_cst::RcstsToCstsExt, string_to_rcst::parse_rcst};
    use itertools::Itertools;

    #[test]
    fn test_csts() {
        test("foo", "foo");

        test("foo\nbar", "foo\nbar");
        test("foo\n\nbar", "foo\n\nbar");
        test("foo\n\n\nbar", "foo\n\n\nbar");
        // test("foo\n\n\n\nbar", "foo\n\n\nbar"); // TODO

        test("foo\nbar\nbaz", "foo\nbar\nbaz");
        test("foo\n bar", "foo\nbar");
        test("foo\n \nbar", "foo\n\nbar");
        test("foo ", "foo");

        test(" ", "");

        // Leading newlines
        test(" \nfoo", "foo");
        test("  \nfoo", "foo");
        test(" \n  \n foo", "foo");

        // Trailing newlines
        test("foo\n ", "foo\n");
        test("foo\n  ", "foo\n");
        test("foo \n  ", "foo\n");
        test("foo\n\n", "foo\n");
        test("foo\n \n ", "foo\n");
    }
    #[test]
    fn test_int() {
        test("1", "1");
        test("123", "123");
    }
    #[test]
    fn test_call() {
        test("foo bar Baz", "foo bar Baz");
        test("foo   bar Baz ", "foo bar Baz");
        test("foo   bar Baz ", "foo bar Baz");
        test(
"foo firstVeryVeryVeryVeryVeryVeryVeryVeryLongArgument secondVeryVeryVeryVeryVeryVeryVeryVeryLongArgument",
            "foo\n  firstVeryVeryVeryVeryVeryVeryVeryVeryLongArgument\n  secondVeryVeryVeryVeryVeryVeryVeryVeryLongArgument",
        );
    }

    fn test(source: &str, expected: &str) {
        let csts = parse_rcst(source).to_csts();
        assert_eq!(source, csts.iter().join(""));

        let formatted = csts.as_slice().format_to_string();
        assert_eq!(formatted, expected);
    }
}
