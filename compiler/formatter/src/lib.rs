#![feature(anonymous_lifetime_in_impl_trait)]
#![feature(box_patterns)]
#![feature(let_chains)]

use crate::width::StringWidth;
use candy_frontend::{
    cst::{Cst, CstError, CstKind},
    position::Offset,
};
use existing_whitespace::{ExistingWhitespace, TrailingWhitespace, NEWLINE, SPACE};
use extension_trait::extension_trait;
use itertools::Itertools;
use std::{borrow::Cow, ops::Range};
use text_edits::TextEdits;
use traversal::dft_post_rev;
use width::{Indentation, Width};

mod existing_whitespace;
mod last_line_width;
mod text_edits;
mod width;

#[extension_trait]
pub impl<C: AsRef<[Cst]>> Formatter for C {
    fn format_to_string(&self) -> String {
        self.format_to_edits().apply()
    }
    fn format_to_edits(&self) -> TextEdits {
        let csts = self.as_ref();
        // TOOD: Is there an elegant way to avoid stringifying the whole CST?
        let source = csts.iter().join("");
        let mut edits = TextEdits::new(source);
        format_csts(&mut edits, csts, &FormatterInfo::default());
        edits
    }
}

#[derive(Clone, Default)]
struct FormatterInfo {
    indentation: Indentation,
    trailing_comma_condition: Option<TrailingCommaCondition>,
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

#[derive(Clone)]
enum TrailingCommaCondition {
    Always,

    /// Add a trailing comma if the element fits in a single line and is at most
    /// this wide.
    UnlessFitsIn(usize),
}

/// The maximum number of empty lines (i.e., containing no expression or comment) that may come
/// consecutively.
const MAX_CONSECUTIVE_EMPTY_LINES: usize = 2;

fn format_csts(edits: &mut TextEdits, csts: &[Cst], info: &FormatterInfo) -> Width {
    // In the formatted output, is this the first line with actual content (i.e., an expression
    // or comment)?
    let mut is_first_content_line = true;

    let mut width = None;

    let mut empty_line_count = 0;
    let mut csts = Cow::Borrowed(csts);
    let mut index = 0;
    let mut pending_newlines = vec![];

    let inject_whitespace = move |whitespace: Vec<Cst>, csts: &mut Cow<'_, [Cst]>, index: usize| {
        csts.to_mut().splice(index + 1..index + 1, whitespace);
    };

    'outer: while index < csts.len() {
        let cst = &csts.as_ref()[index];

        if let CstKind::Newline(_) = cst.kind {
            // Remove leading newlines and limit the number of consecutive empty lines.
            if is_first_content_line || empty_line_count > MAX_CONSECUTIVE_EMPTY_LINES {
                edits.delete(cst.data.span.to_owned());
            } else {
                pending_newlines.push(cst.data.span.to_owned());
                empty_line_count += 1;
            }
            index += 1;

            let remaining_csts = &csts[index..];
            if remaining_csts.iter().all(|it| {
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
                for whitespace in remaining_csts {
                    edits.delete(whitespace.data.span.to_owned());
                }
                break 'outer;
            }

            continue;
        }

        // Indentation
        let (mut cst, indentation_span) = if let CstKind::Whitespace(_)
        | CstKind::Error {
            error: CstError::TooMuchWhitespace,
            ..
        } = &cst.kind
        {
            index += 1;
            (csts.get(index), Some(cst.data.span.to_owned()))
        } else {
            (Some(cst), None)
        };

        // Remove more whitespaces before an actual expression or comment.
        let not_whitespace = loop {
            let Some(next) = cst else {
                    // Remove whitespace at the end of the file.
                    if let Some(indentation_span) = indentation_span {
                        edits.delete(indentation_span);
                    }
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
                    if let Some(indentation_span) = indentation_span {
                        edits.delete(indentation_span);
                    }
                    continue 'outer;
                }
                _ => break next,
            }
        };

        for newline in pending_newlines.drain(..) {
            edits.change(newline, NEWLINE);
        }

