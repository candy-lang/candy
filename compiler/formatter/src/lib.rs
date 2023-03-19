#![feature(anonymous_lifetime_in_impl_trait)]
#![feature(box_patterns)]
#![feature(let_chains)]

use crate::width::StringWidth;
use candy_frontend::{
    cst::{Cst, CstError, CstKind},
    position::Offset,
};
use existing_whitespace::{ExistingWhitespace, TrailingNewlineCount, TrailingWhitespace};
use extension_trait::extension_trait;
use itertools::Itertools;
use text_edits::TextEdits;
use traversal::dft_post_rev;
use width::{Indentation, Width};

mod existing_whitespace;
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

        let formatted = format_csts(
            &mut edits,
            csts,
            Offset::default(),
            &FormatterInfo::default(),
        );
        let trailing = if formatted.child_width == Width::default() {
            TrailingWhitespace::None
        } else {
            TrailingWhitespace::Indentation(Indentation::default())
        };
        _ = formatted.into_trailing(&mut edits, trailing);

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

fn format_csts<'a>(
    edits: &mut TextEdits,
    mut csts: &'a [Cst],
    fallback_offset: Offset,
    info: &FormatterInfo,
) -> FormattedCst<'a> {
    let mut offset = fallback_offset;
    let mut width = Width::default();
    let mut formatted =
        FormattedCst::new(Width::default(), ExistingWhitespace::empty(fallback_offset));
    loop {
        {
            // Whitespace
            let first_expression_index = csts.iter().find_position(|cst| {
                !matches!(
                    cst.kind,
                    CstKind::Whitespace(_)
                        | CstKind::Error {
                            error: CstError::TooMuchWhitespace,
                            ..
                        }
                        | CstKind::Newline(_)
                        | CstKind::Comment { .. },
                )
            });
            let (new_whitespace, rest) =
                if let Some((first_expression_index, _)) = first_expression_index {
                    csts.split_at(first_expression_index)
                } else {
                    (csts, [].as_slice())
                };
            csts = rest;
            let new_whitespace = ExistingWhitespace::new(offset, new_whitespace);
            new_whitespace.into_empty_and_move_comments_to(edits, &mut formatted.whitespace);
        }

        // Expression
        let Some((expression, rest)) = csts.split_first() else { break; };
        csts = rest;

        let is_at_start = offset == fallback_offset;
        width += if is_at_start && !formatted.whitespace.has_comments() {
            formatted.into_empty_trailing(edits)
        } else {
            formatted.into_trailing_with_indentation_detailed(
                edits,
                info.indentation,
                TrailingNewlineCount::Keep,
            )
        };

        formatted = format_cst(edits, expression, info);
        offset = formatted.whitespace.end_offset();
    }

    FormattedCst::new(width + formatted.child_width, formatted.whitespace)
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
            assert!(formatted_octothorpe
                .min_width(info.indentation)
                .is_singleline());

            formatted_octothorpe.into_empty_trailing(edits) + comment.width()
        }
        CstKind::TrailingWhitespace { child, whitespace } => {
            let mut whitespace = ExistingWhitespace::new(child.data.span.end, whitespace);
            let child = format_cst(edits, child, info);
            let child_width = child.into_empty_and_move_comments_to(edits, &mut whitespace);
            return FormattedCst::new(child_width, whitespace);
        }
        CstKind::Identifier(string) | CstKind::Symbol(string) | CstKind::Int { string, .. } => {
            string.width()
        }
        CstKind::OpeningText {
            opening_single_quotes,
            opening_double_quote,
        } => {
            // TODO: Format text
            opening_single_quotes
                .iter()
                .map(|it| format_cst(edits, it, info).min_width(info.indentation))
                .sum::<Width>()
                + format_cst(edits, opening_double_quote, info).min_width(info.indentation)
        }
        CstKind::ClosingText {
            closing_double_quote,
            closing_single_quotes,
        } => {
            // TODO: Format text
            format_cst(edits, closing_double_quote, info).min_width(info.indentation)
                + closing_single_quotes
                    .iter()
                    .map(|it| format_cst(edits, it, info).min_width(info.indentation))
                    .sum::<Width>()
        }
        CstKind::Text {
            opening,
            parts,
            closing,
        } => {
            // TODO: Format text
            format_cst(edits, opening, info).min_width(info.indentation)
                + parts
                    .iter()
                    .map(|it| format_cst(edits, it, info).min_width(info.indentation))
                    .sum::<Width>()
                + format_cst(edits, closing, info).min_width(info.indentation)
        }
        CstKind::TextPart(text) => text.width(),
        CstKind::TextInterpolation {
            opening_curly_braces,
            expression,
            closing_curly_braces,
        } => {
            // TODO: Format text
            opening_curly_braces
                .iter()
                .map(|it| format_cst(edits, it, info).min_width(info.indentation))
                .sum::<Width>()
                + format_cst(edits, expression, info).min_width(info.indentation)
                + closing_curly_braces
                    .iter()
                    .map(|it| format_cst(edits, it, info).min_width(info.indentation))
                    .sum::<Width>()
        }
        CstKind::BinaryBar { left, bar, right } => {
            let mut left = format_cst(edits, left, info);

            let bar_width = format_cst(edits, bar, info)
                .into_space_and_move_comments_to(edits, &mut left.whitespace);

            let (right, right_parentheses) = split_parenthesized(edits, right);
            // Depending on the precedence of `right` and whether there's an opening parenthesis
            // with a comment, we might be able to remove the parentheses. However, we won't insert
            // any by ourselves.
            let right_needs_parentheses = match right.precedence() {
                Some(PrecedenceCategory::High) => right_parentheses
                    .as_ref()
                    .map(|((_, opening_parenthesis_whitespace), _)| {
                        opening_parenthesis_whitespace.has_comments()
                    })
                    .unwrap_or_default(),
                Some(PrecedenceCategory::Low) | None => right_parentheses.is_some(),
            };
            let right_info = if right_needs_parentheses {
                info.with_indent()
            } else {
                info.to_owned()
            };
            let right = format_cst(edits, right, &right_info);

            let (right_width, whitespace) = if let Some((
                (opening_parenthesis, opening_parenthesis_whitespace),
                (closing_parenthesis, closing_parenthesis_whitespace),
            )) = right_parentheses
            {
                if right_needs_parentheses {
                    let opening_parenthesis_width =
                        format_cst(edits, opening_parenthesis, info).into_empty_trailing(edits);
                    let closing_parenthesis_width =
                        format_cst(edits, closing_parenthesis, info).into_empty_trailing(edits);
                    let (opening_parenthesis_whitespace_width, right_width) =
                        if !opening_parenthesis_whitespace.has_comments()
                            && (left.min_width(info.indentation)
                                + Width::SPACE
                                + &bar_width
                                + &opening_parenthesis_width
                                + right.min_width(info.indentation.with_indent())
                                + &closing_parenthesis_width)
                                .fits(info.indentation)
                        {
                            (
                                opening_parenthesis_whitespace.into_empty_trailing(edits),
                                right.into_trailing_with_space(edits),
                            )
                        } else {
                            (
                                opening_parenthesis_whitespace.into_trailing_with_indentation(
                                    edits,
                                    &(Width::Singleline(1) + Width::SPACE + Width::Singleline(1)),
                                    info.indentation.with_indent(),
                                    TrailingNewlineCount::One,
                                    true,
                                ),
                                right.into_trailing_with_indentation(edits, info.indentation),
                            )
                        };
                    (
                        opening_parenthesis_width
                            + opening_parenthesis_whitespace_width
                            + right_width
                            + closing_parenthesis_width,
                        closing_parenthesis_whitespace,
                    )
                } else {
                    edits.delete(opening_parenthesis.data.span.to_owned());
                    opening_parenthesis_whitespace.into_empty_trailing(edits);
                    let right_width = right.into_empty_trailing(edits);
                    edits.delete(closing_parenthesis.data.span.to_owned());
                    (right_width, closing_parenthesis_whitespace)
                }
            } else {
                right.split()
            };

            let left_trailing =
                if (left.min_width(info.indentation) + Width::SPACE + &bar_width + &right_width)
                    .fits(info.indentation)
                {
                    TrailingWhitespace::Space
                } else {
                    TrailingWhitespace::Indentation(info.indentation)
                };

            return FormattedCst::new(
                left.into_trailing(edits, left_trailing) + bar_width + right_width,
                whitespace,
            );
        }
        CstKind::Parenthesized { .. } => {
            // Whenever parentheses are necessary, they are handled by the parent. Hence, we try to
            // remove them here.
            let (child, parentheses) = split_parenthesized(edits, cst);
            let (
                (opening_parenthesis, opening_parenthesis_whitespace),
                (closing_parenthesis, mut whitespace),
            ) = parentheses.unwrap();

            if !opening_parenthesis_whitespace.has_comments() {
                // We can remove the parentheses.
                edits.delete(opening_parenthesis.data.span.to_owned());
                opening_parenthesis_whitespace.into_empty_trailing(edits);
                let child = format_cst(edits, child, info);
                let (child_width, child_whitespace) = child.split();
                child_whitespace.into_empty_and_move_comments_to(edits, &mut whitespace);
                edits.delete(closing_parenthesis.data.span.to_owned());
                return FormattedCst::new(child_width, whitespace);
            }

            let opening_parenthesis_width =
                format_cst(edits, opening_parenthesis, info).into_empty_trailing(edits);
            let opening_parenthesis_whitespace_width = opening_parenthesis_whitespace
                .into_trailing_with_indentation(
                    edits,
                    &Width::Singleline(1),
                    info.indentation.with_indent(),
                    TrailingNewlineCount::One,
                    true,
                );
            let child_width = format_cst(edits, child, &info.with_indent())
                .into_trailing_with_indentation(edits, info.indentation);
            let closing_parenthesis_width =
                format_cst(edits, closing_parenthesis, info).into_empty_trailing(edits);
            return FormattedCst::new(
                opening_parenthesis_width
                    + opening_parenthesis_whitespace_width
                    + child_width
                    + closing_parenthesis_width,
                whitespace,
            );
        }
        CstKind::Call {
            receiver,
            arguments,
        } => {
            let receiver = format_cst(edits, receiver, info);
            if arguments.is_empty() {
                return receiver;
            }

            let mut arguments = arguments
                .iter()
                .map(|argument| Argument::new(edits, argument, info))
                .collect_vec();

            let min_width = &receiver.min_width(info.indentation)
                + arguments
                    .iter()
                    .map(|it| Width::SPACE + &it.min_singleline_width)
                    .sum::<Width>();
            let (is_singleline, argument_info, trailing) = if min_width.fits(info.indentation) {
                (true, info.to_owned(), TrailingWhitespace::Space)
            } else {
                (
                    false,
                    info.with_indent(),
                    TrailingWhitespace::Indentation(info.indentation.with_indent()),
                )
            };

            let width = receiver.into_trailing(edits, trailing);

            let last_argument = arguments.pop().unwrap();
            let width = arguments.into_iter().fold(width, |old_width, argument| {
                let argument = argument.format(edits, &argument_info, &old_width, is_singleline);
                let width = if is_singleline {
                    argument.into_trailing_with_space(edits)
                } else {
                    argument.into_trailing_with_indentation(edits, argument_info.indentation)
                };
                old_width + width
            });
            let (last_argument_width, whitespace) = last_argument
                .format(edits, &argument_info, &width, is_singleline)
                .split();

            return FormattedCst::new(width + last_argument_width, whitespace);
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

            let (comma_width, mut whitespace) = apply_trailing_comma_condition(
                edits,
                comma.as_deref(),
                value_end,
                info,
                value.min_width(info.indentation),
            );

            return FormattedCst::new(
                value.into_empty_and_move_comments_to(edits, &mut whitespace) + comma_width,
                whitespace,
            );
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
                let mut colon = format_cst(edits, colon, &info.with_indent());
                (
                    key.into_empty_and_move_comments_to(edits, &mut colon.whitespace),
                    colon,
                )
            });

            let value_end = value.data.span.end;
            let value = format_cst(edits, value, &info.with_indent());

            let key_and_colon_min_width = key_width_and_colon
                .as_ref()
                .map(|(key_width, colon)| key_width + &colon.min_width(info.indentation))
                .unwrap_or_default();
            let (comma_width, mut whitespace) = apply_trailing_comma_condition(
                edits,
                comma.as_deref(),
                value_end,
                info,
                &key_and_colon_min_width + value.min_width(info.indentation),
            );
            let value_width = value.into_empty_and_move_comments_to(edits, &mut whitespace);
            let min_width = key_and_colon_min_width + &value_width + &comma_width;

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
            let mut struct_ = format_cst(edits, struct_, info);

            let dot_width = format_cst(edits, dot, &info.with_indent())
                .into_empty_and_move_comments_to(edits, &mut struct_.whitespace);

            let key = format_cst(edits, key, &info.with_indent());

            let min_width =
                struct_.min_width(info.indentation) + &dot_width + key.min_width(info.indentation);
            let struct_trailing = if min_width.fits(info.indentation) {
                TrailingWhitespace::None
            } else {
                TrailingWhitespace::Indentation(info.indentation.with_indent())
            };

            let (key_width, whitespace) = key.split();
            return FormattedCst::new(
                struct_.into_trailing(edits, struct_trailing) + dot_width + key_width,
                whitespace,
            );
        }
        CstKind::Match {
            expression,
            percent,
            cases,
        } => {
            let expression = format_cst(edits, expression, info);

            let mut percent = format_cst(edits, percent, info);
            let expression_width =
                expression.into_space_and_move_comments_to(edits, &mut percent.whitespace);

            let only_has_empty_error_case = matches!(
                cases.as_slice(),
                [Cst {
                    kind: CstKind::Error {
                        unparsable_input,
                        error: CstError::MatchMissesCases,
                    },
                    ..
                }] if unparsable_input.is_empty(),
            );
            let (cases, last_case) = if !only_has_empty_error_case && let [cases @ .., last_case] = cases.as_slice() {
                (cases, last_case)
            } else {
                let (percent_width, whitespace) = percent.split();
                return FormattedCst::new(expression_width + percent_width, whitespace, );
            };

            let percent_width =
                percent.into_trailing_with_indentation(edits, info.indentation.with_indent());

            let (last_case_width, whitespace) =
                format_cst(edits, last_case, &info.with_indent()).split();
            return FormattedCst::new(
                expression_width
                    + percent_width
                    + cases
                        .iter()
                        .map(|it| {
                            format_cst(edits, it, &info.with_indent())
                                .into_trailing_with_indentation(
                                    edits,
                                    info.indentation.with_indent(),
                                )
                        })
                        .sum::<Width>()
                    + last_case_width,
                whitespace,
            );
        }
        CstKind::MatchCase {
            pattern,
            arrow,
            body,
        } => {
            let pattern = format_cst(edits, pattern, info);

            let mut arrow = format_cst(edits, arrow, info);
            let pattern_width =
                pattern.into_space_and_move_comments_to(edits, &mut arrow.whitespace);

            let (body_width, whitespace) = format_csts(
                edits,
                body,
                arrow.whitespace.end_offset(),
                &info.with_indent(),
            )
            .split();

            let arrow_trailing = if pattern_width.last_line_fits(
                info.indentation,
                &(arrow.min_width(info.indentation) + Width::SPACE + &body_width),
            ) {
                TrailingWhitespace::Space
            } else {
                TrailingWhitespace::Indentation(info.indentation.with_indent())
            };

            return FormattedCst::new(
                pattern_width + arrow.into_trailing(edits, arrow_trailing) + body_width,
                whitespace,
            );
        }
        CstKind::Lambda {
            opening_curly_brace,
            parameters_and_arrow,
            body,
            closing_curly_brace,
        } => {
            let opening_curly_brace = format_cst(edits, opening_curly_brace, info);

            let parameters_width_and_arrow =
                parameters_and_arrow.as_ref().map(|(parameters, arrow)| {
                    let mut parameters = parameters
                        .iter()
                        .map(|it| format_cst(edits, it, &info.with_indent()))
                        .collect_vec();
                    let arrow = format_cst(edits, arrow, &info.with_indent());

                    let parameters_trailing = if (opening_curly_brace.min_width(info.indentation)
                        + Width::SPACE
                        + parameters
                            .iter()
                            .map(|it| it.min_width(info.indentation) + Width::SPACE)
                            .sum::<Width>()
                        + arrow.min_width(info.indentation))
                    .fits(info.indentation)
                    {
                        TrailingWhitespace::Space
                    } else {
                        TrailingWhitespace::Indentation(info.indentation.with_indent())
                    };
                    let last_parameter = parameters.pop();
                    let parameters_width = parameters
                        .into_iter()
                        .map(|it| it.into_trailing(edits, parameters_trailing.clone()))
                        .sum::<Width>();

                    let last_parameter_width = last_parameter
                        .map(|it| {
                            let trailing = if parameters_width.last_line_fits(
                                info.indentation,
                                &(it.min_width(info.indentation)
                                    + Width::SPACE
                                    + arrow.min_width(info.indentation)),
                            ) {
                                TrailingWhitespace::Space
                            } else {
                                TrailingWhitespace::Indentation(info.indentation.with_indent())
                            };
                            it.into_trailing(edits, trailing)
                        })
                        .unwrap_or_default();

                    (parameters_width + last_parameter_width, arrow)
                });

            let body_fallback_offset = parameters_width_and_arrow
                .as_ref()
                .map(|(_, arrow)| arrow.whitespace.end_offset())
                .unwrap_or_else(|| opening_curly_brace.whitespace.end_offset());
            let body = format_csts(edits, body, body_fallback_offset, &info.with_indent());
            let (closing_curly_brace_width, whitespace) =
                format_cst(edits, closing_curly_brace, info).split();

            let parameters_and_arrow_min_width = parameters_width_and_arrow
                .as_ref()
                .map(|(parameters_width, arrow)| {
                    parameters_width + arrow.min_width(info.indentation)
                })
                .unwrap_or_default();
            let space_if_parameters = if parameters_width_and_arrow.is_some() {
                Width::SPACE
            } else {
                Width::default()
            };
            let body_min_width = body.min_width(info.indentation);
            let body_and_space_width = if body_min_width.is_empty() {
                Width::default()
            } else {
                &body_min_width + Width::SPACE
            };
            let width_until_arrow = opening_curly_brace.min_width(info.indentation)
                + Width::SPACE
                + &parameters_and_arrow_min_width;
            let width_from_body = body_and_space_width + &closing_curly_brace_width;
            let opening_curly_brace_trailing = if parameters_and_arrow.is_some()
                && width_until_arrow.fits(info.indentation)
                || parameters_and_arrow.is_none()
                    && (&width_until_arrow
                        + &body_min_width
                        + Width::SPACE
                        + &closing_curly_brace_width)
                        .fits(info.indentation)
            {
                TrailingWhitespace::Space
            } else if body_min_width.is_empty() {
                TrailingWhitespace::Indentation(info.indentation)
            } else {
                TrailingWhitespace::Indentation(info.indentation.with_indent())
            };
            let body_trailing = if body.child_width.is_empty() {
                TrailingWhitespace::None
            } else if (&width_until_arrow + &space_if_parameters + &width_from_body)
                .fits(info.indentation)
            {
                TrailingWhitespace::Space
            } else {
                TrailingWhitespace::Indentation(info.indentation)
            };
            // TODO
            let opening_curly_brace_width =
                opening_curly_brace.into_trailing(edits, opening_curly_brace_trailing);
            let arrow_trailing = if width_until_arrow
                .last_line_fits(info.indentation, &(space_if_parameters + width_from_body))
            {
                TrailingWhitespace::Space
            } else {
                TrailingWhitespace::Indentation(info.indentation.with_indent())
            };

            return FormattedCst::new(
                opening_curly_brace_width
                    + parameters_width_and_arrow
                        .map(|(parameters_width, arrow)| {
                            parameters_width + arrow.into_trailing(edits, arrow_trailing)
                        })
                        .unwrap_or_default()
                    + body.into_trailing(edits, body_trailing)
                    + closing_curly_brace_width,
                whitespace,
            );
        }
        CstKind::Assignment {
            left,
            assignment_sign,
            body,
        } => {
            let left = format_cst(edits, left, info);
            let left_width = left.into_trailing_with_space(edits);

            let assignment_sign = format_cst(edits, assignment_sign, &info.with_indent());

            let body = format_csts(
                edits,
                body,
                assignment_sign.whitespace.end_offset(),
                &info.with_indent(),
            );
            let body_width = body.into_trailing_with_indentation_detailed(
                edits,
                info.indentation.with_indent(),
                TrailingNewlineCount::Zero,
            );

            let is_body_in_same_line = left_width.last_line_fits(
                info.indentation,
                &(&assignment_sign.min_width(info.indentation) + Width::SPACE + &body_width),
            );
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

struct Argument<'a> {
    argument_start_offset: Offset,
    argument: FormattedCst<'a>,
    precedence: Option<PrecedenceCategory>,
    parentheses: Option<(UnformattedCst<'a>, UnformattedCst<'a>)>,
    min_singleline_width: Width,
}
impl<'a> Argument<'a> {
    fn new(edits: &mut TextEdits, cst: &'a Cst, info: &FormatterInfo) -> Self {
        let (argument, parentheses) = split_parenthesized(edits, cst);
        let argument_start_offset = argument.data.span.start;
        let precedence = argument.precedence();

        let (argument, min_singleline_width) = if let Some(((_, opening_parenthesis_whitespace), _)) = &parentheses && opening_parenthesis_whitespace.has_comments() {
            let argument = format_cst(edits, argument, &info.with_indent().with_indent());
            (argument, Width::multiline())
        } else {
            let argument = format_cst(edits, argument, info);
            let mut min_width = argument.min_width(info.indentation.with_indent());
            const PARENTHESES_WIDTH: Width = Width::Singleline(2);
            match precedence {
                Some(PrecedenceCategory::High) => {},
                Some(PrecedenceCategory::Low) => min_width += PARENTHESES_WIDTH,
                None if parentheses.is_some() => min_width += PARENTHESES_WIDTH,
                None => {},
            }
            (argument, min_width)
        };
        Argument {
            argument_start_offset,
            argument,
            precedence,
            parentheses,
            min_singleline_width,
        }
    }

