use std::ops::Range;
use std::sync::Arc;

use super::ast::{self, Assignment, Ast, AstKind, Identifier, Int, Struct, Symbol, Text};
use super::cst::{self, CstDb};
use super::cst_to_ast::CstToAst;
use super::error::CompilerError;
use super::hir::{self, Body, Expression, Lambda};
use crate::builtin_functions;
use crate::input::Input;
use im::HashMap;

#[salsa::query_group(AstToHirStorage)]
pub trait AstToHir: CstDb + CstToAst {
    fn hir_to_ast_id(&self, id: hir::Id) -> Option<ast::Id>;
    fn hir_to_cst_id(&self, id: hir::Id) -> Option<cst::Id>;
    fn hir_id_to_span(&self, id: hir::Id) -> Option<Range<usize>>;
    fn hir_id_to_display_span(&self, id: hir::Id) -> Option<Range<usize>>;

    fn ast_to_hir_id(&self, id: ast::Id) -> Option<hir::Id>;
    fn cst_to_hir_id(&self, input: Input, id: cst::Id) -> Option<hir::Id>;

    fn hir(&self, input: Input) -> Option<(Arc<Body>, HashMap<hir::Id, ast::Id>)>;
    fn hir_raw(
        &self,
        input: Input,
    ) -> Option<(Arc<Body>, HashMap<hir::Id, ast::Id>, Vec<CompilerError>)>;
}

fn hir_to_ast_id(db: &dyn AstToHir, id: hir::Id) -> Option<ast::Id> {
    let (_, hir_to_ast_id_mapping) = db.hir(id.input.clone()).unwrap();
    hir_to_ast_id_mapping.get(&id).cloned()
}
fn hir_to_cst_id(db: &dyn AstToHir, id: hir::Id) -> Option<cst::Id> {
    db.ast_to_cst_id(db.hir_to_ast_id(id.clone())?)
}
fn hir_id_to_span(db: &dyn AstToHir, id: hir::Id) -> Option<Range<usize>> {
    db.ast_id_to_span(db.hir_to_ast_id(id.clone())?)
}
fn hir_id_to_display_span(db: &dyn AstToHir, id: hir::Id) -> Option<Range<usize>> {
    let cst_id = db.hir_to_cst_id(id.clone())?;
    Some(db.find_cst(id.input, cst_id).display_span())
}

fn ast_to_hir_id(db: &dyn AstToHir, id: ast::Id) -> Option<hir::Id> {
    let (_, hir_to_ast_id_mapping) = db.hir(id.input.clone()).unwrap();
    hir_to_ast_id_mapping
        .iter()
        .find_map(|(key, value)| if value == &id { Some(key) } else { None })
        .cloned()
}
fn cst_to_hir_id(db: &dyn AstToHir, input: Input, id: cst::Id) -> Option<hir::Id> {
    let id = db.cst_to_ast_id(input, id)?;
    db.ast_to_hir_id(id)
}

fn hir(db: &dyn AstToHir, input: Input) -> Option<(Arc<Body>, HashMap<hir::Id, ast::Id>)> {
    db.hir_raw(input)
        .map(|(hir, id_mapping, _)| (hir, id_mapping))
}
fn hir_raw(
    db: &dyn AstToHir,
    input: Input,
) -> Option<(Arc<Body>, HashMap<hir::Id, ast::Id>, Vec<CompilerError>)> {
    let (ast, _) = db.ast(input.clone())?;

    let mut context = Context {
        db,
        input: input.clone(),
    };
    let mut compiler = Compiler::new(&mut context);
    compiler.compile(&ast);
    Some((
        Arc::new(compiler.body),
        compiler.output.id_mapping,
        compiler.output.errors,
    ))
}

struct Context<'c> {
    db: &'c dyn AstToHir,
    input: Input,
}

#[derive(Clone)]
struct Output {
    id_mapping: HashMap<hir::Id, ast::Id>,
    errors: Vec<CompilerError>,
}

struct Compiler<'c> {
    context: &'c Context<'c>,
    output: Output,
    body: Body,
    parent_ids: Vec<usize>,
    next_id: usize,
    identifiers: HashMap<String, hir::Id>,
}
impl<'c> Compiler<'c> {
    fn new(context: &'c Context<'c>) -> Self {
        let builtin_identifiers = builtin_functions::VALUES
            .iter()
            .enumerate()
            .map(|(index, builtin_function)| {
                let string = format!("builtin{:?}", builtin_function);
                (string, hir::Id::new(context.input.clone(), vec![index]))
            })
            .collect::<HashMap<_, _>>();

        let mut compiler = Compiler {
            context,
            output: Output {
                id_mapping: HashMap::new(),
                errors: vec![],
            },
            parent_ids: vec![],
            next_id: builtin_identifiers.len(),
            body: Body::new(),
            identifiers: builtin_identifiers,
        };
        compiler.generate_use();
        compiler
    }