        // In indented bodies, the indentation of the first line is taken care of by the caller.
        //
        // That's also why we don't have to keep track of its width: The exact width is only
        // interesting for single-line bodies in which indentation won't occurr.
        if !is_first_content_line && info.indentation.is_indented() {
            let indentation = info.indentation.to_string();
            if let Some(indentation_span) = indentation_span {
                edits.change(indentation_span, indentation);
            } else if let Some(cst) = cst {
                edits.insert(cst.data.span.start, indentation);
            }
        } else if let Some(indentation_span) = indentation_span {
            edits.delete(indentation_span);
        }

        let (not_whitespace_width, whitespace) = format_cst(edits, not_whitespace, info).split();
        if let Some(whitespace) = whitespace.take() {
            inject_whitespace(whitespace.into_owned(), &mut csts, index);
        }
        if width.is_none() {
            width = Some(not_whitespace_width);
        } else {
            width = Some(Width::Multline);
        }
        index += 1;
        is_first_content_line = false;
        empty_line_count = 0;

        let mut trailing_whitespace_span: Option<Range<Offset>> = None;
        loop {
            let Some(next) = csts.get(index) else {
                // Remove trailing whitespace at the end of the file.
                if let Some(span) = trailing_whitespace_span {
                    edits.delete(span);
                }
                break;
            };

            match next.kind {
                CstKind::Whitespace(_)
                | CstKind::Error {
                    error: CstError::TooMuchWhitespace,
                    ..
                } => {
                    // Remove whitespace after an expression or comment (unless we're between an expression and a
                    // comment on the same line).
                    index += 1;
                    trailing_whitespace_span = Some(next.data.span.to_owned());
                }
                CstKind::Newline(_) => {
                    if let Some(span) = trailing_whitespace_span {
                        edits.delete(span);
                    }
                    break;
                }
                CstKind::Comment { .. } => {
                    // A comment in the same line.
                    if let Some(span) = trailing_whitespace_span {
                        edits.change(span, SPACE);
                        trailing_whitespace_span = None;
                    } else {
                        edits.insert(next.data.span.start, SPACE);
                    }

                    let (_, whitespace) = format_cst(edits, next, info).split();
                    if let Some(whitespace) = whitespace.take() {
                        inject_whitespace(whitespace.into_owned(), &mut csts, index);
                    }
                    index += 1;
                }
                _ => {
                    // Another expression without a newline in between.
                    let whitespace_span =
                        trailing_whitespace_span.unwrap_or_else(|| next.data.span.to_owned());
                    trailing_whitespace_span = None;

                    edits.insert(whitespace_span.start, NEWLINE);
                    width = Some(Width::Multline);

                    edits.change(whitespace_span, info.indentation.to_string());

                    let (_, whitespace) = format_cst(edits, next, info).split();
                    if let Some(whitespace) = whitespace.take() {
                        inject_whitespace(whitespace.into_owned(), &mut csts, index);
                    }

                    index += 1;
                }
            }
        }
    }

    // Add trailing newline (only for a non-empty top-level body).
    if !info.indentation.is_indented() && !is_first_content_line {
        if let Some(newline) = pending_newlines.pop() {
            edits.change(newline, NEWLINE);
        } else {
            let last_cst = csts.last().unwrap();
            edits.insert(last_cst.data.span.end, NEWLINE);
        }
        width = Some(Width::Multline)
    }
    for newline in pending_newlines {
        edits.delete(newline);
    }

    width.unwrap_or_default()
}

