use std::ops::Range;

use im::HashMap;
use lsp_types::{Position, SemanticToken, SemanticTokensLegend};

use crate::{
    compiler::{
        cst::{Cst, CstKind},
        string_to_cst::StringToCst,
    },
    language_server::utils::Utf8ByteOffsetToLsp,
};
use lazy_static::lazy_static;
use lsp_types;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

lazy_static! {
    pub static ref LEGEND: SemanticTokensLegend = SemanticTokensLegend {
        token_types: SemanticTokenType::iter().map(|it| it.to_lsp()).collect(),
        token_modifiers: vec![
            lsp_types::SemanticTokenModifier::DEFINITION,
            lsp_types::SemanticTokenModifier::READONLY,
        ],
    };
}

#[derive(Debug, EnumIter, Hash, PartialEq, Eq, Clone, Copy)]
enum SemanticTokenType {
    Parameter,
    Assignment,
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
    fn to_lsp(&self) -> lsp_types::SemanticTokenType {
        match self {
            SemanticTokenType::Parameter => lsp_types::SemanticTokenType::PARAMETER,
            SemanticTokenType::Assignment => lsp_types::SemanticTokenType::VARIABLE,
            SemanticTokenType::Symbol => lsp_types::SemanticTokenType::ENUM_MEMBER,
            SemanticTokenType::Function => lsp_types::SemanticTokenType::FUNCTION,
            SemanticTokenType::Comment => lsp_types::SemanticTokenType::COMMENT,
            SemanticTokenType::String => lsp_types::SemanticTokenType::STRING,
            SemanticTokenType::Number => lsp_types::SemanticTokenType::NUMBER,
            SemanticTokenType::Operator => lsp_types::SemanticTokenType::OPERATOR,
        }
    }
}

pub fn compute_semantic_tokens(source: &str) -> Vec<SemanticToken> {
    let cst = source.parse_cst();
    let mut context = Context::new(source);
    context.visit_csts(&cst, None);
    context.tokens
}

struct Context<'a> {
    source: &'a str,
    tokens: Vec<SemanticToken>,
    cursor: Position,
}
impl<'a> Context<'a> {
    fn new(source: &str) -> Context {
        Context {
            source,
            tokens: vec![],
            cursor: Position::new(0, 0),
        }
    }
    fn add_token(&mut self, range: Range<usize>, type_: SemanticTokenType) {
        // Reduce the token to multiple single-line tokens.
        let mut cursor = range.start;
        let mut line_start = cursor;
        let mut source = &self.source[range.start..];
        while cursor < range.end {
            let next_char = source.chars().next().unwrap_or('\0');
            match next_char {
                '\n' => {
                    self.add_single_line_token(line_start..cursor, type_);
                    line_start = cursor + 1;
                }
                c => cursor += c.len_utf8(),
            }
            source = &source[next_char.len_utf8()..];
        }
        self.add_single_line_token(line_start..range.end, type_);
    }
    fn add_single_line_token(&mut self, range: Range<usize>, type_: SemanticTokenType) {
        let start = range.start.utf8_byte_offset_to_lsp(self.source);
        assert!(
            start >= self.cursor,
            "Tokens must be added with increasing positions. The cursor was as {:?}, but the new token starts at {:?}.",
            self.cursor,
            start,
        );

        let definition_modifier = if type_ == SemanticTokenType::Assignment {
            0b1
        } else {
            0b0
        };
        let readonly_modifier = 0b10;
        self.tokens.push(SemanticToken {
            delta_line: start.line as u32 - self.cursor.line as u32,
            delta_start: if start.line == self.cursor.line {
                start.character - self.cursor.character
            } else {
                start.character
            },
            length: range.len() as u32,
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
            CstKind::EqualsSign { .. } => self.add_token(cst.span(), SemanticTokenType::Operator),
            CstKind::OpeningParenthesis { .. } => {}
            CstKind::ClosingParenthesis { .. } => {}
            CstKind::OpeningCurlyBrace { .. } => {}
            CstKind::ClosingCurlyBrace { .. } => {}
            CstKind::Arrow { .. } => {}
            CstKind::Int { .. } => self.add_token(cst.span(), SemanticTokenType::Number),
            CstKind::Text { .. } => self.add_token(cst.span(), SemanticTokenType::String),
            CstKind::Identifier { .. } => match token_type_for_identifier {
                Some(type_) => self.add_token(cst.span(), type_),
                None => {
                    panic!("We encountered and identifier, but don't know which type to assign.");
                }
            },
            CstKind::Symbol { .. } => self.add_token(cst.span(), SemanticTokenType::Symbol),
            CstKind::LeadingWhitespace { child, .. } => {
                self.visit_cst(child, token_type_for_identifier)
            }
            CstKind::LeadingComment { value, child } => {
                let span = cst.span();
                self.add_token(
                    span.start..span.start + value.len(),
                    SemanticTokenType::Comment,
                );
                self.visit_cst(child, token_type_for_identifier);
            }
            CstKind::TrailingWhitespace { child, .. } => {
                self.visit_cst(child, token_type_for_identifier);
            }
            CstKind::TrailingComment { child, value } => {
                let span = cst.span();
                self.visit_cst(child, token_type_for_identifier);
                self.add_token(span.end - value.len()..span.end, SemanticTokenType::Comment);
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
            CstKind::Lambda {
                opening_curly_brace,
                parameters_and_arrow,
                body,
                closing_curly_brace,
            } => {
                self.visit_cst(opening_curly_brace, None);
                if let Some((parameters, _)) = parameters_and_arrow {
                    self.visit_csts(parameters, Some(SemanticTokenType::Parameter));
                }
                self.visit_csts(body, None);
                self.visit_cst(closing_curly_brace, None);
            }
            CstKind::Call { name, arguments } => {
                self.visit_cst(name, Some(SemanticTokenType::Function));
                self.visit_csts(arguments, None);
            }
            CstKind::Assignment {
                name,
                parameters,
                equals_sign,
                body,
            } => {
                self.visit_cst(name, Some(SemanticTokenType::Assignment));
                self.visit_csts(&parameters[..], Some(SemanticTokenType::Parameter));
                self.visit_cst(equals_sign, None);
                self.visit_csts(body, None);
            }
            CstKind::Error { .. } => {}
        }
    }
}
