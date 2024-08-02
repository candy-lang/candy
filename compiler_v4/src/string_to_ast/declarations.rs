use super::{
    expression::{expression, statements},
    literal::{
        closing_curly_brace, closing_parenthesis, colon, comma, enum_keyword, equals_sign,
        fun_keyword, let_keyword, opening_curly_brace, opening_parenthesis, struct_keyword,
    },
    parser::{
        Parser, ParserUnwrapOrAstError, ParserWithResultUnwrapOrAstError,
        ParserWithValueUnwrapOrAstError,
    },
    type_::type_,
    whitespace::{
        whitespace, AndTrailingWhitespace, OptionAndTrailingWhitespace,
        OptionWithValueAndTrailingWhitespace, ValueAndTrailingWhitespace,
    },
    word::raw_identifier,
};
use crate::ast::{
    AstAssignment, AstDeclaration, AstEnum, AstEnumVariant, AstFunction, AstParameter, AstStruct,
    AstStructField,
};
use tracing::instrument;

#[instrument(level = "trace")]
pub fn declarations(mut parser: Parser) -> (Parser, Vec<AstDeclaration>) {
    let mut declarations = vec![];
    while !parser.is_at_end() {
        let mut made_progress = false;

        if let Some((new_parser, declaration)) = declaration(parser) {
            parser = new_parser;
            declarations.push(declaration);
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
    (parser, declarations)
}
#[instrument(level = "trace")]
fn declaration<'a>(parser: Parser) -> Option<(Parser, AstDeclaration)> {
    None.or_else(|| struct_(parser).map(|(parser, it)| (parser, AstDeclaration::Struct(it))))
        .or_else(|| enum_(parser).map(|(parser, it)| (parser, AstDeclaration::Enum(it))))
        .or_else(|| assignment(parser).map(|(parser, it)| (parser, AstDeclaration::Assignment(it))))
        .or_else(|| function(parser).map(|(parser, it)| (parser, AstDeclaration::Function(it))))
}

#[instrument(level = "trace")]
fn struct_<'a>(parser: Parser) -> Option<(Parser, AstStruct)> {
    let parser = struct_keyword(parser)?.and_trailing_whitespace();

    let (parser, name) = raw_identifier(parser)
        .and_trailing_whitespace()
        .unwrap_or_ast_error_result(parser, "This struct is missing a name.");

    let (mut parser, opening_curly_brace_error) = opening_curly_brace(parser)
        .and_trailing_whitespace()
        .unwrap_or_ast_error(parser, "This struct is missing an opening curly brace.");

    let mut fields: Vec<AstStructField> = vec![];
    let mut parser_for_missing_comma_error: Option<Parser> = None;
    while let Some((new_parser, field, new_parser_for_missing_comma_error)) = struct_field(parser) {
        if let Some(parser_for_missing_comma_error) = parser_for_missing_comma_error {
            fields.last_mut().unwrap().comma_error = Some(
                parser_for_missing_comma_error
                    .error_at_current_offset("This struct field is missing a comma."),
            );
        }

        parser = new_parser.and_trailing_whitespace();
        fields.push(field);
        parser_for_missing_comma_error = new_parser_for_missing_comma_error;
    }

    let (parser, closing_curly_brace_error) = closing_curly_brace(parser)
        .unwrap_or_ast_error(parser, "This struct is missing a closing curly brace.");

    Some((
        parser,
        AstStruct {
            name,
            opening_curly_brace_error,
            fields,
            closing_curly_brace_error,
        },
    ))
}
#[instrument(level = "trace")]
fn struct_field<'a>(parser: Parser) -> Option<(Parser, AstStructField, Option<Parser>)> {
    let (parser, name) = raw_identifier(parser)?.and_trailing_whitespace();

    let (parser, colon_error) = colon(parser)
        .and_trailing_whitespace()
        .unwrap_or_ast_error(parser, "This struct field is missing a colon.");

    let (parser, type_) = type_(parser)
        .and_trailing_whitespace()
        .unwrap_or_ast_error(parser, "This struct field is missing a type.");

    let (parser, parser_for_missing_comma_error) =
        comma(parser).map_or((parser, Some(parser)), |parser| (parser, None));

    Some((
        parser,
        AstStructField {
            name,
            colon_error,
            type_,
            comma_error: None,
        },
        parser_for_missing_comma_error,
    ))
}

