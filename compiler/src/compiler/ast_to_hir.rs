use super::{
    ast::{
        self, Assignment, Ast, AstKind, AstString, Call, Identifier, Int, List, MatchCase, Struct,
        StructAccess, Symbol, Text, TextPart,
    },
    cst::{self, CstDb},
    cst_to_ast::CstToAst,
    error::{CompilerError, CompilerErrorPayload},
    hir::{self, Body, Expression, HirError, Lambda, Pattern, PatternIdentifierId},
    utils::AdjustCasingOfFirstLetter,
};
use crate::{
    builtin_functions::{self, BuiltinFunction},
    module::Module,
    utils::IdGenerator,
};
use itertools::Itertools;
use rustc_hash::FxHashMap;
use std::{mem, ops::Range, sync::Arc};

#[salsa::query_group(AstToHirStorage)]
pub trait AstToHir: CstDb + CstToAst {
    fn hir_to_ast_id(&self, id: hir::Id) -> Option<ast::Id>;
    fn hir_to_cst_id(&self, id: hir::Id) -> Option<cst::Id>;
    fn hir_id_to_span(&self, id: hir::Id) -> Option<Range<usize>>;
    fn hir_id_to_display_span(&self, id: hir::Id) -> Option<Range<usize>>;

    fn ast_to_hir_id(&self, id: ast::Id) -> Option<hir::Id>;
    fn cst_to_hir_id(&self, module: Module, id: cst::Id) -> Option<hir::Id>;

    fn hir(&self, module: Module) -> Option<HirResult>;
}
type HirResult = (Arc<Body>, Arc<FxHashMap<hir::Id, ast::Id>>);

fn hir_to_ast_id(db: &dyn AstToHir, id: hir::Id) -> Option<ast::Id> {
    let (_, hir_to_ast_id_mapping) = db.hir(id.module.clone()).unwrap();
    hir_to_ast_id_mapping.get(&id).cloned()
}
fn hir_to_cst_id(db: &dyn AstToHir, id: hir::Id) -> Option<cst::Id> {
    db.ast_to_cst_id(db.hir_to_ast_id(id)?)
}
fn hir_id_to_span(db: &dyn AstToHir, id: hir::Id) -> Option<Range<usize>> {
    db.ast_id_to_span(db.hir_to_ast_id(id)?)
}
fn hir_id_to_display_span(db: &dyn AstToHir, id: hir::Id) -> Option<Range<usize>> {
    let cst_id = db.hir_to_cst_id(id.clone())?;
    Some(db.find_cst(id.module, cst_id).display_span())
}

fn ast_to_hir_id(db: &dyn AstToHir, id: ast::Id) -> Option<hir::Id> {
    let (_, hir_to_ast_id_mapping) = db.hir(id.module.clone()).unwrap();
    hir_to_ast_id_mapping
        .iter()
        .find_map(|(key, value)| if value == &id { Some(key) } else { None })
        .cloned()
}
fn cst_to_hir_id(db: &dyn AstToHir, module: Module, id: cst::Id) -> Option<hir::Id> {
    let id = db.cst_to_ast_id(module, id)?;
    db.ast_to_hir_id(id)
}

fn hir(db: &dyn AstToHir, module: Module) -> Option<HirResult> {
    let (ast, _) = db.ast(module.clone())?;
    let (body, id_mapping) = compile_top_level(db, module, &ast);
    Some((Arc::new(body), Arc::new(id_mapping)))
}

fn compile_top_level(
    db: &dyn AstToHir,
    module: Module,
    ast: &[Ast],
) -> (Body, FxHashMap<hir::Id, ast::Id>) {
    let mut context = Context {
        module,
        id_mapping: FxHashMap::default(),
        db,
        public_identifiers: FxHashMap::default(),
        body: Body::default(),
        prefix_keys: vec![],
        identifiers: im::HashMap::new(),
        is_top_level: true,
    };

    context.generate_sparkles();
    context.generate_use();
    context.compile(ast);
    context.generate_exports_struct();

    let id_mapping = context
        .id_mapping
        .into_iter()
        .filter_map(|(key, value)| value.map(|value| (key, value)))
        .collect();
    (context.body, id_mapping)
}

struct Context<'a> {
    module: Module,
    id_mapping: FxHashMap<hir::Id, Option<ast::Id>>,
    db: &'a dyn AstToHir,
    public_identifiers: FxHashMap<String, hir::Id>,
    body: Body,
    prefix_keys: Vec<String>,
    identifiers: im::HashMap<String, hir::Id>,
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
            body: mem::take(&mut self.body),
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
    identifiers: im::HashMap<String, hir::Id>,
    non_top_level_reset_state: NonTopLevelResetState,
}

