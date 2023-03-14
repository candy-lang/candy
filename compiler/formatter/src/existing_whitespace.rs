use crate::{text_edits::TextEdits, Indentation};
use candy_frontend::{
    cst::{Cst, CstError, CstKind},
    position::Offset,
};
use derive_more::From;
use extension_trait::extension_trait;
use std::borrow::Cow;

#[extension_trait]
pub impl SplitTrailingWhitespace for Cst {
    fn split_trailing_whitespace(&self) -> (Cow<Cst>, ExistingWhitespace) {
        // TODO: improve
        let (child, child_whitespace) = match &self.kind {
            CstKind::TrailingWhitespace { child, whitespace } => {
                let (child, child_whitespace) = child.split_trailing_whitespace();
                if whitespace.is_empty() {
                    return (child, child_whitespace);
                }

                let whitespace = ExistingWhitespace {
                    child_end_offset: child.data.span.end,
                    trailing_whitespace: Some(Cow::Borrowed(whitespace)),
                };
                (child, child_whitespace.merge_into_outer(whitespace))
            }
            // CstKind::Parenthesized {
            //     opening_parenthesis,
            //     inner,
            //     closing_parenthesis,
            // } => {
            //     let (closing_parenthesis, closing_parenthesis_whitespace) =
            //         closing_parenthesis.split_trailing_whitespace();
            //     let cst = Cst {
            //         data: self.data.clone(),
            //         kind: CstKind::Parenthesized {
            //             opening_parenthesis: opening_parenthesis.to_owned(),
            //             inner: inner.to_owned(),
            //             closing_parenthesis: Box::new(closing_parenthesis.into_owned()),
            //         },
            //     };
            //     (Cow::Owned(cst), closing_parenthesis_whitespace)
            // }
            // CstKind::Call {
            //     receiver,
            //     arguments,
            // } => {
            //     let arguments = arguments.to_owned();
            //     let last_argument = arguments.pop().unwrap();
            //     let (last_argument, last_argument_whitespace) =
            //         last_argument.split_trailing_whitespace();
            //     arguments.push(last_argument.into_owned());
            //     let cst = Cst {
            //         data: self.data.clone(),
            //         kind: CstKind::Call {
            //             receiver: receiver.to_owned(),
            //             arguments,
            //         },
            //     };
            //     (Cow::Owned(cst), last_argument_whitespace)
            // }
            // CstKind::ListItem { value, comma } => {
            //     // Move potential comments before the comma to the end of the item.
            //     let (value, value_whitespace) = value.split_trailing_whitespace();
            //     let cst = Cst {
            //         data: self.data.clone(),
            //         kind: CstKind::ListItem {
            //             value: Box::new(value.into_owned()),
            //             comma: comma.to_owned(),
            //         },
            //     };
            //     (Cow::Owned(cst), value_whitespace)
            // }
            // CstKind::StructField {
            //     key_and_colon,
            //     value,
            //     comma,
            // } => {
            //     // Move potential comments before the comma to the end of the field.
            //     let (value, value_whitespace) = value.split_trailing_whitespace();
            //     let cst = Cst {
            //         data: self.data.clone(),
            //         kind: CstKind::StructField {
            //             key_and_colon: key_and_colon.to_owned(),
            //             value: Box::new(value.into_owned()),
            //             comma: comma.to_owned(),
            //         },
            //     };
            //     (Cow::Owned(cst), value_whitespace)
            // }
            // TODO: struct access key
            _ => (
                Cow::Borrowed(self),
                ExistingWhitespace {
                    child_end_offset: self.data.span.end,
                    trailing_whitespace: None,
                },
            ),
        };

        if child_whitespace.trailing_whitespace_ref().is_none() {
            let child_whitespace = ExistingWhitespace {
                child_end_offset: child.data.span.end,
                trailing_whitespace: None,
            };
            return (child, child_whitespace);
        }

        (child, child_whitespace)
    }
}

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

impl ExistingWhitespace<'_> {
    pub fn trailing_whitespace_ref(&self) -> Option<&[Cst]> {
        self.trailing_whitespace.as_ref().map(|it| it.as_ref())
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

    pub fn merge_into_outer(self, outer: Self) -> Self {
        assert_eq!(
            self.trailing_whitespace
                .as_ref()
                .map(|it| it.last().unwrap().data.span.end)
                .unwrap_or_else(|| self.child_end_offset),
            outer.child_end_offset,
        );

        match (&self.trailing_whitespace, &outer.trailing_whitespace) {
            (_, None) => self,
            (None, _) => outer,
            (Some(inner_trailing_whitespace), Some(outer_trailing_whitespace)) => {
                let mut trailing_whitespace = inner_trailing_whitespace.to_vec();
                trailing_whitespace.extend(outer_trailing_whitespace.iter().cloned());
                ExistingWhitespace {
                    child_end_offset: self.child_end_offset,
                    trailing_whitespace: Some(Cow::Owned(trailing_whitespace)),
                }
            }
        }
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
    use crate::{
        existing_whitespace::SplitTrailingWhitespace, text_edits::TextEdits, width::Indentation,
    };
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

        let (_, trailing_whitespace) = cst.split_trailing_whitespace();

        let mut edits = TextEdits::new(reduced_source);
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
