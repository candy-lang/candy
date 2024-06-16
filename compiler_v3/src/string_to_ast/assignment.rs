use super::{
    expression::expression,
    lambda::statements,
    literal::{
        arrow, closing_curly_brace, closing_parenthesis, colon, colon_equals_sign, comma,
        equals_sign, opening_curly_brace, opening_parenthesis,
    },
    parser::{Parser, ParserUnwrapOrAstError, ParserWithValueUnwrapOrAstError},
    whitespace::{
        AndTrailingWhitespace, OptionAndTrailingWhitespace, OptionWithValueAndTrailingWhitespace,
        ValueAndTrailingWhitespace,
    },
    word::{identifier, word},
};
use crate::ast::{AstAssignment, AstAssignmentKind, AstError, AstParameter};
use tracing::instrument;

#[instrument(level = "trace")]
pub fn assignment(parser: Parser) -> Option<(Parser, AstAssignment)> {
    let parser = let_(parser)?.and_trailing_whitespace();

    let (parser, name) = identifier(parser)
        .unwrap_or_ast_error(parser, "Assignment is missing a name.")
        .and_trailing_whitespace();

    let (parser, assignment_sign_error, is_public, kind) =
        function_assignment(parser).unwrap_or_else(|| value_assignment(parser));

    Some((
        parser,
        AstAssignment {
            name,
            assignment_sign_error,
            is_public,
            kind,
        },
    ))
}

#[instrument(level = "trace")]
fn let_(parser: Parser) -> Option<Parser> {
    word(parser)
        .take_if(|(_, value)| &*value.string == "let")
        .map(|(parser, _)| parser)
}

#[instrument(level = "trace")]
fn function_assignment(
    parser: Parser,
) -> Option<(Parser, Option<AstError>, bool, AstAssignmentKind)> {
    let parser = opening_parenthesis(parser)?.and_trailing_whitespace();

    let (parser, parameters) = parameters(parser);

    let (parser, closing_parenthesis_error) = closing_parenthesis(parser)
        .unwrap_or_ast_error(
            parser,
            "Function assignment is missing a closing parenthesis.",
        )
        .and_trailing_whitespace();

    let (parser, arrow_error) = arrow(parser)
        .unwrap_or_ast_error(parser, "Function assignment is missing an arrow.")
        .and_trailing_whitespace();

    let (parser, return_type) = expression(parser)
        .unwrap_or_ast_error(parser, "Function assignment is missing a return type.")
        .and_trailing_whitespace();

    let (parser, assignment_sign_error, is_public) = assignment_sign(parser);
    let parser = parser.and_trailing_whitespace();

    let (parser, opening_curly_brace_error) = opening_curly_brace(parser)
        .and_trailing_whitespace()
        .unwrap_or_ast_error(parser, "This function is missing an opening curly brace.");

    let (parser, body) = statements(parser);

    let (parser, closing_curly_brace_error) = closing_curly_brace(parser)
        .unwrap_or_ast_error(parser, "This function is missing a closing curly brace.");

    Some((
        parser,
        assignment_sign_error,
        is_public,
        AstAssignmentKind::Function {
            parameters,
            closing_parenthesis_error,
            arrow_error,
            return_type: return_type.map(Box::new),
            opening_curly_brace_error,
            body,
            closing_curly_brace_error,
        },
    ))
}

#[instrument(level = "trace")]
fn value_assignment(parser: Parser) -> (Parser, Option<AstError>, bool, AstAssignmentKind) {
    let (parser, type_) =
        colon(parser)
            .and_trailing_whitespace()
            .map_or((parser, None), |parser| {
                let (parser, type_) = expression(parser)
                    .and_trailing_whitespace()
                    .unwrap_or_ast_error(parser, "Value assignment is missing a type.");
                (parser, Some(type_.map(Box::new)))
            });

    let (parser, assignment_sign_error, is_public) = assignment_sign(parser);
    let parser = parser.and_trailing_whitespace();

    let (parser, value) =
        expression(parser).unwrap_or_ast_error(parser, "Assignment is missing a value.");

    (
        parser,
        assignment_sign_error,
        is_public,
        AstAssignmentKind::Value {
            type_,
            value: value.map(Box::new),
        },
    )
}

#[instrument(level = "trace")]
pub fn assignment_sign(parser: Parser) -> (Parser, Option<AstError>, bool) {
    equals_sign(parser)
        .map(|parser| (parser, None, false))
        .or_else(|| colon_equals_sign(parser).map(|parser| (parser, None, true)))
        .unwrap_or_else(|| {
            (
                parser,
                Some(parser.error_at_current_offset(
                    "Assignment is missing an assignment sign (`=` or `:=`).",
                )),
                false,
            )
        })
}

#[instrument(level = "trace")]
pub fn parameters(mut parser: Parser) -> (Parser, Vec<AstParameter>) {
    let mut parameters: Vec<AstParameter> = vec![];
    let mut parser_for_missing_comma_error: Option<Parser> = None;
    while let Some((new_parser, parameter, new_parser_for_missing_comma_error)) =
        parameter(parser.and_trailing_whitespace())
    {
        if let Some(parser_for_missing_comma_error) = parser_for_missing_comma_error {
            parameters.last_mut().unwrap().comma_error = Some(
                parser_for_missing_comma_error
                    .error_at_current_offset("This parameter is missing a comma."),
            );
        }

        parser = new_parser;
        parameters.push(parameter);
        parser_for_missing_comma_error = new_parser_for_missing_comma_error;
    }
    (parser, parameters)
}

#[instrument(level = "trace")]
fn parameter(parser: Parser) -> Option<(Parser, AstParameter, Option<Parser>)> {
    let (parser, name) = identifier(parser)?.and_trailing_whitespace();

    let (parser, type_) =
        colon(parser)
            .and_trailing_whitespace()
            .map_or((parser, None), |parser| {
                let (parser, type_) = expression(parser)
                    .and_trailing_whitespace()
                    .unwrap_or_ast_error(parser, "Parameter is missing a type.");
                (parser, Some(type_))
            });

    let (parser, parser_for_missing_comma_error) = if let Some(parser) = comma(parser) {
        (parser, None)
    } else {
        (parser, Some(parser))
    };

    Some((
        parser,
        AstParameter {
            name,
            type_,
            comma_error: None,
        },
        parser_for_missing_comma_error,
    ))
}
