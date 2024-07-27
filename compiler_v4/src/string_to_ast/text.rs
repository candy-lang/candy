use super::{
    expression::expression,
    literal::{closing_curly_brace, double_quote, opening_curly_brace},
    parser::{Parser, ParserUnwrapOrAstError, ParserWithValueUnwrapOrAstError},
    whitespace::{AndTrailingWhitespace, ValueAndTrailingWhitespace},
};
use crate::ast::{AstText, AstTextPart};
use tracing::instrument;

// TODO: It might be a good idea to ignore text interpolations in patterns
#[instrument(level = "trace")]
pub fn text(parser: Parser) -> Option<(Parser, AstText)> {
    let mut parser = double_quote(parser)?;

    let mut parts = vec![];
    let (parser, closing_double_quote_error) = loop {
        let (new_parser, part) = match parser.next_char() {
            Some('"') => break (double_quote(parser).unwrap(), None),
            Some('\r' | '\n') | None => {
                break (
                    parser,
                    Some(parser.error_at_current_offset("This text isn't closed.")),
                )
            }
            Some('{') => text_part_interpolation(parser).unwrap(),
            _ => text_part_text(parser).unwrap(),
        };
        parser = new_parser;
        parts.push(part);
    };

    Some((
        parser,
        AstText {
            parts,
            closing_double_quote_error,
        },
    ))
}

#[instrument(level = "trace")]
fn text_part_interpolation(parser: Parser) -> Option<(Parser, AstTextPart)> {
    let parser = opening_curly_brace(parser)?.and_trailing_whitespace();

    let (parser, expression) = expression(parser)
        .unwrap_or_ast_error(
            parser,
            "Here's a start of a text interpolation without an expression after it.",
        )
        .and_trailing_whitespace();

    let (parser, closing_curly_brace_error) = closing_curly_brace(parser)
        .unwrap_or_ast_error(parser, "This text interpolation isn't closed.");

    Some((
        parser,
        AstTextPart::Interpolation {
            expression: expression.map(Box::new),
            closing_curly_brace_error,
        },
    ))
}

#[instrument(level = "trace")]
fn text_part_text(parser: Parser) -> Option<(Parser, AstTextPart)> {
    parser
        .consume_while(|c| !matches!(c, '{' | '"' | '\r' | '\n'))
        .map(|(parser, text)| (parser, AstTextPart::Text(text.string)))
}
