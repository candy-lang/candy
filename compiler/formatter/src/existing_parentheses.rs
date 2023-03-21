use crate::{
    existing_whitespace::{ExistingWhitespace, TrailingNewlineCount, TrailingWhitespace},
    format::{format_cst, FormattingInfo},
    formatted_cst::{FormattedCst, UnformattedCst},
    text_edits::TextEdits,
    width::Width,
};
use candy_frontend::{
    cst::{Cst, CstKind},
    position::Offset,
};
use std::borrow::Cow;

#[must_use]
pub enum ExistingParentheses<'a> {
    None {
        child_start_offset: Offset,
    },
    Some {
        opening: UnformattedCst<'a>,
        closing: UnformattedCst<'a>,
    },
}
impl<'a> ExistingParentheses<'a> {
    /// Reduces multiple pairs of parentheses around the inner expression to at most one pair that
    /// keeps all comments.
    pub fn split_from(edits: &mut TextEdits, mut cst: &'a Cst) -> (&'a Cst, Self) {
        let mut next_cst = cst;
        let mut trailing_whitespace = vec![];
        let mut parentheses = ExistingParentheses::None {
            child_start_offset: cst.data.span.start,
        };
        loop {
            match &next_cst.kind {
                CstKind::TrailingWhitespace { child, whitespace } => {
                    next_cst = child;
                    trailing_whitespace
                        .push(ExistingWhitespace::new(child.data.span.end, whitespace));
                }
                CstKind::Parenthesized {
                    box opening_parenthesis,
                    inner,
                    box closing_parenthesis,
                } => {
                    next_cst = inner;
                    cst = inner;

                    let new_opening_parenthesis = split_whitespace(opening_parenthesis);
                    let new_closing_parenthesis = {
                        let UnformattedCst { child, whitespace } =
                            split_whitespace(closing_parenthesis);
                        trailing_whitespace.push(whitespace);

                        let mut whitespace = trailing_whitespace.remove(0);
                        for trailing_whitespace in trailing_whitespace.drain(..) {
                            trailing_whitespace
                                .into_empty_and_move_comments_to(edits, &mut whitespace);
                        }
                        UnformattedCst { child, whitespace }
                    };

                    parentheses = match parentheses {
                        ExistingParentheses::None { .. } => ExistingParentheses::Some {
                            opening: new_opening_parenthesis,
                            closing: new_closing_parenthesis,
                        },
                        ExistingParentheses::Some {
                            opening: old_opening_parenthesis,
                            closing: old_closing_parenthesis,
                        } => {
                            pub fn merge<'a>(
                                edits: &mut TextEdits,
                                mut left_parenthesis: UnformattedCst<'a>,
                                right_parenthesis: UnformattedCst<'a>,
                            ) -> UnformattedCst<'a> {
                                if left_parenthesis.whitespace.has_comments() {
                                    edits.delete(right_parenthesis.child.data.span.to_owned());
                                    right_parenthesis
                                        .whitespace
                                        .into_empty_and_move_comments_to(
                                            edits,
                                            &mut left_parenthesis.whitespace,
                                        );
                                    left_parenthesis
                                } else {
                                    edits.delete(left_parenthesis.child.data.span.to_owned());
                                    left_parenthesis.whitespace.into_empty_trailing(edits);
                                    right_parenthesis
                                }
                            }
                            ExistingParentheses::Some {
                                opening: merge(
                                    edits,
                                    old_opening_parenthesis,
                                    new_opening_parenthesis,
                                ),
                                closing: merge(
                                    edits,
                                    new_closing_parenthesis,
                                    old_closing_parenthesis,
                                ),
                            }
                        }
                    };
                }
                _ => break,
            }
        }
        (cst, parentheses)
    }

    pub fn is_some(&self) -> bool {
        match self {
            ExistingParentheses::None { .. } => false,
            ExistingParentheses::Some { .. } => true,
        }
    }
    pub fn are_required_due_to_comments(&self) -> bool {
        match self {
            ExistingParentheses::None { .. } => false,
            ExistingParentheses::Some { opening, .. } => opening.whitespace.has_comments(),
        }
    }

    pub fn into_none(self, edits: &mut TextEdits, child: FormattedCst<'a>) -> FormattedCst<'a> {
        match self {
            ExistingParentheses::None { .. } => child,
            ExistingParentheses::Some {
                opening,
                mut closing,
            } => {
                edits.delete(opening.child.data.span.to_owned());
                opening.whitespace.into_empty_trailing(edits);

                let (child_width, child_whitespace) = child.split();
                child_whitespace.into_empty_and_move_comments_to(edits, &mut closing.whitespace);

                edits.delete(closing.child.data.span.to_owned());

                FormattedCst::new(child_width, closing.whitespace)
            }
        }
    }
    pub fn into_some(
        self,
        edits: &mut TextEdits,
        previous_width: &Width,
        child: FormattedCst<'a>,
        info: &FormattingInfo,
    ) -> FormattedCst<'a> {
        let fits_in_one_line = !self.are_required_due_to_comments()
            && previous_width.last_line_fits(
                info.indentation,
                &(&Width::PARENTHESIS
                    + child.min_width(info.indentation.with_indent())
                    + &Width::PARENTHESIS),
            );
        let child_trailing = if fits_in_one_line {
            TrailingWhitespace::None
        } else {
            TrailingWhitespace::Indentation(info.indentation)
        };
        match self {
            ExistingParentheses::None { child_start_offset } => {
                let (opening, opening_width) = if fits_in_one_line {
                    (Cow::Borrowed("("), Width::PARENTHESIS.clone())
                } else {
                    (
                        Cow::Owned(format!("(\n{}", info.indentation.with_indent())),
                        // We don't have to calculate the exact width here since the child's width
                        // includes a newline.
                        Width::default(),
                    )
                };
                edits.insert(child_start_offset, opening);

                let child_end_offset = child.whitespace.end_offset();
                let child_width = child.into_trailing(edits, child_trailing);

                edits.insert(child_end_offset, ")");

                FormattedCst::new(
                    opening_width + child_width + &Width::PARENTHESIS,
                    ExistingWhitespace::empty(child_end_offset),
                )
            }
            ExistingParentheses::Some { opening, closing } => {
                let opening_width = format_cst(edits, previous_width, opening.child, info)
                    .into_empty_trailing(edits);

                let opening_whitespace_width = if fits_in_one_line {
                    opening.whitespace.into_empty_trailing(edits)
                } else {
                    opening.whitespace.into_trailing_with_indentation(
                        edits,
                        &(previous_width + &Width::PARENTHESIS),
                        info.indentation.with_indent(),
                        TrailingNewlineCount::One,
                        true,
                        false,
                    )
                };

                let child_width = child.into_trailing(edits, child_trailing);

                let width_before_closing = opening_width + opening_whitespace_width + child_width;
                let closing_width = format_cst(
                    edits,
                    &(previous_width + &width_before_closing),
                    closing.child,
                    info,
                )
                .into_empty_trailing(edits);

                FormattedCst::new(width_before_closing + closing_width, closing.whitespace)
            }
        }
    }
}

impl Width {
    pub const PARENTHESIS: Width = Width::Singleline(1);
}

fn split_whitespace(cst: &Cst) -> UnformattedCst {
    if let CstKind::TrailingWhitespace {
        box child,
        whitespace,
    } = &cst.kind
    {
        let mut whitespace = ExistingWhitespace::new(child.data.span.end, whitespace);
        let UnformattedCst {
            child,
            whitespace: child_whitespace,
        } = split_whitespace(child);
        child_whitespace.move_into_outer(&mut whitespace);
        UnformattedCst { child, whitespace }
    } else {
        UnformattedCst {
            child: cst,
            whitespace: ExistingWhitespace::empty(cst.data.span.end),
        }
    }
}
