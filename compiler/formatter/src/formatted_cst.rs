use crate::{
    existing_whitespace::{ExistingWhitespace, TrailingNewlineCount, TrailingWhitespace},
    text_edits::TextEdits,
    width::{Indentation, Width},
};
use candy_frontend::cst::Cst;

pub struct UnformattedCst<'a> {
    pub child: &'a Cst,
    pub whitespace: ExistingWhitespace<'a>,
}

/// When a CST node is formatted, it returns a [FormattedCst] with its own width and whatever
/// trailing whitespace it contained.
///
/// The parent must later decide what to do with the trailing whitespace and call either of the
/// `into…` methods.
#[must_use]
pub struct FormattedCst<'a> {
    /// The minimum width that this CST node could take after formatting.
    ///
    /// If there are trailing comments, this is [Width::Multiline]. Otherwise, it's the child's own
    /// width.
    child_width: Width,
    pub whitespace: ExistingWhitespace<'a>,
}
impl<'a> FormattedCst<'a> {
    pub fn new(child_width: Width, whitespace: ExistingWhitespace<'a>) -> Self {
        Self {
            child_width,
            whitespace,
        }
    }

    #[must_use]
    pub fn child_width(&self) -> &Width {
        &self.child_width
    }
    #[must_use]
    pub fn min_width(&self, indentation: Indentation) -> Width {
        if self.whitespace.has_comments() {
            Width::multiline(indentation.width())
        } else {
            self.child_width.clone()
        }
    }

    #[must_use]
    pub fn split(self) -> (Width, ExistingWhitespace<'a>) {
        (self.child_width, self.whitespace)
    }

    #[must_use]
    pub fn into_space_and_move_comments_to(
        self,
        edits: &mut TextEdits,
        other: &mut ExistingWhitespace<'a>,
    ) -> Width {
        self.whitespace
            .into_space_and_move_comments_to(edits, other);
        self.child_width + Width::SPACE
    }
    #[must_use]
    pub fn into_empty_and_move_comments_to(
        self,
        edits: &mut TextEdits,
        other: &mut ExistingWhitespace<'a>,
    ) -> Width {
        self.whitespace
            .into_empty_and_move_comments_to(edits, other);
        self.child_width
    }

    #[must_use]
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
    #[must_use]
    pub fn into_empty_trailing(self, edits: &mut TextEdits) -> Width {
        self.child_width + self.whitespace.into_empty_trailing(edits)
    }
    #[must_use]
    pub fn into_trailing_with_space(self, edits: &mut TextEdits) -> Width {
        self.child_width + self.whitespace.into_trailing_with_space(edits)
    }
    #[must_use]
    pub fn into_trailing_with_indentation(
        self,
        edits: &mut TextEdits,
        indentation: Indentation,
    ) -> Width {
        self.into_trailing_with_indentation_detailed(
            edits,
            indentation,
            TrailingNewlineCount::One,
            false,
        )
    }
    #[must_use]
    pub fn into_trailing_with_indentation_detailed(
        self,
        edits: &mut TextEdits,
        indentation: Indentation,
        trailing_newline_count: TrailingNewlineCount,
        is_directly_inside_body: bool,
    ) -> Width {
        &self.child_width
            + self.whitespace.into_trailing_with_indentation(
                edits,
                &self.child_width,
                indentation,
                trailing_newline_count,
                !self.child_width.is_empty(),
                is_directly_inside_body,
            )
    }
}
