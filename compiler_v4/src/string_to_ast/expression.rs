use super::{
    lambda::lambda,
    literal::{
        bar, closing_bracket, closing_parenthesis, colon, comma, dot, opening_bracket,
        opening_parenthesis,
    },
    parser::{Parser, ParserUnwrapOrAstError, ParserWithValueUnwrapOrAstError},
    text::text,
    whitespace::{
        whitespace, AndTrailingWhitespace, OptionAndTrailingWhitespace,
        OptionWithValueAndTrailingWhitespace, ValueAndTrailingWhitespace,
    },
    word::{identifier, int, symbol},
};
use crate::ast::{
    AstCall, AstCallArgument, AstExpression, AstOr, AstParenthesized, AstStruct, AstStructAccess,
    AstStructField,
};
use replace_with::replace_with_or_abort;
use tracing::instrument;

#[instrument(level = "trace")]
pub fn expression(parser: Parser) -> Option<(Parser, AstExpression)> {
    // If we start the call list with `if … else …`, the formatting looks weird.
    // Hence, we start with a single `None`.
    let (mut parser, mut result) = None
        .or_else(|| int(parser).map(|(parser, it)| (parser, AstExpression::Int(it))))
        .or_else(|| text(parser).map(|(parser, it)| (parser, AstExpression::Text(it))))
        .or_else(|| identifier(parser).map(|(parser, it)| (parser, AstExpression::Identifier(it))))
        .or_else(|| symbol(parser).map(|(parser, it)| (parser, AstExpression::Symbol(it))))
        .or_else(|| {
            parenthesized(parser).map(|(parser, it)| (parser, AstExpression::Parenthesized(it)))
        })
        .or_else(|| struct_(parser).map(|(parser, it)| (parser, AstExpression::Struct(it))))
        .or_else(|| lambda(parser).map(|(parser, it)| (parser, AstExpression::Lambda(it))))?;

    loop {
        fn parse_suffix<'a>(
            parser: &mut Parser<'a>,
            result: &mut AstExpression,
            parse: fn(Parser<'a>, &mut AstExpression) -> Option<Parser<'a>>,
        ) -> bool {
            parse(*parser, result).map_or(false, |new_parser| {
                *parser = new_parser;
                true
            })
        }

        let mut did_make_progress = false;
        did_make_progress |=
            parse_suffix(&mut parser, &mut result, expression_suffix_struct_access);
        did_make_progress |= parse_suffix(&mut parser, &mut result, expression_suffix_call);
        did_make_progress |= parse_suffix(&mut parser, &mut result, expression_suffix_or);
        if !did_make_progress {
            break;
        }
    }
    Some((parser, result))
}

#[instrument(level = "trace")]
fn parenthesized<'a>(parser: Parser) -> Option<(Parser, AstParenthesized)> {
    let parser = opening_parenthesis(parser)?.and_trailing_whitespace();

    let (parser, inner) = expression(parser)
        .and_trailing_whitespace()
        .unwrap_or_ast_error(parser, "This parenthesized expression is missing a value.");

    let (parser, closing_parenthesis_error) = closing_parenthesis(parser).unwrap_or_ast_error(
        parser,
        "This parenthesized expression is missing a closing parenthesis.",
    );

    Some((
        parser,
        AstParenthesized {
            inner: inner.map(Box::new),
            closing_parenthesis_error,
        },
    ))
}

