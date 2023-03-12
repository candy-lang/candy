#![feature(anonymous_lifetime_in_impl_trait)]
#![feature(let_chains)]

use crate::last_line_width::HasLastLineWidthInfo;
use candy_frontend::{
    cst::{Cst, CstData, CstError, CstKind, Id, IsMultiline},
    id::{CountableId, IdGenerator},
    position::Offset,
};
use existing_whitespace::{ExistingWhitespace, SplitTrailingWhitespace, TrailingWhitespace};
use extension_trait::extension_trait;
use itertools::Itertools;
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

#[derive(Clone, Copy, Default)]
pub struct Indentation(usize);
impl Indentation {
    pub fn level(self) -> usize {
        self.0
    }
    pub fn width(self) -> usize {
        self.0 * 2
    }
    pub fn is_indented(self) -> bool {
        self.0 > 0
    }

    pub fn with_indent(self) -> Self {
        Self(self.0 + 1)
    }

    pub fn to_cst_kind<D>(self) -> CstKind<D> {
        CstKind::Whitespace(" ".repeat(self.width()))
    }
}

#[derive(Clone, Default)]
struct FormatterInfo {
    indentation: Indentation,
    trailing_comma_condition: Option<TrailingCommaCondition>,
}
#[derive(Clone, Copy)]
enum TrailingCommaCondition {
    Always,

