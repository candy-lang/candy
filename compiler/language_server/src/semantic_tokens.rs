use std::ops::Range;

use candy_frontend::position::Offset;
use lazy_static::lazy_static;
use lsp_types::{Position, SemanticToken};
use rustc_hash::FxHashMap;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::utils::range_to_lsp_range_raw;

#[derive(Debug, EnumIter, Hash, PartialEq, Eq, Clone, Copy)]
pub enum SemanticTokenType {
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
    static ref TOKEN_TYPE_MAPPING: FxHashMap<SemanticTokenType, u32> = SemanticTokenType::iter()
        .enumerate()
        .map(|(index, it)| (it, index as u32))
        .collect();
}

impl SemanticTokenType {
    pub fn as_lsp(&self) -> lsp_types::SemanticTokenType {
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

pub struct SemanticTokensBuilder<'a> {
    text: &'a str,
    line_start_offsets: &'a [Offset],
    tokens: Vec<SemanticToken>,
    cursor: Position,
}
impl<'a> SemanticTokensBuilder<'a> {
    pub fn new<S, L>(text: S, line_start_offsets: L) -> Self
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

    pub fn add(&mut self, range: Range<Offset>, type_: SemanticTokenType) {
        // Reduce the token to multiple single-line tokens.
        let mut range = range_to_lsp_range_raw(self.text, self.line_start_offsets, range);

        if range.start.line != range.end.line {
            while range.start.line != range.end.line {
                assert!(range.start.line < range.end.line);

                let line_length = *self.line_start_offsets[(range.start.line as usize) + 1]
                    - *self.line_start_offsets[range.start.line as usize]
                    - 1;
                self.add_single_line(range.start, line_length as u32, type_);
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
        );
    }
    fn add_single_line(&mut self, start: Position, length: u32, type_: SemanticTokenType) {
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

    pub fn finish(self) -> Vec<SemanticToken> {
        self.tokens
    }
}
