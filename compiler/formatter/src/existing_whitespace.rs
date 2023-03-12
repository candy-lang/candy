use candy_frontend::{
    cst::{Cst, CstData, CstError, CstKind, Id},
    id::IdGenerator,
};
use extension_trait::extension_trait;
use std::{borrow::Cow, ops::Range};

pub fn indentation<D>(indentation_level: usize) -> CstKind<D> {
    CstKind::Whitespace("  ".repeat(indentation_level))
}

#[extension_trait]
pub impl SplitTrailingWhitespace for Cst {
    fn split_trailing_whitespace(&self) -> (&Cst, ExistingWhitespace) {
        match &self.kind {
            CstKind::TrailingWhitespace { child, whitespace } => (
                child,
                ExistingWhitespace::Some {
                    id: self.data.id,
                    trailing_whitespace: Cow::Borrowed(whitespace),
                },
            ),
            _ => (self, ExistingWhitespace::None),
        }
    }
}

pub enum ExistingWhitespace<'a> {
    None,
    Some {
        id: Id,
        trailing_whitespace: Cow<'a, [Cst]>,
    },
}
#[derive(Clone)]
pub enum TrailingWhitespace {
    None,
    Space,
    Indentation(usize),
}
impl ExistingWhitespace<'_> {
    fn trailing_whitespace(&self) -> Option<&[Cst]> {
        match self {
            ExistingWhitespace::None => None,
            ExistingWhitespace::Some {
                trailing_whitespace,
                ..
            } => Some(trailing_whitespace),
        }
    }
    pub fn has_comments(&self) -> bool {
        self.trailing_whitespace()
            .map(|it| {
                it.iter()
                    .any(|it| matches!(it.kind, CstKind::Comment { .. }))
            })
            .unwrap_or_default()
    }

    pub fn merge_into(self, other: Self) -> Self {
        match (self, other) {
            (this, ExistingWhitespace::None) => this,
            (ExistingWhitespace::None, other) => other,
            (
                ExistingWhitespace::Some {
                    trailing_whitespace: self_trailing_whitespace,
                    ..
                },
                ExistingWhitespace::Some {
                    id,
                    trailing_whitespace: other_trailing_whitespace,
                },
            ) => {
                let mut trailing_whitespace = self_trailing_whitespace.to_vec();
                trailing_whitespace.extend(other_trailing_whitespace.iter().cloned());
                ExistingWhitespace::Some {
                    id,
                    trailing_whitespace: Cow::Owned(trailing_whitespace),
                }
            }
        }
    }

    pub fn into_trailing(
        self,
        id_generator: &mut IdGenerator<Id>,
        child: Cst,
        trailing: TrailingWhitespace,
    ) -> Cst {
        match trailing {
            TrailingWhitespace::None => self.into_empty_trailing(child),
            TrailingWhitespace::Space => self.into_trailing_with_space(id_generator, child),
            TrailingWhitespace::Indentation(indentation_level) => {
                self.into_trailing_with_indentation(id_generator, child, indentation_level)
            }
        }
    }
    pub fn into_empty_trailing(self, child: Cst) -> Cst {
        assert!(!self.has_comments());

        child
    }
    pub fn into_trailing_with_space(self, id_generator: &mut IdGenerator<Id>, child: Cst) -> Cst {
        assert!(!self.has_comments());

        let final_whitespace_id = self
            .trailing_whitespace()
            .unwrap_or_default()
            .iter()
            .find(|it| matches!(it.kind, CstKind::Whitespace(_)))
            .map(|it| it.data.id)
            .unwrap_or(id_generator.generate());
        let whitespace = vec![Cst {
            data: CstData {
                id: final_whitespace_id,
                span: Range::default(),
            },
            kind: CstKind::Whitespace(" ".to_owned()),
        }];
        self.into_trailing_helper(id_generator, child, whitespace)
    }
    pub fn into_trailing_with_indentation(
        self,
        id_generator: &mut IdGenerator<Id>,
        child: Cst,
        indentation_level: usize,
    ) -> Cst {
        let trailing_whitespace = self.trailing_whitespace().unwrap_or_default();
        let last_comment_index = trailing_whitespace
            .iter()
            .rposition(|it| matches!(it.kind, CstKind::Comment { .. }));
        let split_index = last_comment_index.map(|it| it + 1).unwrap_or_default();
        let (comments_and_whitespace, final_whitespace) = trailing_whitespace.split_at(split_index);

        let mut whitespace = Self::format_trailing_comments(
            comments_and_whitespace,
            id_generator,
            indentation_level,
        );

        let existing_newline_index = final_whitespace
            .iter()
            .position(|it| matches!(it.kind, CstKind::Newline(_)));
        let newline_id = existing_newline_index
            .map(|it| final_whitespace[it].data.id)
            .unwrap_or(id_generator.generate());
        whitespace.push(Cst {
            data: CstData {
                id: newline_id,
                span: Range::default(),
            },
            kind: CstKind::Newline("\n".to_owned()),
        });

        if indentation_level > 0 {
            let search_start_index = existing_newline_index.map(|it| it + 1).unwrap_or_default();
            let indentation_id = final_whitespace[search_start_index..]
                .iter()
                .find(|it| matches!(it.kind, CstKind::Whitespace(_)))
                .map(|it| it.data.id)
                .unwrap_or(id_generator.generate());
            whitespace.push(Cst {
                data: CstData {
                    id: indentation_id,
                    span: Range::default(),
                },
                kind: indentation(indentation_level),
            });
        }

        self.into_trailing_helper(id_generator, child, whitespace)
    }
    fn format_trailing_comments(
        comments_and_whitespace: &[Cst],
        id_generator: &mut IdGenerator<Id>,
        indentation_level: usize,
    ) -> Vec<Cst> {
        let mut whitespace = vec![];
        let mut is_comment_on_same_line = true;
        let mut last_newline_id = None;
        let mut last_whitespace_id = None;
        for item in comments_and_whitespace {
            match &item.kind {
                CstKind::Whitespace(_)
                | CstKind::Error {
                    error: CstError::TooMuchWhitespace,
                    ..
                } => {
                    last_whitespace_id = Some(item.data.id);
                }
                CstKind::Newline(_) => {
                    is_comment_on_same_line = false;
                    last_newline_id = Some(item.data.id);
                    last_whitespace_id = None;
                }
                CstKind::Comment { .. } => {
                    if is_comment_on_same_line {
                        assert_eq!(last_newline_id, None);
                        whitespace.push(Cst {
                            data: CstData {
                                id: last_whitespace_id.unwrap_or(id_generator.generate()),
                                span: Range::default(),
                            },
                            kind: CstKind::Whitespace(" ".to_owned()),
                        });
                    } else {
                        whitespace.push(Cst {
                            data: CstData {
                                id: last_newline_id.unwrap_or(id_generator.generate()),
                                span: Range::default(),
                            },
                            kind: CstKind::Newline("\n".to_owned()),
                        });
                        whitespace.push(Cst {
                            data: CstData {
                                id: last_whitespace_id.unwrap_or(id_generator.generate()),
                                span: Range::default(),
                            },
                            kind: indentation(indentation_level),
                        });
                    }
                    whitespace.push(item.clone());
                    last_newline_id = None;
                    last_whitespace_id = None;
                }
                _ => unreachable!(),
            }
        }
        whitespace
    }
    fn into_trailing_helper(
        self,
        id_generator: &mut IdGenerator<Id>,
        child: Cst,
        whitespace: Vec<Cst>,
    ) -> Cst {
        Cst {
            data: CstData {
                id: match self {
                    ExistingWhitespace::None => id_generator.generate(),
                    ExistingWhitespace::Some { id, .. } => id,
                },
                span: Range::default(),
            },
            kind: CstKind::TrailingWhitespace {
                child: Box::new(child),
                whitespace,
            },
        }
    }
}

#[cfg(test)]
mod test {
    use crate::existing_whitespace::SplitTrailingWhitespace;
    use candy_frontend::{
        cst::CstKind, id::IdGenerator, rcst_to_cst::RcstsToCstsExt, string_to_rcst::parse_rcst,
    };

    use super::TrailingWhitespace::{self, *};

    #[test]
    fn test_empty_trailing() {
        test("foo End", None, "foo");
        test("foo  End", None, "foo");
    }

    #[test]
    fn test_trailing_with_space() {
        test("foo End", Space, "foo ");
        test("foo  End", Space, "foo ");
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

    fn test(source: &str, trailing: TrailingWhitespace, expected: &str) {
        let mut csts = parse_rcst(source).to_csts();
        assert_eq!(csts.len(), 1);

        let cst = match csts.pop().unwrap().kind {
            CstKind::Call { receiver, .. } => receiver,
            _ => panic!("Expected a call"),
        };

        let (cst, trailing_whitespace) = cst.split_trailing_whitespace();

        let mut id_generator = IdGenerator::default();
        let formatted = trailing_whitespace
            .into_trailing(&mut id_generator, cst.into_owned(), trailing)
            .to_string();
        assert_eq!(formatted, expected);
    }
}