    /// Add a trailing comma if the element fits in a single line and is at most
    /// this wide.
    UnlessFitsIn(usize),
}
impl FormatterInfo {
    fn with_indent(&self) -> Self {
        Self {
            indentation: self.indentation.with_indent(),
            // Only applies for direct descendants.
            trailing_comma_condition: None,
        }
    }
    fn with_trailing_comma_condition(&self, condition: TrailingCommaCondition) -> Self {
        Self {
            indentation: self.indentation,
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
        let mut pending_newlines = vec![];
        'outer: while index < csts.len() {
            let cst = &csts[index];

            if let CstKind::Newline(_) = cst.kind {
                // Remove leading newlines and limit to at most two empty lines.
                if saw_non_whitespace && empty_line_count <= 2 {
                    pending_newlines.push(cst.to_owned());
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

            result.append(&mut pending_newlines);

            // In indented bodies, the indentation of the first line is taken care of by the caller.
            if saw_non_whitespace && info.indentation.is_indented() {
                result.push(Cst {
                    data: CstData {
                        id: indentation_id.unwrap_or_else(|| self.id_generator.generate()),
                        span: Range::default(),
                    },
                    kind: info.indentation.to_cst_kind(),
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

        // Add trailing newline (only for top-level bodies).
        if !info.indentation.is_indented() && !result.is_empty() {
            let trailing_newline = pending_newlines.pop().unwrap_or_else(|| Cst {
                data: CstData {
                    id: self.id_generator.generate(),
                    span: Range::default(),
                },
                kind: CstKind::Newline("\n".to_string()),
            });
            result.push(trailing_newline);
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
            } => {
                let (opening_parenthesis, opening_parenthesis_whitespace) =
                    self.format_child(opening_parenthesis, info);
                assert!(opening_parenthesis.is_singleline());

                let (inner, inner_whitespace) = self.format_child(inner, &info.with_indent());

                let (closing_parenthesis, closing_parenthesis_whitespace) =
                    self.format_child(closing_parenthesis, info);
                assert!(closing_parenthesis.is_singleline());

                let is_singleline = !opening_parenthesis_whitespace.has_comments()
                    && inner.is_singleline()
                    && !inner_whitespace.has_comments()
                    && !closing_parenthesis_whitespace.has_comments()
                    && info.indentation.width()
                        + opening_parenthesis.last_line_width()
                        + inner.last_line_width()
                        + closing_parenthesis.last_line_width()
                        <= MAX_LINE_LENGTH;
                let (opening_parenthesis_trailing, inner_trailing) = if is_singleline {
                    (TrailingWhitespace::None, TrailingWhitespace::None)
                } else {
                    (
                        TrailingWhitespace::Indentation(info.indentation.with_indent()),
                        TrailingWhitespace::Indentation(info.indentation),
                    )
                };

                CstKind::Parenthesized {
                    opening_parenthesis: Box::new(opening_parenthesis_whitespace.into_trailing(
                        &mut self.id_generator,
                        opening_parenthesis,
                        opening_parenthesis_trailing,
                    )),
                    inner: Box::new(inner_whitespace.into_trailing(
                        &mut self.id_generator,
                        inner,
                        inner_trailing,
                    )),
                    closing_parenthesis: Box::new(
                        closing_parenthesis_whitespace.into_empty_trailing(closing_parenthesis),
                    ),
                }
            }
            CstKind::Call {
                receiver,
                arguments,
            } => {
                let (receiver, receiver_whitespace) = self.format_child(receiver, info);

                let mut arguments = arguments
                    .iter()
                    .map(|argument| self.format_child(argument, &info.with_indent()))
                    .collect_vec();

                let are_arguments_singleline = !receiver_whitespace.has_comments()
                    && arguments.iter().all(|(argument, argument_whitespace)| {
                        argument.is_singleline() && !argument_whitespace.has_comments()
                    })
                    && info.indentation.width()
                        + receiver.last_line_width()
                        + arguments
                            .iter()
                            .map(|(it, _)| 1 + it.last_line_width())
                            .sum::<usize>()
                        <= MAX_LINE_LENGTH;
                let trailing = if are_arguments_singleline {
                    TrailingWhitespace::Space
                } else {
                    TrailingWhitespace::Indentation(info.indentation.with_indent())
                };

                let receiver = receiver_whitespace.into_trailing(
                    &mut self.id_generator,
                    receiver,
                    trailing.clone(),
                );

                let last_argument = arguments.pop().unwrap().0;
                let mut arguments = arguments
                    .into_iter()
                    .map(|(argument, argument_whitespace)| {
                        argument_whitespace.into_trailing(
                            &mut self.id_generator,
                            argument,
                            trailing.clone(),
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
            } => {
                let (opening_parenthesis, opening_parenthesis_whitespace) =
                    self.format_child(opening_parenthesis, info);
                assert!(opening_parenthesis.is_singleline());

                let (closing_parenthesis, closing_parenthesis_whitespace) =
                    self.format_child(closing_parenthesis, info);
                assert!(closing_parenthesis.is_singleline());

                // As soon as we find out that the list has to be multiline, we no longer track the
                // exact width.
                let mut width = if opening_parenthesis_whitespace.has_comments() {
                    None
                } else {
                    Some(
                        info.indentation.width()
                            + opening_parenthesis.last_line_width()
                            + closing_parenthesis.last_line_width(),
                    )
                };
                let item_info = info
                    .with_indent()
                    .with_trailing_comma_condition(TrailingCommaCondition::Always);
                let items = items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        let is_single_item = items.len() == 1;
                        let is_last_item = index == items.len() - 1;

                        let info = if is_last_item && !is_single_item && let Some(width) = width {
                            // We're looking at the last item and everything might fit in one line.
                            let max_width = MAX_LINE_LENGTH - width;
                            assert!(max_width > 0);

                            item_info.with_trailing_comma_condition(
                                TrailingCommaCondition::UnlessFitsIn(max_width),
                            )
                        } else {
                            item_info.clone()
                        };
                        let (item, item_whitespace) = self.format_child(item, &info);

                        if let Some(old_width) = width {
                            if item.is_multiline() || item_whitespace.has_comments() {
                                width = None;
                            } else {
                                let (new_width, max_width) = if is_single_item || is_last_item {
                                    (old_width + item.last_line_width(), MAX_LINE_LENGTH)
                                } else {
                                    // We need an additional column for the trailing space after the
                                    // comma.
                                    let new_width = old_width + item.last_line_width() + 1;

                                    // The last item needs at least one column of space.
                                    let max_width = MAX_LINE_LENGTH - 1;

                                    (new_width, max_width)
                                };
                                if new_width > max_width {
                                    width = None;
                                } else {
                                    width = Some(new_width);
                                }
                            }
                        }

                        (item, item_whitespace)
                    })
                    .collect_vec();
                if let Some(width) = width {
                    assert!(width <= MAX_LINE_LENGTH);
                }

                let (opening_parenthesis_trailing, item_trailing, last_item_trailing) =
                    if width.is_some() {
                        (
                            TrailingWhitespace::None,
                            TrailingWhitespace::Space,
                            TrailingWhitespace::None,
                        )
                    } else {
                        (
                            TrailingWhitespace::Indentation(info.indentation.with_indent()),
                            TrailingWhitespace::Indentation(info.indentation.with_indent()),
                            TrailingWhitespace::Indentation(info.indentation),
                        )
                    };

                let last_item_index = items.len() - 1;
                CstKind::List {
                    opening_parenthesis: Box::new(opening_parenthesis_whitespace.into_trailing(
                        &mut self.id_generator,
                        opening_parenthesis,
                        opening_parenthesis_trailing,
                    )),
                    items: items
                        .into_iter()
                        .enumerate()
                        .map(|(index, (item, item_whitespace))| {
                            item_whitespace.into_trailing(
                                &mut self.id_generator,
                                item,
                                if index == last_item_index {
                                    last_item_trailing.clone()
                                } else {
                                    item_trailing.clone()
                                },
                            )
                        })
                        .collect(),
                    closing_parenthesis: Box::new(
                        closing_parenthesis_whitespace.into_empty_trailing(closing_parenthesis),
                    ),
                }
            }
            CstKind::ListItem { value, comma } => {
                let (value, value_whitespace) = self.format_child(value, info);

                let should_have_comma = match info.trailing_comma_condition {
                    Some(TrailingCommaCondition::Always) => true,
                    Some(TrailingCommaCondition::UnlessFitsIn(max_width)) => {
                        value.is_multiline()
                            || value_whitespace.has_comments()
                            || value.last_line_width() > max_width
                    }
                    None => comma.is_some(),
                };
                let comma = if should_have_comma {
                    let comma = comma
                        .as_ref()
                        .map(|it| self.format_cst(it, info))
                        .unwrap_or_else(|| Cst {
                            data: CstData {
                                id: self.id_generator.generate(),
                                span: Range::default(),
                            },
                            kind: CstKind::Comma,
                        });
                    Some(Box::new(comma))
                } else {
                    None
                };

                CstKind::ListItem {
                    value: Box::new(value),
                    comma,
                }
            }
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
                let (struct_, struct_whitespace) = self.format_child(struct_, info);

                let (dot, dot_whitespace) = self.format_child(dot, &info.with_indent());
                assert!(dot.is_singleline());
                let struct_whitespace = dot_whitespace.merge_into(struct_whitespace);

                let key = self.format_cst(key, &info.with_indent());
                assert!(key.is_singleline());

                let is_access_singleline = !struct_whitespace.has_comments()
                    && info.indentation.width()
                        + struct_.last_line_width()
                        + dot.last_line_width()
                        + key.last_line_width()
                        <= MAX_LINE_LENGTH;
                let struct_ = if is_access_singleline {
                    struct_
                } else {
                    struct_whitespace.into_trailing_with_indentation(
                        &mut self.id_generator,
                        struct_,
                        info.indentation.with_indent(),
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
            } => {
                let (left, left_whitespace) = self.format_child(left, info);
                let left_trailing = if left_whitespace.has_comments() {
                    TrailingWhitespace::Indentation(info.indentation.with_indent())
                } else {
                    TrailingWhitespace::Space
                };
                let left =
                    left_whitespace.into_trailing(&mut self.id_generator, left, left_trailing);

                let (assignment_sign, assignment_sign_whitespace) =
                    self.format_child(assignment_sign, &info.with_indent());
                assert!(assignment_sign.is_singleline());

                let body = self.format_csts(body, &info.with_indent());

                let is_body_in_same_line = !assignment_sign_whitespace.has_comments()
                    && body.is_singleline()
                    && info.indentation.width()
                        + left.last_line_width()
                        + assignment_sign.last_line_width()
                        + 1
                        + body.last_line_width()
                        <= MAX_LINE_LENGTH;
                let assignment_sign_trailing = if is_body_in_same_line {
                    TrailingWhitespace::Space
                } else {
                    TrailingWhitespace::Indentation(info.indentation.with_indent())
                };
                let assignment_sign = assignment_sign_whitespace.into_trailing(
                    &mut self.id_generator,
                    assignment_sign,
                    assignment_sign_trailing,
                );

                CstKind::Assignment {
                    left: Box::new(left),
                    assignment_sign: Box::new(assignment_sign),
                    body,
                }
            }
            CstKind::Error { .. } => return cst.to_owned(),
        };
        Cst {
            data: cst.data.clone(),
            kind: new_kind,
        }
    }

    fn format_child<'a>(
        &mut self,
        child: &'a Cst,
        info: &FormatterInfo,
    ) -> (Cst, ExistingWhitespace<'a>) {
        let (child, child_whitespace) = child.split_trailing_whitespace();
        let child = self.format_cst(child.as_ref(), info);
        (child, child_whitespace)
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
    fn test_parenthesized() {
        test("(foo)", "(foo)\n");
        test(" ( foo ) ", "(foo)\n");
        test("(\n  foo)", "(foo)\n");
        test("(\n  foo\n)", "(foo)\n");
        test(
            "(veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryItemmm)",
            "(veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryItemmm)\n",
        );
        test(
            "(veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryItemmmm)",
            "(\n  veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryItemmmm\n)\n",
        );
        test(
            "(\n  veryVeryVeryVeryVeryVeryVeryVeryLongReceiver veryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongArgumentt)",
            "(veryVeryVeryVeryVeryVeryVeryVeryLongReceiver veryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongArgumentt)\n",
        );
        test(
            "(veryVeryVeryVeryVeryVeryVeryVeryLongReceiver veryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongArgumenttt)",
            "(\n  veryVeryVeryVeryVeryVeryVeryVeryLongReceiver\n    veryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongArgumenttt\n)\n",
        );

        // Comments
        test("(foo) # abc", "(foo) # abc\n");
        test("(foo)# abc", "(foo) # abc\n");
        test("(foo# abc\n)", "(\n  foo # abc\n)\n");
        test("(foo # abc\n)", "(\n  foo # abc\n)\n");
        test("(# abc\n  foo)", "( # abc\n  foo\n)\n");
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
    fn test_list() {
        // Empty
        test("(,)", "(,)\n");
        test(" ( , ) ", "(,)\n");
        test("(\n  , ) ", "(,)\n");
        test("(\n  ,\n) ", "(,)\n");

        // Single item
        test("(foo,)", "(foo,)\n");
        test("(foo,)\n", "(foo,)\n");
        test("(foo, )\n", "(foo,)\n");
        test("(foo ,)\n", "(foo,)\n");
        test("( foo, )\n", "(foo,)\n");
        test("(foo,)\n", "(foo,)\n");
        test("(\n  foo,\n)\n", "(foo,)\n");
        test("(\n  foo,\n)\n", "(foo,)\n");
        test(" ( foo , ) \n", "(foo,)\n");
        test(
            "(veryVeryVeryVeryVeryVeryVeryVeryLongVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongItemm,)",
            "(veryVeryVeryVeryVeryVeryVeryVeryLongVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongItemm,)\n",
        );
        test(
            "(veryVeryVeryVeryVeryVeryVeryVeryLongVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongItemmm,)",
            "(\n  veryVeryVeryVeryVeryVeryVeryVeryLongVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongItemmm,\n)\n",
        );

        // Multiple items
        test("(foo, bar)", "(foo, bar)\n");
        test("(foo, bar,)", "(foo, bar)\n");
        test("(foo, bar, baz)", "(foo, bar, baz)\n");
        test("(foo, bar, baz,)", "(foo, bar, baz)\n");
        test("( foo ,bar ,baz , )", "(foo, bar, baz)\n");
        test("(\n  foo,\n  bar,\n  baz,\n)", "(foo, bar, baz)\n");
        test(
            "(firstVeryVeryVeryVeryVeryVeryVeryVeryLongVeryItem, secondVeryVeryVeryVeryVeryVeryVeryVeryVeryLongItem)",
            "(\n  firstVeryVeryVeryVeryVeryVeryVeryVeryLongVeryItem,\n  secondVeryVeryVeryVeryVeryVeryVeryVeryVeryLongItem,\n)\n",
        );

        // Comments
        test("(foo,) # abc", "(foo,) # abc\n");
        test("(foo,)# abc", "(foo,) # abc\n");
        test("(foo,# abc\n)", "(\n  foo, # abc\n)\n");
        test("(foo, # abc\n)", "(\n  foo, # abc\n)\n");
        test("(# abc\n  foo,)", "( # abc\n  foo,\n)\n");
        test("(foo# abc\n  , bar,)", "(\n  foo, # abc\n  bar,\n)\n");
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

    #[test]
    fn test_assignment() {
        // Simple assignment
        test("foo = bar", "foo = bar\n");
        test("foo=bar", "foo = bar\n");
        test("foo = bar", "foo = bar\n");
        test("foo =\n  bar ", "foo = bar\n");
        test("foo := bar", "foo := bar\n");
        test(
            "foo = veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongExpression",
            "foo = veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongExpression\n",
        );
        test(
            "foo = veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongExpression",
            "foo =\n  veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongExpression\n",
        );

        // Function definition
        test("foo bar=baz ", "foo bar = baz\n");
        test("foo\n  bar=baz ", "foo bar = baz\n");
        test("foo\n  bar\n  =\n  baz ", "foo bar = baz\n");
        test(
            "foo firstVeryVeryVeryVeryVeryVeryVeryVeryLongArgument secondVeryVeryVeryVeryVeryVeryVeryVeryLongArgument = bar",
            "foo\n  firstVeryVeryVeryVeryVeryVeryVeryVeryLongArgument\n  secondVeryVeryVeryVeryVeryVeryVeryVeryLongArgument = bar\n",
        );
        test(
            "foo firstVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongArgument = bar",
            "foo\n  firstVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongArgument =\n  bar\n",
        );
        test(
            "foo argument = veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongExpression\n",
            "foo argument =\n  veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongExpression\n",
        );

        // Comments
        test("foo = bar # abc\n", "foo = bar # abc\n");
        test("foo=bar# abc\n", "foo = bar # abc\n");
    }

    fn test(source: &str, expected: &str) {
        let csts = parse_rcst(source).to_csts();
        assert_eq!(source, csts.iter().join(""));

        let formatted = csts.as_slice().format_to_string();
        assert_eq!(formatted, expected);
    }
}