    fn format(
        self,
        edits: &mut TextEdits,
        info: &FormatterInfo,
        old_width: &Width,
        is_singleline: bool,
    ) -> FormattedCst<'a> {
        if let Some((
            (opening_parenthesis, opening_parenthesis_whitespace),
            (closing_parenthesis, mut whitespace),
        )) = self.parentheses
        {
            // We already have parentheses …
            let argument_width = if is_singleline
                && self.precedence != Some(PrecedenceCategory::High)
                || opening_parenthesis_whitespace.has_comments()
            {
                // … and we actually need them.
                let opening_parenthesis_width =
                    format_cst(edits, opening_parenthesis, info).into_empty_trailing(edits);
                let closing_parenthesis_width =
                    format_cst(edits, closing_parenthesis, info).into_empty_trailing(edits);
                if is_singleline
                    && old_width.last_line_fits(info.indentation, &self.min_singleline_width)
                    || !is_singleline
                        && old_width.last_line_fits(
                            info.indentation.with_indent(),
                            &self.min_singleline_width,
                        )
                {
                    // The argument fits in one line.
                    assert!(is_singleline);
                    let opening_parenthesis_whitespace_width =
                        opening_parenthesis_whitespace.into_empty_trailing(edits);
                    opening_parenthesis_width
                        + opening_parenthesis_whitespace_width
                        + self.argument.into_empty_trailing(edits)
                        + closing_parenthesis_width
                } else {
                    // The argument goes over multiple lines.
                    let opening_parenthesis_whitespace_width = opening_parenthesis_whitespace
                        .into_trailing_with_indentation(
                            edits,
                            &(old_width + Width::Singleline(1)),
                            info.indentation.with_indent(),
                            TrailingNewlineCount::One,
                            true,
                        );
                    opening_parenthesis_width
                        + opening_parenthesis_whitespace_width
                        + self
                            .argument
                            .into_trailing_with_indentation(edits, info.indentation)
                        + closing_parenthesis_width
                }
            } else {
                // … but we don't need them.
                edits.delete(opening_parenthesis.data.span.to_owned());
                opening_parenthesis_whitespace.into_empty_trailing(edits);
                edits.delete(closing_parenthesis.data.span.to_owned());
                let (argument_width, argument_whitespace) = self.argument.split();
                argument_whitespace.into_empty_and_move_comments_to(edits, &mut whitespace);
                argument_width
            };
            FormattedCst::new(argument_width, whitespace)
        } else {
            // We don't have parentheses …
            if is_singleline && self.precedence == Some(PrecedenceCategory::Low) {
                // … but we need them.
                // This can only be the case if the whole call fits on one line.
                edits.insert(self.argument_start_offset, "(");
                edits.insert(self.argument.whitespace.start_offset(), ")");
                let (argument_width, whitespace) = self.argument.split();
                FormattedCst::new(
                    Width::Singleline(1) + argument_width + Width::Singleline(1),
                    whitespace,
                )
            } else {
                // … and we don't need them.
                self.argument
            }
        }
    }
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
        + &opening_punctuation.min_width(info.indentation)
        + &closing_punctuation.min_width(info.indentation);
    let item_info = info
        .with_indent()
        .with_trailing_comma_condition(TrailingCommaCondition::Always);
    let items = items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let is_single_item = items.len() == 1;
            let is_last_item = index == items.len() - 1;

            let is_comma_required_due_to_single_item =
                is_single_item && is_comma_required_for_single_item;
            let is_comma_required =
                is_comma_required_due_to_single_item || !is_last_item || item.has_comments();
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
                    && let Width::Singleline(item_min_width) = item.min_width(info.indentation) {
                let (item_min_width, max_width) = if is_last_item {
                    (item_min_width, Width::MAX)
                } else {
                    // We need an additional column for the trailing space after the comma.
                    let item_min_width = item_min_width + 1;

                    // The last item needs at least one column of space.
                    let max_width = Width::MAX - 1;

                    (item_min_width, max_width)
                };
                min_width = Width::from_width_and_max(old_min_width + item_min_width, max_width);
            } else {
                min_width = Width::multiline();
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

type UnformattedCst<'a> = (&'a Cst, ExistingWhitespace<'a>);
fn split_parenthesized<'a>(
    edits: &mut TextEdits,
    mut cst: &'a Cst,
) -> (&'a Cst, Option<(UnformattedCst<'a>, UnformattedCst<'a>)>) {
    let mut parentheses: Option<(UnformattedCst, UnformattedCst)> = None;
    while let CstKind::Parenthesized {
        box opening_parenthesis,
        inner,
        box closing_parenthesis,
    } = &cst.kind
    {
        cst = inner;

        let (new_opening_parenthesis, new_opening_parenthesis_whitespace) =
            split_whitespace(opening_parenthesis);
        let (new_closing_parenthesis, new_closing_parenthesis_whitespace) =
            split_whitespace(closing_parenthesis);
        let new_parentheses = if let Some((
            (old_opening_parenthesis, mut old_opening_parenthesis_whitespace),
            (old_closing_parenthesis, mut old_closing_parenthesis_whitespace),
        )) = parentheses
        {
            // TODO: helper function
            let opening = if old_opening_parenthesis_whitespace.has_comments() {
                edits.delete(new_opening_parenthesis.data.span.to_owned());
                new_opening_parenthesis_whitespace.into_empty_and_move_comments_to(
                    edits,
                    &mut old_opening_parenthesis_whitespace,
                );
                (old_opening_parenthesis, old_opening_parenthesis_whitespace)
            } else {
                edits.delete(old_opening_parenthesis.data.span.to_owned());
                old_opening_parenthesis_whitespace.into_empty_trailing(edits);
                (new_opening_parenthesis, new_opening_parenthesis_whitespace)
            };
            let closing = if old_closing_parenthesis_whitespace.has_comments() {
                edits.delete(new_closing_parenthesis.data.span.to_owned());
                new_closing_parenthesis_whitespace.into_empty_and_move_comments_to(
                    edits,
                    &mut old_closing_parenthesis_whitespace,
                );
                (old_closing_parenthesis, old_closing_parenthesis_whitespace)
            } else {
                edits.delete(old_closing_parenthesis.data.span.to_owned());
                old_closing_parenthesis_whitespace.into_empty_trailing(edits);
                (new_closing_parenthesis, new_closing_parenthesis_whitespace)
            };
            (opening, closing)
        } else {
            (
                (new_opening_parenthesis, new_opening_parenthesis_whitespace),
                (new_closing_parenthesis, new_closing_parenthesis_whitespace),
            )
        };
        parentheses = Some(new_parentheses);
    }
    (cst, parentheses)
}

