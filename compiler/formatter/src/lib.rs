#![feature(anonymous_lifetime_in_impl_trait)]
#![feature(let_chains)]

use candy_frontend::{
    cst::{Cst, CstData, CstError, CstKind, Id, IsMultiline},
    id::{CountableId, IdGenerator},
    position::Offset,
};
use existing_whitespace::SplitTrailingWhitespace;
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
        state.format_csts(self.as_ref().iter(), &FormatterInfo::default())
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
#[derive(Default)]
struct FormatterInfo {
    indentation_level: usize,
    trailing_comma_condition: Option<TrailingCommaCondition>,
}
#[derive(Clone, Copy)]
enum TrailingCommaCondition {
    Always,

    /// Add a trailing comma if the element fits in a single line and is at most
    /// this wide.
    IfFitsIn(usize),
}
impl FormatterInfo {
    fn with_indent(&self) -> Self {
        Self {
            indentation_level: self.indentation_level + 1,
            trailing_comma_condition: self.trailing_comma_condition,
        }
    }
    fn with_trailing_comma_condition(&self, condition: TrailingCommaCondition) -> Self {
        Self {
            indentation_level: self.indentation_level,
            trailing_comma_condition: Some(condition),
        }
    }
}

struct FormatterState {
    id_generator: IdGenerator<Id>,
}
impl FormatterState {
    fn format_csts(&mut self, csts: impl AsRef<[Cst]>, info: &FormatterInfo) -> Vec<Cst> {
        let mut result = vec![];

        let mut saw_non_whitespace = false;
        let mut empty_line_count = 0;
        let csts = csts.as_ref();
        let mut index = 0;
        'outer: while index < csts.len() {
            let cst = &csts[index];

            if let CstKind::Newline(_) = cst.kind {
                // Remove leading newlines and limit to at most two empty lines.
                if saw_non_whitespace && empty_line_count <= 2 {
                    result.push(cst.to_owned());
                    empty_line_count += 1;
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

            if info.indentation_level > 0 {
                result.push(Cst {
                    data: CstData {
                        id: indentation_id.unwrap_or_else(|| self.id_generator.generate()),
                        span: Range::default(),
                    },
                    kind: CstKind::Whitespace("  ".repeat(info.indentation_level)),
                });
            }

            result.push(self.format_cst(not_whitespace, info));
            index += 1;
            saw_non_whitespace = true;
            empty_line_count = 0;

            let mut trailing_whitespace_id = None;
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
                        trailing_whitespace_id = Some(next.data.id);
                    }
                    CstKind::Newline(_) => break,
                    CstKind::Comment { .. } => {
                        // A comment in the same line.
                        result.push(Cst {
                            data: CstData {
                                id: trailing_whitespace_id
                                    .unwrap_or_else(|| self.id_generator.generate()),
                                span: Range::default(),
                            },
                            kind: CstKind::Whitespace(" ".to_string()),
                        });

                        result.push(self.format_cst(next, info));
                        index += 1;
                    }
                    _ => {
                        // Another expression without a newline in between.
                        result.push(Cst {
                            data: CstData {
                                id: self.id_generator.generate(),
                                span: Range::default(),
                            },
                            kind: CstKind::Newline("\n".to_string()),
                        });

                        result.push(self.format_cst(next, info));
                        index += 1;
                    }
                }
            }
        }

        // Add trailing newline.
        if let Some(last) = result.last() && !matches!(
            last,
            Cst {
                kind: CstKind::Newline(_),
                ..
            },
        ) {
            result.push(Cst {
                data: CstData {
                    id: self.id_generator.generate(),
                    span: Range::default(),
                },
                kind: CstKind::Newline("\n".to_string()),
            });
        }

        result
    }

    fn format_cst(&mut self, cst: &Cst, info: &FormatterInfo) -> Cst {
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
            CstKind::Identifier(_)
            | CstKind::Symbol(_)
            | CstKind::Int { .. }
            | CstKind::OpeningText { .. }
            | CstKind::ClosingText { .. } => return cst.to_owned(),
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
                let receiver = self.format_cst(receiver, info);

                let mut arguments = arguments
                    .iter()
                    .map(|argument| {
                        let (argument, argument_whitespace) = argument.split_trailing_whitespace();
                        let argument = self.format_cst(argument, &info.with_indent());
                        (argument, argument_whitespace)
                    })
                    .collect_vec();

                let are_arguments_singleline = !receiver_whitespace.has_comments()
                    && arguments.iter().all(|(argument, argument_whitespace)| {
                        argument.is_singleline() && !argument_whitespace.has_comments()
                    })
                    && arguments
                        .iter()
                        .map(|(it, _)| 1 + it.last_line_width())
                        .sum::<usize>()
                        + receiver.last_line_width()
                        <= MAX_LINE_LENGTH;
                let indentation_level = if are_arguments_singleline {
                    None
                } else {
                    Some(info.indentation_level + 1)
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
            CstKind::StructAccess { struct_, dot, key } => {
                let (struct_, struct_whitespace) = struct_.split_trailing_whitespace();
                let struct_ = self.format_cst(struct_, info);

                let (dot, dot_whitespace) = dot.split_trailing_whitespace();
                let dot = self.format_cst(dot, &info.with_indent());
                assert!(dot.is_singleline());
                let struct_whitespace = dot_whitespace.merge_into(struct_whitespace);

                let key = self.format_cst(key, &info.with_indent());
                assert!(key.is_singleline());

                let is_access_singleline = !struct_whitespace.has_comments()
                    && struct_.last_line_width() + dot.last_line_width() + key.last_line_width()
                        <= MAX_LINE_LENGTH;
                let struct_ = if is_access_singleline {
                    struct_
                } else {
                    struct_whitespace.into_trailing_with_indentation(
                        &mut self.id_generator,
                        struct_,
                        info.indentation_level + 1,
                    )
                };

                CstKind::StructAccess {
                    struct_: Box::new(struct_),
                    dot: Box::new(dot),
                    key: Box::new(key),
                }
            }
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

#[cfg(test)]
mod test {
    use crate::Formatter;
    use candy_frontend::{rcst_to_cst::RcstsToCstsExt, string_to_rcst::parse_rcst};
    use itertools::Itertools;

    #[test]
    fn test_csts() {
        test(" ", "");
        test("foo", "foo\n");
        test("foo\n", "foo\n");

        // Consecutive newlines
        test("foo\nbar", "foo\nbar\n");
        test("foo\n\nbar", "foo\n\nbar\n");
        test("foo\n\n\nbar", "foo\n\n\nbar\n");
        test("foo\n\n\n\nbar", "foo\n\n\nbar\n");
        test("foo\n\n\n\n\nbar", "foo\n\n\nbar\n");

        // Consecutive expressions
        test("foo\nbar\nbaz", "foo\nbar\nbaz\n");
        test("foo\n bar", "foo\nbar\n");
        test("foo\n \nbar", "foo\n\nbar\n");
        test("foo ", "foo\n");

        // Leading newlines
        test(" \nfoo", "foo\n");
        test("  \nfoo", "foo\n");
        test(" \n  \n foo", "foo\n");

        // Trailing newlines
        test("foo\n ", "foo\n");
        test("foo\n  ", "foo\n");
        test("foo \n  ", "foo\n");
        test("foo\n\n", "foo\n");
        test("foo\n \n ", "foo\n");

        // Comments
        test("# abc\nfoo", "# abc\nfoo\n");
        test("foo# abc", "foo # abc\n");
        test("foo # abc", "foo # abc\n");
        test("foo\n# abc", "foo\n# abc\n");
        test("foo\n # abc", "foo\n# abc\n");
    }
    #[test]
    fn test_int() {
        test("1", "1\n");
        test("123", "123\n");
    }
    #[test]
    fn test_call() {
        test("foo bar Baz", "foo bar Baz\n");
        test("foo   bar Baz ", "foo bar Baz\n");
        test("foo   bar Baz ", "foo bar Baz\n");
        test(
            "foo firstVeryVeryVeryVeryVeryVeryVeryVeryLongArgument secondVeryVeryVeryVeryVeryVeryVeryVeryLongArgument",
            "foo\n  firstVeryVeryVeryVeryVeryVeryVeryVeryLongArgument\n  secondVeryVeryVeryVeryVeryVeryVeryVeryLongArgument\n",
        );

        test("foo # abc\n  bar\n  Baz", "foo # abc\n  bar\n  Baz\n");
        test("foo\n  bar # abc\n  Baz", "foo\n  bar # abc\n  Baz\n");
    }
    #[test]
    fn test_struct_access() {
        test("foo.bar", "foo.bar\n");
        test("foo.bar.baz", "foo.bar.baz\n");
        test("foo . bar. baz .blub ", "foo.bar.baz.blub\n");
        test(
            "foo.firstVeryVeryVeryVeryVeryVeryVeryVeryLongArgument.secondVeryVeryVeryVeryVeryVeryVeryVeryLongArgument",
            "foo.firstVeryVeryVeryVeryVeryVeryVeryVeryLongArgument\n  .secondVeryVeryVeryVeryVeryVeryVeryVeryLongArgument\n",
        );
        test(
            "foo.firstVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongArgument.secondVeryVeryVeryVeryVeryVeryVeryVeryLongArgument",
            "foo\n  .firstVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongArgument\n  .secondVeryVeryVeryVeryVeryVeryVeryVeryLongArgument\n",
        );

        // Comments
        test("foo# abc\n  .bar", "foo # abc\n  .bar\n");
        test("foo # abc\n  .bar", "foo # abc\n  .bar\n");
        test("foo  # abc\n  .bar", "foo # abc\n  .bar\n");
        test("foo .# abc\n  bar", "foo # abc\n  .bar\n");
        test("foo . # abc\n  bar", "foo # abc\n  .bar\n");
        test("foo .bar# abc", "foo.bar # abc\n");
        test("foo .bar # abc", "foo.bar # abc\n");
    }

    fn test(source: &str, expected: &str) {
        let csts = parse_rcst(source).to_csts();
        assert_eq!(source, csts.iter().join(""));

        let formatted = csts.as_slice().format_to_string();
        assert_eq!(formatted, expected);
    }
}
