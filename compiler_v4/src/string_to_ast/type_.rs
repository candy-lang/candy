use super::{parser::Parser, word::raw_identifier};
use crate::ast::AstType;
use tracing::instrument;

#[instrument(level = "trace")]
pub fn type_(parser: Parser) -> Option<(Parser, AstType)> {
    raw_identifier(parser)
        .map(|(parser, name)| (parser, AstType::Named(crate::ast::AstNamedType { name })))
}
