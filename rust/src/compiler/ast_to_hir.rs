use std::collections::HashMap;

use super::ast::{self, Assignment, Ast, Int, Symbol, Text};
use super::hir::{Expression, Id, Lambda};

pub trait CompileVecAstsToHir {
    fn compile_to_hir(self) -> Lambda;
}
impl CompileVecAstsToHir for Vec<Ast> {
    fn compile_to_hir(self) -> Lambda {
        let mut compiler = Compiler {
            lambda: Lambda::new(0, 0),
            identifiers: HashMap::new(),
        };
        compiler.compile(self);
        compiler.lambda
    }
}

struct Compiler {
    lambda: Lambda,
    identifiers: HashMap<String, Id>,
}
impl Compiler {
    fn push(&mut self, action: Expression) -> Id {
        self.lambda.push(action)
    }
}

impl Compiler {
    fn compile(&mut self, asts: Vec<Ast>) {
        if asts.is_empty() {
            self.lambda.out = self.push(Expression::nothing());
        } else {
            for ast in asts.into_iter() {
                self.lambda.out = self.compile_single(ast);
            }
        }
    }
    fn compile_single(&mut self, ast: Ast) -> Id {
        match ast {
            Ast::Int(Int(int)) => self.push(Expression::Int(int)),
            Ast::Text(Text(string)) => self.push(Expression::Text(string)),
            Ast::Symbol(Symbol(symbol)) => self.push(Expression::Symbol(symbol)),
            Ast::Lambda(ast::Lambda { parameters, body }) => {
                let mut inner = Compiler {
                    lambda: Lambda::new(self.lambda.next_id(), parameters.len()),
                    identifiers: self.identifiers.clone(),
                };
                for (index, parameter) in parameters.iter().enumerate() {
                    inner
                        .lambda
                        .identifiers
                        .insert(inner.lambda.first_id + index, parameter.to_owned());
                }

                inner.compile(body);
                self.push(Expression::Lambda(inner.lambda))
            }
            Ast::Call(ast::Call { name, arguments }) => {
                let argument_ids = arguments
                    .iter()
                    .map(|argument| self.compile_single(argument.to_owned()))
                    .collect();
                self.push(Expression::Call {
                    function: *self
                        .identifiers
                        .get(&name)
                        .expect(&format!("Name `{}` not found.", name)),
                    arguments: argument_ids,
                })
            }
            Ast::Assignment(Assignment {
                name,
                parameters,
                body,
            }) => {
                let mut inner = Compiler {
                    lambda: Lambda::new(self.lambda.next_id(), parameters.len()),
                    identifiers: self.identifiers.clone(),
                };
                for (index, parameter) in parameters.iter().enumerate() {
                    inner
                        .identifiers
                        .insert(parameter.to_owned(), inner.lambda.first_id + index);
                }
                inner.compile(body);
                let id = self.push(Expression::Lambda(inner.lambda));
                self.identifiers.insert(name.clone(), id);
                self.lambda.identifiers.insert(id, name);
                id
            }
        }
    }
}