fn apply_trailing_comma_condition<'a>(
    edits: &mut TextEdits,
    comma: Option<&'a Cst>,
    fallback_offset: Offset,
    info: &FormatterInfo,
    min_width_except_comma: Width,
) -> (Width, ExistingWhitespace<'a>) {
    let should_have_comma = match info.trailing_comma_condition {
        Some(TrailingCommaCondition::Always) => true,
        Some(TrailingCommaCondition::UnlessFitsIn(max_width)) => {
            !min_width_except_comma.fits_in(max_width)
        }
        None => comma.is_some(),
    };
    if should_have_comma {
        let whitespace = if let Some(comma) = comma {
            let comma = format_cst(edits, comma, info);
            assert_eq!(comma.child_width, Width::Singleline(1));
            comma.whitespace
        } else {
            edits.insert(fallback_offset, ",");
            ExistingWhitespace::empty(fallback_offset)
        };
        (Width::Singleline(1), whitespace)
    } else if let Some(comma) = comma {
        if comma.has_comments() {
            // This last item can't fit on one line, so we do have to keep the comma.
            format_cst(edits, comma, info).split()
        } else {
            edits.delete(comma.data.span.to_owned());
            (
                Width::default(),
                ExistingWhitespace::empty(comma.data.span.end),
            )
        }
    } else {
        (Width::default(), ExistingWhitespace::empty(fallback_offset))
    }
}

