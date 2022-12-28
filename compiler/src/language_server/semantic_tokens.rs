use super::utils::LspPositionConversion;
use crate::{
    compiler::{
        cst::{Cst, CstKind},
        rcst_to_cst::RcstToCst,
    },
    language_server::utils::TupleToPosition,
    module::Module,
};
use lazy_static::lazy_static;
use lsp_types::{self, Position, SemanticToken, SemanticTokensLegend};
use std::{collections::HashMap, ops::Range};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[salsa::query_group(SemanticTokenDbStorage)]
pub trait SemanticTokenDb: LspPositionConversion + RcstToCst {
    fn semantic_tokens(&self, module: Module) -> Vec<SemanticToken>;
}

fn semantic_tokens(db: &dyn SemanticTokenDb, module: Module) -> Vec<SemanticToken> {
    let mut context = Context::new(db, module.clone());
    let cst = db.cst(module).unwrap();
    context.visit_csts(&cst, None);
    context.tokens
}

lazy_static! {
    pub static ref LEGEND: SemanticTokensLegend = SemanticTokensLegend {
        token_types: SemanticTokenType::iter().map(|it| it.as_lsp()).collect(),
        token_modifiers: vec![
            lsp_types::SemanticTokenModifier::DEFINITION,
            lsp_types::SemanticTokenModifier::READONLY,
        ],
    };
}

#[derive(Debug, EnumIter, Hash, PartialEq, Eq, Clone, Copy)]
enum SemanticTokenType {
    Parameter,
    Variable,
    Symbol,
    Function,
    Comment,
    String,
    Number,
    Operator,
}
lazy_static! {
    static ref TOKEN_TYPE_MAPPING: HashMap<SemanticTokenType, u32> = SemanticTokenType::iter()
        .enumerate()
        .map(|(index, it)| (it, index as u32))
        .collect();
}

impl SemanticTokenType {
    fn as_lsp(&self) -> lsp_types::SemanticTokenType {
        match self {
            SemanticTokenType::Parameter => lsp_types::SemanticTokenType::PARAMETER,
            SemanticTokenType::Variable => lsp_types::SemanticTokenType::VARIABLE,
            SemanticTokenType::Symbol => lsp_types::SemanticTokenType::ENUM_MEMBER,
            SemanticTokenType::Function => lsp_types::SemanticTokenType::FUNCTION,
            SemanticTokenType::Comment => lsp_types::SemanticTokenType::COMMENT,
            SemanticTokenType::String => lsp_types::SemanticTokenType::STRING,
            SemanticTokenType::Number => lsp_types::SemanticTokenType::NUMBER,
            SemanticTokenType::Operator => lsp_types::SemanticTokenType::OPERATOR,
        }
    }
}

