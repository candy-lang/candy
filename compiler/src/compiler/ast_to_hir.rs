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
use std::{mem, ops::Range, sync::Arc};

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
    let (body, id_mapping) = compile_top_level(db, input, &ast);
    Some((Arc::new(body), id_mapping))
}

fn compile_top_level(
    db: &dyn AstToHir,
    input: Input,
    ast: &[Ast],
) -> (Body, HashMap<hir::Id, ast::Id>) {
    let mut context = Context {
        input,
        id_mapping: HashMap::new(),
        db,
        public_identifiers: HashMap::new(),
        body: Body::new(),
        prefix_keys: vec![],
        identifiers: HashMap::new(),
        is_top_level: true,
    };

    context.generate_sparkles();
    context.generate_use_asset();
    context.generate_use();
    context.compile(&mut &ast);
    context.generate_exports_struct();

    let id_mapping = context
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
    (context.body, id_mapping)
}

struct Context<'a> {
    input: Input,
    id_mapping: HashMap<hir::Id, Option<ast::Id>>,
    db: &'a dyn AstToHir,
    public_identifiers: HashMap<String, hir::Id>,
    body: Body,
    prefix_keys: Vec<String>,
    identifiers: HashMap<String, hir::Id>,
    is_top_level: bool,
}

impl<'a> Context<'a> {
    fn start_non_top_level(&mut self) -> NonTopLevelResetState {
        NonTopLevelResetState(mem::replace(&mut self.is_top_level, false))
    }
    fn end_non_top_level(&mut self, reset_state: NonTopLevelResetState) {
        self.is_top_level = reset_state.0;
    }
}
struct NonTopLevelResetState(bool);

impl<'a> Context<'a> {
    fn start_scope(&mut self) -> ScopeResetState {
        ScopeResetState {
            body: mem::replace(&mut self.body, Body::new()),
            prefix_keys: self.prefix_keys.clone(),
            identifiers: self.identifiers.clone(),
            non_top_level_reset_state: self.start_non_top_level(),
        }
    }
    fn end_scope(&mut self, reset_state: ScopeResetState) -> Body {
        let inner_body = mem::replace(&mut self.body, reset_state.body);
        self.prefix_keys = reset_state.prefix_keys;
        self.identifiers = reset_state.identifiers;
        self.end_non_top_level(reset_state.non_top_level_reset_state);
        inner_body
    }
}
struct ScopeResetState {
    body: Body,
    prefix_keys: Vec<String>,
    identifiers: HashMap<String, hir::Id>,
    non_top_level_reset_state: NonTopLevelResetState,
}

