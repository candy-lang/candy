use std::ops::Range;

use candy_frontend::position::Offset;
use enumset::{EnumSet, EnumSetType};
use lazy_static::lazy_static;
use lsp_types::{Position, SemanticToken, SemanticTokensLegend};
use rustc_hash::FxHashMap;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::utils::range_to_lsp_range_raw;

#[derive(Debug, EnumIter, Hash, PartialEq, Eq, Clone, Copy)]
pub enum SemanticTokenType {
    Module,
    Parameter,
    Variable,
    Symbol,
    Function,
    Comment,
    Text,
    Int,
    Operator,
    Address,
    Constant,
}
lazy_static! {
    static ref TOKEN_TYPE_MAPPING: FxHashMap<SemanticTokenType, u32> = SemanticTokenType::iter()
        .enumerate()
        .map(|(index, it)| (it, index.try_into().unwrap()))
        .collect();
}

impl SemanticTokenType {
    pub const fn as_lsp(self) -> lsp_types::SemanticTokenType {
        match self {
            Self::Module => lsp_types::SemanticTokenType::NAMESPACE,
            Self::Parameter => lsp_types::SemanticTokenType::PARAMETER,
            Self::Variable => lsp_types::SemanticTokenType::VARIABLE,
            Self::Symbol => lsp_types::SemanticTokenType::ENUM_MEMBER,
            Self::Function => lsp_types::SemanticTokenType::FUNCTION,
            Self::Comment => lsp_types::SemanticTokenType::COMMENT,
            Self::Text => lsp_types::SemanticTokenType::STRING,
            Self::Int => lsp_types::SemanticTokenType::NUMBER,
            Self::Operator => lsp_types::SemanticTokenType::OPERATOR,
            Self::Address => lsp_types::SemanticTokenType::EVENT,
            Self::Constant => lsp_types::SemanticTokenType::VARIABLE,
        }
    }
}

#[derive(Debug, EnumIter, EnumSetType)]
#[enumset(repr = "u32")]
pub enum SemanticTokenModifier {
    Definition,
    Readonly,
    Builtin,
}
lazy_static! {
    pub static ref LEGEND: SemanticTokensLegend = SemanticTokensLegend {
        token_types: SemanticTokenType::iter()
            .map(SemanticTokenType::as_lsp)
            .collect(),
        token_modifiers: SemanticTokenModifier::iter()
            .map(SemanticTokenModifier::as_lsp)
            .collect(),
    };
}
impl SemanticTokenModifier {
    pub const fn as_lsp(self) -> lsp_types::SemanticTokenModifier {
        match self {
            Self::Definition => lsp_types::SemanticTokenModifier::DEFINITION,
            Self::Readonly => lsp_types::SemanticTokenModifier::READONLY,
            Self::Builtin => lsp_types::SemanticTokenModifier::DEFAULT_LIBRARY,
        }
    }
}

pub struct SemanticTokensBuilder<'a> {
    text: &'a str,
    line_start_offsets: &'a [Offset],
    tokens: Vec<SemanticToken>,
    cursor: Position,
}
impl<'a> SemanticTokensBuilder<'a> {
    pub fn new<S, L>(text: &'a S, line_start_offsets: &'a L) -> Self
    where
        S: AsRef<str>,
        L: AsRef<[Offset]>,
    {
        Self {
            text: text.as_ref(),
            line_start_offsets: line_start_offsets.as_ref(),
            tokens: Vec::new(),
            cursor: Position::new(0, 0),
        }
    }

    pub fn add(
        &mut self,
        range: Range<Offset>,
        type_: SemanticTokenType,
        modifiers: EnumSet<SemanticTokenModifier>,
    ) {
        // Reduce the token to multiple single-line tokens.
        let mut range = range_to_lsp_range_raw(self.text, self.line_start_offsets, &range);

        if range.start.line != range.end.line {
            while range.start.line != range.end.line {
                assert!(range.start.line < range.end.line);

                let line_length = *self.line_start_offsets[(range.start.line as usize) + 1]
                    - *self.line_start_offsets[range.start.line as usize]
                    - 1;
                self.add_single_line(
                    range.start,
                    line_length.try_into().unwrap(),
                    type_,
                    modifiers,
                );
                range.start = Position {
                    line: range.start.line + 1,
                    character: 0,
                };
            }
        }
        assert_eq!(range.start.line, range.end.line);

        self.add_single_line(
            range.start,
            range.end.character - range.start.character,
            type_,
            modifiers,
        );
    }
    fn add_single_line(
        &mut self,
        start: Position,
        length: u32,
        type_: SemanticTokenType,
        mut modifiers: EnumSet<SemanticTokenModifier>,
    ) {
        assert!(
            start >= self.cursor,
            "Tokens must be added with increasing positions. The cursor was as {:?}, but the new token starts at {start:?}.",
            self.cursor,
        );

        if type_ == SemanticTokenType::Variable {
            modifiers.insert(SemanticTokenModifier::Definition);
        }
        modifiers.insert(SemanticTokenModifier::Readonly);

        self.tokens.push(SemanticToken {
            delta_line: start.line - self.cursor.line,
            delta_start: if start.line == self.cursor.line {
                start.character - self.cursor.character
            } else {
                start.character
            },
            length,
            token_type: TOKEN_TYPE_MAPPING[&type_],
            token_modifiers_bitset: modifiers.as_repr(),
        });
        self.cursor.line = start.line;
        self.cursor.character = start.character;
    }

    pub fn finish(self) -> Vec<SemanticToken> {
        self.tokens
    }
}
