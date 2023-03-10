use candy_frontend::{
    cst::{Cst, CstData, CstKind, Id},
    id::IdGenerator,
};
use extension_trait::extension_trait;
use itertools::Itertools;
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
        indentation_level: Option<usize>,
    ) -> Cst {
        match indentation_level {
            Some(indentation_level) => {
                self.into_trailing_with_indentation(id_generator, child, indentation_level)
            }
            None => self.into_trailing_with_space(id_generator, child),
        }
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
        let (comments, final_whitespace) = trailing_whitespace.split_at(split_index);
        // TODO: format comments
        let mut whitespace = comments.iter().cloned().collect_vec();

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
