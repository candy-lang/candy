use super::{
    declarations::assignment,
    literal::{
        closing_curly_brace, closing_parenthesis, comma, dot, opening_curly_brace,
        opening_parenthesis,
    },
    parser::{
        Parser, ParserUnwrapOrAstError, ParserWithResultUnwrapOrAstError,
        ParserWithValueUnwrapOrAstError,
    },
    text::text,
    whitespace::{
        whitespace, AndTrailingWhitespace, OptionWithValueAndTrailingWhitespace,
        ValueAndTrailingWhitespace,
    },
    word::{raw_identifier, word},
};
use crate::ast::{
    AstArgument, AstBody, AstCall, AstError, AstExpression, AstIdentifier, AstInt, AstNavigation,
    AstParenthesized, AstResult, AstStatement,
};
use replace_with::replace_with_or_abort;
use tracing::instrument;

#[instrument(level = "trace")]
pub fn expression(parser: Parser) -> Option<(Parser, AstExpression)> {
    // If we start the call list with `if … else …`, the formatting looks weird.
    // Hence, we start with a single `None`.
    let (mut parser, mut result) = None
        .or_else(|| {
            raw_identifier(parser).map(|(parser, identifier)| {
                (
                    parser,
                    AstExpression::Identifier(AstIdentifier { identifier }),
                )
            })
        })
        .or_else(|| int(parser).map(|(parser, it)| (parser, AstExpression::Int(it))))
        .or_else(|| text(parser).map(|(parser, it)| (parser, AstExpression::Text(it))))
        .or_else(|| {
            parenthesized(parser).map(|(parser, it)| (parser, AstExpression::Parenthesized(it)))
        })
        .or_else(|| body(parser).map(|(parser, it)| (parser, AstExpression::Body(it))))?;

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
        did_make_progress |= parse_suffix(&mut parser, &mut result, expression_suffix_navigation);
        did_make_progress |= parse_suffix(&mut parser, &mut result, expression_suffix_call);
        if !did_make_progress {
            break;
        }
    }
    Some((parser, result))
}

#[instrument(level = "trace")]
pub fn int(parser: Parser) -> Option<(Parser, AstInt)> {
    let (parser, string) = word(parser)?;
    if !string.string.chars().next().unwrap().is_ascii_digit() {
        return None;
    }

    let value = if string.string.chars().all(|c| c.is_ascii_digit()) {
        AstResult::ok(str::parse(&string.string).expect("Couldn't parse int."))
    } else {
        AstResult::error(
            None,
            AstError {
                unparsable_input: string.clone(),
                error: "This integer contains characters that are not digits.".to_string(),
            },
        )
    };
    Some((parser, AstInt { value, string }))
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
fn body<'a>(parser: Parser) -> Option<(Parser, AstBody)> {
    let parser = opening_curly_brace(parser)?.and_trailing_whitespace();

    let (parser, statements) = statements(parser);

    let (parser, closing_curly_brace_error) = closing_curly_brace(parser)
        .unwrap_or_ast_error(parser, "This body is missing a closing curly brace.");

    Some((
        parser,
        AstBody {
            statements,
            closing_curly_brace_error,
        },
    ))
}
#[instrument(level = "trace")]
pub fn statements(mut parser: Parser) -> (Parser, Vec<AstStatement>) {
    let mut statements = vec![];
    while !parser.is_at_end() {
        let mut made_progress = false;

        if let Some((new_parser, statement)) = statement(parser) {
            parser = new_parser;
            statements.push(statement);
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
#[instrument(level = "trace")]
pub fn statement(parser: Parser) -> Option<(Parser, AstStatement)> {
    assignment(parser)
        .map(|(parser, it)| (parser, AstStatement::Assignment(it)))
        .or_else(|| expression(parser).map(|(parser, it)| (parser, AstStatement::Expression(it))))
}

#[instrument(level = "trace")]
fn expression_suffix_navigation<'s>(
    parser: Parser<'s>,
    current: &mut AstExpression,
) -> Option<Parser<'s>> {
    let parser = whitespace(parser).unwrap_or(parser);

    let parser = dot(parser)?.and_trailing_whitespace();

    let (parser, key) = raw_identifier(parser)
        .unwrap_or_ast_error_result(parser, "This struct access is missing a key.");

    replace_with_or_abort(current, |current| {
        AstExpression::Navigation(AstNavigation {
            receiver: Box::new(current),
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

    let mut arguments: Vec<AstArgument> = vec![];
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
fn argument<'a>(parser: Parser) -> Option<(Parser, AstArgument, Option<Parser>)> {
    let (parser, value) = expression(parser)?.and_trailing_whitespace();

    let (parser, parser_for_missing_comma_error) =
        comma(parser).map_or((parser, Some(parser)), |parser| (parser, None));

    Some((
        parser,
        AstArgument {
            value,
            comma_error: None,
        },
        parser_for_missing_comma_error,
    ))
}
