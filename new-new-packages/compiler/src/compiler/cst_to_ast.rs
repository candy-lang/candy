use super::ast::{self, Ast, Call, Int, Lambda, Symbol, Text};
use super::cst::Cst;

pub trait LowerCstToAst {
    fn into_ast(self) -> (Vec<Ast>, Vec<String>);
}
impl LowerCstToAst for Vec<Cst> {
    fn into_ast(self) -> (Vec<Ast>, Vec<String>) {
        lower_csts(self)
    }
}

fn lower_csts(csts: Vec<Cst>) -> (Vec<Ast>, Vec<String>) {
    let mut asts = vec![];
    let mut errors = vec![];
    for cst in csts {
        let (ast, mut new_errors) = lower_cst(cst);
        asts.push(ast);
        errors.append(&mut new_errors);
    }
    (asts, errors)
}
fn lower_cst(cst: Cst) -> (Ast, Vec<String>) {
    match cst {
        Cst::EqualsSign { .. } => (Ast::Error, vec![]),
        Cst::OpeningParenthesis { .. } => (Ast::Error, vec![]),
        Cst::ClosingParenthesis { .. } => (Ast::Error, vec![]),
        Cst::OpeningCurlyBrace { .. } => (Ast::Error, vec![]),
        Cst::ClosingCurlyBrace { .. } => (Ast::Error, vec![]),
        Cst::Arrow { .. } => (Ast::Error, vec![]),
        Cst::Int { value, .. } => (Ast::Int(Int(value)), vec![]),
        Cst::Text { value, .. } => (Ast::Text(Text(value)), vec![]),
        Cst::Identifier { .. } => {
            panic!("Tried to lower an identifier from CST to AST.")
        }
        Cst::Symbol { value, .. } => (Ast::Symbol(Symbol(value)), vec![]),
        Cst::LeadingWhitespace { child, .. } => lower_cst(*child),
        Cst::LeadingComment { child, .. } => lower_cst(*child),
        Cst::TrailingWhitespace { child, .. } => lower_cst(*child),
        Cst::TrailingComment { child, .. } => lower_cst(*child),
        Cst::Parenthesized {
            opening_parenthesis,
            inner,
            closing_parenthesis,
        } => {
            assert!(
                matches!(*opening_parenthesis, Cst::OpeningParenthesis { .. }),
                "Expected an opening parenthesis to start a parenthesized expression, but found `{}`.",
                *opening_parenthesis
            );
            assert!(
                matches!(*closing_parenthesis, Cst::ClosingParenthesis { .. }),
                "Expected a closing parenthesis to end a parenthesized expression, but found `{}`.",
                *closing_parenthesis
            );
            lower_cst(*inner)
        }
        Cst::Lambda {
            opening_curly_brace,
            parameters_and_arrow,
            body,
            closing_curly_brace,
        } => {
            let mut errors = vec![];
            assert!(
                matches!(
                    opening_curly_brace.unwrap_whitespace_and_comment(),
                    Cst::OpeningCurlyBrace { .. }
                ),
                "Expected an opening curly brace at the beginning of a lambda, but found {}.",
                opening_curly_brace,
            );
            let parameters = if let Some((parameters, arrow)) = parameters_and_arrow {
                let (parameters, mut parameters_errors) = lower_parameters(parameters);
                errors.append(&mut parameters_errors);

                assert!(
                    matches!(*arrow, Cst::Arrow { .. }),
                    "Expected an arrow after the parameters in a lambda, but found `{}`.",
                    arrow
                );

                parameters
            } else {
                vec![]
            };

            let (body, mut body_errors) = lower_csts(body);
            errors.append(&mut body_errors);

            assert!(
                matches!(
                    closing_curly_brace.unwrap_whitespace_and_comment(),
                    Cst::ClosingCurlyBrace { .. }
                ),
                "Expected a closing curly brace at the end of a lambda, but found {}.",
                closing_curly_brace
            );

            (Ast::Lambda(Lambda { parameters, body }), errors)
        }
        Cst::Call { name, arguments } => {
            let name = match *name {
                Cst::Identifier { value, .. } => value,
                _ => {
                    panic!(
                        "Expected a symbol for the name of a call, but found `{}`.",
                        name
                    );
                }
            };

            let (arguments, errors) = lower_csts(arguments);
            (Ast::Call(Call { name, arguments }), errors)
        }
        Cst::Assignment {
            name,
            parameters,
            equals_sign,
            body,
        } => {
            let mut errors = vec![];
            let (parameters, mut parameters_errors) = lower_parameters(parameters);
            errors.append(&mut parameters_errors);
            assert!(
                matches!(
                    equals_sign.unwrap_whitespace_and_comment(),
                    Cst::EqualsSign { .. }
                ),
                "Expected an equals sign for the assignment."
            );
            let (body, mut body_errors) = lower_csts(body);
            errors.append(&mut body_errors);
            (
                Ast::Assignment(ast::Assignment {
                    name: lower_identifier(*name),
                    parameters,
                    body,
                }),
                errors,
            )
        }
        Cst::Error { message, .. } => (Ast::Error, vec![message]),
    }
}

fn lower_parameters(csts: Vec<Cst>) -> (Vec<String>, Vec<String>) {
    let mut parameters = vec![];
    let mut errors = vec![];
    for cst in csts {
        let (parameter, mut new_errors) = lower_parameter(cst);
        if let Some(parameter) = parameter {
            parameters.push(parameter);
        }
        errors.append(&mut new_errors);
    }
    (parameters, errors)
}
fn lower_parameter(cst: Cst) -> (Option<String>, Vec<String>) {
    match cst.clone() {
        Cst::Call { name, arguments } => {
            let name = lower_identifier(*name);

            if !arguments.is_empty() {
                (
                    Some(name.clone()),
                    vec![format!(
                        "Parameters can't have parameters themselves, but this one does: `{}`",
                        cst
                    )],
                )
            } else {
                (Some(name), vec![])
            }
        }
        _ => (None, vec![format!("Expected parameter, found `{}`.", cst)]),
    }
}
fn lower_identifier(cst: Cst) -> String {
    match cst {
        Cst::Identifier { value, .. } => value,
        _ => {
            panic!("Expected an identifier, but found `{}`.", cst);
        }
    }
}
