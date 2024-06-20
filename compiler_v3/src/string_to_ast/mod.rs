use crate::ast::AstAssignment;
use assignment::assignment;

use parser::Parser;
use std::path::Path;
use tracing::{error, instrument};
use whitespace::whitespace;

mod assignment;
mod expression;
mod lambda;
mod literal;
mod parser;
mod text;
mod whitespace;
mod word;

#[must_use]
pub fn string_to_ast(path: &Path, source: &str) -> Vec<AstAssignment> {
    let (parser, assignments) = assignments(Parser::new(path, source));

    let rest = parser.rest().trim_end();
    if !rest.is_empty() {
        // TODO: report error for unparsed rest
        error!("The parser couldn't parse this rest: {rest:?}");
        // assignments.push(AstStatement::Expression(AstExpression::Error(AstError {
        //     unparsable_input: parser.string_to(Offset(*parser.offset() + rest.len())),
        //     error: "The parser couldn't parse this rest.".to_string(),
        // })));
    }

    assignments
}

#[instrument(level = "trace")]
fn assignments(mut parser: Parser) -> (Parser, Vec<AstAssignment>) {
    let mut assignments = vec![];
    while !parser.is_at_end() {
        let mut made_progress = false;

        if let Some((new_parser, assignment)) = assignment(parser) {
            parser = new_parser;
            assignments.push(assignment);
            made_progress = true;
        }

        if let Some(new_parser) = whitespace(parser) {
            parser = new_parser;
            made_progress = true;
        }

        if !made_progress {
            break;
        }
    }
    (parser, assignments)
}