impl<'a> Context<'a> {
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
                        return self.push_error(
                            Some(symbol.id.clone()),
                            ast.id.input.clone(),
                            self.db.ast_id_to_span(ast.id.clone()).unwrap(),
                            HirError::UnknownReference {
                                symbol: symbol.value.clone(),
                            },
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
                let name_string = name.value.to_owned();
                let body = match body {
                    ast::AssignmentBody::Lambda(lambda) => {
                        self.compile_lambda(ast.id.clone(), lambda, Some(name_string.clone()))
                    }
                    ast::AssignmentBody::Body(body) => {
                        let reset_state = self.start_non_top_level();
                        let body = self.compile(body);
                        self.end_non_top_level(reset_state);

                        self.push(Some(ast.id.clone()), Expression::Reference(body), None)
                    }
                };
                self.push(
                    Some(name.id.clone()),
                    Expression::Reference(body.clone()),
                    Some(name_string.clone()),
                );
                if *is_public {
                    if self.is_top_level {
                        if self.public_identifiers.contains_key(&name_string) {
                            self.push_error(
                                None,
                                ast.id.input.clone(),
                                self.db.ast_id_to_span(ast.id.clone()).unwrap(),
                                HirError::PublicAssignmentWithSameName {
                                    name: name_string.clone(),
                                },
                            );
                        }
                        self.public_identifiers.insert(name_string, body.clone());
                    } else {
                        self.push_error(
                            None,
                            ast.id.input.clone(),
                            self.db.ast_id_to_span(ast.id.clone()).unwrap(),
                            HirError::PublicAssignmentInNotTopLevel,
                        );
                    }
                }
                body
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
        let assignment_reset_state = self.start_scope();
        let lambda_id = self.create_next_id(Some(id), identifier.clone());

        for parameter in lambda.parameters.iter() {
            let name = parameter.value.to_string();
            let id = hir::Id::new(self.input.clone(), add_keys(&lambda_id.keys, name.clone()));
            self.id_mapping
                .insert(id.clone(), Some(parameter.id.clone()));
            self.body.identifiers.insert(id.clone(), name.clone());
            self.identifiers.insert(name, id);
        }

        let lambda_reset_state = self.start_scope();
        self.prefix_keys
            .push(lambda_id.keys.last().unwrap().clone());

        self.compile(&lambda.body);

        let inner_body = self.end_scope(lambda_reset_state);
        self.end_scope(assignment_reset_state);

        self.push_with_existing_id(
            lambda_id.clone(),
            Expression::Lambda(Lambda {
                parameters: lambda
                    .parameters
                    .iter()
                    .map(|parameter| {
                        hir::Id::new(
                            self.input.clone(),
                            add_keys(&lambda_id.keys[..], parameter.value.to_string()),
                        )
                    })
                    .collect(),
                body: inner_body,
                fuzzable: lambda.fuzzable,
            }),
            None,
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
        let function = match call.receiver.clone() {
            CallReceiver::Identifier(name) => {
                if name.value == "needs" {
                    let expression = match &self.lower_call_arguments(&call.arguments[..])[..] {
                        [condition, reason] => Expression::Needs {
                            condition: Box::new(condition.clone()),
                            reason: Box::new(reason.clone()),
                        },
                        [condition] => Expression::Needs {
                            condition: Box::new(condition.clone()),
                            reason: Box::new(self.push(
                                None,
                                Expression::Text("needs not satisfied".to_string()),
                                None,
                            )),
                        },
                        _ => Expression::Error {
                            child: None,
                            errors: vec![CompilerError {
                                input: name.id.input.clone(),
                                span: self.db.ast_id_to_span(name.id.clone()).unwrap(),
                                payload: CompilerErrorPayload::Hir(
                                    HirError::NeedsWithWrongNumberOfArguments,
                                ),
                            }],
                        },
                    };
                    return self.push(id, expression, None);
                }

                match self.identifiers.get(&name.value).map(|id| id.clone()) {
                    Some(function) => {
                        self.push(Some(name.id), Expression::Reference(function), None)
                    }
                    None => {
                        return self.push_error(
                            Some(name.id.clone()),
                            name.id.input.clone(),
                            self.db.ast_id_to_span(name.id.clone()).unwrap(),
                            HirError::UnknownFunction {
                                name: name.value.clone(),
                            },
                        );
                    }
                }
            }
            CallReceiver::StructAccess(struct_access) => {
                self.lower_struct_access(None, &struct_access)
            }
            CallReceiver::Call(call) => self.lower_call(None, &*call),
        };
        let arguments = self.lower_call_arguments(&call.arguments[..]);
        self.push(
            id,
            Expression::Call {
                function,
                arguments,
            },
            None,
        )
    }
    fn lower_call_arguments(&mut self, arguments: &[Ast]) -> Vec<hir::Id> {
        arguments
            .iter()
            .map(|argument| self.compile_single(argument))
            .collect_vec()
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
    fn push_error(
        &mut self,
        ast_id: Option<ast::Id>,
        input: Input,
        span: Range<usize>,
        error: HirError,
    ) -> hir::Id {
        self.push(
            ast_id,
            Expression::Error {
                child: None,
                errors: vec![CompilerError {
                    input,
                    span,
                    payload: CompilerErrorPayload::Hir(error),
                }],
            },
            None,
        )
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
            let id = hir::Id::new(self.input.clone(), add_keys(&self.prefix_keys, last_part));
            if !self.id_mapping.contains_key(&id) {
                assert!(self.id_mapping.insert(id.to_owned(), ast_id).is_none());
                return id;
            }
        }
        unreachable!()
    }
}

impl<'a> Context<'a> {
    fn generate_sparkles(&mut self) {
        let mut sparkles_map = HashMap::new();

        for builtin_function in builtin_functions::VALUES.iter() {
            let symbol = self.push(
                None,
                Expression::Symbol(format!("{builtin_function:?}")),
                None,
            );
            let builtin = self.push(None, Expression::Builtin(*builtin_function), None);
            sparkles_map.insert(symbol, builtin);
        }

        let sparkles_map = Expression::Struct(sparkles_map);
        self.push(None, sparkles_map, Some("✨".to_string()));
    }

