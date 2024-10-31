use crate::ast::Ast;

use list::list_of;
use parser::Parser;
use std::path::Path;
use tracing::error;

mod declarations;
mod expression;
mod list;
mod literal;
mod parser;
mod text;
mod type_;
mod whitespace;
mod word;

#[must_use]
pub fn string_to_ast(path: &Path, source: &str) -> Ast {
    let (parser, declarations) = list_of(Parser::new(path, source), declarations::declaration);

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
