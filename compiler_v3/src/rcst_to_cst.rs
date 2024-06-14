use super::{
    cst::{Cst, CstKind},
    rcst::Rcst,
};
use crate::{
    cst::{CstData, Id},
    id::IdGenerator,
    position::Offset,
};

pub fn rcst_to_cst(rcsts: &[Rcst]) -> Vec<Cst> {
    rcsts.lower(&mut State::default())
}

#[derive(Default)]
struct State {
    offset: Offset,
    id_generator: IdGenerator<Id>,
}

trait Lower {
    type Output;

    fn lower(&self, state: &mut State) -> Self::Output;
}

impl<R: Lower> Lower for Vec<R> {
    type Output = Vec<R::Output>;

    fn lower(&self, state: &mut State) -> Self::Output {
        self.as_slice().lower(state)
    }
}
impl<R: Lower> Lower for [R] {
    type Output = Vec<R::Output>;

    fn lower(&self, state: &mut State) -> Self::Output {
        self.iter().map(|rcst| rcst.lower(state)).collect()
    }
}
impl<R: Lower> Lower for Option<R> {
    type Output = Option<R::Output>;

    fn lower(&self, state: &mut State) -> Self::Output {
        self.as_ref().map(|rcst| rcst.lower(state))
    }
}
impl<R: Lower> Lower for Box<R> {
    type Output = Box<R::Output>;

    fn lower(&self, state: &mut State) -> Self::Output {
        Box::new(self.as_ref().lower(state))
    }
}
impl<R0: Lower, R1: Lower> Lower for (R0, R1) {
    type Output = (R0::Output, R1::Output);

    fn lower(&self, state: &mut State) -> Self::Output {
        (self.0.lower(state), self.1.lower(state))
    }
}
impl Lower for Rcst {
    type Output = Cst;

