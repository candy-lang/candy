use crate::{text_edits::TextEdits, width::Width, Indentation};
use candy_frontend::{
    cst::{Cst, CstError, CstKind},
    position::Offset,
};
use derive_more::From;
use itertools::Itertools;
use std::borrow::Cow;

#[derive(Clone, Debug)]
pub struct ExistingWhitespace<'a> {
    start_offset: Offset,
    adopted_whitespace_before: Cow<'a, [Cst]>,
    whitespace: Cow<'a, [Cst]>,
    adopted_whitespace_after: Cow<'a, [Cst]>,
}
#[derive(Clone, Debug, From)]
pub enum TrailingWhitespace {
    None,
    Space,
    Indentation(Indentation),
}

pub const SPACE: &str = " ";
pub const NEWLINE: &str = "\n";

impl<'a> ExistingWhitespace<'a> {
    pub fn empty(start_offset: Offset) -> Self {
        Self {
            start_offset,
            adopted_whitespace_before: Cow::default(),
            whitespace: Cow::default(),
            adopted_whitespace_after: Cow::default(),
        }
    }
    pub fn new(start_offset: Offset, whitespace: impl Into<Cow<'a, [Cst]>>) -> Self {
        let whitespace = whitespace.into();
        if whitespace.is_empty() {
            return Self::empty(start_offset);
        }

        Self {
            start_offset,
            adopted_whitespace_before: Cow::default(),
            whitespace,
            adopted_whitespace_after: Cow::default(),
        }
    }

    pub fn end_offset(&self) -> Offset {
        self.whitespace
            .as_ref()
            .last()
            .map(|it| it.data.span.end)
            .unwrap_or(self.start_offset)
    }
    pub fn whitespace_ref(&self) -> &[Cst] {
        self.whitespace.as_ref()
    }

