use super::{
    ast::{self, Assignment, Ast, AstKind, Identifier, Int, Struct, Symbol, Text},
    cst::{self, CstDb},
    cst_to_ast::CstToAst,
    error::{CompilerError, CompilerErrorPayload},
    hir::{self, Body, Expression, HirError, Lambda},
};
use crate::{builtin_functions, input::Input};
use im::HashMap;
use std::{ops::Range, sync::Arc};

#[salsa::query_group(AstToHirStorage)]
pub trait AstToHir: CstDb + CstToAst {
    fn hir_to_ast_id(&self, id: hir::Id) -> Option<ast::Id>;
    fn hir_to_cst_id(&self, id: hir::Id) -> Option<cst::Id>;
    fn hir_id_to_span(&self, id: hir::Id) -> Option<Range<usize>>;
    fn hir_id_to_display_span(&self, id: hir::Id) -> Option<Range<usize>>;

    fn ast_to_hir_id(&self, id: ast::Id) -> Option<hir::Id>;
    fn cst_to_hir_id(&self, input: Input, id: cst::Id) -> Option<hir::Id>;

    fn hir(&self, input: Input) -> Option<(Arc<Body>, HashMap<hir::Id, ast::Id>)>;
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
    let (ast, _) = db.ast(input.clone())?;

    let mut context = Context {
        db,
        input: input.clone(),
    };
    let mut compiler = Compiler::new(&mut context);
    compiler.compile(&ast);
    Some((Arc::new(compiler.body), compiler.id_mapping))
}

struct Context<'c> {
    db: &'c dyn AstToHir,
    input: Input,
}

struct Compiler<'c> {
    context: &'c Context<'c>,
    id_mapping: HashMap<hir::Id, ast::Id>,
    body: Body,
    parent_keys: Vec<String>,
    identifiers: HashMap<String, hir::Id>,
}
impl<'c> Compiler<'c> {
    fn new(context: &'c Context<'c>) -> Self {
        let builtin_identifiers = builtin_functions::VALUES
            .iter()
            .map(|builtin_function| {
                let string = format!("builtin{:?}", builtin_function);
                (
                    string.clone(),
                    hir::Id::new(context.input.clone(), vec![string]),
                )
            })
            .collect::<HashMap<_, _>>();

        let mut compiler = Compiler {
            context,
            id_mapping: HashMap::new(),
            parent_keys: vec![],
            body: Body::new(),
            identifiers: builtin_identifiers,
        };
        compiler.generate_use();
        compiler
    }

    fn generate_use(&mut self) {
        // HirId(project-file:test.candy:11) = body {
        //   HirId(project-file:test.candy:11:0) = lambda { target ->
        //     HirId(project-file:test.candy:11:0:1) = int 0
        //     HirId(project-file:test.candy:11:0:2) = text "test.candy"
        //     HirId(project-file:test.candy:11:0:3) = struct [
        //       HirId(project-file:test.candy:11:0:1): HirId(project-file:test.candy:11:0:2),
        //     ]
        //     HirId(project-file:test.candy:11:0:4) = call HirId(project-file:test.candy:10) with these arguments:
        //       HirId(project-file:test.candy:11:0:3)
        //       HirId(project-file:test.candy:11:0:0)
        //   }
        // }
        let mut assignment_inner = Compiler::<'c> {
            context: &mut self.context,
            id_mapping: self.id_mapping.clone(),
            body: Body::new(),
            parent_keys: add_keys(&self.parent_keys, "use".to_string()),
            identifiers: self.identifiers.clone(),
        };

