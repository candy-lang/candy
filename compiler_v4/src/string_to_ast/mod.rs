use crate::ast::AstDeclaration;

use parser::Parser;
use std::path::Path;
use tracing::error;

mod declarations;
mod expression;
mod literal;
mod parser;
mod text;
mod type_;
mod whitespace;
mod word;

#[must_use]
pub fn string_to_ast(path: &Path, source: &str) -> Vec<AstDeclaration> {
    let (parser, declarations) = declarations::declarations(Parser::new(path, source));

    let rest = parser.rest().trim_end();
    if !rest.is_empty() {
        // TODO: report error for unparsed rest
        error!("The parser couldn't parse this rest: {rest:?}");
        // declarations.push(AstStatement::Expression(AstExpression::Error(AstError {
        //     unparsable_input: parser.string_to(Offset(*parser.offset() + rest.len())),
        //     error: "The parser couldn't parse this rest.".to_string(),
        // })));
    }

    declarations
}
