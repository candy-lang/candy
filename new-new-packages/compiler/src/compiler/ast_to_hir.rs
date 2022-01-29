use std::ops::Range;
use std::sync::Arc;

use super::ast::{self, Assignment, Ast, AstKind, Int, Symbol, Text};
use super::cst::{self, Cst, CstVecExtension};
use super::cst_to_ast::CstToAst;
use super::error::CompilerError;
use super::hir::{self, Expression, Lambda};
use crate::builtin_functions;
use crate::input::InputReference;
use im::HashMap;

#[salsa::query_group(AstToHirStorage)]
pub trait AstToHir: CstToAst {
    fn hir(
        &self,
        input_reference: InputReference,
    ) -> Option<(Arc<Lambda>, HashMap<hir::Id, ast::Id>)>;
    fn hir_raw(
        &self,
        input_reference: InputReference,
    ) -> Option<(Arc<Lambda>, HashMap<hir::Id, ast::Id>, Vec<CompilerError>)>;
}

fn hir(
    db: &dyn AstToHir,
    input_reference: InputReference,
) -> Option<(Arc<Lambda>, HashMap<hir::Id, ast::Id>)> {
    db.hir_raw(input_reference)
        .map(|(hir, id_mapping, _)| (hir, id_mapping))
}
fn hir_raw(
    db: &dyn AstToHir,
    input_reference: InputReference,
) -> Option<(Arc<Lambda>, HashMap<hir::Id, ast::Id>, Vec<CompilerError>)> {
    let cst = db.cst(input_reference.clone())?;
    let (ast, ast_cst_id_mapping) = db
        .ast(input_reference)
        .expect("AST must exist if the CST exists for the same input.");

    let builtin_identifiers = builtin_functions::VALUES
        .iter()
        .enumerate()
        .map(|(index, builtin_function)| {
            let string = format!("builtin{:?}", builtin_function);
            (string, hir::Id(index))
        })
        .collect::<HashMap<_, _>>();
    let mut compiler = Compiler::new(&cst, ast_cst_id_mapping, builtin_identifiers);
    compiler.compile(&ast);
    Some((
        Arc::new(compiler.lambda),
        compiler.context.id_mapping,
        compiler.context.errors,
    ))
}

#[derive(Default)]
struct Context<'a> {
    cst: &'a [Cst],
    ast_cst_id_mapping: HashMap<ast::Id, cst::Id>,
    id_mapping: HashMap<hir::Id, ast::Id>,
    errors: Vec<CompilerError>,
}
struct Compiler<'a> {
    context: Context<'a>,
    lambda: Lambda,
    identifiers: HashMap<String, hir::Id>,
}
impl<'a> Compiler<'a> {
    fn new(
        cst: &'a [Cst],
        ast_cst_id_mapping: HashMap<ast::Id, cst::Id>,
        builtin_identifiers: HashMap<String, hir::Id>,
    ) -> Self {
        Compiler {
            context: Context {
                cst,
                ast_cst_id_mapping,
                id_mapping: HashMap::new(),
                errors: vec![],
            },
            lambda: Lambda::new(hir::Id(builtin_identifiers.len()), 0),
            identifiers: builtin_identifiers,
        }
    }

    fn compile(&mut self, asts: &[Ast]) {
        if asts.is_empty() {
            self.lambda.out = self.push_without_ast_mapping(Expression::nothing());
        } else {
            for ast in asts.into_iter() {
                self.lambda.out = self.compile_single(ast);
            }
        }
    }
    fn compile_single(&mut self, ast: &Ast) -> hir::Id {
        match &ast.kind {
            AstKind::Int(Int(int)) => self.push(ast.id, Expression::Int(int.to_owned())),
            AstKind::Text(Text(string)) => {
                self.push(ast.id, Expression::Text(string.value.to_owned()))
            }
            AstKind::Symbol(Symbol(symbol)) => {
                self.push(ast.id, Expression::Symbol(symbol.value.to_owned()))
            }
            AstKind::Lambda(ast::Lambda { parameters, body }) => {
                let context = std::mem::take(&mut self.context);
                let mut inner = Compiler {
                    context,
                    lambda: Lambda::new(self.lambda.next_id(), parameters.len()),
                    identifiers: self.identifiers.clone(),
                };
                for (index, parameter) in parameters.iter().enumerate() {
                    inner.lambda.identifiers.insert(
                        hir::Id(inner.lambda.first_id.0 + index),
                        parameter.value.to_owned(),
                    );
                }

                inner.compile(&body);
                self.context = inner.context;
                self.push(ast.id, Expression::Lambda(inner.lambda))
            }
            AstKind::Call(ast::Call { name, arguments }) => {
                let argument_ids = arguments
                    .iter()
                    .map(|argument| self.compile_single(argument))
                    .collect();
                let function = match self.identifiers.get(&name.value) {
                    Some(function) => *function,
                    None => {
                        self.context.errors.push(CompilerError {
                            message: format!("Unknown function: {}", name.value),
                            span: self.ast_id_to_span(&name.id),
                        });
                        return self.push(name.id, Expression::Error);
                    }
                };
                self.push(
                    ast.id,
                    Expression::Call {
                        function,
                        arguments: argument_ids,
                    },
                )
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
                inner.compile(&body);
                self.context = inner.context;
                let id = self.push(ast.id, Expression::Lambda(inner.lambda));
                self.identifiers.insert(name.value.clone(), id);
                self.lambda.identifiers.insert(id, name.value.to_owned());
                id
            }
            AstKind::Error => self.push(ast.id, Expression::Error),
        }
    }

    fn push(&mut self, ast_id: ast::Id, expression: Expression) -> hir::Id {
        let hir_id = self.push_without_ast_mapping(expression);
        self.context.id_mapping.insert(hir_id, ast_id);
        hir_id
    }
    fn push_without_ast_mapping(&mut self, expression: Expression) -> hir::Id {
        self.lambda.push(expression)
    }

    fn ast_id_to_span(&self, id: &ast::Id) -> Range<usize> {
        self.context
            .cst
            .find(self.context.ast_cst_id_mapping.get(id).unwrap())
            .expect("AST has no corresponding CST")
            .span()
    }
}
