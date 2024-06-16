use crate::{
    ast::{AstError, AstResult, AstString},
    position::Offset,
};
use extension_trait::extension_trait;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Parser<'s> {
    pub file: &'s Path,
    source: &'s str,
    offset: Offset,
}
impl<'s> Parser<'s> {
    #[must_use]
    pub const fn new(file: &'s Path, source: &'s str) -> Self {
        Self {
            file,
            source,
            offset: Offset(0),
        }
    }

    #[must_use]
    pub const fn offset(self) -> Offset {
        self.offset
    }

    #[must_use]
    pub fn rest(self) -> &'s str {
        &self.source[*self.offset..]
    }
    #[must_use]
    pub fn next_char(self) -> Option<char> {
        self.rest().chars().next()
    }
    #[must_use]
    pub fn is_at_end(self) -> bool {
        self.offset == Offset(self.source.len())
    }

    #[must_use]
    pub fn string(self, start_offset: Offset) -> AstString {
        assert!(start_offset <= self.offset);
        AstString {
            string: self.source[*start_offset..*self.offset].into(),
            file: self.file.to_path_buf(),
            span: start_offset..self.offset,
        }
    }
    #[must_use]
    pub fn string_to(self, end_offset: Offset) -> AstString {
        assert!(self.offset <= end_offset);
        AstString {
            string: self.source[*self.offset..*end_offset].into(),
            file: self.file.to_path_buf(),
            span: self.offset..end_offset,
        }
    }

    #[must_use]
    pub fn error_at_current_offset(self, message: impl Into<String>) -> AstError {
        AstError {
            unparsable_input: AstString {
                string: "".into(),
                file: self.file.to_path_buf(),
                span: self.offset..self.offset,
            },
            error: message.into(),
        }
    }

    #[must_use]
    pub fn consume_literal(mut self, literal: &'static str) -> Option<Parser<'s>> {
        if self.rest().starts_with(literal) {
            self.offset = Offset(*self.offset + literal.len());
            Some(self)
        } else {
            None
        }
    }
    #[must_use]
    pub fn consume_while(
        mut self,
        mut predicate: impl FnMut(char) -> bool,
    ) -> Option<(Parser<'s>, AstString)> {
        let start_offset = self.offset();
        while let Some((new_parser, c)) = self.consume_char() {
            if !predicate(c) {
                break;
            }
            self = new_parser;
        }
        if start_offset == self.offset {
            None
        } else {
            Some((self, self.string(start_offset)))
        }
    }
    #[must_use]
    fn consume_char(self) -> Option<(Parser<'s>, char)> {
        self.next_char().map(|c| {
            (
                Parser {
                    file: self.file,
                    source: self.source,
                    offset: Offset(*self.offset + c.len_utf8()),
                },
                c,
            )
        })
    }
}

#[extension_trait]
pub impl<'s> ParserUnwrapOrAstError<'s> for Option<Parser<'s>> {
    #[must_use]
    fn unwrap_or_ast_error(
        self,
        original_parser: Parser<'s>,
        error_message: impl Into<String>,
    ) -> (Parser<'s>, Option<AstError>) {
        if let Some(parser) = self {
            (parser, None)
        } else {
            (
                original_parser,
                Some(original_parser.error_at_current_offset(error_message)),
            )
        }
    }
}
#[extension_trait]
pub impl<T, 's> ParserWithValueUnwrapOrAstError<T, 's> for Option<(Parser<'s>, T)> {
    #[must_use]
    fn unwrap_or_ast_error(
        self,
        original_parser: Parser<'s>,
        error_message: impl Into<String>,
    ) -> (Parser<'s>, AstResult<T>) {
        if let Some((parser, value)) = self {
            (parser, AstResult::ok(value))
        } else {
            (
                original_parser,
                AstResult::error(None, original_parser.error_at_current_offset(error_message)),
            )
        }
    }
}
#[extension_trait]
pub impl<T, 's> ParserWithResultUnwrapOrAstError<T, 's> for Option<(Parser<'s>, AstResult<T>)> {
    #[must_use]
    fn unwrap_or_ast_error(
        self,
        original_parser: Parser<'s>,
        error_message: impl Into<String>,
    ) -> (Parser<'s>, AstResult<T>) {
        if let Some((parser, value)) = self {
            (parser, value)
        } else {
            (
                original_parser,
                AstResult::error(None, original_parser.error_at_current_offset(error_message)),
            )
        }
    }
}