#[instrument(level = "trace")]
fn enum_<'a>(parser: Parser) -> Option<(Parser, AstEnum)> {
    let parser = enum_keyword(parser)?.and_trailing_whitespace();

    let (parser, name) = raw_identifier(parser)
        .and_trailing_whitespace()
        .unwrap_or_ast_error_result(parser, "This enum is missing a name.");

    let (mut parser, opening_curly_brace_error) = opening_curly_brace(parser)
        .and_trailing_whitespace()
        .unwrap_or_ast_error(parser, "This enum is missing an opening curly brace.");

    let mut variants: Vec<AstEnumVariant> = vec![];
    let mut parser_for_missing_comma_error: Option<Parser> = None;
    while let Some((new_parser, variant, new_parser_for_missing_comma_error)) = enum_variant(parser)
    {
        if let Some(parser_for_missing_comma_error) = parser_for_missing_comma_error {
            variants.last_mut().unwrap().comma_error = Some(
                parser_for_missing_comma_error
                    .error_at_current_offset("This enum variant is missing a comma."),
            );
        }

        parser = new_parser.and_trailing_whitespace();
        variants.push(variant);
        parser_for_missing_comma_error = new_parser_for_missing_comma_error;
    }

    let (parser, closing_curly_brace_error) = closing_curly_brace(parser)
        .unwrap_or_ast_error(parser, "This enum is missing a closing curly brace.");

    Some((
        parser,
        AstEnum {
            name,
            opening_curly_brace_error,
            variants,
            closing_curly_brace_error,
        },
    ))
}
#[instrument(level = "trace")]
fn enum_variant<'a>(parser: Parser) -> Option<(Parser, AstEnumVariant, Option<Parser>)> {
    let (parser, name) = raw_identifier(parser)?.and_trailing_whitespace();

    let (parser, type_) = if let Some(parser) = colon(parser).and_trailing_whitespace() {
        let (parser, type_) = type_(parser)
            .and_trailing_whitespace()
            .unwrap_or_ast_error(parser, "This enum variant is missing a type.");
        (parser, Some(type_))
    } else {
        (parser, None)
    };

    let (parser, parser_for_missing_comma_error) =
        comma(parser).map_or((parser, Some(parser)), |parser| (parser, None));

    Some((
        parser,
        AstEnumVariant {
            name,
            type_,
            comma_error: None,
        },
        parser_for_missing_comma_error,
    ))
}

#[instrument(level = "trace")]
pub fn assignment<'a>(parser: Parser) -> Option<(Parser, AstAssignment)> {
    let parser = let_keyword(parser)?.and_trailing_whitespace();

    let (parser, name) = raw_identifier(parser)
        .and_trailing_whitespace()
        .unwrap_or_ast_error_result(parser, "This assignment is missing a name.");

    let (parser, type_) = if let Some(parser) = colon(parser).and_trailing_whitespace() {
        let (parser, type_) = type_(parser)
            .and_trailing_whitespace()
            .unwrap_or_ast_error(parser, "This assignment is missing a type after the colon.");
        (parser, Some(type_))
    } else {
        (parser, None)
    };

    let (parser, equals_sign_error) = equals_sign(parser)
        .and_trailing_whitespace()
        .unwrap_or_ast_error(parser, "This assignment is missing an equals sign.");

    let (parser, value) =
        expression(parser).unwrap_or_ast_error(parser, "This assignment is missing a value.");

    Some((
        parser,
        AstAssignment {
            name,
            type_,
            equals_sign_error,
            value,
        },
    ))
}

#[instrument(level = "trace")]
fn function<'a>(parser: Parser) -> Option<(Parser, AstFunction)> {
    let parser = fun_keyword(parser)?.and_trailing_whitespace();

    let (parser, name) = raw_identifier(parser)
        .and_trailing_whitespace()
        .unwrap_or_ast_error_result(parser, "This function is missing a name.");

    let (mut parser, opening_parenthesis_error) = opening_parenthesis(parser)
        .and_trailing_whitespace()
        .unwrap_or_ast_error(parser, "This function is missing an opening parenthesis.");

    let mut parameters: Vec<AstParameter> = vec![];
    let mut parser_for_missing_comma_error: Option<Parser> = None;
    while let Some((new_parser, variant, new_parser_for_missing_comma_error)) = parameter(parser) {
        if let Some(parser_for_missing_comma_error) = parser_for_missing_comma_error {
            parameters.last_mut().unwrap().comma_error = Some(
                parser_for_missing_comma_error
                    .error_at_current_offset("This function variant is missing a comma."),
            );
        }

        parser = new_parser.and_trailing_whitespace();
        parameters.push(variant);
        parser_for_missing_comma_error = new_parser_for_missing_comma_error;
    }

    let (parser, closing_parenthesis_error) = closing_parenthesis(parser)
        .and_trailing_whitespace()
        .unwrap_or_ast_error(parser, "This function is missing an closing parenthesis.");

    let (parser, return_type) = type_(parser)
        .map(|(parser, it)| (parser, Some(it)))
        .unwrap_or((parser, None))
        .and_trailing_whitespace();

    let (parser, opening_curly_brace_error) = opening_curly_brace(parser)
        .and_trailing_whitespace()
        .unwrap_or_ast_error(parser, "This function is missing an opening curly brace.");

    let (parser, body) = statements(parser);

    let (parser, closing_curly_brace_error) = closing_curly_brace(parser)
        .unwrap_or_ast_error(parser, "This function is missing a closing curly brace.");

    Some((
        parser,
        AstFunction {
            name,
            opening_parenthesis_error,
            parameters,
            closing_parenthesis_error,
            return_type,
            opening_curly_brace_error,
            body,
            closing_curly_brace_error,
        },
    ))
}
#[instrument(level = "trace")]
fn parameter<'a>(parser: Parser) -> Option<(Parser, AstParameter, Option<Parser>)> {
    let (parser, name) = raw_identifier(parser)?.and_trailing_whitespace();

    let (parser, colon_error) = colon(parser)
        .and_trailing_whitespace()
        .unwrap_or_ast_error(parser, "This struct field is missing a colon.");

    let (parser, type_) = type_(parser)
        .and_trailing_whitespace()
        .unwrap_or_ast_error(parser, "This struct field is missing a type.");

    let (parser, parser_for_missing_comma_error) =
        comma(parser).map_or((parser, Some(parser)), |parser| (parser, None));

    Some((
        parser,
        AstParameter {
            name,
            colon_error,
            type_,
            comma_error: None,
        },
        parser_for_missing_comma_error,
    ))
}