    pub fn take(self) -> Cow<'a, [Cst]> {
        self.whitespace
    }
    pub fn empty_and_move_comments_to(
        self,
        edits: &mut TextEdits,
        other: &mut ExistingWhitespace<'a>,
    ) {
        if self.adopted_whitespace_before.is_empty()
            && self.whitespace.is_empty()
            && self.adopted_whitespace_after.is_empty()
        {
            return;
        }

        let self_end_offset = self.end_offset();
        if self.start_offset <= other.start_offset {
            assert!(self_end_offset <= other.start_offset);
            if self_end_offset == other.start_offset
                && self.adopted_whitespace_before.is_empty()
                && self.adopted_whitespace_after.is_empty()
                && other.adopted_whitespace_before.is_empty()
            {
                prepend(self.whitespace, &mut other.whitespace);
                prepend(self.adopted_whitespace_before, &mut other.whitespace);
                return;
            }

            if let Some(other_adopted_first) = &other.adopted_whitespace_before.first() {
                let other_adopted_start_offset = other_adopted_first.data.span.start;
                assert!(self_end_offset <= other_adopted_start_offset);
            }
            prepend(
                self.adopted_whitespace_after,
                &mut other.adopted_whitespace_before,
            );
            prepend(self.whitespace, &mut other.adopted_whitespace_before);
            prepend(
                self.adopted_whitespace_before,
                &mut other.adopted_whitespace_before,
            );
        } else {
            let other_end_offset = other
                .whitespace
                .last()
                .map(|it| it.data.span.end)
                .unwrap_or_else(|| other.start_offset);
            if self.start_offset == other_end_offset
                && other.adopted_whitespace_after.is_empty()
                && self.adopted_whitespace_before.is_empty()
                && self.adopted_whitespace_after.is_empty()
            {
                append(self.whitespace, &mut other.whitespace);
                append(self.adopted_whitespace_after, &mut other.whitespace);
                return;
            }

            if let Some(other_adopted_last) = &other.adopted_whitespace_after.last() {
                let other_adopted_end_offset = other_adopted_last.data.span.end;
                assert!(other_adopted_end_offset <= self.start_offset);
            }
            append(
                self.adopted_whitespace_before,
                &mut other.adopted_whitespace_after,
            );
            append(self.whitespace, &mut other.adopted_whitespace_after);
            append(
                self.adopted_whitespace_after,
                &mut other.adopted_whitespace_after,
            );
        }
        edits.delete(self.start_offset..self_end_offset);
    }

    pub fn has_comments(&self) -> bool {
        fn check(whitespace: &[Cst]) -> bool {
            whitespace
                .iter()
                .any(|it| matches!(it.kind, CstKind::Comment { .. }))
        }

        check(&self.adopted_whitespace_before)
            || check(&self.whitespace)
            || check(&self.adopted_whitespace_after)
    }

    pub fn into_trailing(
        self,
        edits: &mut TextEdits,
        trailing: impl Into<TrailingWhitespace>,
    ) -> Width {
        match trailing.into() {
            TrailingWhitespace::None => {
                self.into_empty_trailing(edits);
                Width::default()
            }
            TrailingWhitespace::Space => {
                self.into_trailing_with_space(edits);
                Width::SPACE
            }
            TrailingWhitespace::Indentation(indentation) => {
                self.into_trailing_with_indentation(edits, indentation);
                Width::Multline
            }
        }
    }
    pub fn into_empty_trailing(self, edits: &mut TextEdits) {
        assert!(!self.has_comments());

        for whitespace in self.whitespace_ref() {
            edits.delete(whitespace.data.span.to_owned());
        }
    }
    pub fn into_trailing_with_space(self, edits: &mut TextEdits) {
        assert!(!self.has_comments());

        if let Some((first, last)) = first_and_last(self.whitespace.as_ref()) {
            edits.change(first.data.span.start..last.data.span.end, SPACE);
        } else {
            edits.insert(self.start_offset, SPACE);
        }
    }
    pub fn into_trailing_with_indentation(self, edits: &mut TextEdits, indentation: Indentation) {
        fn iter_whitespace(
            whitespace: &[Cst],
            offset_override: impl Into<Option<Offset>>,
        ) -> impl Iterator<Item = (&Cst, Option<Offset>)> {
            let offset_override = offset_override.into();
            whitespace.iter().map(move |it| (it, offset_override))
        }

        // For adopted items, we need an offset override: The position where adopted comments will
        // be inserted.
        let whitespace = iter_whitespace(&self.adopted_whitespace_before, self.start_offset)
            .chain(iter_whitespace(&self.whitespace, None))
            .chain(iter_whitespace(
                &self.adopted_whitespace_after,
                self.end_offset(),
            ))
            .collect_vec();
        // `.chain(…)` doesn't produce an `ExactSizeIterator`, so it's easier to collect everything
        // into a `Vec` first.
        let last_comment_index = whitespace
            .iter()
            .rposition(|(it, _)| matches!(it.kind, CstKind::Comment { .. }));
        let split_index = last_comment_index.map(|it| it + 1).unwrap_or_default();
        let (comments_and_whitespace, final_whitespace) = whitespace.split_at(split_index);

        Self::format_trailing_comments(comments_and_whitespace, edits, indentation);

        let owned_final_whitespace = final_whitespace
            .iter()
            .filter(|(_, offset_override)| offset_override.is_none())
            .map(|(it, _)| it);
        let range = if let Some((first, last)) = first_and_last(owned_final_whitespace) {
            first.data.span.start..last.data.span.end
        } else {
            let offset = self.end_offset();
            offset..offset
        };
        edits.change(range, format!("{NEWLINE}{}", indentation.to_string()));
    }
    fn format_trailing_comments(
        comments_and_whitespace: &[(&Cst, Option<Offset>)],
        edits: &mut TextEdits,
        indentation: Indentation,
    ) {
        let mut is_comment_on_same_line = true;
        let mut last_reusable_whitespace_range = None;
        for (item, offset_override) in comments_and_whitespace {
            let is_adopted = offset_override.is_some();
            match &item.kind {
                CstKind::Whitespace(_)
                | CstKind::Error {
                    error: CstError::TooMuchWhitespace,
                    ..
                } => {
                    if !is_adopted {
                        if let Some(range) = last_reusable_whitespace_range {
                            edits.delete(range);
                        }
                        last_reusable_whitespace_range = Some(item.data.span.to_owned());
                    }
                }
                CstKind::Newline(_) => {
                    if is_comment_on_same_line {
                        if let Some(range) = last_reusable_whitespace_range {
                            // Delete trailing spaces in the previous line.
                            edits.delete(range);
                            last_reusable_whitespace_range = None;
                        }

                        is_comment_on_same_line = false;
                        edits.change(item.data.span.to_owned(), NEWLINE);
                    } else {
                        // We already encountered and kept a newline, so we can delete this one.
                        edits.delete(item.data.span.to_owned());
                    }
                }
                CstKind::Comment { comment, .. } => {
                    let space = if is_comment_on_same_line {
                        Cow::Borrowed(SPACE)
                    } else {
                        Cow::Owned(indentation.to_string())
                    };
                    if let Some(range) = last_reusable_whitespace_range {
                        edits.change(range, space);
                    } else {
                        edits.insert(offset_override.unwrap_or(item.data.span.start), space);
                    }

                    if let Some(offset_override) = offset_override {
                        edits.insert(*offset_override, format!("#{comment}"));
                    }

                    is_comment_on_same_line = false;
                    last_reusable_whitespace_range = None;
                    // TODO: Handle multiple comments on the same line.
                }
                _ => unreachable!(),
            }
        }
        assert!(
            last_reusable_whitespace_range.is_none(),
            "The last CST should be a comment, so we should have consumed all whitespace.",
        );
    }
}