    fn generate_use(&mut self) {
        let mut assignment_inner = Compiler::<'c> {
            context: &mut self.context,
            output: self.output.clone(),
            body: Body::new(),
            parent_ids: add_ids(&self.parent_ids, self.next_id),
            next_id: 0,
            identifiers: self.identifiers.clone(),
        };

        let lambda_id = add_ids(&assignment_inner.parent_ids, assignment_inner.next_id);
        let lambda_parameter_id = hir::Id::new(
            assignment_inner.context.input.clone(),
            add_ids(&lambda_id[..], 0),
        );
        let mut lambda_inner = Compiler::<'c> {
            context: &mut assignment_inner.context,
            output: assignment_inner.output.clone(),
            body: Body::new(),
            parent_ids: lambda_id.clone(),
            next_id: 1, // one parameter
            identifiers: assignment_inner.identifiers.clone(),
        };

        let panic_id = lambda_inner.identifiers["builtinPanic"].clone();
        match &lambda_inner.context.input {
            Input::File(path) => {
                let current_path_content = path
                    .iter()
                    .enumerate()
                    .map(|(index, it)| {
                        (
                            lambda_inner
                                .push_without_ast_mapping(Expression::Int(index as u64), None),
                            lambda_inner
                                .push_without_ast_mapping(Expression::Text(it.to_owned()), None),
                        )
                    })
                    .collect();
                let current_path = lambda_inner
                    .push_without_ast_mapping(Expression::Struct(current_path_content), None);
                lambda_inner.push_without_ast_mapping(
                    Expression::Call {
                        function: lambda_inner.identifiers["builtinUse"].clone(),
                        arguments: vec![current_path, lambda_parameter_id.clone()],
                    },
                    None,
                );
            }
            Input::ExternalFile(_) => {
                let message_id = lambda_inner.push_without_ast_mapping(
                    Expression::Text(
                        "File doesn't belong to the currently opened project.".to_owned(),
                    ),
                    None,
                );
                lambda_inner.push_without_ast_mapping(
                    Expression::Call {
                        function: panic_id,
                        arguments: vec![message_id],
                    },
                    None,
                );
            }
            Input::Untitled(_) => {
                let message_id = lambda_inner.push_without_ast_mapping(
                    Expression::Text("Untitled files can't call `use`.".to_owned()),
                    None,
                );
                lambda_inner.push_without_ast_mapping(
                    Expression::Call {
                        function: panic_id,
                        arguments: vec![message_id],
                    },
                    None,
                );
            }
        }

        assignment_inner.output = lambda_inner.output;
        assignment_inner.push_without_ast_mapping(
            Expression::Lambda(Lambda {
                first_id: lambda_parameter_id,
                parameters: vec!["target".to_owned()],
                body: lambda_inner.body,
            }),
            None,
        );

        self.output = assignment_inner.output;

        self.push_without_ast_mapping(
            Expression::Body(assignment_inner.body),
            Some("use".to_owned()),
        );
    }

    fn compile(&mut self, asts: &[Ast]) {
        if asts.is_empty() {
            self.push_without_ast_mapping(Expression::nothing(), None);
        } else {
            for ast in asts.into_iter() {
                self.compile_single(ast);
            }
        }
    }
    fn compile_single(&mut self, ast: &Ast) -> hir::Id {
        match &ast.kind {
            AstKind::Int(Int(int)) => {
                self.push(ast.id.clone(), Expression::Int(int.to_owned()), None)
            }
            AstKind::Text(Text(string)) => self.push(
                ast.id.clone(),
                Expression::Text(string.value.to_owned()),
                None,
            ),
            AstKind::Identifier(Identifier(symbol)) => {
                let reference = match self.identifiers.get(&symbol.value) {
                    Some(reference) => reference.to_owned(),
                    None => {
                        self.output.errors.push(CompilerError {
                            message: format!("Unknown reference: {}", symbol.value),
                            span: self.context.db.ast_id_to_span(symbol.id.clone()).unwrap(),
                        });
                        return self.push(symbol.id.clone(), Expression::Error, None);
                    }
                };
                self.push(
                    ast.id.clone(),
                    Expression::Reference(reference.to_owned()),
                    None,
                )
            }
            AstKind::Symbol(Symbol(symbol)) => self.push(
                ast.id.clone(),
                Expression::Symbol(symbol.value.to_owned()),
                None,
            ),
            AstKind::Struct(Struct { entries }) => {
                let entries = entries
                    .iter()
                    .map(|(key, value)| (self.compile_single(key), self.compile_single(value)))
                    .collect();
                self.push(ast.id.clone(), Expression::Struct(entries), None)
            }
            AstKind::Lambda(ast::Lambda {
                parameters,
                body: body_asts,
            }) => {
                let mut body = Body::new();
                let lambda_id = add_ids(&self.parent_ids, self.next_id);
                let mut identifiers = self.identifiers.clone();

                for (parameter_index, parameter) in parameters.iter().enumerate() {
                    let id = hir::Id::new(
                        self.context.input.clone(),
                        add_ids(&lambda_id, parameter_index),
                    );
                    self.output
                        .id_mapping
                        .insert(id.clone(), parameter.id.clone());
                    body.identifiers
                        .insert(id.to_owned(), parameter.value.to_owned());
                    identifiers.insert(parameter.value.to_owned(), id);
                }
                let mut inner = Compiler::<'c> {
                    context: &mut self.context,
                    output: self.output.clone(),
                    body,
                    parent_ids: lambda_id.to_owned(),
                    next_id: parameters.len(),
                    identifiers,
                };

                inner.compile(&body_asts);
                self.output = inner.output;
                self.push(
                    ast.id.clone(),
                    Expression::Lambda(Lambda {
                        first_id: hir::Id::new(
                            self.context.input.clone(),
                            add_ids(&lambda_id[..], 0),
                        ),
                        parameters: parameters.iter().map(|it| it.value.to_owned()).collect(),
                        body: inner.body,
                    }),
                    None,
                )
            }
            AstKind::Call(ast::Call { name, arguments }) => {
                let arguments = arguments
                    .iter()
                    .map(|argument| self.compile_single(argument))
                    .collect();

                let function = match self.identifiers.get(&name.value) {
                    Some(function) => function.to_owned(),
                    None => {
                        self.output.errors.push(CompilerError {
                            message: format!("Unknown function: {}", name.value),
                            span: self.context.db.ast_id_to_span(name.id.clone()).unwrap(),
                        });
                        return self.push(name.id.clone(), Expression::Error, None);
                    }
                };
                self.push(
                    ast.id.clone(),
                    Expression::Call {
                        function,
                        arguments,
                    },
                    None,
                )
            }
            AstKind::Assignment(Assignment { name, body }) => {
                let mut inner = Compiler::<'c> {
                    context: &mut self.context,
                    output: self.output.clone(),
                    body: Body::new(),
                    parent_ids: add_ids(&self.parent_ids, self.next_id),
                    next_id: 0,
                    identifiers: self.identifiers.clone(),
                };
                inner.compile(&body);
                self.output = inner.output;
                self.push(
                    ast.id.clone(),
                    Expression::Body(inner.body),
                    Some(name.value.to_owned()),
                )
            }
            AstKind::Error => self.push(ast.id.clone(), Expression::Error, None),
        }
    }

    fn push(
        &mut self,
        ast_id: ast::Id,
        expression: Expression,
        identifier: Option<String>,
    ) -> hir::Id {
        let id = self.create_next_id(ast_id);
        self.body.push(id.clone(), expression, identifier.clone());
        if let Some(identifier) = identifier {
            self.identifiers.insert(identifier, id.clone());
        }
        id
    }
    fn push_without_ast_mapping(
        &mut self,
        expression: Expression,
        identifier: Option<String>,
    ) -> hir::Id {
        let id = self.create_next_id_without_ast_mapping();
        self.body.push(id.to_owned(), expression, None);
        if let Some(identifier) = identifier {
            self.identifiers.insert(identifier, id.clone());
        }
        id
    }

    fn create_next_id(&mut self, ast_id: ast::Id) -> hir::Id {
        let id = self.create_next_id_without_ast_mapping();
        assert!(matches!(
            self.output.id_mapping.insert(id.to_owned(), ast_id),
            None
        ));
        id
    }
    fn create_next_id_without_ast_mapping(&mut self) -> hir::Id {
        let id = hir::Id::new(
            self.context.input.clone(),
            add_ids(&self.parent_ids, self.next_id),
        );
        self.next_id += 1;
        id
    }
}

fn add_ids(parents: &[usize], id: usize) -> Vec<usize> {
    parents.iter().map(|it| *it).chain(vec![id]).collect()
}