pub(crate) fn format_cst<'a>(
    edits: &mut TextEdits,
    cst: &'a Cst,
    info: &FormatterInfo,
) -> FormattedCst<'a> {
    let width = match &cst.kind {
        CstKind::EqualsSign | CstKind::Comma | CstKind::Dot | CstKind::Colon => {
            Width::Singleline(1)
        }
        CstKind::ColonEqualsSign => Width::Singleline(2),
        CstKind::Bar
        | CstKind::OpeningParenthesis
        | CstKind::ClosingParenthesis
        | CstKind::OpeningBracket
        | CstKind::ClosingBracket
        | CstKind::OpeningCurlyBrace
        | CstKind::ClosingCurlyBrace => Width::Singleline(1),
        CstKind::Arrow => Width::Singleline(2),
        CstKind::SingleQuote | CstKind::DoubleQuote | CstKind::Percent | CstKind::Octothorpe => {
            Width::Singleline(1)
        }
        CstKind::Whitespace(_) | CstKind::Newline(_) => {
            panic!("Whitespace and newlines should be handled separately.")
        }
        CstKind::Comment {
            octothorpe,
            comment,
        } => {
            let formatted_octothorpe = format_cst(edits, octothorpe, info);
            assert!(formatted_octothorpe.min_width.is_singleline());

            formatted_octothorpe.into_empty_trailing(edits) + comment.width()
        }
        CstKind::TrailingWhitespace { child, whitespace } => {
            let (child_width, child_whitespace) = format_cst(edits, child, info).split();
            let mut whitespace = ExistingWhitespace::new(child.data.span.end, whitespace);
            child_whitespace.empty_and_move_comments_to(edits, &mut whitespace);
            return FormattedCst::new(child_width, whitespace);
        }
        CstKind::Identifier(string) | CstKind::Symbol(string) | CstKind::Int { string, .. } => {
            string.width()
        }
        CstKind::OpeningText { .. } | CstKind::ClosingText { .. } => todo!(),
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
            let opening_parenthesis = format_cst(edits, opening_parenthesis, info);
            let inner = format_cst(edits, inner, &info.with_indent());
            let closing_parenthesis = format_cst(edits, closing_parenthesis, info);

            let min_width =
                &opening_parenthesis.min_width + &inner.min_width + &closing_parenthesis.min_width;
            let (opening_parenthesis_trailing, inner_trailing) = if min_width.fits(info.indentation)
            {
                (TrailingWhitespace::None, TrailingWhitespace::None)
            } else {
                (
                    TrailingWhitespace::Indentation(info.indentation.with_indent()),
                    TrailingWhitespace::Indentation(info.indentation),
                )
            };

            let (closing_parenthesis_width, whitespace) = closing_parenthesis.split();
            return FormattedCst::new(
                opening_parenthesis.into_trailing(edits, opening_parenthesis_trailing)
                    + inner.into_trailing(edits, inner_trailing)
                    + closing_parenthesis_width,
                whitespace,
            );
        }
        CstKind::Call {
            receiver,
            arguments,
        } => {
            let receiver = format_cst(edits, receiver, info);
            let mut arguments = arguments
                .iter()
                .map(|argument| format_cst(edits, argument, &info.with_indent()))
                .collect_vec();

            let min_width = &receiver.min_width
                + arguments
                    .iter()
                    .map(|it| Width::SPACE + &it.min_width)
                    .sum::<Width>();
            let trailing = if min_width.fits(info.indentation) {
                TrailingWhitespace::Space
            } else {
                TrailingWhitespace::Indentation(info.indentation.with_indent())
            };

            let (last_argument_width, whitespace) = arguments.pop().unwrap().split();
            return FormattedCst::new(
                receiver.into_trailing(edits, trailing.clone())
                    + arguments
                        .into_iter()
                        .map(|it| it.into_trailing(edits, trailing.clone()))
                        .sum::<Width>()
                    + last_argument_width,
                whitespace,
            );
        }
        CstKind::List {
            opening_parenthesis,
            items,
            closing_parenthesis,
        } => {
            return format_collection(
                edits,
                opening_parenthesis,
                items,
                closing_parenthesis,
                true,
                info,
            );
        }
        CstKind::ListItem { value, comma } => {
            let value_end = value.data.span.end;
            let value = format_cst(edits, value, info);
            let value_width = value.into_empty_trailing(edits);

            let (comma_width, whitespace) = apply_trailing_comma_condition(
                edits,
                comma.as_deref(),
                value_end,
                info,
                &value_width,
            );

            return FormattedCst::new(value_width + comma_width, whitespace);
        }
        CstKind::Struct {
            opening_bracket,
            fields,
            closing_bracket,
        } => {
            return format_collection(edits, opening_bracket, fields, closing_bracket, false, info);
        }
        CstKind::StructField {
            key_and_colon,
            value,
            comma,
        } => {
            let key_width_and_colon = key_and_colon.as_ref().map(|box (key, colon)| {
                let key = format_cst(edits, key, &info.with_indent());
                let key_width = key.into_empty_trailing(edits);

                let colon = format_cst(edits, colon, &info.with_indent());

                (key_width, colon)
            });

            let value_end = value.data.span.end;
            let value_width =
                format_cst(edits, value, &info.with_indent()).into_empty_trailing(edits);

            let key_and_colon_min_width = key_width_and_colon
                .as_ref()
                .map(|(key_width, colon)| key_width + &colon.min_width)
                .unwrap_or_default();
            let min_width_before_comma = key_and_colon_min_width + &value_width;
            let (comma_width, whitespace) = apply_trailing_comma_condition(
                edits,
                comma.as_deref(),
                value_end,
                info,
                &min_width_before_comma,
            );
            let min_width = min_width_before_comma + &comma_width;

            return FormattedCst::new(
                key_width_and_colon
                    .map(|(key_width, colon)| {
                        let colon_trailing = if min_width.fits(info.indentation) {
                            TrailingWhitespace::Space
                        } else {
                            TrailingWhitespace::Indentation(info.indentation.with_indent())
                        };
                        key_width + colon.into_trailing(edits, colon_trailing)
                    })
                    .unwrap_or_default()
                    + value_width
                    + comma_width,
                whitespace,
            );
        }
        CstKind::StructAccess { struct_, dot, key } => {
            // TODO: child_width vs min_width
            let (struct_width, mut struct_whitespace) = format_cst(edits, struct_, info).split();

            let (dot_width, dot_whitespace) = format_cst(edits, dot, &info.with_indent()).split();
            dot_whitespace.empty_and_move_comments_to(edits, &mut struct_whitespace);

            let key = format_cst(edits, key, &info.with_indent());

            let min_width = &struct_width + &dot_width + &key.min_width;
            let struct_trailing = if min_width.fits(info.indentation) {
                TrailingWhitespace::None
            } else {
                TrailingWhitespace::Indentation(info.indentation.with_indent())
            };

            let (key_width, whitespace) = key.split();
            return FormattedCst::new(
                struct_whitespace.into_trailing(edits, struct_trailing) + dot_width + key_width,
                whitespace,
            );
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
            let left = format_cst(edits, left, info);
            let left_width = left.into_trailing_with_space(edits);

            let assignment_sign = format_cst(edits, assignment_sign, &info.with_indent());

            let body_width = format_csts(edits, body, &info.with_indent());

            let is_body_in_same_line =
                (&left_width + &assignment_sign.min_width + Width::SPACE + &body_width)
                    .fits(info.indentation);
            let assignment_sign_trailing = if is_body_in_same_line {
                TrailingWhitespace::Space
            } else {
                TrailingWhitespace::Indentation(info.indentation.with_indent())
            };

            left_width + assignment_sign.into_trailing(edits, assignment_sign_trailing) + body_width
        }
        CstKind::Error {
            unparsable_input, ..
        } => unparsable_input.width(),
    };
    FormattedCst::new(width, ExistingWhitespace::empty(cst.data.span.end))
}