struct Context<'a> {
    db: &'a dyn SemanticTokenDb,
    module: Module,
    tokens: Vec<SemanticToken>,
    cursor: Position,
}
impl<'a> Context<'a> {
    fn new(db: &'a dyn SemanticTokenDb, module: Module) -> Context {
        Context {
            db,
            module,
            tokens: vec![],
            cursor: Position::new(0, 0),
        }
    }
    fn add_token(&mut self, range: Range<usize>, type_: SemanticTokenType) {
        // Reduce the token to multiple single-line tokens.

        let mut start = self
            .db
            .offset_to_lsp(self.module.clone(), range.start)
            .to_position();
        let end = self
            .db
            .offset_to_lsp(self.module.clone(), range.end)
            .to_position();

        if start.line != end.line {
            let line_start_offsets = self.db.line_start_utf8_byte_offsets(self.module.clone());
            while start.line != end.line {
                assert!(start.line < end.line);

                let line_length = line_start_offsets[(start.line as usize) + 1]
                    - line_start_offsets[start.line as usize]
                    - 1;
                self.add_single_line_token(start, line_length as u32, type_);
                start = Position {
                    line: start.line + 1,
                    character: 0,
                };
            }
        }
        assert_eq!(start.line, end.line);

        self.add_single_line_token(start, end.character - start.character, type_);
    }
    fn add_single_line_token(&mut self, start: Position, length: u32, type_: SemanticTokenType) {
        assert!(
            start >= self.cursor,
            "Tokens must be added with increasing positions. The cursor was as {:?}, but the new token starts at {start:?}.",
            self.cursor,
        );

        let definition_modifier = (type_ == SemanticTokenType::Variable) as u32;
        let readonly_modifier = 0b10;
        self.tokens.push(SemanticToken {
            delta_line: start.line - self.cursor.line,
            delta_start: if start.line == self.cursor.line {
                start.character - self.cursor.character
            } else {
                start.character
            },
            length,
            token_type: TOKEN_TYPE_MAPPING[&type_],
            token_modifiers_bitset: definition_modifier | readonly_modifier,
        });
        self.cursor.line = start.line;
        self.cursor.character = start.character;
    }
    fn visit_csts(&mut self, csts: &[Cst], token_type_for_identifier: Option<SemanticTokenType>) {
        for cst in csts {
            self.visit_cst(cst, token_type_for_identifier)
        }
    }
    fn visit_cst(&mut self, cst: &Cst, token_type_for_identifier: Option<SemanticTokenType>) {
        match &cst.kind {
            CstKind::EqualsSign => self.add_token(cst.span.clone(), SemanticTokenType::Operator),
            CstKind::Comma
            | CstKind::Dot
            | CstKind::Colon
            | CstKind::ColonEqualsSign
            | CstKind::Bar
            | CstKind::OpeningParenthesis
            | CstKind::ClosingParenthesis
            | CstKind::OpeningBracket
            | CstKind::ClosingBracket
            | CstKind::OpeningCurlyBrace
            | CstKind::ClosingCurlyBrace => {}
            CstKind::Arrow => self.add_token(cst.span.clone(), SemanticTokenType::Operator),
            CstKind::SingleQuote => {} // handled by parent
            CstKind::DoubleQuote => {} // handled by parent
            CstKind::Octothorpe => {}  // handled by parent
            CstKind::Whitespace(_) | CstKind::Newline(_) => {}
            CstKind::Comment { octothorpe, .. } => {
                self.visit_cst(octothorpe, None);
                self.add_token(cst.span.clone(), SemanticTokenType::Comment);
            }
            CstKind::TrailingWhitespace { child, whitespace } => {
                self.visit_cst(child, token_type_for_identifier);
                self.visit_csts(whitespace, token_type_for_identifier);
            }
            CstKind::Identifier { .. } => self.add_token(
                cst.span.clone(),
                token_type_for_identifier.unwrap_or(SemanticTokenType::Variable),
            ),
            CstKind::Symbol { .. } => self.add_token(cst.span.clone(), SemanticTokenType::Symbol),
            CstKind::Int { .. } => self.add_token(cst.span.clone(), SemanticTokenType::Number),
            CstKind::OpeningText {
                opening_single_quotes,
                opening_double_quote,
            } => {
                for opening_single_quote in opening_single_quotes {
                    self.add_token(opening_single_quote.span.clone(), SemanticTokenType::String);
                }
                self.add_token(opening_double_quote.span.clone(), SemanticTokenType::String);
            }
            CstKind::ClosingText {
                closing_double_quote,
                closing_single_quotes,
            } => {
                self.add_token(closing_double_quote.span.clone(), SemanticTokenType::String);
                for closing_single_quote in closing_single_quotes {
                    self.add_token(closing_single_quote.span.clone(), SemanticTokenType::String);
                }
            }
            CstKind::Text {
                opening,
                parts,
                closing,
            } => {
                self.visit_cst(opening, None);
                for part in parts {
                    self.visit_cst(part, None);
                }
                self.visit_cst(closing, None);
            }
            CstKind::TextPart(_) => self.add_token(cst.span.clone(), SemanticTokenType::String),
            CstKind::TextInterpolation {
                opening_curly_braces,
                expression,
                closing_curly_braces,
            } => {
                for opening_curly_brace in opening_curly_braces {
                    self.visit_cst(opening_curly_brace, None);
                }
                self.visit_cst(expression, None);
                for closing_curly_brace in closing_curly_braces {
                    self.visit_cst(closing_curly_brace, None);
                }
            }
            CstKind::Pipe {
                receiver,
                bar,
                call,
            } => {
                self.visit_cst(receiver, None);
                self.visit_cst(bar, None);
                self.visit_cst(call, None);
            }
            CstKind::Parenthesized {
                opening_parenthesis,
                inner,
                closing_parenthesis,
            } => {
                self.visit_cst(opening_parenthesis, None);
                self.visit_cst(inner, None);
                self.visit_cst(closing_parenthesis, None);
            }
            CstKind::Call {
                receiver,
                arguments,
            } => {
                self.visit_cst(receiver, Some(SemanticTokenType::Function));
                self.visit_csts(arguments, None);
            }
            CstKind::List {
                opening_parenthesis,
                items,
                closing_parenthesis,
            } => {
                self.visit_cst(opening_parenthesis, None);
                self.visit_csts(items, token_type_for_identifier);
                self.visit_cst(closing_parenthesis, None);
            }
            CstKind::ListItem { value, comma } => {
                self.visit_cst(value, token_type_for_identifier);
                if let Some(comma) = comma {
                    self.visit_cst(comma, None);
                }
            }
            CstKind::Struct {
                opening_bracket,
                fields,
                closing_bracket,
            } => {
                self.visit_cst(opening_bracket, None);
                self.visit_csts(fields, token_type_for_identifier);
                self.visit_cst(closing_bracket, None);
            }
            CstKind::StructField {
                key_and_colon,
                value,
                comma,
            } => {
                if let Some(box (key, colon)) = key_and_colon {
                    self.visit_cst(key, token_type_for_identifier);
                    self.visit_cst(colon, None);
                }
                self.visit_cst(value, token_type_for_identifier);
                if let Some(comma) = comma {
                    self.visit_cst(comma, None);
                }
            }
            CstKind::StructAccess { struct_, dot, key } => {
                self.visit_cst(struct_, None);
                self.visit_cst(dot, None);
                self.visit_cst(
                    key,
                    Some(token_type_for_identifier.unwrap_or(SemanticTokenType::Symbol)),
                );
            }
            CstKind::Lambda {
                opening_curly_brace,
                parameters_and_arrow,
                body,
                closing_curly_brace,
            } => {
                self.visit_cst(opening_curly_brace, None);
                if let Some((parameters, arrow)) = parameters_and_arrow {
                    self.visit_csts(parameters, Some(SemanticTokenType::Parameter));
                    self.visit_cst(arrow, None);
                }
                self.visit_csts(body, None);
                self.visit_cst(closing_curly_brace, None);
            }
            CstKind::Assignment {
                name_or_pattern,
                parameters,
                assignment_sign,
                body,
            } => {
                self.visit_cst(name_or_pattern, Some(SemanticTokenType::Variable));
                self.visit_csts(&parameters[..], Some(SemanticTokenType::Parameter));
                self.visit_cst(assignment_sign, None);
                self.visit_csts(body, None);
            }
            CstKind::Error { .. } => {}
        }
    }
}
