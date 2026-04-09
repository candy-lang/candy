use crate::{
    ast::{Ast, AstDeclaration, AstError, AstLocalUseLine, AstPackageUseLine, AstUseLine},
    string_to_ast::{
        literal::{dot, use_keyword},
        parser::OptionOfParserWithResult,
        whitespace::{AndTrailingWhitespace, OptionWithValueAndTrailingWhitespace},
        word::raw_identifier,
    },
};

use list::list_of;
use parser::Parser;
use std::path::Path;
use tracing::{error, instrument};

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
    enum UseLineOrDeclaration {
        UseLine(AstUseLine),
        Declaration(AstDeclaration),
    }
    let (parser, items) = list_of(Parser::new(path, source), |parser| {
        use_line(parser)
            .map(|(parser, use_line)| (parser, UseLineOrDeclaration::UseLine(use_line)))
            .or_else(|| {
                declarations::declaration(parser).map(|(parser, declaration)| {
                    (parser, UseLineOrDeclaration::Declaration(declaration))
                })
            })
    });
    // TODO(JonasWanke): validate that there are no use lines after declarations
    let mut use_lines = vec![];
    let mut declarations = vec![];
    for item in items {
        match item {
            UseLineOrDeclaration::UseLine(use_line) => use_lines.push(use_line),
            UseLineOrDeclaration::Declaration(declaration) => declarations.push(declaration),
        }
    }

    let rest = parser.rest().trim_end();
    if !rest.is_empty() {
        // TODO: report error for unparsed rest
        error!("The parser couldn't parse this rest: {rest:?}");
        // declarations.push(AstStatement::Expression(AstExpression::Error(AstError {
        //     unparsable_input: parser.string_to(Offset(*parser.offset() + rest.len())),
        //     error: "The parser couldn't parse this rest.".to_string(),
        // })));
    }

    Ast {
        use_lines,
        declarations,
    }
}

#[instrument(level = "trace")]
fn use_line<'a>(parser: Parser) -> Option<(Parser, AstUseLine)> {
    let start_offset = parser.offset();
    let parser = use_keyword(parser)?.and_trailing_whitespace();
    let use_keyword_span = start_offset..parser.offset();

    let (parser, dots) = list_of(parser, |parser| dot(parser).map(|it| (it, ())));

    let (parser, mut name) = raw_identifier(parser)
        .and_trailing_whitespace()
        .unwrap_or_ast_error_result(parser, "This use line is missing a name.");

    let use_line = if name
        .value()
        .as_ref()
        .unwrap()
        .chars()
        .next()
        .unwrap()
        .is_uppercase()
    {
        AstUseLine::Local(AstLocalUseLine {
            use_keyword_span,
            dot_count: dots.len(),
            path: name,
        })
    } else {
        if !dots.is_empty() {
            name.add_error(AstError {
                unparsable_input: (*name.value().as_ref().unwrap()).clone(),
                error: "Use lines for packages cannot start with dots.".to_string(),
            });
        }
        AstUseLine::Package(AstPackageUseLine {
            use_keyword_span,
            package_name: name,
        })
    };

    Some((parser, use_line))
}