fn format_collection<'a>(
    edits: &mut TextEdits,
    opening_punctuation: &Cst,
    items: &[Cst],
    closing_punctuation: &'a Cst,
    is_comma_required_for_single_item: bool,
    info: &FormatterInfo,
) -> FormattedCst<'a> {
    let opening_punctuation = format_cst(edits, opening_punctuation, info);
    let closing_punctuation = format_cst(edits, closing_punctuation, info);

    let mut min_width = Width::Singleline(info.indentation.width())
        + &opening_punctuation.min_width
        + &closing_punctuation.min_width;
    let item_info = info
        .with_indent()
        .with_trailing_comma_condition(TrailingCommaCondition::Always);
    let items = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let is_single_item = items.len() == 1;
            let is_last_item = index == items.len() - 1;

            let item_has_comments =
                dft_post_rev(item, |it| it.children().into_iter()).any(|(_, it)| match it.kind {
                    CstKind::Comment { .. } => true,
                    _ => false,
                });

            let is_comma_required_due_to_single_item =
                is_single_item && is_comma_required_for_single_item;
            let is_comma_required =
                is_comma_required_due_to_single_item || !is_last_item || item_has_comments;
            let info = if !is_comma_required && let Width::Singleline(min_width) = min_width {
                    // We're looking at the last item and everything might fit in one line.
                    let max_width = Width::MAX - min_width;
                    assert!(max_width > 0);

                    item_info.with_trailing_comma_condition(
                        TrailingCommaCondition::UnlessFitsIn(max_width),
                    )
                } else {
                    item_info.clone()
                };
            let item = format_cst(edits, item, &info);

            if let Width::Singleline(old_min_width) = min_width
                    && let Width::Singleline(item_width) = item.min_width {
                let (item_width, max_width) = if is_last_item {
                    (item_width, Width::MAX)
                } else {
                    // We need an additional column for the trailing space after the comma.
                    let item_width = item_width + 1;

                    // The last item needs at least one column of space.
                    let max_width = Width::MAX - 1;

                    (item_width, max_width)
                };
                min_width = Width::from_width_and_max(old_min_width + item_width, max_width);
            } else {
                min_width = Width::Multline;
            }

            item
        })
        .collect_vec();

    let (opening_punctuation_trailing, item_trailing, last_item_trailing) =
        if min_width.is_singleline() {
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

    let last_item_index = items.len().checked_sub(1);
    let (closing_punctuation_width, whitespace) = closing_punctuation.split();
    FormattedCst::new(
        opening_punctuation.into_trailing(edits, opening_punctuation_trailing)
            + items
                .into_iter()
                .enumerate()
                .map(|(index, item)| {
                    item.into_trailing(
                        edits,
                        if last_item_index == Some(index) {
                            last_item_trailing.clone()
                        } else {
                            item_trailing.clone()
                        },
                    )
                })
                .sum::<Width>()
            + closing_punctuation_width,
        whitespace,
    )
}

