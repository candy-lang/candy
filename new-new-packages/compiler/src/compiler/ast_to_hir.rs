use std::ops::Range;

use super::ast::{self, Assignment, Ast, AstId, AstKind, Int, Symbol, Text};
use super::cst::{Cst, CstId, CstVecExtension};
use super::error::CompilerError;
use super::hir::{self, Expression, Id, Lambda};
use crate::builtin_functions;
use im::HashMap;

pub trait CompileVecAstsToHir {
    fn compile_to_hir(
        self,
        cst: Vec<Cst>,
        ast_cst_id_mapping: HashMap<AstId, CstId>,
    ) -> (Lambda, HashMap<Id, AstId>, Vec<CompilerError>);
}
impl CompileVecAstsToHir for Vec<Ast> {
    fn compile_to_hir(
        self,
        cst: Vec<Cst>,
        ast_cst_id_mapping: HashMap<AstId, CstId>,
    ) -> (Lambda, HashMap<Id, AstId>, Vec<CompilerError>) {
        let builtin_identifiers = builtin_functions::VALUES
            .iter()
            .enumerate()
            .map(|(index, builtin_function)| {
                let string = format!("builtin{:?}", builtin_function);
                (string, Id(index))
            })
            .collect::<HashMap<_, _>>();
        let mut compiler = Compiler::new(cst, ast_cst_id_mapping, builtin_identifiers);
        compiler.compile(self);
        (
            compiler.lambda,
            compiler.context.id_mapping,
            compiler.context.errors,
        )
    }
}

#[derive(Default)]
struct CompilerContext {
    cst: Vec<Cst>,
    ast_cst_id_mapping: HashMap<AstId, CstId>,
    id_mapping: HashMap<Id, AstId>,
    errors: Vec<CompilerError>,
}
struct Compiler {
    context: CompilerContext,
    lambda: Lambda,
    identifiers: HashMap<String, Id>,
}
impl Compiler {
    fn new(
        cst: Vec<Cst>,
        ast_cst_id_mapping: HashMap<AstId, CstId>,
        builtin_identifiers: HashMap<String, Id>,
    ) -> Self {
        Compiler {
            context: CompilerContext {
                cst,
                ast_cst_id_mapping,
                id_mapping: HashMap::new(),
                errors: vec![],
            },
            lambda: Lambda::new(Id(builtin_identifiers.len()), 0),
            identifiers: builtin_identifiers,
        }
    }

    fn push(&mut self, action: Expression) -> Id {
        self.lambda.push(action)
    }

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
                let context = std::mem::take(&mut self.context);
                let mut inner = Compiler {
                    context,
                    lambda: Lambda::new(self.lambda.next_id(), parameters.len()),
                    identifiers: self.identifiers.clone(),
                };
                for (index, parameter) in parameters.iter().enumerate() {
                    inner.lambda.identifiers.insert(
                        Id(inner.lambda.first_id.0 + index),
                        parameter.value.to_owned(),
                    );
                }

                inner.compile(body);
                self.context = inner.context;
                self.push(Expression::Lambda(inner.lambda))
            }
            AstKind::Call(ast::Call { name, arguments }) => {
                let argument_ids = arguments
                    .iter()
                    .map(|argument| self.compile_single(argument.to_owned()))
                    .collect();
                let function = match self.identifiers.get(&*name) {
                    Some(function) => *function,
                    None => {
                        self.context.errors.push(CompilerError {
                            message: format!("Unknown function: {}", *name),
                            span: self.ast_id_to_span(&name.id),
                        });
                        return self.push(Expression::Error);
                    }
                };
                self.push(Expression::Call {
                    function,
                    arguments: argument_ids,
                })
            }
            AstKind::Assignment(Assignment {
                name,
                parameters,
                body,
            }) => {
                let context = std::mem::take(&mut self.context);
                let mut inner = Compiler {
                    context,
                    lambda: Lambda::new(self.lambda.next_id(), parameters.len()),
                    identifiers: self.identifiers.clone(),
                };
                for (index, parameter) in parameters.iter().enumerate() {
                    inner.identifiers.insert(
                        parameter.value.to_owned(),
                        hir::Id(inner.lambda.first_id.0 + index),
                    );
                }
                inner.compile(body);
                self.context = inner.context;
                let id = self.push(Expression::Lambda(inner.lambda));
                self.identifiers.insert(name.value.clone(), id);
                self.lambda.identifiers.insert(id, name.value);
                id
            }
            AstKind::Error => self.push(Expression::Error),
        }
    }

    fn ast_id_to_span(&self, id: &AstId) -> Range<usize> {
        self.context
            .cst
            .find(self.context.ast_cst_id_mapping.get(id).unwrap())
            .expect("AST has no corresponding CST")
            .span()
    }
}