    fn lower(&self, state: &mut State) -> Cst {
        let start_offset = state.offset;
        let id = state.id_generator.generate();
        let kind = match &self.kind {
            CstKind::EqualsSign => {
                *state.offset += 1;
                CstKind::EqualsSign
            }
            CstKind::Comma => {
                *state.offset += 1;
                CstKind::Comma
            }
            CstKind::Dot => {
                *state.offset += 1;
                CstKind::Dot
            }
            CstKind::Colon => {
                *state.offset += 1;
                CstKind::Colon
            }
            CstKind::ColonEqualsSign => {
                *state.offset += 2;
                CstKind::ColonEqualsSign
            }
            CstKind::OpeningParenthesis => {
                *state.offset += 1;
                CstKind::OpeningParenthesis
            }
            CstKind::ClosingParenthesis => {
                *state.offset += 1;
                CstKind::ClosingParenthesis
            }
            CstKind::OpeningBracket => {
                *state.offset += 1;
                CstKind::OpeningBracket
            }
            CstKind::ClosingBracket => {
                *state.offset += 1;
                CstKind::ClosingBracket
            }
            CstKind::OpeningCurlyBrace => {
                *state.offset += 1;
                CstKind::OpeningCurlyBrace
            }
            CstKind::ClosingCurlyBrace => {
                *state.offset += 1;
                CstKind::ClosingCurlyBrace
            }
            CstKind::Arrow => {
                *state.offset += 2;
                CstKind::Arrow
            }
            CstKind::DoubleQuote => {
                *state.offset += 1;
                CstKind::DoubleQuote
            }
            CstKind::Octothorpe => {
                *state.offset += 1;
                CstKind::Octothorpe
            }
            CstKind::Let => {
                *state.offset += 3;
                CstKind::Let
            }
            CstKind::Whitespace(whitespace) => {
                *state.offset += whitespace.len();
                CstKind::Whitespace(whitespace.clone())
            }
            CstKind::Comment {
                octothorpe,
                comment,
            } => {
                let octothorpe = octothorpe.lower(state);
                *state.offset += comment.len();
                CstKind::Comment {
                    octothorpe,
                    comment: comment.clone(),
                }
            }
            CstKind::TrailingWhitespace { child, whitespace } => CstKind::TrailingWhitespace {
                child: child.lower(state),
                whitespace: whitespace.lower(state),
            },
            CstKind::Identifier(identifier) => {
                *state.offset += identifier.len();
                CstKind::Identifier(identifier.clone())
            }
            CstKind::Symbol(symbol) => {
                *state.offset += symbol.len();
                CstKind::Symbol(symbol.clone())
            }
            CstKind::Int { value, string } => {
                *state.offset += string.len();
                CstKind::Int {
                    value: *value,
                    string: string.clone(),
                }
            }
            CstKind::Text {
                opening_double_quote,
                parts,
                closing_double_quote,
            } => CstKind::Text {
                opening_double_quote: opening_double_quote.lower(state),
                parts: parts.lower(state),
                closing_double_quote: closing_double_quote.lower(state),
            },
            CstKind::TextPart(text) => {
                *state.offset += text.len();
                CstKind::TextPart(text.clone())
            }
            CstKind::TextInterpolation {
                opening_curly_brace,
                expression,
                closing_curly_brace,
            } => CstKind::TextInterpolation {
                opening_curly_brace: opening_curly_brace.lower(state),
                expression: expression.lower(state),
                closing_curly_brace: closing_curly_brace.lower(state),
            },
            CstKind::Parenthesized {
                opening_parenthesis,
                inner,
                closing_parenthesis,
            } => CstKind::Parenthesized {
                opening_parenthesis: opening_parenthesis.lower(state),
                inner: inner.lower(state),
                closing_parenthesis: closing_parenthesis.lower(state),
            },
            CstKind::Call {
                receiver,
                opening_parenthesis,
                arguments,
                closing_parenthesis,
            } => CstKind::Call {
                receiver: receiver.lower(state),
                opening_parenthesis: opening_parenthesis.lower(state),
                arguments: arguments.lower(state),
                closing_parenthesis: closing_parenthesis.lower(state),
            },
            CstKind::CallArgument { value, comma } => CstKind::CallArgument {
                value: value.lower(state),
                comma: comma.lower(state),
            },
            CstKind::Struct {
                opening_bracket,
                fields,
                closing_bracket,
            } => CstKind::Struct {
                opening_bracket: opening_bracket.lower(state),
                fields: fields.lower(state),
                closing_bracket: closing_bracket.lower(state),
            },
            CstKind::StructField {
                key,
                colon,
                value,
                comma,
            } => CstKind::StructField {
                key: key.lower(state),
                colon: colon.lower(state),
                value: value.lower(state),
                comma: comma.lower(state),
            },
            CstKind::StructAccess { struct_, dot, key } => CstKind::StructAccess {
                struct_: struct_.lower(state),
                dot: dot.lower(state),
                key: key.lower(state),
            },
            CstKind::Lambda {
                opening_curly_brace,
                parameters_and_arrow,
                body,
                closing_curly_brace,
            } => CstKind::Lambda {
                opening_curly_brace: opening_curly_brace.lower(state),
                parameters_and_arrow: parameters_and_arrow.lower(state),
                body: body.lower(state),
                closing_curly_brace: closing_curly_brace.lower(state),
            },
            CstKind::Assignment {
                let_keyword,
                name,
                kind,
                assignment_sign,
                body,
            } => CstKind::Assignment {
                let_keyword: let_keyword.lower(state),
                name: name.lower(state),
                kind: kind.lower(state),
                assignment_sign: assignment_sign.lower(state),
                body: body.lower(state),
            },
            CstKind::AssignmentValue { colon_and_type } => CstKind::AssignmentValue {
                colon_and_type: colon_and_type.lower(state),
            },
            CstKind::AssignmentFunction {
                opening_parenthesis,
                parameters,
                closing_parenthesis,
                arrow,
                return_type,
            } => CstKind::AssignmentFunction {
                opening_parenthesis: opening_parenthesis.lower(state),
                parameters: parameters.lower(state),
                closing_parenthesis: closing_parenthesis.lower(state),
                arrow: arrow.lower(state),
                return_type: return_type.lower(state),
            },
            CstKind::Parameter {
                name,
                colon_and_type,
                comma,
            } => CstKind::Parameter {
                name: name.lower(state),
                colon_and_type: colon_and_type.lower(state),
                comma: comma.lower(state),
            },
            CstKind::Error {
                unparsable_input,
                error,
            } => {
                *state.offset += unparsable_input.len();
                CstKind::Error {
                    unparsable_input: unparsable_input.clone(),
                    error: error.clone(),
                }
            }
        };

        Cst {
            data: CstData {
                id,
                span: start_offset..state.offset,
            },
            kind,
        }
    }
}