    fn generate_panicking_code(&mut self, reason: String) -> hir::Id {
        let condition = self.push(
            None,
            Expression::Symbol("False".to_string()),
            Some("false".to_string()),
        );
        let reason = self.push(None, Expression::Text(reason), Some("reason".to_string()));
        self.push(
            None,
            Expression::Needs {
                condition: Box::new(condition),
                reason: Box::new(reason),
            },
            None,
        )
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

        match self.input.clone() {
            Input::File(path) => {
                let current_path_content = path
                    .into_iter()
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
            Input::ExternalFile(_) => self.generate_panicking_code(
                "file doesn't belong to the currently opened project.".to_string(),
            ),
            Input::Untitled(_) => self.generate_panicking_code(
                "untitled files can't call `use` or `useAsset`.".to_string(),
            ),
        }
    }

    fn generate_use_asset(&mut self) {
        // HirId(~:test.candy:useAsset) = lambda { HirId(~:test.candy:useAsset:target) ->
        //   HirId(~:test.candy:useAsset:key) = int 0
        //   HirId(~:test.candy:useAsset:raw_path) = text "test.candy"
        //   HirId(~:test.candy:useAsset:currentPath) = struct [
        //     HirId(~:test.candy:useAsset:key): HirId(~:test.candy:useAsset:raw_path),
        //   ]
        //   HirId(~:test.candy:useAsset:useAsset) = builtinUseAsset
        //   HirId(~:test.candy:useAsset:importedFileContent) = call HirId(~:test.candy:useAsset:useAsset) with these arguments:
        //     HirId(~:test.candy:useAsset:currentPath)
        //     HirId(~:test.candy:useAsset:target)
        // }

        let reset_state = self.start_scope();
        self.prefix_keys.push("useAsset".to_string());
        let lambda_parameter_id = hir::Id::new(
            self.input.clone(),
            add_keys(&self.prefix_keys[..], "target".to_string()),
        );

        let current_path = self.generate_current_path_struct();
        let use_id = self.push(
            None,
            Expression::Builtin(BuiltinFunction::UseAsset),
            Some("useAsset".to_string()),
        );
        self.push(
            None,
            Expression::Call {
                function: use_id,
                arguments: vec![current_path, lambda_parameter_id.clone()],
            },
            Some("importedFileContent".to_string()),
        );

        let inner_body = self.end_scope(reset_state);

        self.push(
            None,
            Expression::Lambda(Lambda {
                parameters: vec![lambda_parameter_id],
                body: inner_body,
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

        let reset_state = self.start_scope();
        self.prefix_keys.push("use".to_string());
        let lambda_parameter_id = hir::Id::new(
            self.input.clone(),
            add_keys(&self.prefix_keys[..], "target".to_string()),
        );

        let current_path = self.generate_current_path_struct();
        let use_id = self.push(
            None,
            Expression::Builtin(BuiltinFunction::UseLocalModule),
            Some("useLocalModule".to_string()),
        );
        self.push(
            None,
            Expression::Call {
                function: use_id,
                arguments: vec![current_path, lambda_parameter_id.clone()],
            },
            Some("importedModule".to_string()),
        );

        let inner_body = self.end_scope(reset_state);

        self.push(
            None,
            Expression::Lambda(Lambda {
                parameters: vec![lambda_parameter_id],
                body: inner_body,
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
}

fn add_keys(parents: &[String], id: String) -> Vec<String> {
    parents
        .iter()
        .map(|it| it.to_string())
        .chain(vec![id])
        .collect()
}
