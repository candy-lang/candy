use crate::{
    cst::{Cst, CstKind},
    rich_ir::{RichIrBuilder, ToRichIr},
};

pub type Rcst = Cst<()>;

impl From<CstKind<()>> for Rcst {
    fn from(value: CstKind<()>) -> Self {
        Self {
            data: (),
            kind: value,
        }
    }
}

impl ToRichIr for Rcst {
    fn build_rich_ir(&self, builder: &mut RichIrBuilder) {
        builder.push(format!("{self:#?}"), None, EnumSet::empty());
    }
}

pub trait SplitOuterTrailingWhitespace {
    fn split_outer_trailing_whitespace(self) -> (Vec<Rcst>, Self);
}
impl SplitOuterTrailingWhitespace for Rcst {
    fn split_outer_trailing_whitespace(self) -> (Vec<Rcst>, Self) {
        match self.kind {
            CstKind::TrailingWhitespace { child, whitespace } => (whitespace, *child),
            _ => (vec![], self),
        }
    }
}

impl<T: SplitOuterTrailingWhitespace> SplitOuterTrailingWhitespace for Vec<T> {
    fn split_outer_trailing_whitespace(mut self) -> (Vec<Rcst>, Self) {
        self.pop().map_or_else(
            || (vec![], vec![]),
            |last| {
                let (whitespace, last) = last.split_outer_trailing_whitespace();
                self.push(last);
                (whitespace, self)
            },
        )
    }
}

impl<T: SplitOuterTrailingWhitespace> SplitOuterTrailingWhitespace for Option<T> {
    fn split_outer_trailing_whitespace(self) -> (Vec<Rcst>, Self) {
        self.map_or_else(
            || (vec![], None),
            |it| {
                let (whitespace, it) = it.split_outer_trailing_whitespace();
                (whitespace, Some(it))
            },
        )
    }
}

impl<A: SplitOuterTrailingWhitespace, B: SplitOuterTrailingWhitespace> SplitOuterTrailingWhitespace
    for (A, Vec<B>)
{
    fn split_outer_trailing_whitespace(self) -> (Vec<Rcst>, Self) {
        let (left, right) = self;
        if right.is_empty() {
            let (whitespace, first) = left.split_outer_trailing_whitespace();
            (whitespace, (first, right))
        } else {
            let (whitespace, second) = right.split_outer_trailing_whitespace();
            (whitespace, (left, second))
        }
    }
}
