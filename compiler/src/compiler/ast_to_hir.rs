use super::{
    ast::{
        self, Assignment, Ast, AstKind, Call, CallReceiver, Identifier, Int, Struct, StructAccess,
        Symbol, Text,
    },
    cst::{self, CstDb},
    cst_to_ast::CstToAst,
    error::{CompilerError, CompilerErrorPayload},
    hir::{self, Body, Expression, HirError, Lambda},
    utils::AdjustCasingOfFirstLetter,
};
use crate::{
    builtin_functions::{self, BuiltinFunction},
    input::Input,
};
use im::HashMap;
use itertools::Itertools;
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
    compiler.generate_exports_struct();
    let id_mapping_of_existing_ids = compiler
        .id_mapping
        .into_iter()
        .filter_map(|(key, value)| {
            if let Some(value) = value {
                Some((key, value))
            } else {
                None
            }
        })
        .collect();
    Some((Arc::new(compiler.body), id_mapping_of_existing_ids))
}

struct Context<'c> {
    db: &'c dyn AstToHir,
    input: Input,
}

struct Compiler<'c> {
    context: &'c Context<'c>,
    id_mapping: HashMap<hir::Id, Option<ast::Id>>,
    body: Body,
    parent_keys: Vec<String>,
    identifiers: HashMap<String, hir::Id>,
    public_identifiers: HashMap<String, hir::Id>,
}
impl<'c> Compiler<'c> {
    fn new(context: &'c Context<'c>) -> Self {
        let mut compiler = Compiler {
            context,
            id_mapping: HashMap::new(),
            parent_keys: vec![],
            body: Body::new(),
            identifiers: HashMap::new(),
            public_identifiers: HashMap::new(),
        };
        compiler.generate_sparkles();
        compiler.generate_use_asset();
        compiler.generate_use();
        compiler
    }

    fn generate_sparkles(&mut self) {
        let mut sparkles_map = HashMap::new();

        for builtin_function in builtin_functions::VALUES.iter() {
            let symbol = self.push(
                None,
                Expression::Symbol(format!("{:?}", builtin_function)),
                None,
            );
            let builtin = self.push(None, Expression::Builtin(*builtin_function), None);
            sparkles_map.insert(symbol, builtin);
        }

        let sparkles_map = Expression::Struct(sparkles_map);
        self.push(None, sparkles_map, Some("✨".to_string()));
    }

    // Generates a struct that contains the current path as a struct. Generates
    // panicking code if the current file is not on the file system and of the
    // current project.
    fn generate_current_path_struct(&mut self) -> hir::Id {
        // HirId(~:test.candy:something:key) = int 0
        // HirId(~:test.candy:something:raw_path) = text "test.candy"
        // HirId(~:test.candy:something:currentPath) = struct [
        //   HirId(~:test.candy:something:key): HirId(~:test.candy:something:raw_path),
        // ]
        let panic_id = self.push(
            None,
            Expression::Builtin(BuiltinFunction::Panic),
            Some("panic".to_string()),
        );
        match &self.context.input {
            Input::File(path) => {
                let current_path_content = path
                    .iter()
                    .filter(|path| *path != ".candy")
                    .enumerate()
                    .map(|(index, it)| {
                        (
                            self.push(None, Expression::Int(index as u64), Some("key".to_string())),
                            self.push(
                                None,
                                Expression::Text(it.to_owned()),
                                Some("rawPath".to_string()),
                            ),
                        )
                    })
                    .collect();
                self.push(
                    None,
                    Expression::Struct(current_path_content),
                    Some("currentPath".to_string()),
                )
            }
            Input::ExternalFile(_) => {
                let message_id = self.push(
                    None,
                    Expression::Text(
                        "File doesn't belong to the currently opened project.".to_string(),
                    ),
                    Some("message".to_string()),
                );
                self.push(
                    None,
                    Expression::Call {
                        function: panic_id,
                        arguments: vec![message_id],
                    },
                    Some("panicked".to_string()),
                )
            }
            Input::Untitled(_) => {
                let message_id = self.push(
                    None,
                    Expression::Text("Untitled files can't call `useAsset`.".to_string()),
                    Some("message".to_string()),
                );
                self.push(
                    None,
                    Expression::Call {
                        function: panic_id,
                        arguments: vec![message_id],
                    },
                    Some("panicked".to_string()),
                )
            }
        }
    }

