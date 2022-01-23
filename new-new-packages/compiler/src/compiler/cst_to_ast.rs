use super::ast::{self, Ast, Call, Int, Lambda, Symbol, Text};
use super::cst::{Cst, CstKind};

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
    match cst.kind {
        CstKind::EqualsSign { .. } => (Ast::Error, vec![]),
        CstKind::OpeningParenthesis { .. } => (Ast::Error, vec![]),
        CstKind::ClosingParenthesis { .. } => (Ast::Error, vec![]),
        CstKind::OpeningCurlyBrace { .. } => (Ast::Error, vec![]),
        CstKind::ClosingCurlyBrace { .. } => (Ast::Error, vec![]),
        CstKind::Arrow { .. } => (Ast::Error, vec![]),
        CstKind::Int { value, .. } => (Ast::Int(Int(value)), vec![]),
        CstKind::Text { value, .. } => (Ast::Text(Text(value)), vec![]),
        CstKind::Identifier { .. } => {
            panic!("Tried to lower an identifier from CST to AST.")
        }
        CstKind::Symbol { value, .. } => (Ast::Symbol(Symbol(value)), vec![]),
        CstKind::LeadingWhitespace { child, .. } => lower_cst(*child),
        CstKind::LeadingComment { child, .. } => lower_cst(*child),
        CstKind::TrailingWhitespace { child, .. } => lower_cst(*child),
        CstKind::TrailingComment { child, .. } => lower_cst(*child),
        CstKind::Parenthesized {
            opening_parenthesis,
            inner,
            closing_parenthesis,
        } => {
            assert!(
                matches!(opening_parenthesis.unwrap_whitespace_and_comment().kind, CstKind::OpeningParenthesis { .. }),
                "Expected an opening parenthesis to start a parenthesized expression, but found `{}`.",
                *opening_parenthesis
            );
            assert!(
                matches!(
                    closing_parenthesis.unwrap_whitespace_and_comment().kind,
                    CstKind::ClosingParenthesis { .. }
                ),
                "Expected a closing parenthesis to end a parenthesized expression, but found `{}`.",
                *closing_parenthesis
            );
            lower_cst(*inner)
        }
        CstKind::Lambda {
            opening_curly_brace,
            parameters_and_arrow,
            body,
            closing_curly_brace,
        } => {
            let mut errors = vec![];
            assert!(
                matches!(
                    opening_curly_brace.unwrap_whitespace_and_comment().kind,
                    CstKind::OpeningCurlyBrace { .. }
                ),
                "Expected an opening curly brace at the beginning of a lambda, but found {}.",
                opening_curly_brace,
            );
            let parameters = if let Some((parameters, arrow)) = parameters_and_arrow {
                let (parameters, mut parameters_errors) = lower_parameters(parameters);
                errors.append(&mut parameters_errors);

                assert!(
                    matches!(
                        arrow.unwrap_whitespace_and_comment().kind,
                        CstKind::Arrow { .. }
                    ),
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
                    closing_curly_brace.unwrap_whitespace_and_comment().kind,
                    CstKind::ClosingCurlyBrace { .. }
                ),
                "Expected a closing curly brace at the end of a lambda, but found {}.",
                closing_curly_brace
            );

            (Ast::Lambda(Lambda { parameters, body }), errors)
        }
        CstKind::Call { name, arguments } => {
            let name = name.unwrap_whitespace_and_comment();
            let name = match &name.kind {
                CstKind::Identifier { value, .. } => value.to_owned(),
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
        CstKind::Assignment {
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
                    equals_sign.unwrap_whitespace_and_comment().kind,
                    CstKind::EqualsSign { .. }
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
        CstKind::Error { message, .. } => (Ast::Error, vec![message]),
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
    let cst = cst.unwrap_whitespace_and_comment();
    match cst.kind.clone() {
        CstKind::Call { name, arguments } => {
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
    let cst = cst.unwrap_whitespace_and_comment();
    match &cst.kind {
        CstKind::Identifier { value, .. } => value.clone(),
        _ => {
            panic!("Expected an identifier, but found `{}`.", cst);
        }
    }
}