#[instrument(level = "trace")]
fn struct_<'a>(parser: Parser) -> Option<(Parser, AstStruct)> {
    let mut parser = opening_bracket(parser)?.and_trailing_whitespace();

    let mut fields: Vec<AstStructField> = vec![];
    let mut parser_for_missing_comma_error: Option<Parser> = None;
    while let Some((new_parser, field, new_parser_for_missing_comma_error)) = struct_field(parser) {
        if let Some(parser_for_missing_comma_error) = parser_for_missing_comma_error {
            fields.last_mut().unwrap().comma_error = Some(
                parser_for_missing_comma_error
                    .error_at_current_offset("This parameter is missing a comma."),
            );
        }

        parser = new_parser.and_trailing_whitespace();
        fields.push(field);
        parser_for_missing_comma_error = new_parser_for_missing_comma_error;
    }

    let (parser, closing_bracket_error) = closing_bracket(parser)
        .unwrap_or_ast_error(parser, "This struct is missing a closing bracket.");

    Some((
        parser,
        AstStruct {
            fields,
            closing_bracket_error,
        },
    ))
}
#[instrument(level = "trace")]
fn struct_field<'a>(parser: Parser) -> Option<(Parser, AstStructField, Option<Parser>)> {
    let (parser, key) = identifier(parser)?.and_trailing_whitespace();

    let (parser, colon_error) = colon(parser)
        .and_trailing_whitespace()
        .unwrap_or_ast_error(parser, "This struct field is missing a colon.");

    let (parser, value) = expression(parser)
        .and_trailing_whitespace()
        .unwrap_or_ast_error(parser, "This struct field is missing a value.");

    let (parser, parser_for_missing_comma_error) =
        comma(parser).map_or((parser, Some(parser)), |parser| (parser, None));

    Some((
        parser,
        AstStructField {
            key,
            colon_error,
            value: value.map(Box::new),
            comma_error: None,
        },
        parser_for_missing_comma_error,
    ))
}

#[instrument(level = "trace")]
fn expression_suffix_struct_access<'s>(
    parser: Parser<'s>,
    current: &mut AstExpression,
) -> Option<Parser<'s>> {
    let parser = whitespace(parser).unwrap_or(parser);

    let parser = dot(parser)?.and_trailing_whitespace();

    let (parser, key) =
        identifier(parser).unwrap_or_ast_error(parser, "This struct access is missing a key.");

    replace_with_or_abort(current, |current| {
        AstExpression::StructAccess(AstStructAccess {
            struct_: Box::new(current),
            key,
        })
    });

    Some(parser)
}

#[instrument(level = "trace")]
fn expression_suffix_call<'s>(
    parser: Parser<'s>,
    current: &mut AstExpression,
) -> Option<Parser<'s>> {
    let parser = whitespace(parser).unwrap_or(parser);

    let mut parser = opening_parenthesis(parser)?.and_trailing_whitespace();

    let mut arguments: Vec<AstCallArgument> = vec![];
    let mut parser_for_missing_comma_error: Option<Parser> = None;
    while let Some((new_parser, argument, new_parser_for_missing_comma_error)) =
        argument(parser.and_trailing_whitespace())
    {
        if let Some(parser_for_missing_comma_error) = parser_for_missing_comma_error {
            arguments.last_mut().unwrap().comma_error = Some(
                parser_for_missing_comma_error
                    .error_at_current_offset("This parameter is missing a comma."),
            );
        }

        parser = new_parser;
        arguments.push(argument);
        parser_for_missing_comma_error = new_parser_for_missing_comma_error;
    }
    let parser = parser.and_trailing_whitespace();

    let (parser, closing_parenthesis_error) = closing_parenthesis(parser)
        .unwrap_or_ast_error(parser, "This call is missing a closing parenthesis.");

    replace_with_or_abort(current, |current| {
        AstExpression::Call(AstCall {
            receiver: Box::new(current),
            arguments,
            closing_parenthesis_error,
        })
    });

    Some(parser)
}

#[instrument(level = "trace")]
fn expression_suffix_or<'s>(parser: Parser<'s>, current: &mut AstExpression) -> Option<Parser<'s>> {
    let parser = whitespace(parser).unwrap_or(parser);

    let parser = bar(parser)?.and_trailing_whitespace();

    let (parser, right) =
        expression(parser).unwrap_or_ast_error(parser, "This or is missing a right side.");

    replace_with_or_abort(current, |current| {
        AstExpression::Or(AstOr {
            left: Box::new(current),
            right: right.map(Box::new),
        })
    });

    Some(parser)
}
#[instrument(level = "trace")]
fn argument<'a>(parser: Parser) -> Option<(Parser, AstCallArgument, Option<Parser>)> {
    let (parser, value) = expression(parser)?.and_trailing_whitespace();

    let (parser, parser_for_missing_comma_error) =
        comma(parser).map_or((parser, Some(parser)), |parser| (parser, None));

    Some((
        parser,
        AstCallArgument {
            value,
            comma_error: None,
        },
        parser_for_missing_comma_error,
    ))
}
