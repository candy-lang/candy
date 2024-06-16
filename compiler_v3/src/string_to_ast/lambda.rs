use super::{
    assignment::{assignment, parameters},
    expression::expression,
    literal::{arrow, closing_curly_brace, opening_curly_brace},
    parser::{Parser, ParserUnwrapOrAstError},
    whitespace::{whitespace, AndTrailingWhitespace},
};
use crate::ast::{AstLambda, AstStatement};
use tracing::instrument;

#[instrument(level = "trace")]
pub fn lambda(parser: Parser) -> Option<(Parser, AstLambda)> {
    let parser = opening_curly_brace(parser)?.and_trailing_whitespace();

    let (parser_with_parameters, parameters) = parameters(parser);
    let (parser, parameters) = if parameters.is_empty() {
        (arrow(parser_with_parameters).unwrap_or(parser), vec![])
    } else if let Some(parser) = arrow(parser_with_parameters) {
        (parser, parameters)
    } else {
        (parser, vec![])
    };

    let (parser, body) = statements(parser);

    let (parser, closing_curly_brace_error) = closing_curly_brace(parser)
        .unwrap_or_ast_error(parser, "This lambda is missing a closing curly brace.");

    Some((
        parser,
        AstLambda {
            parameters,
            body,
            closing_curly_brace_error,
        }
        .into(),
    ))
}

#[instrument(level = "trace")]
pub fn statements(mut parser: Parser) -> (Parser, Vec<AstStatement>) {
    let mut statements = vec![];
    while !parser.is_at_end() {
        let mut made_progress = false;

        if let Some((new_parser, assignment)) = assignment(parser) {
            parser = new_parser;
            statements.push(AstStatement::Assignment(assignment));
            made_progress = true;
        }

        if let Some((new_parser, expression)) = expression(parser) {
            parser = new_parser;
            statements.push(AstStatement::Expression(expression));
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
    (parser, statements)
}