fn split_whitespace(cst: &Cst) -> (&Cst, ExistingWhitespace) {
    if let CstKind::TrailingWhitespace {
        box child,
        whitespace,
    } = &cst.kind
    {
        let mut whitespace = ExistingWhitespace::new(child.data.span.end, whitespace);
        let (child, child_whitespace) = split_whitespace(child);
        child_whitespace.move_to_outer(&mut whitespace);
        (child, whitespace)
    } else {
        (cst, ExistingWhitespace::empty(cst.data.span.end))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PrecedenceCategory {
    High,
    Low,
}

#[extension_trait]
impl<D> CstHasCommentsAndPrecedence for Cst<D> {
    fn has_comments(&self) -> bool {
        dft_post_rev(self, |it| it.children().into_iter())
            .any(|(_, it)| matches!(it.kind, CstKind::Comment { .. }))
    }

    fn precedence(&self) -> Option<PrecedenceCategory> {
        match &self.kind {
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
            | CstKind::Comment { .. } => None,
            CstKind::TrailingWhitespace { child, .. } => child.precedence(),
            CstKind::Identifier(_) | CstKind::Symbol(_) | CstKind::Int { .. } => {
                Some(PrecedenceCategory::High)
            }
            CstKind::OpeningText { .. } | CstKind::ClosingText { .. } => None,
            CstKind::Text { .. } => Some(PrecedenceCategory::High),
            CstKind::TextPart(_) => todo!(),
            CstKind::TextInterpolation { .. } => None,
            CstKind::BinaryBar { .. } => Some(PrecedenceCategory::Low),
            CstKind::Parenthesized { .. } => Some(PrecedenceCategory::High),
            CstKind::Call { .. } => Some(PrecedenceCategory::Low),
            CstKind::List { .. } => Some(PrecedenceCategory::High),
            CstKind::ListItem { .. } => None,
            CstKind::Struct { .. } => Some(PrecedenceCategory::High),
            CstKind::StructField { .. } => None,
            CstKind::StructAccess { .. } => Some(PrecedenceCategory::High),
            CstKind::Match { .. } => Some(PrecedenceCategory::Low),
            CstKind::MatchCase { .. } => None,
            CstKind::Lambda { .. } => Some(PrecedenceCategory::High),
            CstKind::Assignment { .. } | CstKind::Error { .. } => None,
        }
    }
}

#[must_use]
struct FormattedCst<'a> {
    /// The minimum width that this CST node could take after formatting.
    ///
    /// If there are trailing comments, this is [Width::Multiline]. Otherwise, it's the child's own
    /// width.
    child_width: Width,
    whitespace: ExistingWhitespace<'a>,
}
impl<'a> FormattedCst<'a> {
    pub fn new(child_width: Width, whitespace: ExistingWhitespace<'a>) -> Self {
        Self {
            child_width,
            whitespace,
        }
    }

    #[must_use]
    pub fn min_width(&self, indentation: Indentation) -> Width {
        if self.whitespace.has_comments() {
            Width::Multiline {
                last_line_width: Some(indentation.width()),
            }
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
        self.into_trailing_with_indentation_detailed(edits, indentation, TrailingNewlineCount::One)
    }
    #[must_use]
    pub fn into_trailing_with_indentation_detailed(
        self,
        edits: &mut TextEdits,
        indentation: Indentation,
        trailing_newline_count: TrailingNewlineCount,
    ) -> Width {
        &self.child_width
            + self.whitespace.into_trailing_with_indentation(
                edits,
                &self.child_width,
                indentation,
                trailing_newline_count,
                !self.child_width.is_empty(),
            )
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
    fn test_binary_bar() {
        test("foo | bar", "foo | bar\n");
        test("foo|bar", "foo | bar\n");
        test("foo  |  bar", "foo | bar\n");
        test("foo\n\n|   bar", "foo | bar\n");
        test("foo | (bar)", "foo | bar\n");
        test("foo | (\n  bar\n)", "foo | bar\n");
        test(
            "veryVeryVeryVeryVeryVeryVeryVeryLongReceiver | (veryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongFunction)",
            "veryVeryVeryVeryVeryVeryVeryVeryLongReceiver | veryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongFunction\n",
        );
        test(
            "veryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongReceiver | veryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongFunction",
            "veryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongReceiver\n| veryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongFunction\n",
        );
        test(
            "foo | veryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongFunction0 veryVeryVeryVeryVeryVeryVeryLongArgument0",
            "foo\n| veryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongFunction0 veryVeryVeryVeryVeryVeryVeryLongArgument0\n",
        );
        test(
            "veryVeryVeryVeryVeryVeryVeryVeryLongReceiver | veryVeryVeryVeryVeryVeryVeryVeryLongFunction longArgument",
            "veryVeryVeryVeryVeryVeryVeryVeryLongReceiver\n| veryVeryVeryVeryVeryVeryVeryVeryLongFunction longArgument\n",
        );
        test(
            "veryVeryVeryVeryVeryVeryVeryVeryLongReceiver | veryVeryVeryVeryVeryVeryVeryVeryLongFunction0 | veryVeryVeryVeryVeryVeryVeryVeryLongFunction1",
            "veryVeryVeryVeryVeryVeryVeryVeryLongReceiver | veryVeryVeryVeryVeryVeryVeryVeryLongFunction0\n| veryVeryVeryVeryVeryVeryVeryVeryLongFunction1\n",
        );
        test(
            "veryVeryVeryVeryVeryVeryVeryVeryLongReceiver | veryVeryVeryVeryVeryVeryVeryVeryLongFunction0 longArgument0 | veryVeryVeryVeryVeryVeryVeryVeryLongFunction1 longArgument1 longArgument2",
            "veryVeryVeryVeryVeryVeryVeryVeryLongReceiver\n| veryVeryVeryVeryVeryVeryVeryVeryLongFunction0 longArgument0\n| veryVeryVeryVeryVeryVeryVeryVeryLongFunction1 longArgument1 longArgument2\n",
        );
        // test(
        //     "veryVeryVeryVeryVeryVeryVeryVeryLongReceiver | veryVeryVeryVeryVeryVeryVeryVeryLongFunction longArgument0 longArgument1 longArgument2 longArgument3",
        //     "veryVeryVeryVeryVeryVeryVeryVeryLongReceiver\n| veryVeryVeryVeryVeryVeryVeryVeryLongFunction\n  longArgument0\n  longArgument1\n  longArgument2\n  longArgument3\n",
        // ); // FIXME
        test(
            "foo | veryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongFunction0 veryVeryVeryVeryVeryVeryVeryLongArgument0 | function1",
            "foo\n| veryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongFunction0 veryVeryVeryVeryVeryVeryVeryLongArgument0\n| function1\n",
        );
        // Comments
        test("foo | bar # abc", "foo | bar # abc\n");
        test("foo | # abc\n  bar", "foo # abc\n| bar\n");
        test("foo # abc\n| bar", "foo # abc\n| bar\n");
    }
    #[test]
    fn test_parenthesized() {
        test("(foo)", "foo\n");
        test(" ( foo ) ", "foo\n");
        test("(\n  foo)", "foo\n");
        test("(\n  foo\n)", "foo\n");
        test(
            "(veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryItemmm)",
            "veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryItemmm\n",
        );
        test(
            "(\n  veryVeryVeryVeryVeryVeryVeryVeryLongReceiver veryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongArgumentt)",
            "veryVeryVeryVeryVeryVeryVeryVeryLongReceiver veryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongArgumentt\n",
        );
        test(
            "(veryVeryVeryVeryVeryVeryVeryVeryLongReceiver veryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongArgumenttt)",
            "veryVeryVeryVeryVeryVeryVeryVeryLongReceiver veryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongArgumenttt\n",
        );

        // Comments
        test("(foo) # abc", "foo # abc\n");
        test("(foo)# abc", "foo # abc\n");
        test("(foo# abc\n)", "foo # abc\n");
        test("(foo # abc\n)", "foo # abc\n");
        test("(# abc\n  foo)", "( # abc\n  foo\n)\n");
        test("(((# abc\n  foo)))", "( # abc\n  foo\n)\n");
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

        // Parentheses
        test("foo (bar)", "foo bar\n");
        test("foo (bar baz)", "foo (bar baz)\n");
        test("foo\n  bar baz", "foo (bar baz)\n");
        test(
            "foo (firstVeryVeryVeryVeryVeryVeryVeryVeryLongArgument secondVeryVeryVeryVeryVeryVeryVeryLongArgument)",
            "foo\n  firstVeryVeryVeryVeryVeryVeryVeryVeryLongArgument secondVeryVeryVeryVeryVeryVeryVeryLongArgument\n",
        );
        test("foo (# abc\n  bar\n)", "foo\n  ( # abc\n    bar\n  )\n");

        // Comments
        test("foo # abc\n  bar\n  Baz", "foo # abc\n  bar\n  Baz\n");
        test("foo\n  # abc\n  bar\n  Baz", "foo\n  # abc\n  bar\n  Baz\n");
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
        test("[foo: bar # abc\n]", "[\n  foo: bar, # abc\n]\n");
        test("[foo: # abc\n  bar\n]", "[\n  foo: # abc\n    bar,\n]\n");
        test("[# abc\n  foo: bar]", "[ # abc\n  foo: bar,\n]\n");
        test(
            "[foo: bar # abc\n  , baz]",
            "[\n  foo: bar, # abc\n  baz,\n]\n",
        );
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
    fn test_match() {
        test("foo % ", "foo %\n");
        test(
            "foo %\n  Foo -> Foo\n  Bar -> Bar",
            "foo %\n  Foo -> Foo\n  Bar -> Bar\n",
        );
        test(
            "foo%\n  Foo->Foo\n\n  Bar  ->  Bar",
            "foo %\n  Foo -> Foo\n  Bar -> Bar\n",
        );

        // Comments
        test("foo%# abc\n  Bar -> Baz", "foo % # abc\n  Bar -> Baz\n");
        test(
            "foo %\n  Bar # abc\n  -> Baz",
            "foo %\n  Bar -> # abc\n    Baz\n",
        );
    }
    #[test]
    fn test_lambda() {
        // No parameters
        test("{}", "{ }\n");
        test("{ }", "{ }\n");
        test("{ foo }", "{ foo }\n");
        test("{\n  foo\n}", "{ foo }\n");
        test("{\n  foo\n  bar\n}", "{\n  foo\n  bar\n}\n");
        test("{\n  foo\n \n  bar\n}", "{\n  foo\n\n  bar\n}\n");
        test(
            "{ veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongBodyy }",
            "{\n  veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongBodyy\n}\n",
        );

        test("{ foo -> }", "{ foo -> }\n");
        test("{ foo -> bar }", "{ foo -> bar }\n");
        test(
            "{ parameter veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongParameter -> foo }",
            "{\n  parameter veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongParameter ->\n  foo\n}\n",
        );
        test(
            "{ parameter veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongParameterr -> foo }",
            "{\n  parameter\n  veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongParameterr -> foo\n}\n",
        );
        test(
            "{ parameter veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongParameter -> foo }",
            "{\n  parameter\n  veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongParameter ->\n  foo\n}\n",
        );
        test(
            "{ parameter veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongParameter -> foo }",
            "{\n  parameter\n  veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongParameter\n  -> foo\n}\n",
        );
        test(
            "{ parameter -> veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongBody\n}\n",
            "{ parameter ->\n  veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongBody\n}\n",
        );

        // Comments
        test("{ # abc\n}", "{ # abc\n}\n");
        test("{ foo # abc\n}", "{\n  foo # abc\n}\n");
        test("{ foo -> bar # abc\n}", "{ foo ->\n  bar # abc\n}\n");
        test("{ foo -> # abc\n  bar\n}", "{ foo -> # abc\n  bar\n}\n");
        test(
            "{ foo# abc\n  ->\n  bar\n}",
            "{\n  foo # abc\n  -> bar\n}\n",
        );
        test(
            "{ # abc\n  foo ->\n  bar\n}",
            "{\n  # abc\n  foo -> bar\n}\n",
        );
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
        test(
            "foo = bar # veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongComment\n",
            "foo =\n  bar # veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongComment\n",
        );
        test(
            "foo = bar # veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongComment\n",
            "foo =\n  bar\n  # veryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryVeryLongComment\n",
        );
    }

    fn test(source: &str, expected: &str) {
        let csts = parse_rcst(source).to_csts();
        assert_eq!(source, csts.iter().join(""));

        let formatted = csts.as_slice().format_to_string();
        assert_eq!(formatted, expected);
    }
}
