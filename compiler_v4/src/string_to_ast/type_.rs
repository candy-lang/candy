use super::{
    literal::{closing_bracket, comma, opening_bracket},
    parser::{OptionOfParser, OptionOfParserWithValue, Parser},
    whitespace::{AndTrailingWhitespace, ValueAndTrailingWhitespace},
    word::raw_identifier,
};
use crate::ast::{AstError, AstString, AstType, AstTypeArgument, AstTypeArguments};
use tracing::instrument;

#[instrument(level = "trace")]
pub fn type_(parser: Parser) -> Option<(Parser, AstType)> {
    let (parser, name) = raw_identifier(parser)?.and_trailing_whitespace();
    let (parser, type_arguments) = type_arguments(parser).optional(parser);
    Some((
        parser,
        AstType {
            name,
            type_arguments,
        },
    ))
}

#[instrument(level = "trace")]
pub fn type_arguments<'s>(parser: Parser<'s>) -> Option<(Parser, AstTypeArguments)> {
    let start_offset = parser.offset();
    let mut parser = opening_bracket(parser)?.and_trailing_whitespace();

    let mut arguments: Vec<AstTypeArgument> = vec![];
    let mut parser_for_missing_comma_error: Option<Parser> = None;
    while let Some((new_parser, argument, new_parser_for_missing_comma_error)) =
        type_argument(parser.and_trailing_whitespace())
    {
        if let Some(parser_for_missing_comma_error) = parser_for_missing_comma_error {
            arguments.last_mut().unwrap().comma_error = Some(
                parser_for_missing_comma_error
                    .error_at_current_offset("This type argument is missing a comma."),
            );
        }

        parser = new_parser;
        arguments.push(argument);
        parser_for_missing_comma_error = new_parser_for_missing_comma_error;
    }
    let parser = parser.and_trailing_whitespace();

    let (parser, closing_bracket_error) = closing_bracket(parser).unwrap_or_ast_error(
        parser,
        "These type arguments are missing a closing bracket.",
    );

    let span = start_offset..parser.offset();
    let empty_arguments_error = if arguments.is_empty() {
        Some(AstError {
            unparsable_input: AstString {
                string: parser.source()[*start_offset..*parser.offset()].into(),
                file: parser.file.to_path_buf(),
                span: span.clone(),
            },
            error: "Type argument brackets must not be empty.".into(),
        })
    } else {
        None
    };

    Some((
        parser,
        AstTypeArguments {
            span,
            arguments,
            empty_arguments_error,
            closing_bracket_error,
        },
    ))
}
#[instrument(level = "trace")]
fn type_argument<'a>(parser: Parser) -> Option<(Parser, AstTypeArgument, Option<Parser>)> {
    let (parser, type_) = type_(parser)?.and_trailing_whitespace();

    let (parser, parser_for_missing_comma_error) =
        comma(parser).map_or((parser, Some(parser)), |parser| (parser, None));

    Some((
        parser,
        AstTypeArgument {
            type_,
            comma_error: None,
        },
        parser_for_missing_comma_error,
    ))
}