fn apply_trailing_comma_condition<'a>(
    edits: &mut TextEdits,
    comma: Option<&'a Cst>,
    fallback_offset: Offset,
    info: &FormatterInfo,
    min_width_before_comma: &Width,
) -> (Width, ExistingWhitespace<'a>) {
    let should_have_comma = match info.trailing_comma_condition {
        Some(TrailingCommaCondition::Always) => true,
        Some(TrailingCommaCondition::UnlessFitsIn(max_width)) => {
            !min_width_before_comma.fits_in(max_width)
        }
        None => comma.is_some(),
    };
    let (width, whitespace) = if should_have_comma {
        let whitespace = if let Some(comma) = comma {
            let comma = format_cst(edits, comma, info);
            assert_eq!(comma.min_width, Width::Singleline(1));
            Some(comma.whitespace)
        } else {
            edits.insert(fallback_offset, ",");
            None
        };
        (Width::Singleline(1), whitespace)
    } else {
        let whitespace = comma.map(|comma| {
            // TODO: Keep comments
            edits.delete(comma.data.span.to_owned());
            ExistingWhitespace::empty(comma.data.span.end)
        });
        (Width::default(), whitespace)
    };
    (
        width,
        whitespace.unwrap_or_else(|| ExistingWhitespace::empty(fallback_offset)),
    )
}

#[must_use]
struct FormattedCst<'a> {
    /// The minimum width that this CST node could take after formatting.
    ///
    /// If there are trailing comments, this is [Width::Multiline]. Otherwise, it's the child's own
    /// width.
    min_width: Width,
    whitespace: ExistingWhitespace<'a>,
}
impl<'a> FormattedCst<'a> {
    pub fn new(child_width: Width, whitespace: ExistingWhitespace<'a>) -> Self {
        Self {
            min_width: if whitespace.has_comments() {
                Width::Multline
            } else {
                child_width
            },
            whitespace,
        }
    }

    pub fn split(self) -> (Width, ExistingWhitespace<'a>) {
        (self.min_width, self.whitespace)
    }

