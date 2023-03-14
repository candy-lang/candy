use crate::{text_edits::TextEdits, Indentation};
use candy_frontend::{
    cst::{Cst, CstError, CstKind},
    position::Offset,
};
use derive_more::From;
use std::borrow::Cow;

#[derive(Clone, Debug)]
pub struct ExistingWhitespace<'a> {
    child_end_offset: Offset,
    trailing_whitespace: Option<Cow<'a, [Cst]>>,
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
    pub fn empty(child_end_offset: Offset) -> Self {
        Self {
            child_end_offset,
            trailing_whitespace: None,
        }
    }
    pub fn new(child_end_offset: Offset, trailing_whitespace: impl Into<Cow<'a, [Cst]>>) -> Self {
        let trailing_whitespace = trailing_whitespace.into();
        if trailing_whitespace.is_empty() {
            return Self::empty(child_end_offset);
        }

        Self {
            child_end_offset,
            trailing_whitespace: Some(trailing_whitespace),
        }
    }

    pub fn child_end_offset(&self) -> Offset {
        self.child_end_offset
    }
    pub fn trailing_whitespace_ref(&self) -> Option<&[Cst]> {
        self.trailing_whitespace.as_ref().map(|it| it.as_ref())
    }
    pub fn take(self) -> Option<Cow<'a, [Cst]>> {
        self.trailing_whitespace
    }

    pub fn has_comments(&self) -> bool {
        self.trailing_whitespace
            .as_ref()
            .map(|it| {
                it.iter()
                    .any(|it| matches!(it.kind, CstKind::Comment { .. }))
            })
            .unwrap_or_default()
    }

    pub fn into_empty_trailing(self, edits: &mut TextEdits) {
        assert!(!self.has_comments());

        for whitespace in self.trailing_whitespace_ref().unwrap_or_default() {
            edits.delete(whitespace.data.span.to_owned());
        }
    }
    pub fn into_trailing_with_space(self, edits: &mut TextEdits) {
        assert!(!self.has_comments());

        if let Some(whitespace) = self.trailing_whitespace_ref() {
            edits.change(
                whitespace.first().unwrap().data.span.start
                    ..whitespace.last().unwrap().data.span.end,
                SPACE,
            );
        } else {
            edits.insert(self.child_end_offset, SPACE);
        }
    }
    pub fn into_trailing_with_indentation(self, edits: &mut TextEdits, indentation: Indentation) {
        let trailing_whitespace = self.trailing_whitespace_ref().unwrap_or_default();
        let last_comment_index = trailing_whitespace
            .iter()
            .rposition(|it| matches!(it.kind, CstKind::Comment { .. }));
        let split_index = last_comment_index.map(|it| it + 1).unwrap_or_default();
        let (comments_and_whitespace, final_whitespace) = trailing_whitespace.split_at(split_index);

        Self::format_trailing_comments(comments_and_whitespace, edits, indentation);

        let range = if final_whitespace.is_empty() {
            let offset = comments_and_whitespace
                .last()
                .map(|it| it.data.span.end)
                .unwrap_or(self.child_end_offset);
            offset..offset
        } else {
            final_whitespace.first().unwrap().data.span.start
                ..final_whitespace.last().unwrap().data.span.end
        };
        edits.change(range, format!("{NEWLINE}{}", indentation.to_string()));
    }
    fn format_trailing_comments(
        comments_and_whitespace: &[Cst],
        edits: &mut TextEdits,
        indentation: Indentation,
    ) {
        let mut is_comment_on_same_line = true;
        let mut last_whitespace_range = None;
        for item in comments_and_whitespace {
            match &item.kind {
                CstKind::Whitespace(_)
                | CstKind::Error {
                    error: CstError::TooMuchWhitespace,
                    ..
                } => {
                    if let Some(range) = last_whitespace_range {
                        edits.delete(range);
                    }
                    last_whitespace_range = Some(item.data.span.to_owned());
                }
                CstKind::Newline(_) => {
                    if is_comment_on_same_line {
                        if let Some(range) = last_whitespace_range {
                            // Delete trailing spaces in the previous line.
                            edits.delete(range);
                            last_whitespace_range = None;
                        }

                        is_comment_on_same_line = false;
                        edits.change(item.data.span.to_owned(), NEWLINE);
                    } else {
                        // We already encountered and kept a newline, so we can delete this one.
                        edits.delete(item.data.span.to_owned());
                    }
                }
                CstKind::Comment { .. } => {
                    let space = if is_comment_on_same_line {
                        Cow::Borrowed(SPACE)
                    } else {
                        Cow::Owned(indentation.to_string())
                    };
                    if let Some(range) = last_whitespace_range {
                        edits.change(range, space);
                    } else {
                        edits.insert(item.data.span.start, space);
                    }

                    is_comment_on_same_line = false;
                    last_whitespace_range = None;
                    // TODO: Handle multiple comments on the same line.
                }
                _ => unreachable!(),
            }
        }
    }
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
        let (_, trailing_whitespace) =
            format_cst(&mut edits, &cst, &FormatterInfo::default()).split();
        match trailing.into() {
            TrailingWhitespace::None => trailing_whitespace.into_empty_trailing(&mut edits),
            TrailingWhitespace::Space => trailing_whitespace.into_trailing_with_space(&mut edits),
            TrailingWhitespace::Indentation(indentation) => {
                trailing_whitespace.into_trailing_with_indentation(&mut edits, indentation)
            }
        }
        assert_eq!(edits.apply(), expected);
    }
}