fn append<'a>(source: Cow<'a, [Cst]>, target: &mut Cow<'a, [Cst]>) {
    if source.is_empty() {
        return;
    }

    if target.is_empty() {
        *target = source;
    } else {
        match source {
            Cow::Borrowed(source) => target.to_mut().extend_from_slice(source),
            Cow::Owned(mut source) => target.to_mut().append(&mut source),
        }
    }
}
fn prepend<'a>(source: Cow<'a, [Cst]>, target: &mut Cow<'a, [Cst]>) {
    if source.is_empty() {
        return;
    }

    if target.is_empty() {
        *target = source;
    } else {
        target
            .to_mut()
            .splice(0..0, source.as_ref().iter().cloned());
    }
}
fn first_and_last<I: IntoIterator>(
    iterator: I,
) -> Option<(<I as IntoIterator>::Item, <I as IntoIterator>::Item)>
where
    <I as IntoIterator>::Item: Copy,
{
    let mut result = None;
    for item in iterator {
        let first = result.map(|(first, _)| first).unwrap_or(item);
        result = Some((first, item));
    }
    result
}

#[cfg(test)]
mod test {
    use super::TrailingWhitespace;
    use crate::{format_cst, text_edits::TextEdits, width::Indentation, FormatterInfo};
    use candy_frontend::{cst::CstKind, rcst_to_cst::RcstsToCstsExt, string_to_rcst::parse_rcst};

    #[test]
    fn test_empty_trailing() {
        test("foo End", TrailingWhitespace::None, "foo");
        test("foo  End", TrailingWhitespace::None, "foo");
    }

    #[test]
    fn test_trailing_with_space() {
        test("foo End", TrailingWhitespace::Space, "foo ");
        test("foo  End", TrailingWhitespace::Space, "foo ");
    }

    #[test]
    fn test_trailing_with_indentation() {
        test("foo\n  End", Indentation(1), "foo\n  ");
        test("foo \n  End", Indentation(1), "foo\n  ");
        test("foo End", Indentation(2), "foo\n    ");
        test("foo \n  End", Indentation(2), "foo\n    ");

        // Comments
        test("foo# abc\n  End", Indentation(1), "foo # abc\n  ");
        test("foo # abc\n  End", Indentation(1), "foo # abc\n  ");
        test("foo  # abc\n  End", Indentation(1), "foo # abc\n  ");
        test("foo\n  # abc\n  End", Indentation(1), "foo\n  # abc\n  ");
    }

    fn test(source: &str, trailing: impl Into<TrailingWhitespace>, expected: &str) {
        let mut csts = parse_rcst(source).to_csts();
        assert_eq!(csts.len(), 1);

        let cst = match csts.pop().unwrap().kind {
            CstKind::Call { receiver, .. } => receiver,
            _ => panic!("Expected a call"),
        };
        let reduced_source = cst.to_string();

        let mut edits = TextEdits::new(reduced_source);
        let (_, whitespace) = format_cst(&mut edits, &cst, &FormatterInfo::default()).split();
        match trailing.into() {
            TrailingWhitespace::None => whitespace.into_empty_trailing(&mut edits),
            TrailingWhitespace::Space => whitespace.into_trailing_with_space(&mut edits),
            TrailingWhitespace::Indentation(indentation) => {
                whitespace.into_trailing_with_indentation(&mut edits, indentation)
            }
        }
        assert_eq!(edits.apply(), expected);
    }
}
