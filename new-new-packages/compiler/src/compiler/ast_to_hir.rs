use super::ast::{self, Assignment, Ast, AstKind, Int, Symbol, Text};
use super::hir::{Expression, Id, Lambda};
use crate::builtin_functions;
use im::HashMap;

pub trait CompileVecAstsToHir {
    fn compile_to_hir(self) -> Lambda;
}
impl CompileVecAstsToHir for Vec<Ast> {
    fn compile_to_hir(self) -> Lambda {
        let builtin_identifiers = builtin_functions::VALUES
            .iter()
            .enumerate()
            .map(|(index, builtin_function)| {
                let string = format!("builtin{:?}", builtin_function);
                (string, index)
            })
            .collect::<HashMap<_, _>>();
        let mut compiler = Compiler {
            lambda: Lambda::new(builtin_identifiers.len(), 0),
            identifiers: builtin_identifiers,
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
        match ast.kind {
            AstKind::Int(Int(int)) => self.push(Expression::Int(int)),
            AstKind::Text(Text(string)) => self.push(Expression::Text(string.value)),
            AstKind::Symbol(Symbol(symbol)) => self.push(Expression::Symbol(symbol.value)),
            AstKind::Lambda(ast::Lambda { parameters, body }) => {
                let mut inner = Compiler {
                    lambda: Lambda::new(self.lambda.next_id(), parameters.len()),
                    identifiers: self.identifiers.clone(),
                };
                for (index, parameter) in parameters.iter().enumerate() {
                    inner
                        .lambda
                        .identifiers
                        .insert(inner.lambda.first_id + index, parameter.value.to_owned());
                }

                inner.compile(body);
                self.push(Expression::Lambda(inner.lambda))
            }
            AstKind::Call(ast::Call { name, arguments }) => {
                let argument_ids = arguments
                    .iter()
                    .map(|argument| self.compile_single(argument.to_owned()))
                    .collect();
                self.push(Expression::Call {
                    function: *self
                        .identifiers
                        .get(&*name)
                        .expect(&format!("Name `{}` not found.", *name)),
                    arguments: argument_ids,
                })
            }
            AstKind::Assignment(Assignment {
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
                        .insert(parameter.value.to_owned(), inner.lambda.first_id + index);
                }
                inner.compile(body);
                let id = self.push(Expression::Lambda(inner.lambda));
                self.identifiers.insert(name.value.clone(), id);
                self.lambda.identifiers.insert(id, name.value);
                id
            }
            // TODO
            AstKind::Error => panic!("Error in AST"),
        }
    }
}
