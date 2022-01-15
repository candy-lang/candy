use super::ast::{self, Ast};
use super::cst::{self, Cst};

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
        if let Some(ast) = ast {
            asts.push(ast);
        }
        errors.append(&mut new_errors);
    }
    (asts, errors)
}
fn lower_cst(cst: Cst) -> (Option<Ast>, Vec<String>) {
    match cst {
        Cst::Int(int) => (Some(Ast::Int(lower_int(int))), vec![]),
        Cst::Text(text) => (Some(Ast::Text(lower_text(text))), vec![]),
        Cst::Symbol(symbol) => (Some(Ast::Symbol(lower_symbol(symbol))), vec![]),
        Cst::Parenthesized(cst) => lower_cst(*cst),
        Cst::Lambda(lambda) => {
            let (lambda, errors) = lower_lambda(lambda);
            (Some(Ast::Lambda(lambda)), errors)
        }
        Cst::Call(call) => {
            let (call, errors) = lower_call(call);
            (Some(Ast::Call(call)), errors)
        }
        Cst::Assignment(assignment) => {
            let (assignment, errors) = lower_assignment(assignment);
            (Some(Ast::Assignment(assignment)), errors)
        }
        Cst::Error { message, .. } => (None, vec![message]),
    }
}
fn lower_int(cst: cst::Int) -> ast::Int {
    ast::Int(cst.0)
}
fn lower_text(cst: cst::Text) -> ast::Text {
    ast::Text(cst.0)
}
fn lower_symbol(cst: cst::Symbol) -> ast::Symbol {
    ast::Symbol(cst.0)
}

fn lower_lambda(cst: cst::Lambda) -> (ast::Lambda, Vec<String>) {
    let mut errors = vec![];
    let (parameters, mut parameters_errors) = lower_parameters(cst.parameters);
    errors.append(&mut parameters_errors);
    let (body, mut body_errors) = lower_csts(cst.body);
    errors.append(&mut body_errors);
    (ast::Lambda { parameters, body }, errors)
}

fn lower_call(cst: cst::Call) -> (ast::Call, Vec<String>) {
    let (arguments, errors) = lower_csts(cst.arguments);
    (
        ast::Call {
            name: cst.name,
            arguments,
        },
        errors,
    )
}

fn lower_assignment(cst: cst::Assignment) -> (ast::Assignment, Vec<String>) {
    let mut errors = vec![];
    let (parameters, mut parameters_errors) = lower_parameters(cst.parameters);
    errors.append(&mut parameters_errors);
    let (body, mut body_errors) = lower_csts(cst.body);
    errors.append(&mut body_errors);
    (
        ast::Assignment {
            name: cst.name,
            parameters,
            body,
        },
        errors,
    )
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
    match cst {
        Cst::Call(cst::Call { name, arguments }) => {
            if !arguments.is_empty() {
                (
                    Some(name.clone()),
                    vec![format!(
                        "Parameters can't have parameters themselves, but this one does: {:?}",
                        cst::Call { name, arguments }
                    )],
                )
            } else {
                (Some(name), vec![])
            }
        }
        _ => (None, vec![format!("Expected parameter, found {:?}.", cst)]),
    }
}
