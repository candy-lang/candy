use super::{
    expression::expression,
    literal::{closing_curly_brace, double_quote, opening_curly_brace},
    parser::{OptionOfParser, OptionOfParserWithValue, Parser},
    whitespace::{AndTrailingWhitespace, ValueAndTrailingWhitespace},
};
use crate::ast::{AstError, AstResult, AstText, AstTextPart};
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
fn text_part_text(mut parser: Parser) -> Option<(Parser, AstTextPart)> {
    let mut text = String::new();
    let mut current_start = parser.offset();
    let mut errors = vec![];
    while let Some((new_parser, char)) = parser.consume_char() {
        match char {
            '{' | '"' | '\r' | '\n' => break,
            '\\' => {
                let Some((new_new_parser, char)) = new_parser.consume_char() else {
                    errors.push(new_parser.error_at_current_offset(
                        "Unexpected end of file, expected escaped character",
                    ));
                    break;
                };
                text.push_str(parser.str(current_start));
                match char {
                    '\\' => text.push('\\'),
                    'n' => text.push('\n'),
                    '"' => text.push('"'),
                    _ => errors.push(AstError {
                        unparsable_input: new_new_parser.string(new_parser.offset()),
                        error: "Invalid escape character.".to_string(),
                    }),
                }
                current_start = new_new_parser.offset();
                parser = new_new_parser;
            }
            _ => parser = new_parser,
        }
    }

    text.push_str(parser.str(current_start));
    if text.is_empty() && errors.is_empty() {
        return None;
    }
    Some((
        parser,
        AstTextPart::Text(AstResult::errors(text.into_boxed_str(), errors)),
    ))
}