        let lambda_keys = add_keys(&assignment_inner.parent_keys, "0".to_string());
        let lambda_parameter_id = hir::Id::new(
            assignment_inner.context.input.clone(),
            add_keys(&lambda_keys[..], "target".to_string()),
        );
        let mut lambda_inner = Compiler::<'c> {
            context: &mut assignment_inner.context,
            id_mapping: assignment_inner.id_mapping.clone(),
            body: Body::new(),
            parent_keys: lambda_keys.clone(),
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
                            lambda_inner.push_without_ast_mapping(
                                Expression::Int(index as u64),
                                Some("key".to_string()),
                            ),
                            lambda_inner.push_without_ast_mapping(
                                Expression::Text(it.to_owned()),
                                Some("value".to_string()),
                            ),
                        )
                    })
                    .collect();
                let current_path = lambda_inner.push_without_ast_mapping(
                    Expression::Struct(current_path_content),
                    Some("currentPath".to_string()),
                );
                lambda_inner.push_without_ast_mapping(
                    Expression::Call {
                        function: lambda_inner.identifiers["builtinUse"].clone(),
                        arguments: vec![current_path, lambda_parameter_id.clone()],
                    },
                    Some("module".to_string()),
                );
            }
            Input::ExternalFile(_) => {
                let message_id = lambda_inner.push_without_ast_mapping(
                    Expression::Text(
                        "File doesn't belong to the currently opened project.".to_string(),
                    ),
                    Some("message".to_string()),
                );
                lambda_inner.push_without_ast_mapping(
                    Expression::Call {
                        function: panic_id,
                        arguments: vec![message_id],
                    },
                    Some("panicked".to_string()),
                );
            }
            Input::Untitled(_) => {
                let message_id = lambda_inner.push_without_ast_mapping(
                    Expression::Text("Untitled files can't call `use`.".to_string()),
                    Some("message".to_string()),
                );
                lambda_inner.push_without_ast_mapping(
                    Expression::Call {
                        function: panic_id,
                        arguments: vec![message_id],
                    },
                    Some("panicked".to_string()),
                );
            }
        }

        assignment_inner.id_mapping = lambda_inner.id_mapping;
        assignment_inner.push_without_ast_mapping(
            Expression::Lambda(Lambda {
                parameters: vec![lambda_parameter_id],
                body: lambda_inner.body,
            }),
            None,
        );

        self.id_mapping = assignment_inner.id_mapping;

        self.push_without_ast_mapping(
            Expression::Body(assignment_inner.body),
            Some("use".to_string()),
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
                        return self.push(
                            symbol.id.clone(),
                            Expression::Error {
                                child: None,
                                errors: vec![CompilerError {
                                    input: ast.id.input.clone(),
                                    span: self.context.db.ast_id_to_span(ast.id.clone()).unwrap(),
                                    payload: CompilerErrorPayload::Hir(
                                        HirError::UnknownReference {
                                            symbol: symbol.value.clone(),
                                        },
                                    ),
                                }],
                            },
                            None,
                        );
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
            AstKind::Struct(Struct { fields }) => {
                let fields = fields
                    .iter()
                    .map(|(key, value)| (self.compile_single(key), self.compile_single(value)))
                    .collect();
                self.push(ast.id.clone(), Expression::Struct(fields), None)
            }
            AstKind::Lambda(ast::Lambda {
                parameters,
                body: body_asts,
            }) => {
                let mut body = Body::new();
                let lambda_id = self.create_next_id(ast.id.clone(), None);
                let mut identifiers = self.identifiers.clone();

                for parameter in parameters.iter() {
                    let name = parameter.value.to_string();
                    let id = hir::Id::new(
                        self.context.input.clone(),
                        add_keys(&lambda_id.keys, name.clone()),
                    );
                    self.id_mapping.insert(id.clone(), parameter.id.clone());
                    body.identifiers.insert(id.clone(), name.clone());
                    identifiers.insert(name, id);
                }
                let mut inner = Compiler::<'c> {
                    context: &mut self.context,
                    id_mapping: self.id_mapping.clone(),
                    body,
                    parent_keys: lambda_id.keys.clone(),
                    identifiers,
                };

                inner.compile(&body_asts);
                self.id_mapping = inner.id_mapping;
                self.push_with_existing_id(
                    lambda_id.clone(),
                    Expression::Lambda(Lambda {
                        parameters: parameters
                            .iter()
                            .map(|parameter| {
                                hir::Id::new(
                                    self.context.input.clone(),
                                    add_keys(&lambda_id.keys[..], parameter.value.to_string()),
                                )
                            })
                            .collect(),
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
                        return self.push(
                            name.id.clone(),
                            Expression::Error {
                                child: None,
                                errors: vec![CompilerError {
                                    input: name.id.input.clone(),
                                    span: self.context.db.ast_id_to_span(name.id.clone()).unwrap(),
                                    payload: CompilerErrorPayload::Hir(HirError::UnknownFunction {
                                        name: name.value.clone(),
                                    }),
                                }],
                            },
                            None,
                        );
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
                let name = name.value.to_owned();
                let mut inner = Compiler::<'c> {
                    context: &mut self.context,
                    id_mapping: self.id_mapping.clone(),
                    body: Body::new(),
                    parent_keys: add_keys(&self.parent_keys, name.clone()),
                    identifiers: self.identifiers.clone(),
                };
                inner.compile(&body);
                self.id_mapping = inner.id_mapping;
                self.push(ast.id.clone(), Expression::Body(inner.body), Some(name))
            }
            AstKind::Error { child, errors } => {
                let child = if let Some(child) = child {
                    Some(self.compile_single(&*child))
                } else {
                    None
                };
                self.push(
                    ast.id.clone(),
                    Expression::Error {
                        child,
                        errors: errors.clone(),
                    },
                    None,
                )
            }
        }
    }

    fn push(
        &mut self,
        ast_id: ast::Id,
        expression: Expression,
        identifier: Option<String>,
    ) -> hir::Id {
        let id = self.create_next_id(ast_id, identifier.clone());
        self.push_with_existing_id(id, expression, identifier)
    }
    fn push_without_ast_mapping(
        &mut self,
        expression: Expression,
        identifier: Option<String>,
    ) -> hir::Id {
        let id = self.create_next_id_without_ast_mapping(identifier.clone());
        self.push_with_existing_id(id, expression, identifier)
    }
    fn push_with_existing_id(
        &mut self,
        id: hir::Id,
        expression: Expression,
        identifier: Option<String>,
    ) -> hir::Id {
        self.body
            .push(id.to_owned(), expression, identifier.clone());
        if let Some(identifier) = identifier {
            self.identifiers.insert(identifier, id.clone());
        }
        id
    }

    fn create_next_id(&mut self, ast_id: ast::Id, key: Option<String>) -> hir::Id {
        let id = self.create_next_id_without_ast_mapping(key);
        assert!(self.id_mapping.insert(id.to_owned(), ast_id).is_none());
        id
    }
    fn create_next_id_without_ast_mapping(&mut self, key: Option<String>) -> hir::Id {
        for disambiguator in 0.. {
            let last_part = if let Some(key) = &key {
                let disambiguator = if disambiguator == 0 {
                    "".to_string()
                } else {
                    format!("${}", disambiguator - 1)
                };
                format!("{}{}", key, disambiguator)
            } else {
                format!("{}", disambiguator)
            };
            let id = hir::Id::new(
                self.context.input.clone(),
                add_keys(&self.parent_keys, last_part),
            );
            if !self.id_mapping.contains_key(&id) {
                return id;
            }
        }
        unreachable!()
    }
}

fn add_keys(parents: &[String], id: String) -> Vec<String> {
    parents
        .iter()
        .map(|it| it.to_string())
        .chain(vec![id])
        .collect()
}