    fn generate_use_asset(&mut self) {
        // HirId(~:test.candy:useAsset) = lambda { HirId(~:test.candy:useAsset:target) ->
        //   HirId(~:test.candy:useAsset:panic) = builtinPanic
        //   HirId(~:test.candy:useAsset:useAsset) = builtinUseAsset
        //   HirId(~:test.candy:useAsset:key) = int 0
        //   HirId(~:test.candy:useAsset:raw_path) = text "test.candy"
        //   HirId(~:test.candy:useAsset:currentPath) = struct [
        //     HirId(~:test.candy:useAsset:key): HirId(~:test.candy:useAsset:raw_path),
        //   ]
        //   HirId(~:test.candy:useAsset:importedFileContent) = call HirId(~:test.candy:useAsset:useAsset) with these arguments:
        //     HirId(~:test.candy:useAsset:currentPath)
        //     HirId(~:test.candy:useAsset:target)
        // }
        let mut assignment_inner = Compiler::<'c> {
            context: &mut self.context,
            id_mapping: self.id_mapping.clone(),
            body: Body::new(),
            parent_keys: add_keys(&self.parent_keys, "useAsset".to_string()),
            identifiers: self.identifiers.clone(),
            public_identifiers: HashMap::new(),
        };

        let lambda_keys = assignment_inner.parent_keys;
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
            public_identifiers: HashMap::new(),
        };

        let current_path = lambda_inner.generate_current_path_struct();
        let use_id = lambda_inner.push(
            None,
            Expression::Builtin(BuiltinFunction::UseAsset),
            Some("useAsset".to_string()),
        );
        lambda_inner.push(
            None,
            Expression::Call {
                function: use_id,
                arguments: vec![current_path, lambda_parameter_id.clone()],
            },
            Some("importedFileContent".to_string()),
        );

        assignment_inner.id_mapping = lambda_inner.id_mapping;
        self.id_mapping = assignment_inner.id_mapping;
        self.push(
            None,
            Expression::Lambda(Lambda {
                parameters: vec![lambda_parameter_id],
                body: lambda_inner.body,
                fuzzable: false,
            }),
            Some("useAsset".to_string()),
        );
    }

    fn generate_use(&mut self) {
        // HirId(~:test.candy:use) = lambda { HirId(~:test.candy:use:target) ->
        //   HirId(~:test.candy:use:panic) = builtinPanic
        //   HirId(~:test.candy:use:key) = int 0
        //   HirId(~:test.candy:use:rawPath) = text "test.candy"
        //   HirId(~:test.candy:use:currentPath) = struct [
        //     HirId(~:test.candy:use:key): HirId(~:test.candy:use:rawPath),
        //   ]
        //   HirId(~:test.candy:use:useLocalModule) = builtinUseLocalModule
        //   HirId(~:test.candy:use:importedModule) = call HirId(~:test.candy:use:useLocalModule) with these arguments:
        //     HirId(~:test.candy:use:currentPath)
        //     HirId(~:test.candy:use:target)
        //  }
        let mut assignment_inner = Compiler::<'c> {
            context: &mut self.context,
            id_mapping: self.id_mapping.clone(),
            body: Body::new(),
            parent_keys: add_keys(&self.parent_keys, "use".to_string()),
            identifiers: self.identifiers.clone(),
            public_identifiers: HashMap::new(),
        };

        let lambda_keys = assignment_inner.parent_keys;
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
            public_identifiers: HashMap::new(),
        };

        let current_path = lambda_inner.generate_current_path_struct();
        let use_id = lambda_inner.push(
            None,
            Expression::Builtin(BuiltinFunction::UseLocalModule),
            Some("useLocalModule".to_string()),
        );
        lambda_inner.push(
            None,
            Expression::Call {
                function: use_id,
                arguments: vec![current_path, lambda_parameter_id.clone()],
            },
            Some("importedModule".to_string()),
        );

        assignment_inner.id_mapping = lambda_inner.id_mapping;
        self.id_mapping = assignment_inner.id_mapping;
        self.push(
            None,
            Expression::Lambda(Lambda {
                parameters: vec![lambda_parameter_id],
                body: lambda_inner.body,
                fuzzable: false,
            }),
            Some("use".to_string()),
        );
    }

    fn generate_exports_struct(&mut self) -> hir::Id {
        // HirId(~:test.candy:100) = symbol Foo
        // HirId(~:test.candy:102) = struct [
        //   HirId(~:test.candy:100): HirId(~:test.candy:101),
        // ]
        let mut exports = HashMap::new();
        for (name, id) in self.public_identifiers.clone() {
            exports.insert(
                self.push(
                    None,
                    Expression::Symbol(name.uppercase_first_letter()),
                    None,
                ),
                id,
            );
        }
        self.push(None, Expression::Struct(exports), None)
    }

    fn compile(&mut self, asts: &[Ast]) -> hir::Id {
        if asts.is_empty() {
            self.push(None, Expression::nothing(), None)
        } else {
            let mut last_id = None;
            for ast in asts.into_iter() {
                last_id = Some(self.compile_single(ast));
            }
            last_id.unwrap()
        }
    }
    fn compile_single(&mut self, ast: &Ast) -> hir::Id {
        match &ast.kind {
            AstKind::Int(Int(int)) => {
                self.push(Some(ast.id.clone()), Expression::Int(int.to_owned()), None)
            }
            AstKind::Text(Text(string)) => self.push(
                Some(ast.id.clone()),
                Expression::Text(string.value.to_owned()),
                None,
            ),
            AstKind::Identifier(Identifier(symbol)) => {
                let reference = match self.identifiers.get(&symbol.value) {
                    Some(reference) => reference.to_owned(),
                    None => {
                        return self.push(
                            Some(symbol.id.clone()),
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
                    Some(ast.id.clone()),
                    Expression::Reference(reference.to_owned()),
                    None,
                )
            }
            AstKind::Symbol(Symbol(symbol)) => self.push(
                Some(ast.id.clone()),
                Expression::Symbol(symbol.value.to_owned()),
                None,
            ),
            AstKind::Struct(Struct { fields }) => {
                let fields = fields
                    .iter()
                    .map(|(key, value)| (self.compile_single(key), self.compile_single(value)))
                    .collect();
                self.push(Some(ast.id.clone()), Expression::Struct(fields), None)
            }
            AstKind::StructAccess(struct_access) => {
                self.lower_struct_access(Some(ast.id.clone()), struct_access)
            }
            AstKind::Lambda(lambda) => self.compile_lambda(ast.id.clone(), lambda, None),
            AstKind::Call(call) => self.lower_call(Some(ast.id.clone()), call),
            AstKind::Assignment(Assignment {
                name,
                is_public,
                body,
            }) => {
                let name = name.value.to_owned();
                let id = match body {
                    ast::AssignmentBody::Lambda(lambda) => {
                        self.compile_lambda(ast.id.clone(), lambda, Some(name.clone()))
                    }
                    ast::AssignmentBody::Body(body) => {
                        let body = self.compile(body);
                        self.push(
                            Some(ast.id.clone()),
                            Expression::Reference(body),
                            Some(name.clone()),
                        )
                    }
                };
                if *is_public {
                    self.public_identifiers.insert(name, id.clone());
                }
                id
            }
            AstKind::Error { child, errors } => {
                let child = if let Some(child) = child {
                    Some(self.compile_single(&*child))
                } else {
                    None
                };
                self.push(
                    Some(ast.id.clone()),
                    Expression::Error {
                        child,
                        errors: errors.clone(),
                    },
                    None,
                )
            }
        }
    }
    fn compile_lambda(
        &mut self,
        id: ast::Id,
        lambda: &ast::Lambda,
        identifier: Option<String>,
    ) -> hir::Id {
        let mut body = Body::new();
        let lambda_id = self.create_next_id(Some(id), identifier.clone());
        let mut identifiers = self.identifiers.clone();

        for parameter in lambda.parameters.iter() {
            let name = parameter.value.to_string();
            let id = hir::Id::new(
                self.context.input.clone(),
                add_keys(&lambda_id.keys, name.clone()),
            );
            self.id_mapping
                .insert(id.clone(), Some(parameter.id.clone()));
            body.identifiers.insert(id.clone(), name.clone());
            identifiers.insert(name, id);
        }
        let mut inner = Compiler::<'c> {
            context: &mut self.context,
            id_mapping: self.id_mapping.clone(),
            body,
            parent_keys: lambda_id.keys.clone(),
            identifiers,
            public_identifiers: HashMap::new(),
        };

        inner.compile(&lambda.body);
        self.id_mapping = inner.id_mapping;
        self.push_with_existing_id(
            lambda_id.clone(),
            Expression::Lambda(Lambda {
                parameters: lambda
                    .parameters
                    .iter()
                    .map(|parameter| {
                        hir::Id::new(
                            self.context.input.clone(),
                            add_keys(&lambda_id.keys[..], parameter.value.to_string()),
                        )
                    })
                    .collect(),
                body: inner.body,
                fuzzable: lambda.fuzzable,
            }),
            identifier,
        )
    }
    fn lower_struct_access(
        &mut self,
        id: Option<ast::Id>,
        struct_access: &StructAccess,
    ) -> hir::Id {
        let struct_ = self.compile_single(&*struct_access.struct_);
        let key_id = self.push(
            Some(struct_access.key.id.clone()),
            Expression::Symbol(struct_access.key.value.uppercase_first_letter()),
            None,
        );
        let struct_get_id = self.push(None, Expression::Builtin(BuiltinFunction::StructGet), None);
        self.push(
            id,
            Expression::Call {
                function: struct_get_id,
                arguments: vec![struct_, key_id],
            },
            None,
        )
    }
    fn lower_call(&mut self, id: Option<ast::Id>, call: &Call) -> hir::Id {
        let arguments = call
            .arguments
            .iter()
            .map(|argument| self.compile_single(argument))
            .collect_vec();

        let function = match call.receiver.clone() {
            CallReceiver::Identifier(name) => {
                if name.value == "needs" {
                    let expression = match arguments.len() {
                        2 => Expression::Needs {
                            condition: Box::new(arguments.first().unwrap().clone()),
                            message: Box::new(arguments.last().unwrap().clone()),
                        },
                        1 => Expression::Needs {
                            condition: Box::new(arguments.first().unwrap().clone()),
                            message: Box::new(self.push(
                                None,
                                Expression::Text("needs not satisfied".to_string()),
                                None,
                            )),
                        },
                        _ => Expression::Error {
                            child: None,
                            errors: vec![CompilerError {
                                input: name.id.input.clone(),
                                span: self.context.db.ast_id_to_span(name.id.clone()).unwrap(),
                                payload: CompilerErrorPayload::Hir(
                                    HirError::NeedsWithWrongNumberOfArguments,
                                ),
                            }],
                        },
                    };
                    return self.push(id, expression, None);
                }
                match self.identifiers.get(&name.value) {
                    Some(function) => function.to_owned(),
                    None => {
                        return self.push(
                            Some(name.id.clone()),
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
                }
            }
            CallReceiver::StructAccess(struct_access) => {
                self.lower_struct_access(None, &struct_access)
            }
            CallReceiver::Call(call) => self.lower_call(None, &*call),
        };
        self.push(
            id,
            Expression::Call {
                function,
                arguments,
            },
            None,
        )
    }

    fn push(
        &mut self,
        ast_id: Option<ast::Id>,
        expression: Expression,
        identifier: Option<String>,
    ) -> hir::Id {
        let id = self.create_next_id(ast_id, identifier.clone());
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

    fn create_next_id(&mut self, ast_id: Option<ast::Id>, key: Option<String>) -> hir::Id {
        for disambiguator in 0.. {
            let last_part = if let Some(key) = &key {
                if disambiguator == 0 {
                    key.to_string()
                } else {
                    format!("{key}${}", disambiguator - 1)
                }
            } else {
                format!("{}", disambiguator)
            };
            let id = hir::Id::new(
                self.context.input.clone(),
                add_keys(&self.parent_keys, last_part),
            );
            if !self.id_mapping.contains_key(&id) {
                assert!(self.id_mapping.insert(id.to_owned(), ast_id).is_none());
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