impl<'a> Context<'a> {
    fn compile(&mut self, asts: &[Ast]) -> hir::Id {
        if asts.is_empty() {
            self.push(None, Expression::nothing(), None)
        } else {
            let mut last_id = None;
            for ast in asts {
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
            AstKind::Text(text) => self.lower_text(Some(ast.id.clone()), text),
            AstKind::TextPart(TextPart(string)) => self.push(
                Some(ast.id.clone()),
                Expression::Text(string.value.to_owned()),
                None,
            ),
            AstKind::Identifier(Identifier(name)) => {
                let reference = match self.identifiers.get(&name.value) {
                    Some(reference) => reference.to_owned(),
                    None => {
                        return self.push_error(
                            Some(name.id.clone()),
                            ast.id.module.clone(),
                            self.db.ast_id_to_display_span(ast.id.clone()).unwrap(),
                            HirError::UnknownReference {
                                name: name.value.clone(),
                            },
                        );
                    }
                };
                self.push(Some(ast.id.clone()), Expression::Reference(reference), None)
            }
            AstKind::Symbol(Symbol(symbol)) => self.push(
                Some(ast.id.clone()),
                Expression::Symbol(symbol.value.to_owned()),
                None,
            ),
            AstKind::List(List(items)) => {
                let hir_items = items
                    .iter()
                    .map(|item| self.compile_single(item))
                    .collect_vec();
                self.push(Some(ast.id.clone()), Expression::List(hir_items), None)
            }
            AstKind::Struct(Struct { fields }) => {
                let fields = fields
                    .iter()
                    .map(|(key, value)| {
                        let key = key
                            .as_ref()
                            .map(|key| self.compile_single(key))
                            .unwrap_or_else(|| match &value.kind {
                                AstKind::Identifier(Identifier(name)) => self.push(
                                    Some(value.id.clone()),
                                    Expression::Symbol(name.value.uppercase_first_letter()),
                                    None,
                                ),
                                AstKind::Error { errors, .. } => self.push(
                                    Some(ast.id.clone()),
                                    Expression::Error {
                                        child: None,
                                        // TODO: These errors are already reported for the value itself.
                                        errors: errors.clone(),
                                    },
                                    None,
                                ),
                                _ => panic!(
                                    "Expected identifier in struct shorthand, got {value:?}."
                                ),
                            });
                        (key, self.compile_single(value))
                    })
                    .collect();
                self.push(Some(ast.id.clone()), Expression::Struct(fields), None)
            }
            AstKind::StructAccess(struct_access) => {
                self.lower_struct_access(Some(ast.id.clone()), struct_access)
            }
            AstKind::Lambda(lambda) => self.compile_lambda(ast.id.clone(), lambda, None),
            AstKind::Call(call) => self.lower_call(Some(ast.id.clone()), call),
            AstKind::Assignment(Assignment { is_public, body }) => {
                let (names, body) = match body {
                    ast::AssignmentBody::Lambda { name, lambda } => {
                        let name_string = name.value.to_owned();
                        let body = self.compile_lambda(ast.id.clone(), lambda, Some(name_string));
                        let name_id = self.push(
                            Some(name.id.clone()),
                            Expression::Reference(body.clone()),
                            Some(name.value.to_owned()),
                        );
                        (vec![(name.value.to_owned(), name_id)], body)
                    }
                    ast::AssignmentBody::Body { pattern, body } => {
                        let reset_state = self.start_non_top_level();
                        let body = self.compile(body);
                        self.end_non_top_level(reset_state);

                        let (pattern, identifier_ids) = PatternContext::compile(pattern);
                        let body = self.push(
                            Some(ast.id.clone()),
                            Expression::Destructure {
                                expression: body,
                                pattern,
                            },
                            None,
                        );

                        let names = identifier_ids
                            .into_iter()
                            .sorted_by_key(|(_, (_, identifier_id))| identifier_id.0)
                            .map(|(name, (ast_id, identifier_id))| {
                                let id = self.push(
                                    Some(ast_id),
                                    Expression::PatternIdentifierReference {
                                        destructuring: body.clone(),
                                        identifier_id,
                                    },
                                    Some(name.to_owned()),
                                );
                                (name, id)
                            })
                            .collect_vec();
                        (names, body)
                    }
                };
                if *is_public {
                    if self.is_top_level {
                        for (name, id) in names {
                            if self.public_identifiers.contains_key(&name) {
                                self.push_error(
                                    None,
                                    ast.id.module.clone(),
                                    self.db.ast_id_to_display_span(ast.id.clone()).unwrap(),
                                    HirError::PublicAssignmentWithSameName {
                                        name: name.to_owned(),
                                    },
                                );
                            }
                            self.public_identifiers.insert(name, id);
                        }
                    } else {
                        self.push_error(
                            None,
                            ast.id.module.clone(),
                            self.db.ast_id_to_display_span(ast.id.clone()).unwrap(),
                            HirError::PublicAssignmentInNotTopLevel,
                        );
                    }
                }
                body
            }
            AstKind::Match(ast::Match { expression, cases }) => {
                let expression = self.compile_single(expression);

                let cases = cases.iter().for_each(|case| match case.kind {
                    AstKind::MatchCase(MatchCase { box pattern, body }) => {
                        let (pattern, pattern_identifiers) = PatternContext::compile(&pattern);

                        let reset_state = self.start_scope();
                        for (name, (ast_id, identifier_id)) in pattern_identifiers {
                            self.push(
                                Some(ast_id),
                                Expression::PatternIdentifierReference {
                                    destructuring: expression.clone(),
                                    identifier_id,
                                },
                                Some(name.to_owned()),
                            );
                        }
                        let body = self.compile(body.as_ref());
                        self.end_scope(reset_state);
                    }
                    AstKind::Error { errors, .. } => (
                        Pattern::Error {
                            child: None,
                            errors,
                        },
                        Body::default(),
                    ),
                    _ => unreachable!("Expected match case in match cases, got {value:?}."),
                });
                let hir_cases = vec![];
            }
            AstKind::MatchCase(_) => {
                unreachable!("Match cases should be handled in match directly.")
            }
            AstKind::Error { child, errors } => {
                let child = child.as_ref().map(|child| self.compile_single(child));
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

    fn lower_text(&mut self, id: Option<ast::Id>, text: &Text) -> hir::Id {
        // TODO: Convert parts to text
        let builtin_text_concatenate = self.push(
            None,
            Expression::Builtin(BuiltinFunction::TextConcatenate),
            None,
        );

        let compiled_parts = text
            .0
            .iter()
            .map(|part| self.compile_single(part))
            .collect_vec();

        compiled_parts
            .into_iter()
            .reduce(|left, right| {
                self.push(
                    None,
                    Expression::Call {
                        function: builtin_text_concatenate.clone(),
                        arguments: vec![left, right],
                    },
                    None,
                )
            })
            .unwrap_or_else(|| self.push(id, Expression::Text("".to_string()), None))
    }

    fn compile_lambda(
        &mut self,
        id: ast::Id,
        lambda: &ast::Lambda,
        identifier: Option<String>,
    ) -> hir::Id {
        let assignment_reset_state = self.start_scope();
        let lambda_id = self.create_next_id(Some(id), identifier);

        for parameter in lambda.parameters.iter() {
            let name = parameter.value.to_string();
            let id = hir::Id::new(self.module.clone(), add_keys(&lambda_id.keys, name.clone()));
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
                            self.module.clone(),
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
        let struct_ = self.compile_single(&struct_access.struct_);
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
        let function = match &call.receiver.kind {
            AstKind::Identifier(Identifier(AstString {
                id: name_id,
                value: name,
            })) if name == "needs" => {
                let expression = match &self.lower_call_arguments(&call.arguments[..])[..] {
                    [condition, reason] => Expression::Needs {
                        condition: condition.clone(),
                        reason: reason.clone(),
                    },
                    [condition] => Expression::Needs {
                        condition: condition.clone(),
                        reason: self.push(
                            None,
                            Expression::Text(
                                match self.db.ast_id_to_span(call.arguments[0].id.clone()) {
                                    Some(span) => format!(
                                        "`{}` was not satisfied",
                                        &self
                                            .db
                                            .get_module_content_as_string(
                                                call.arguments[0].id.module.clone()
                                            )
                                            .unwrap()[span],
                                    ),
                                    None => "the needs of a function were not met".to_string(),
                                },
                            ),
                            None,
                        ),
                    },
                    _ => {
                        return self.push_error(
                            id,
                            name_id.module.clone(),
                            self.db.ast_id_to_span(name_id.to_owned()).unwrap(),
                            HirError::NeedsWithWrongNumberOfArguments {
                                num_args: call.arguments.len(),
                            },
                        );
                    }
                };
                return self.push(id, expression, None);
            }
            _ => self.compile_single(call.receiver.as_ref()),
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
        module: Module,
        span: Range<usize>,
        error: HirError,
    ) -> hir::Id {
        self.push(
            ast_id,
            Expression::Error {
                child: None,
                errors: vec![CompilerError {
                    module,
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
            let id = hir::Id::new(self.module.clone(), add_keys(&self.prefix_keys, last_part));
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
        let mut sparkles_map = FxHashMap::default();

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

    fn generate_use(&mut self) {
        // HirId(~:test.candy:use) = lambda { HirId(~:test.candy:use:relativePath) ->
        //   HirId(~:test.candy:use:importedFileContent) = useModule
        //     currently in ~:test.candy:use:importedFileContent
        //     relative path: HirId(~:test.candy:use:relativePath)
        //  }

        let reset_state = self.start_scope();
        self.prefix_keys.push("use".to_string());
        let relative_path = hir::Id::new(
            self.module.clone(),
            add_keys(&self.prefix_keys[..], "relativePath".to_string()),
        );

        self.push(
            None,
            Expression::UseModule {
                current_module: self.module.clone(),
                relative_path: relative_path.clone(),
            },
            Some("importedModule".to_string()),
        );

        let inner_body = self.end_scope(reset_state);

        self.push(
            None,
            Expression::Lambda(Lambda {
                parameters: vec![relative_path],
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

        let mut exports = FxHashMap::default();
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

/// The `ast::Id` is the ID of the first occurrence of this identifier in the
/// AST.
type PatternIdentifierIds = FxHashMap<String, (ast::Id, PatternIdentifierId)>;

#[derive(Default)]
struct PatternContext {
    identifier_id_generator: IdGenerator<PatternIdentifierId>,
    identifier_ids: PatternIdentifierIds,
}
impl PatternContext {
    fn compile(ast: &Ast) -> (Pattern, PatternIdentifierIds) {
        let mut context = PatternContext::default();
        let pattern = context.compile_pattern(ast);
        (pattern, context.identifier_ids)
    }

    fn compile_pattern(&mut self, ast: &Ast) -> Pattern {
        match &ast.kind {
            AstKind::Int(Int(int)) => Pattern::Int(int.to_owned()),
            AstKind::Text(Text(text)) => Pattern::Text(
                text.iter()
                    .map(|part| match &part.kind {
                        AstKind::TextPart(TextPart(string)) => string.value.to_owned(),
                        _ => panic!("AST pattern can't contain text interpolations."),
                    })
                    .join(""),
            ),
            AstKind::TextPart(_) => unreachable!("TextPart should not occur in AST patterns."),
            AstKind::Identifier(Identifier(name)) => {
                let (_, pattern_id) = self
                    .identifier_ids
                    .entry(name.value.to_owned())
                    .or_insert_with(|| {
                        (ast.id.to_owned(), self.identifier_id_generator.generate())
                    });
                Pattern::NewIdentifier(pattern_id.to_owned())
            }
            AstKind::Symbol(Symbol(symbol)) => Pattern::Symbol(symbol.value.to_owned()),
            AstKind::List(List(items)) => {
                let items = items
                    .iter()
                    .map(|item| self.compile_pattern(item))
                    .collect_vec();
                Pattern::List(items)
            }
            AstKind::Struct(Struct { fields }) => {
                let fields = fields
                    .iter()
                    .map(|(key, value)| {
                        let key = key
                            .as_ref()
                            .map(|key| self.compile_pattern(key))
                            .unwrap_or_else(|| match &value.kind {
                                AstKind::Identifier(Identifier(name)) => {
                                    Pattern::Symbol(name.value.uppercase_first_letter())
                                }
                                AstKind::Error { errors, .. } => Pattern::Error {
                                    child: None,
                                    // TODO: These errors are already reported for the value itself.
                                    errors: errors.to_owned(),
                                },
                                _ => panic!(
                                    "Expected identifier in struct shorthand, got {value:?}."
                                ),
                            });
                        (key, self.compile_pattern(value))
                    })
                    .collect();
                Pattern::Struct(fields)
            }
            AstKind::StructAccess(_)
            | AstKind::Lambda(_)
            | AstKind::Call(_)
            | AstKind::Assignment(_)
            | AstKind::Match(_)
            | AstKind::MatchCase(_) => {
                unreachable!(
                    "AST pattern can't contain struct access, lambda, call, assignment, match, or match case."
                )
            }
            AstKind::Error { child, errors, .. } => {
                let child = child
                    .as_ref()
                    .map(|child| Box::new(self.compile_pattern(child)));
                Pattern::Error {
                    child,
                    errors: errors.to_owned(),
                }
            }
        }
    }
}
