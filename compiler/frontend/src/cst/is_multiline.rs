use super::{Cst, CstKind};

pub trait IsMultiline {
    fn is_multiline(&self) -> bool;
}
impl<D> IsMultiline for Cst<D> {
    fn is_multiline(&self) -> bool {
        self.kind.is_multiline()
    }
}
impl<D> IsMultiline for CstKind<D> {
    fn is_multiline(&self) -> bool {
        match self {
            CstKind::EqualsSign => false,
            CstKind::Comma => false,
            CstKind::Dot => false,
            CstKind::Colon => false,
            CstKind::ColonEqualsSign => false,
            CstKind::Bar => false,
            CstKind::OpeningParenthesis => false,
            CstKind::ClosingParenthesis => false,
            CstKind::OpeningBracket => false,
            CstKind::ClosingBracket => false,
            CstKind::OpeningCurlyBrace => false,
            CstKind::ClosingCurlyBrace => false,
            CstKind::Arrow => false,
            CstKind::SingleQuote => false,
            CstKind::DoubleQuote => false,
            CstKind::Percent => false,
            CstKind::Octothorpe => false,
            CstKind::Whitespace(_) => false,
            CstKind::Newline(_) => true,
            CstKind::Comment { .. } => false,
            CstKind::TrailingWhitespace { child, whitespace } => {
                child.is_multiline() || whitespace.is_multiline()
            }
            CstKind::Identifier(_) => false,
            CstKind::Symbol(_) => false,
            CstKind::Int { .. } => false,
            CstKind::OpeningText { .. } => false,
            CstKind::ClosingText { .. } => false,
            CstKind::Text {
                opening,
                parts,
                closing,
            } => opening.is_multiline() || parts.is_multiline() || closing.is_multiline(),
            CstKind::TextPart(_) => false,
            CstKind::TextInterpolation { .. } => false,
            CstKind::BinaryBar { left, bar, right } => {
                left.is_multiline() || bar.is_multiline() || right.is_multiline()
            }
            CstKind::Parenthesized {
                opening_parenthesis,
                inner,
                closing_parenthesis,
            } => {
                opening_parenthesis.is_multiline()
                    || inner.is_multiline()
                    || closing_parenthesis.is_multiline()
            }
            CstKind::Call {
                receiver,
                arguments,
            } => receiver.is_multiline() || arguments.is_multiline(),
            CstKind::List {
                opening_parenthesis,
                items,
                closing_parenthesis,
            } => {
                opening_parenthesis.is_multiline()
                    || items.is_multiline()
                    || closing_parenthesis.is_multiline()
            }
            CstKind::ListItem { value, comma } => {
                value.is_multiline()
                    || comma
                        .as_ref()
                        .map(|comma| comma.is_multiline())
                        .unwrap_or(false)
            }
            CstKind::Struct {
                opening_bracket,
                fields,
                closing_bracket,
            } => {
                opening_bracket.is_multiline()
                    || fields.is_multiline()
                    || closing_bracket.is_multiline()
            }
            CstKind::StructField {
                key_and_colon,
                value,
                comma,
            } => {
                key_and_colon
                    .as_ref()
                    .map(|box (key, colon)| key.is_multiline() || colon.is_multiline())
                    .unwrap_or(false)
                    || value.is_multiline()
                    || comma
                        .as_ref()
                        .map(|comma| comma.is_multiline())
                        .unwrap_or(false)
            }
            CstKind::StructAccess { struct_, dot, key } => {
                struct_.is_multiline() || dot.is_multiline() || key.is_multiline()
            }
            CstKind::Match {
                expression,
                percent,
                cases,
            } => expression.is_multiline() || percent.is_multiline() || cases.is_multiline(),
            CstKind::MatchCase {
                pattern,
                arrow,
                body,
            } => pattern.is_multiline() || arrow.is_multiline() || body.is_multiline(),
            CstKind::Lambda {
                opening_curly_brace,
                parameters_and_arrow,
                body,
                closing_curly_brace,
            } => {
                opening_curly_brace.is_multiline()
                    || parameters_and_arrow
                        .as_ref()
                        .map(|(parameters, arrow)| {
                            parameters.is_multiline() || arrow.is_multiline()
                        })
                        .unwrap_or(false)
                    || body.is_multiline()
                    || closing_curly_brace.is_multiline()
            }
            CstKind::Assignment {
                left,
                assignment_sign,
                body,
            } => left.is_multiline() || assignment_sign.is_multiline() || body.is_multiline(),
            CstKind::Error {
                unparsable_input, ..
            } => unparsable_input.is_multiline(),
        }
    }
}

impl IsMultiline for str {
    fn is_multiline(&self) -> bool {
        self.contains('\n')
    }
}

impl<C: IsMultiline> IsMultiline for Vec<C> {
    fn is_multiline(&self) -> bool {
        self.iter().any(|cst| cst.is_multiline())
    }
}

impl<T: IsMultiline> IsMultiline for Option<T> {
    fn is_multiline(&self) -> bool {
        match self {
            Some(it) => it.is_multiline(),
            None => false,
        }
    }
}

impl<A: IsMultiline, B: IsMultiline> IsMultiline for (A, B) {
    fn is_multiline(&self) -> bool {
        self.0.is_multiline() || self.1.is_multiline()
    }
}