    pub fn into_trailing(
        self,
        edits: &mut TextEdits,
        trailing: impl Into<TrailingWhitespace>,
    ) -> Width {
        match trailing.into() {
            TrailingWhitespace::None => self.into_empty_trailing(edits),
            TrailingWhitespace::Space => self.into_trailing_with_space(edits),
            TrailingWhitespace::Indentation(indentation) => {
                self.into_trailing_with_indentation(edits, indentation)
            }
        }
    }
    #[deprecated]
    pub fn into_empty_trailing(self, edits: &mut TextEdits) -> Width {
        self.whitespace.into_empty_trailing(edits);
        self.min_width
    }
    pub fn into_trailing_with_space(self, edits: &mut TextEdits) -> Width {
        self.whitespace.into_trailing_with_space(edits);
        self.min_width + Width::SPACE
    }
    pub fn into_trailing_with_indentation(
        self,
        edits: &mut TextEdits,
        indentation: Indentation,
    ) -> Width {
        self.whitespace
            .into_trailing_with_indentation(edits, indentation);
        Width::Multline
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
        // test("(foo# abc\n  , bar,)", "(\n  foo, # abc\n  bar,\n)\n"); // FIXME
    }
    #[test]
    fn test_struct() {
        // Empty
        test("[]", "[]\n");
        test("[ ]", "[]\n");
        test("[\n]", "[]\n");

        // Single item
        test("[foo]", "[foo]\n");
        test("[foo ]", "[foo]\n");
        test("[\n  foo]", "[foo]\n");
        test("[\n  foo\n]", "[foo]\n");
        test("[foo: bar]", "[foo: bar]\n");
        test("[ foo :bar ] ", "[foo: bar]\n");
        test("[\n  foo:\n    bar,\n]", "[foo: bar]\n");
        test(
            "[veryVeryVeryVeryVeryVeryVeryVeryLongVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongItemmm]",
            "[veryVeryVeryVeryVeryVeryVeryVeryLongVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongItemmm]\n",
        );
        test(
            "[veryVeryVeryVeryVeryVeryVeryVeryLongVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongItemmmm]",
            "[\n  veryVeryVeryVeryVeryVeryVeryVeryLongVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongItemmmm,\n]\n",
        );
        test(
            "[\n  veryVeryVeryLongVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongKey: value\n]",
            "[veryVeryVeryLongVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongKey: value]\n",
        );
        test(
            "[veryVeryLongVeryVeryVeryVeryVeryVeryVeryVeryLongKey: veryLongVeryVeryVeryVeryVeryVeryVeryVeryVeryLongValue]",
            "[\n  veryVeryLongVeryVeryVeryVeryVeryVeryVeryVeryLongKey:\n    veryLongVeryVeryVeryVeryVeryVeryVeryVeryVeryLongValue,\n]\n",
        );

        // Multiple items
        test("[foo: bar, baz]", "[foo: bar, baz]\n");
        test("[foo: bar, baz,]", "[foo: bar, baz]\n");
        test("[foo: bar, baz: blub,]", "[foo: bar, baz: blub]\n");
        test("[ foo :bar ,baz , ]", "[foo: bar, baz]\n");
        test("[\n  foo :\n    bar ,\n  baz ,\n]", "[foo: bar, baz]\n");
        test(
            "[item1, veryVeryLongVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongKey: value]",
            "[\n  item1,\n  veryVeryLongVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongKey: value,\n]\n",
        );

        // Comments
        test("[foo] # abc", "[foo] # abc\n");
        test("[foo: bar] # abc", "[foo: bar] # abc\n");
        // test("[foo: bar # abc\n]", "[\n  foo: bar, # abc\n]\n"); // FIXME
        test("[foo: # abc\n  bar\n]", "[\n  foo: # abc\n    bar,\n]\n");
        test("[# abc\n  foo: bar]", "[ # abc\n  foo: bar,\n]\n");
        // test(
        //     "[foo: bar # abc\n  , baz]",
        //     "[\n  foo: bar, # abc\n  baz,\n]\n",
        // ); // FIXME
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
        // test(
        //     "foo firstVeryVeryVeryVeryVeryVeryVeryVeryLongArgument secondVeryVeryVeryVeryVeryVeryVeryVeryLongArgument = bar",
        //     "foo\n  firstVeryVeryVeryVeryVeryVeryVeryVeryLongArgument\n  secondVeryVeryVeryVeryVeryVeryVeryVeryLongArgument = bar\n",
        // ); // FIXME
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
