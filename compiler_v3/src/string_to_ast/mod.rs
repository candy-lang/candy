use crate::{
    ast::{AstError, AstExpression, AstStatement},
    position::Offset,
};
use lambda::statements;
use parser::Parser;
use std::path::Path;

mod assignment;
mod expression;
mod lambda;
mod literal;
mod parser;
mod text;
mod whitespace;
mod word;

#[must_use]
pub fn string_to_ast(path: &Path, source: &str) -> Vec<AstStatement> {
    let (parser, mut statements) = statements(Parser::new(path, source));

    let rest = parser.rest().trim_end();
    if !rest.is_empty() {
        statements.push(AstStatement::Expression(AstExpression::Error(AstError {
            unparsable_input: parser.string_to(Offset(*parser.offset() + rest.len())),
            error: "The parser couldn't parse this rest.".to_string(),
        })));
    }

    statements
}
