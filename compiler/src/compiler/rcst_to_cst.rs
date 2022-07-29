use super::{
    cst::{self, Cst, CstKind},
    rcst::Rcst,
    string_to_rcst::{InvalidInputError, StringToRcst},
};
use crate::input::Input;
use std::sync::Arc;

#[salsa::query_group(RcstToCstStorage)]
pub trait RcstToCst: StringToRcst {
    fn cst(&self, input: Input) -> Result<Arc<Vec<Cst>>, InvalidInputError>;
}

fn cst(db: &dyn RcstToCst, input: Input) -> Result<Arc<Vec<Cst>>, InvalidInputError> {
    let rcsts = db.rcst(input)?;

    let mut state = State {
        offset: 0,
        next_id: 0,
    };
    let csts = (*rcsts).clone().to_csts(&mut state);

    Ok(Arc::new(csts))
}

struct State {
    offset: usize,
    next_id: usize,
}

trait RcstToCstExt {
    fn to_cst(self, state: &mut State) -> Cst;
    fn to_cst_kind(self, state: &mut State) -> CstKind;
}
impl RcstToCstExt for Rcst {
    fn to_cst(self, state: &mut State) -> Cst {
        let id = state.next_id;
        state.next_id += 1;
        let start_offset = state.offset;
        let kind = self.to_cst_kind(state);
        let end_offset = state.offset;
        Cst {
            id: cst::Id(id),
            span: start_offset..end_offset,
            kind,
        }
    }
    fn to_cst_kind(self, state: &mut State) -> CstKind {
        match self {
            Rcst::EqualsSign => {
                state.offset += 1;
                CstKind::EqualsSign
            }
            Rcst::Comma => {
                state.offset += 1;
                CstKind::Comma
            }
            Rcst::Dot => {
                state.offset += 1;
                CstKind::Dot
            }
            Rcst::Colon => {
                state.offset += 1;
                CstKind::Colon
            }
            Rcst::ColonEqualsSign => {
                state.offset += 2;
                CstKind::ColonEqualsSign
            }
            Rcst::OpeningParenthesis => {
                state.offset += 1;
                CstKind::OpeningParenthesis
            }
            Rcst::ClosingParenthesis => {
                state.offset += 1;
                CstKind::ClosingParenthesis
            }
            Rcst::OpeningBracket => {
                state.offset += 1;
                CstKind::OpeningBracket
            }
            Rcst::ClosingBracket => {
                state.offset += 1;
                CstKind::ClosingBracket
            }
            Rcst::OpeningCurlyBrace => {
                state.offset += 1;
                CstKind::OpeningCurlyBrace
            }
            Rcst::ClosingCurlyBrace => {
                state.offset += 1;
                CstKind::ClosingCurlyBrace
            }
            Rcst::Arrow => {
                state.offset += 2;
                CstKind::Arrow
            }
            Rcst::DoubleQuote => {
                state.offset += 1;
                CstKind::DoubleQuote
            }
            Rcst::Octothorpe => {
                state.offset += 1;
                CstKind::Octothorpe
            }
            Rcst::Whitespace(whitespace) => {
                state.offset += whitespace.len();
                CstKind::Whitespace(whitespace)
            }
            Rcst::Newline(newline) => {
                state.offset += newline.len();
                CstKind::Newline(newline)
            }
            Rcst::Comment {
                octothorpe,
                comment,
            } => {
                let octothorpe = octothorpe.to_cst(state);
                state.offset += comment.len();
                CstKind::Comment {
                    octothorpe: Box::new(octothorpe),
                    comment,
                }
            }
            Rcst::TrailingWhitespace { child, whitespace } => CstKind::TrailingWhitespace {
                child: Box::new(child.to_cst(state)),
                whitespace: whitespace.to_csts(state),
            },
            Rcst::Identifier(identifier) => {
                state.offset += identifier.len();
                CstKind::Identifier(identifier)
            }
            Rcst::Symbol(symbol) => {
                state.offset += symbol.len();
                CstKind::Symbol(symbol)
            }
            Rcst::Int { value, string } => {
                state.offset += string.len();
                CstKind::Int { value, string }
            }
            Rcst::Text {
                opening_quote,
                parts,
                closing_quote,
            } => CstKind::Text {
                opening_quote: Box::new(opening_quote.to_cst(state)),
                parts: parts.to_csts(state),
                closing_quote: Box::new(closing_quote.to_cst(state)),
            },
            Rcst::TextPart(text) => {
                state.offset += text.len();
                CstKind::TextPart(text)
            }
            Rcst::Call { name, arguments } => CstKind::Call {
                name: Box::new(name.to_cst(state)),
                arguments: arguments.to_csts(state),
            },
            Rcst::Struct {
                opening_bracket,
                fields,
                closing_bracket,
            } => CstKind::Struct {
                opening_bracket: Box::new(opening_bracket.to_cst(state)),
                fields: fields.to_csts(state),
                closing_bracket: Box::new(closing_bracket.to_cst(state)),
            },
            Rcst::StructField {
                key_and_colon,
                value,
                comma,
            } => CstKind::StructField {
                key_and_colon: key_and_colon
                    .map(|box (key, colon)| Box::new((key.to_cst(state), colon.to_cst(state)))),
                value: Box::new(value.to_cst(state)),
                comma: comma.map(|comma| Box::new(comma.to_cst(state))),
            },
            Rcst::StructAccess { struct_, dot, key } => CstKind::StructAccess {
                struct_: Box::new(struct_.to_cst(state)),
                dot: Box::new(dot.to_cst(state)),
                key: Box::new(key.to_cst(state)),
            },
            Rcst::Parenthesized {
                opening_parenthesis,
                inner,
                closing_parenthesis,
            } => CstKind::Parenthesized {
                opening_parenthesis: Box::new(opening_parenthesis.to_cst(state)),
                inner: Box::new(inner.to_cst(state)),
                closing_parenthesis: Box::new(closing_parenthesis.to_cst(state)),
            },
            Rcst::Lambda {
                opening_curly_brace,
                parameters_and_arrow,
                body,
                closing_curly_brace,
            } => CstKind::Lambda {
                opening_curly_brace: Box::new(opening_curly_brace.to_cst(state)),
                parameters_and_arrow: parameters_and_arrow.map(|(parameters, arrow)| {
                    (parameters.to_csts(state), Box::new(arrow.to_cst(state)))
                }),
                body: body.to_csts(state),
                closing_curly_brace: Box::new(closing_curly_brace.to_cst(state)),
            },
            Rcst::Assignment {
                name,
                parameters,
                assignment_sign,
                body,
            } => CstKind::Assignment {
                name: Box::new(name.to_cst(state)),
                parameters: parameters.to_csts(state),
                assignment_sign: Box::new(assignment_sign.to_cst(state)),
                body: body.to_csts(state),
            },
            Rcst::Error {
                unparsable_input,
                error,
            } => {
                state.offset += unparsable_input.len();
                CstKind::Error {
                    unparsable_input,
                    error,
                }
            }
        }
    }
}

trait RcstsToCstsExt {
    fn to_csts(self, state: &mut State) -> Vec<Cst>;
}
impl RcstsToCstsExt for Vec<Rcst> {
    fn to_csts(self, state: &mut State) -> Vec<Cst> {
        let mut csts = vec![];
        for rcst in self {
            csts.push(rcst.to_cst(state));
        }
        csts
    }
}
