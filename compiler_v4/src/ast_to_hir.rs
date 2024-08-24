use crate::{
    ast::{
        Ast, AstArguments, AstAssignment, AstBody, AstCall, AstDeclaration, AstEnum, AstExpression,
        AstFunction, AstParameter, AstResult, AstStatement, AstStruct, AstSwitch, AstTextPart,
        AstType, AstTypeParameter, AstTypeParameters,
    },
    error::CompilerError,
    hir::{
        Assignment, Body, BodyOrBuiltin, BuiltinFunction, Expression, ExpressionKind, Function,
        Hir, Id, Parameter, SwitchCase, Type, TypeDeclaration, TypeParameter, TypeParameterId,
    },
    id::IdGenerator,
    position::Offset,
    utils::HashMapExtension,
};
use itertools::{Itertools, Position};
use petgraph::{
    algo::toposort,
    graph::{DiGraph, NodeIndex},
};
use rustc_hash::{FxHashMap, FxHashSet};
use std::{collections::hash_map::Entry, iter, ops::Range, path::Path};
use strum::VariantArray;

pub fn ast_to_hir(path: &Path, ast: &Ast) -> (Hir, Vec<CompilerError>) {
    let mut context = Context::new(path, ast);
    context.add_builtin_functions();
    context.lower_declarations();
    context.into_hir()
}

#[derive(Debug)]
struct Context<'a> {
    path: &'a Path,
    ast: &'a Ast,
    id_generator: IdGenerator<Id>,
    type_parameter_id_generator: IdGenerator<TypeParameterId>,
    global_identifiers: FxHashMap<Box<str>, Named>,
    assignments: FxHashMap<Id, AssignmentDeclaration<'a>>,
    assignment_dependency_graph: DiGraph<Id, ()>,
    functions: FxHashMap<Id, FunctionDeclaration<'a>>,
    errors: Vec<CompilerError>,
    hir: Hir,
}
#[derive(Debug, Eq, PartialEq)]
enum Named {
    Assignment(Id),
    Functions(Vec<Id>),
}
#[derive(Debug)]
struct AssignmentDeclaration<'a> {
    ast: &'a AstAssignment,
    type_: Type,
    graph_index: NodeIndex,
    body: Option<Body>,
}
#[derive(Debug)]
struct FunctionDeclaration<'a> {
    ast: Option<&'a AstFunction>,
    name: Box<str>,
    type_parameters: Box<[TypeParameter]>,
    parameters: Box<[Parameter]>,
    return_type: Type,
    body: Option<BodyOrBuiltin>,
}
impl<'a> FunctionDeclaration<'a> {
    fn signature_to_string(&self) -> String {
        format!(
            "{}{}({})",
            self.name,
            if self.type_parameters.is_empty() {
                String::new()
            } else {
                format!(
                    "[{}]",
                    self.type_parameters.iter().map(|it| &it.name).join(", ")
                )
            },
            self.parameters
                .iter()
                .map(|it| format!("{}: {}", it.name, it.type_))
                .join(", "),
        )
    }

    fn call_signature_to_string(function_name: &str, argument_types: &[Type]) -> String {
        format!("{}({})", function_name, argument_types.iter().join(", "))
    }
}

impl<'a> Context<'a> {
    fn new(path: &'a Path, ast: &'a Ast) -> Self {
        Self {
            path,
            ast,
            id_generator: IdGenerator::start_at(BuiltinFunction::VARIANTS.len()),
            type_parameter_id_generator: IdGenerator::default(),
            global_identifiers: FxHashMap::default(),
            assignments: FxHashMap::default(),
            assignment_dependency_graph: DiGraph::new(),
            functions: FxHashMap::default(),
            errors: vec![],
            hir: Hir::default(),
        }
    }

    fn into_hir(mut self) -> (Hir, Vec<CompilerError>) {
        self.hir.main_function_id = self.find_main_function().unwrap_or_default();

        if let Err(cycle) = toposort(&self.assignment_dependency_graph, None) {
            let id = *self
                .assignment_dependency_graph
                .node_weight(cycle.node_id())
                .unwrap();
            self.add_error(
                // TODO: report actual error location
                Offset(0)..Offset(0),
                // TODO: print full cycle
                format!(
                    "Cycle in global assignments including `{}`",
                    self.global_identifiers
                        .iter()
                        .find(|(_, named)| *named == &Named::Assignment(id))
                        .unwrap()
                        .0,
                ),
            );
        }

        let mut assignments = vec![];
        let mut functions = vec![];
        for (name, named) in self.global_identifiers {
            match named {
                Named::Assignment(id) => {
                    let AssignmentDeclaration { type_, body, .. } =
                        self.assignments.remove(&id).unwrap();
                    assignments.push((
                        id,
                        name.clone(),
                        Assignment {
                            type_,
                            body: body.unwrap(),
                        },
                    ));
                }
                Named::Functions(ids) => {
                    for id in ids {
                        let FunctionDeclaration {
                            type_parameters,
                            parameters,
                            return_type,
                            body,
                            ..
                        } = self.functions.remove(&id).unwrap();
                        functions.push((
                            id,
                            name.clone(),
                            Function {
                                type_parameters,
                                parameters,
                                return_type,
                                body: body.unwrap(),
                            },
                        ));
                    }
                }
            };
        }
        self.hir.assignments = assignments.into();
        self.hir.functions = functions.into();

        (self.hir, self.errors)
    }
    fn find_main_function(&mut self) -> Option<Id> {
        if let Some(named) = self.global_identifiers.get("main") {
            match named {
                Named::Assignment(assignment) => {
                    let span = self.assignments[assignment]
                        .ast
                        .name
                        .value()
                        .unwrap()
                        .span
                        .clone();
                    self.add_error(span, "`main` must be a function");
                    None
                }
                Named::Functions(ids) => {
                    assert!(!ids.is_empty());

                    let function = &self.functions[ids.first().unwrap()];
                    if ids.len() > 1 {
                        self.add_error(
                            function.ast.unwrap().name.value().unwrap().span.clone(),
                            "Main function may not be overloaded",
                        );
                        None
                    } else if !function.parameters.is_empty() {
                        self.add_error(
                            function.ast.unwrap().name.value().unwrap().span.clone(),
                            "Main function must not have parameters",
                        );
                        None
                    } else if function.return_type != Type::Error
                        && function.return_type != Type::int()
                    {
                        self.add_error(
                            function.ast.unwrap().name.value().unwrap().span.clone(),
                            "Main function must return an Int",
                        );
                        None
                    } else {
                        Some(ids[0])
                    }
                }
            }
        } else {
            self.add_error(Offset(0)..Offset(0), "Program is missing a main function");
            None
        }
    }

    fn add_builtin_functions(&mut self) {
        for builtin_function in BuiltinFunction::VARIANTS {
            let id = builtin_function.id();
            let signature = builtin_function.signature();
            let type_parameters = signature
                .type_parameters
                .into_vec()
                .into_iter()
                .map(|name| TypeParameter {
                    id: self.type_parameter_id_generator.generate(),
                    name,
                })
                .collect::<Box<_>>();
            let parameters = signature
                .parameters
                .into_vec()
                .into_iter()
                .map(|(name, type_)| Parameter {
                    id: self.id_generator.generate(),
                    name,
                    type_,
                })
                .collect::<Box<_>>();
            self.functions.force_insert(
                id,
                FunctionDeclaration {
                    ast: None,
                    name: signature.name.clone(),
                    type_parameters,
                    parameters,
                    return_type: signature.return_type,
                    body: Some(BodyOrBuiltin::Builtin(*builtin_function)),
                },
            );
            self.global_identifiers
                .force_insert(signature.name, Named::Functions(vec![id]));
        }
    }

    fn lower_declarations(&mut self) {
        let mut assignments_to_lower = vec![];
        let mut functions_to_lower = vec![];
        for declaration in self.ast {
            match declaration {
                AstDeclaration::Struct(struct_) => self.lower_struct(struct_),
                AstDeclaration::Enum(enum_) => self.lower_enum(enum_),
                AstDeclaration::Assignment(assignment) => {
                    if let Some(id) = self.lower_assignment_signature(assignment) {
                        assignments_to_lower.push(id);
                    }
                }
                AstDeclaration::Function(function) => {
                    if let Some(id) = self.lower_function_signature(function) {
                        functions_to_lower.push(id);
                    }
                }
            }
        }
        for id in assignments_to_lower {
            self.lower_assignment(id);
        }
        for id in functions_to_lower {
            self.lower_function(id);
        }
    }

    fn lower_struct(&mut self, struct_type: &'a AstStruct) {
        let Some(name) = struct_type.name.value() else {
            return;
        };

        let type_parameters = self.lower_type_parameters(struct_type.type_parameters.as_ref());

        let fields = struct_type
            .fields
            .iter()
            .filter_map(|field| {
                let name = field.name.value()?;

                let type_ = self.lower_type(&type_parameters, field.type_.value());
                Some((name.string.clone(), type_))
            })
            .collect();

        match self.hir.type_declarations.entry(name.string.clone()) {
            Entry::Occupied(_) => {
                self.add_error(name.span.clone(), "Duplicate type name");
            }
            Entry::Vacant(entry) => {
                entry.insert(TypeDeclaration::Struct {
                    type_parameters,
                    fields,
                });
            }
        };
    }
    fn lower_enum(&mut self, enum_type: &'a AstEnum) {
        let Some(name) = enum_type.name.value() else {
            return;
        };

        let type_parameters = self.lower_type_parameters(enum_type.type_parameters.as_ref());

        let variants = enum_type
            .variants
            .iter()
            .filter_map(|variant| {
                let name = variant.name.value()?;

                let type_ = variant
                    .type_
                    .as_ref()
                    .map(|it| self.lower_type(&type_parameters, it.value()));
                Some((name.string.clone(), type_))
            })
            .collect();

        match self.hir.type_declarations.entry(name.string.clone()) {
            Entry::Occupied(_) => {
                self.add_error(name.span.clone(), "Duplicate type name");
            }
            Entry::Vacant(entry) => {
                entry.insert(TypeDeclaration::Enum {
                    type_parameters,
                    variants,
                });
            }
        };
    }

    fn lower_type_parameters(
        &mut self,
        type_parameters: Option<&AstTypeParameters>,
    ) -> Box<[TypeParameter]> {
        type_parameters.map_or_else(Box::default, |it| {
            it.parameters
                .iter()
                .filter_map(|it| {
                    let name = it.name.value()?;
                    let id = self.type_parameter_id_generator.generate();
                    Some(TypeParameter {
                        id,
                        name: name.string.clone(),
                    })
                })
                .collect()
        })
    }
    fn lower_type(
        &mut self,
        type_parameters: &[TypeParameter],
        type_: impl Into<Option<&AstType>>,
    ) -> Type {
        let type_: Option<&AstType> = type_.into();
        let Some(type_) = type_ else {
            return Type::Error;
        };

        let Some(name) = type_.name.value() else {
            return Type::Error;
        };

        if let Some((name, id)) = Self::resolve_type_parameter(type_parameters, &name.string) {
            if let Some(type_arguments) = &type_.type_arguments {
                self.add_error(
                    type_arguments.span.clone(),
                    "Type parameters can't have type arguments",
                );
            }
            return Type::Parameter { name, id };
        }

        let type_arguments = type_
            .type_arguments
            .as_ref()
            .map_or_else(Box::default, |it| {
                it.arguments
                    .iter()
                    .map(|it| self.lower_type(type_parameters, &it.type_))
                    .collect::<Box<_>>()
            });

        if &*name.string == "Int" {
            if !type_arguments.is_empty() {
                self.add_error(
                    type_.type_arguments.as_ref().unwrap().span.clone(),
                    "Int does not take type arguments",
                );
            }
            return Type::int();
        }
        if &*name.string == "Text" {
            if !type_arguments.is_empty() {
                self.add_error(
                    type_.type_arguments.as_ref().unwrap().span.clone(),
                    "Text does not take type arguments",
                );
            }
            return Type::text();
        }

        let Some(type_parameters) = self.ast.iter().find_map(|it| match it {
            AstDeclaration::Struct(AstStruct {
                name: it_name,
                type_parameters,
                ..
            })
            | AstDeclaration::Enum(AstEnum {
                name: it_name,
                type_parameters,
                ..
            }) if it_name.value().map(|it| &it.string) == Some(&name.string) => Some(
                type_parameters
                    .as_ref()
                    .map_or::<&[AstTypeParameter], _>(&[], |it| &it.parameters),
            ),
            _ => None,
        }) else {
            self.add_error(name.span.clone(), format!("Unknown type: `{}`", **name));
            return Type::Error;
        };

        let type_arguments: Box<[Type]> = if type_arguments.len() == type_parameters.len() {
            type_arguments
        } else {
            self.add_error(
                type_.type_arguments.as_ref().unwrap().span.clone(),
                format!(
                    "Expected {} type {}, got {}.",
                    type_parameters.len(),
                    if type_parameters.len() == 1 {
                        "argument"
                    } else {
                        "arguments"
                    },
                    type_arguments.len(),
                ),
            );
            if type_arguments.len() < type_parameters.len() {
                let missing_count = type_parameters.len() - type_arguments.len();
                type_arguments
                    .into_vec()
                    .into_iter()
                    .chain(iter::repeat_n(Type::Error, missing_count))
                    .collect()
            } else {
                let mut type_arguments = type_arguments.into_vec();
                type_arguments.truncate(type_parameters.len());
                type_arguments.into_boxed_slice()
            }
        };

        Type::Named {
            name: name.string.clone(),
            type_arguments,
        }
    }
    fn resolve_type_parameter(
        type_parameters: &[TypeParameter],
        name: &str,
    ) -> Option<(Box<str>, TypeParameterId)> {
        type_parameters
            .iter()
            .find(|it| &*it.name == name)
            .map(|it| (it.name.clone(), it.id))
    }

    fn lower_assignment_signature(&mut self, assignment: &'a AstAssignment) -> Option<Id> {
        let name = assignment.name.value()?;

        let id = self.id_generator.generate();
        // TODO: infer type
        let type_ = assignment
            .type_
            .as_ref()
            .map_or(Type::Error, |it| self.lower_type(&[], it.value()));

        match self.global_identifiers.entry(name.string.clone()) {
            Entry::Occupied(mut entry) => {
                self.errors.push(CompilerError {
                    path: self.path.to_path_buf(),
                    span:
                    name.span.clone(),
                    message: match entry.get_mut() {
                Named::Functions(_) => "A top-level assignment can't have the same name as a top-level function.".to_string(),
                Named::Assignment(_) => "Top-level assignments can't have the same name.".to_string(),
                    },
                });
                return None;
            }
            Entry::Vacant(entry) => {
                entry.insert(Named::Assignment(id));
            }
        }

        let graph_index = self.assignment_dependency_graph.add_node(id);

        self.assignments.force_insert(
            id,
            AssignmentDeclaration {
                ast: assignment,
                type_,
                graph_index,
                body: None,
            },
        );
        Some(id)
    }
    fn lower_assignment(&mut self, id: Id) {
        let declaration = self.assignments.get(&id).unwrap();
        let value = declaration.ast.value.clone();
        let type_ = declaration.type_.clone();
        let graph_index = declaration.graph_index;

        let (hir_body, global_assignment_dependencies) = BodyBuilder::build(self, &[], |builder| {
            if let Some(value) = value.value() {
                builder.lower_expression(value, Some(&type_));
            } else {
                builder.push_error();
            }
        });

        for dependency_id in global_assignment_dependencies {
            let dependency = self.assignments.get(&dependency_id).unwrap();
            self.assignment_dependency_graph
                .add_edge(graph_index, dependency.graph_index, ());
        }

        self.assignments.get_mut(&id).unwrap().body = Some(hir_body);
    }

    fn lower_function_signature(&mut self, function: &'a AstFunction) -> Option<Id> {
        let name = function.name.value()?;

        let id = self.id_generator.generate();

        let type_parameters = self.lower_type_parameters(function.type_parameters.as_ref());

        let parameters = self.lower_parameters(&type_parameters, &function.parameters);
        let return_type = function
            .return_type
            .as_ref()
            .map_or_else(Type::nothing, |it| self.lower_type(&type_parameters, it));
        match self.global_identifiers.entry(name.string.clone()) {
            Entry::Occupied(mut entry) => match entry.get_mut() {
                Named::Functions(functions) => {
                    // TODO: check for invalid overloads
                    functions.push(id);
                }
                Named::Assignment(_) => {
                    self.add_error(
                        name.span.clone(),
                        "A top-level function can't have the same name as a top-level assignment.",
                    );
                    return None;
                }
            },
            Entry::Vacant(entry) => {
                entry.insert(Named::Functions(vec![id]));
            }
        }
        self.functions.force_insert(
            id,
            FunctionDeclaration {
                ast: Some(function),
                name: name.string.clone(),
                type_parameters,
                parameters,
                return_type,
                body: None,
            },
        );
        Some(id)
    }
    fn lower_parameters(
        &mut self,
        type_parameters: &[TypeParameter],
        parameters: &'a [AstParameter],
    ) -> Box<[Parameter]> {
        let mut parameter_names = FxHashSet::default();
        parameters
            .iter()
            .filter_map(|parameter| try {
                let name = parameter.name.value()?.clone();
                if !parameter_names.insert(name.clone()) {
                    self.add_error(
                        name.span.clone(),
                        format!("Duplicate parameter name: {}", *name),
                    );
                    return None;
                }

                let type_ = self.lower_type(type_parameters, parameter.type_.value());

                let id = self.id_generator.generate();
                Parameter {
                    id,
                    name: name.string,
                    type_,
                }
            })
            .collect()
    }
    fn lower_function(&mut self, id: Id) {
        let function = self.functions.get(&id).unwrap();
        let body = function.ast.unwrap().body.clone();
        let type_parameters = function.type_parameters.clone();
        let parameters = function.parameters.clone();
        let return_type = function.return_type.clone();

        let (hir_body, _) = BodyBuilder::build(self, &type_parameters, |builder| {
            for parameter in parameters.iter() {
                builder.push_parameter(parameter.clone());
            }

            builder.lower_statements(&body, Some(&return_type));
        });

        self.functions.get_mut(&id).unwrap().body = Some(BodyOrBuiltin::Body(hir_body));
    }

    fn is_assignable_to(from: &Type, to: &Type) -> bool {
        match (from, to) {
            (Type::Error, _) | (_, Type::Error) => true,
            (
                Type::Named {
                    name: from_name,
                    type_arguments: from_type_arguments,
                },
                Type::Named {
                    name: to_name,
                    type_arguments: to_type_arguments,
                },
            ) => {
                from_name == to_name
                    && from_type_arguments
                        .iter()
                        .zip_eq(to_type_arguments.iter())
                        .all(|(from, to)| Self::is_assignable_to(from, to))
            }
            (Type::Parameter { id: from, name: _ }, Type::Parameter { id: to, name: _ }) => {
                from == to
            }
            _ => false,
        }
    }

    fn add_error(&mut self, span: Range<Offset>, message: impl Into<String>) {
        self.errors.push(CompilerError {
            path: self.path.to_path_buf(),
            span,
            message: message.into(),
        });
    }
}

struct BodyBuilder<'c, 'a> {
    context: &'c mut Context<'a>,
    global_assignment_dependencies: FxHashSet<Id>,
    type_parameters: &'c [TypeParameter],
    local_identifiers: Vec<(Box<str>, Id, Type)>,
    body: Body,
}
impl<'c, 'a> BodyBuilder<'c, 'a> {
    #[must_use]
    fn build(
        context: &'c mut Context<'a>,
        type_parameters: &'c [TypeParameter],
        fun: impl FnOnce(&mut BodyBuilder),
    ) -> (Body, FxHashSet<Id>) {
        let mut builder = Self {
            context,
            global_assignment_dependencies: FxHashSet::default(),
            type_parameters,
            local_identifiers: vec![],
            body: Body::default(),
        };
        fun(&mut builder);
        (builder.body, builder.global_assignment_dependencies)
    }
    #[must_use]
    fn build_inner(&mut self, fun: impl FnOnce(&mut BodyBuilder)) -> Body {
        BodyBuilder::build(self.context, self.type_parameters, |builder| {
            builder.local_identifiers = self.local_identifiers.clone();
            fun(builder);
            self.global_assignment_dependencies
                .extend(&builder.global_assignment_dependencies);
        })
        .0
    }

    fn lower_statements(
        &mut self,
        statements: &[AstStatement],
        context_type: Option<&Type>,
    ) -> (Id, Type) {
        let mut last_expression = None;
        for (position, statement) in statements.iter().with_position() {
            let statement_context_type = if matches!(position, Position::Last | Position::Only) {
                context_type
            } else {
                None
            };

            match statement {
                AstStatement::Assignment(assignment) => {
                    let Some(name) = assignment.name.value().cloned() else {
                        continue;
                    };

                    let type_ = assignment
                        .type_
                        .as_ref()
                        .map(|it| self.context.lower_type(self.type_parameters, it.value()));

                    let (id, type_) = if let Some(value) = assignment.value.value() {
                        self.lower_expression(value, type_.as_ref())
                    } else {
                        (self.push_error(), Type::Error)
                    };
                    self.push(name.string.clone(), ExpressionKind::Reference(id), type_);
                    last_expression = None;
                }
                AstStatement::Expression(expression) => {
                    last_expression =
                        Some(self.lower_expression(expression, statement_context_type));
                }
            }
        }

        let last_expression = last_expression.unwrap_or_else(|| {
            // TODO: check return type
            let id = self.push_nothing();
            (id, Type::nothing())
        });
        let return_type = context_type.cloned().unwrap_or(last_expression.1);
        (last_expression.0, return_type)
    }

    fn lower_expression(
        &mut self,
        expression: &AstExpression,
        context_type: Option<&Type>,
    ) -> (Id, Type) {
        match self.lower_expression_raw(expression, context_type) {
            LoweredExpression::Expression { id, type_ } => {
                if let Some(context_type) = context_type
                    && !Context::is_assignable_to(&type_, context_type)
                {
                    // TODO: report actual error location
                    self.context.add_error(
                        Offset(0)..Offset(0),
                        format!("Expected type `{context_type:?}`, got `{type_:?}`."),
                    );
                    (self.push_error(), Type::Error)
                } else {
                    (id, type_)
                }
            }
            LoweredExpression::FunctionReferences { .. } => {
                // TODO: report actual error location
                self.context
                    .add_error(Offset(0)..Offset(0), "Function must be called.");
                (self.push_error(), Type::Error)
            }
            LoweredExpression::NamedTypeReference(_)
            | LoweredExpression::TypeParameterReference { .. } => {
                // TODO: report actual error location
                self.context
                    .add_error(Offset(0)..Offset(0), "Type must be instantiated.");
                (self.push_error(), Type::Error)
            }
            LoweredExpression::EnumVariantReference { enum_, variant } => {
                // TODO: report actual error location
                self.context.add_error(
                    Offset(0)..Offset(0),
                    format!("Enum variant `{enum_:?}.{variant}` must be instantiated."),
                );
                (self.push_error(), Type::Error)
            }
            LoweredExpression::Error => (self.push_error(), Type::Error),
        }
    }
    fn lower_expression_raw(
        &mut self,
        expression: &AstExpression,
        context_type: Option<&Type>,
    ) -> LoweredExpression {
        match expression {
            AstExpression::Identifier(identifier) => {
                let Some(identifier) = identifier.identifier.value() else {
                    return LoweredExpression::Error;
                };

                let name = &identifier.string;
                if let Some((id, type_)) = self.lookup_local_identifier(identifier) {
                    self.push_lowered(None, ExpressionKind::Reference(id), type_.clone())
                } else if let Some(named) = self.context.global_identifiers.get(name) {
                    match named {
                        Named::Assignment(id) => {
                            let id = *id;
                            self.global_assignment_dependencies.insert(id);
                            let type_ = self.context.assignments.get(&id).unwrap().type_.clone();
                            self.push_lowered(None, ExpressionKind::Reference(id), type_)
                        }
                        Named::Functions(function_ids) => {
                            assert!(!function_ids.is_empty());
                            LoweredExpression::FunctionReferences {
                                receiver: None,
                                function_ids: function_ids.iter().copied().collect(),
                            }
                        }
                    }
                } else if let Some((name, id)) =
                    Context::resolve_type_parameter(self.type_parameters, name)
                {
                    LoweredExpression::TypeParameterReference { name, id }
                } else if self.context.hir.type_declarations.get(name).is_some() {
                    LoweredExpression::NamedTypeReference(name.clone())
                } else {
                    self.context.add_error(
                        identifier.span.clone(),
                        format!("Unknown reference: {name}"),
                    );
                    LoweredExpression::Error
                }
            }
            AstExpression::Int(int) => self.push_lowered(
                None,
                int.value
                    .value()
                    .map_or(ExpressionKind::Error, |it| ExpressionKind::Int(*it)),
                Type::int(),
            ),
            AstExpression::Text(text) => {
                let text = text
                    .parts
                    .iter()
                    .map::<Id, _>(|it| match it {
                        AstTextPart::Text(text) => {
                            self.push(None, ExpressionKind::Text(text.clone()), Type::text())
                        }
                        AstTextPart::Interpolation { expression, .. } => {
                            if let Some(expression) = expression.value() {
                                self.lower_expression(expression, Some(&Type::text())).0
                            } else {
                                self.push_error()
                            }
                        }
                    })
                    .collect_vec()
                    .into_iter()
                    .reduce(|lhs, rhs| {
                        self.push(
                            None,
                            ExpressionKind::Call {
                                function: BuiltinFunction::TextConcat.id(),
                                type_arguments: Box::default(),
                                arguments: [lhs, rhs].into(),
                            },
                            Type::text(),
                        )
                    })
                    .unwrap_or_else(|| {
                        self.push(None, ExpressionKind::Text("".into()), Type::text())
                    });
                LoweredExpression::Expression {
                    id: text,
                    type_: Type::text(),
                }
            }
            AstExpression::Parenthesized(parenthesized) => {
                return parenthesized
                    .inner
                    .value()
                    .map_or(LoweredExpression::Error, |it| {
                        self.lower_expression_raw(it, context_type)
                    });
            }
            AstExpression::Call(call) => {
                fn lower_arguments(
                    builder: &mut BodyBuilder,
                    call: &AstCall,
                    arguments: &AstResult<AstArguments>,
                    parameter_types: &[Type],
                ) -> Option<Box<[Id]>> {
                    let arguments = arguments
                        .arguments_or_default()
                        .iter()
                        .enumerate()
                        .map(|(index, argument)| {
                            builder
                                .lower_expression(&argument.value, parameter_types.get(index))
                                .0
                        })
                        .collect::<Box<_>>();
                    if arguments.len() == parameter_types.len() {
                        Some(arguments)
                    } else {
                        builder.context.add_error(
                            if arguments.len() < parameter_types.len() {
                                // TODO: report actual error location
                                call.arguments.value().map_or(Offset(0)..Offset(0), |it| {
                                    it.opening_parenthesis_span.clone()
                                })
                            } else {
                                let arguments = &call.arguments.value().unwrap().arguments;
                                arguments[parameter_types.len()].span.start
                                    ..arguments.last().unwrap().span.end
                            },
                            format!(
                                "Expected {} argument(s), got {}.",
                                parameter_types.len(),
                                arguments.len(),
                            ),
                        );
                        None
                    }
                }

                let receiver = self.lower_expression_raw(&call.receiver, None);

                match receiver {
                    LoweredExpression::Expression { .. } => {
                        // TODO: report actual error location
                        self.context
                            .add_error(Offset(0)..Offset(0), "Cannot call this type");
                        LoweredExpression::Error
                    }
                    LoweredExpression::FunctionReferences {
                        receiver,
                        function_ids,
                    } => {
                        assert!(!function_ids.is_empty());

                        let type_arguments = call.type_arguments.as_ref().map(|it| {
                            it.arguments
                                .iter()
                                .map(|it| self.context.lower_type(self.type_parameters, &it.type_))
                                .collect::<Box<_>>()
                        });

                        let arguments = receiver
                            .into_iter()
                            .chain(
                                call.arguments
                                    .arguments_or_default()
                                    .iter()
                                    .map(|argument| self.lower_expression(&argument.value, None)),
                            )
                            .collect::<Box<_>>();

                        let matches = function_ids
                            .iter()
                            .map(|id| (*id, &self.context.functions[id]))
                            .collect_vec();

                        // Check type parameter count
                        let matches = if let Some(type_arguments) = &type_arguments {
                            let (matches, mismatches) =
                                matches.iter().partition::<Vec<_>, _>(|(_, it)| {
                                    it.type_parameters.len() == type_arguments.len()
                                });
                            if matches.is_empty() {
                                self.context.add_error(
                                    call.type_arguments.as_ref().unwrap().span.clone(),
                                    format!(
                                        "No overload accepts exactly {} {}:\n{}",
                                        arguments.len(),
                                        if arguments.len() == 1 {
                                            "type argument"
                                        } else {
                                            "type arguments"
                                        },
                                        mismatches
                                            .iter()
                                            .map(|(_, it)| it.signature_to_string())
                                            .join("\n"),
                                    ),
                                );
                                return LoweredExpression::Error;
                            }
                            matches
                        } else {
                            matches
                        };

                        // TODO: report actual error location
                        let arguments_start_span =
                            call.arguments.value().map_or(Offset(0)..Offset(0), |it| {
                                it.opening_parenthesis_span.clone()
                            });

                        // Check parameter count
                        let matches = {
                            let (matches, mismatches) =
                                matches.iter().partition::<Vec<_>, _>(|(_, it)| {
                                    it.parameters.len() == arguments.len()
                                });
                            if matches.is_empty() {
                                self.context.add_error(
                                    arguments_start_span,
                                    format!(
                                        "No overload accepts exactly {} {}:\n{}",
                                        arguments.len(),
                                        if arguments.len() == 1 {
                                            "argument"
                                        } else {
                                            "arguments"
                                        },
                                        mismatches
                                            .iter()
                                            .map(|(_, it)| it.signature_to_string())
                                            .join("\n"),
                                    ),
                                );
                                return LoweredExpression::Error;
                            }
                            matches
                        };

                        // Check argument types
                        // FIXME: Unify types
                        let argument_types = arguments
                            .iter()
                            .map(|(_, type_)| type_.clone())
                            .collect::<Box<_>>();
                        let old_matches = matches;
                        let mut matches = vec![];
                        let mut mismatches = vec![];
                        'outer: for (id, function) in old_matches {
                            let mut type_solver = TypeSolver::new(&function.type_parameters);
                            // Type arguments
                            if let Some(type_arguments) = &type_arguments {
                                for (type_argument, type_parameter) in type_arguments
                                    .iter()
                                    .zip_eq(function.type_parameters.iter())
                                {
                                    match type_solver.unify(
                                        type_argument,
                                        &Type::Parameter {
                                            name: type_parameter.name.clone(),
                                            id: type_parameter.id,
                                        },
                                    ) {
                                        Ok(true) => {}
                                        Ok(false) => unreachable!(),
                                        Err(reason) => {
                                            mismatches.push((id, function, Some(reason)));
                                            break 'outer;
                                        }
                                    };
                                }
                            }

                            // Arguments
                            for (argument_type, parameter) in
                                argument_types.iter().zip_eq(function.parameters.iter())
                            {
                                match type_solver.unify(argument_type, &parameter.type_) {
                                    Ok(true) => {}
                                    Ok(false) => {
                                        mismatches.push((id, function, None));
                                        break 'outer;
                                    }
                                    Err(reason) => {
                                        mismatches.push((id, function, Some(reason)));
                                        break 'outer;
                                    }
                                };
                            }

                            match type_solver.finish() {
                                Ok(environment) => matches.push((id, function, environment)),
                                Err(error) => mismatches.push((id, function, Some(error))),
                            }
                        }

                        if matches.is_empty() {
                            self.context.add_error(
                                arguments_start_span,
                                format!(
                                    "No matching function found for:\n  {}\n{}:{}",
                                    FunctionDeclaration::call_signature_to_string(
                                        mismatches.first().unwrap().1.name.as_ref(),
                                        argument_types.as_ref()
                                    ),
                                    if mismatches.len() == 1 {
                                        "This is the candidate function"
                                    } else {
                                        "These are candidate functions"
                                    },
                                    mismatches
                                        .iter()
                                        .map(|(_, it, reason)| format!(
                                            "\n• {}{}",
                                            it.signature_to_string(),
                                            reason
                                                .as_ref()
                                                .map_or_else(String::new, |reason| format!(
                                                    " ({reason})"
                                                )),
                                        ))
                                        .join(""),
                                ),
                            );
                            return LoweredExpression::Error;
                        } else if matches.len() > 1 {
                            self.context.add_error(
                                arguments_start_span,
                                format!(
                                    "Multiple matching function found for:\n  {}\nThese are candidate functions:{}",
                                    FunctionDeclaration::call_signature_to_string(matches.first().unwrap().1.name.as_ref(), argument_types.as_ref()),
                                    matches
                                        .iter()
                                        .map(|(_,it,_)| format!("\n• {}", it.signature_to_string()))
                                        .join(""),
                                ),
                            );
                            return LoweredExpression::Error;
                        }

                        let (function, signature, environment) = matches.pop().unwrap();
                        self.push_lowered(
                            None,
                            ExpressionKind::Call {
                                function,
                                type_arguments: signature
                                    .type_parameters
                                    .iter()
                                    .map(|it| environment.get(&it.id).unwrap().clone())
                                    .collect(),
                                arguments: arguments.iter().map(|(id, _)| *id).collect(),
                            },
                            signature.return_type.substitute(&environment),
                        )
                        // let parameter_types = function
                        //     .parameters
                        //     .iter()
                        //     .map(|it| it.type_.clone())
                        //     .collect_vec();
                        // let return_type = function.return_type.clone();

                        //   if full_matches.is_empty() then return error[LookupFunSolution, Str]({
                        //     var out = string_builder().&
                        //     out.
                        //       "This call doesn't work:{newline}
                        //       ' > {call_signature(name, type_args, arg_types)}{newline}{newline}"
                        //     if name_matches.is_empty()
                        //     then out.'"There are no defintions named "{{name}}"."'
                        //     else {
                        //       out."These definitions have the same name, but arguments don't match:"
                        //       for match in name_matches do
                        //         out."{newline} - {AstDef.fun_(match).signature()}"
                        //     }
                        //     out.to_str()
                        //   })
                        //   if full_matches.len.is_greater_than(1) then return error[LookupFunSolution, Str]({
                        //     var out = string_builder().&
                        //     out.
                        //       "This call doesn't work:{newline}
                        //       ' > {call_signature(name, type_args, arg_types)}{newline}{newline}
                        //       'Multiple definitions match:"
                        //     for match in full_matches do {
                        //       var padded_signature = "{AstDef.fun_(match.fun_).signature()}"
                        //         .pad_right(30, # )
                        //       out."{newline} - {padded_signature}"
                        //       if match.type_env.is_not_empty() then {
                        //         out." with "
                        //         var first = true
                        //         for entry in match.type_env do {
                        //           if first then first = false else out.", "
                        //           out."{entry.key} = {entry.value}"
                        //         }
                        //       }
                        //     }
                        //     out.to_str()
                        //   })
                        //   ok[LookupFunSolution, Str](full_matches.get(0))
                    }
                    LoweredExpression::NamedTypeReference(type_) => {
                        match self.context.hir.type_declarations.get(&type_) {
                            Some(TypeDeclaration::Struct {
                                type_parameters,
                                fields,
                            }) => {
                                if !type_parameters.is_empty() {
                                    todo!("Use type solver");
                                }

                                let fields = lower_arguments(
                                    self,
                                    call,
                                    &call.arguments,
                                    &fields.iter().map(|(_, type_)| type_.clone()).collect_vec(),
                                );
                                let type_ = Type::Named {
                                    name: type_.clone(),
                                    type_arguments: Box::default(),
                                };
                                fields.map_or(LoweredExpression::Error, |fields| {
                                    self.push_lowered(
                                        None,
                                        ExpressionKind::CreateStruct {
                                            struct_: type_.clone(),
                                            fields,
                                        },
                                        type_,
                                    )
                                })
                            }
                            Some(TypeDeclaration::Enum { .. }) => {
                                // TODO: report actual error location
                                self.context
                                    .add_error(Offset(0)..Offset(0), "Enum variant is missing.");
                                LoweredExpression::Error
                            }
                            None => {
                                // TODO: report actual error location
                                self.context.add_error(
                                    Offset(0)..Offset(0),
                                    format!("Can't instantiate builtin type {type_} directly."),
                                );
                                LoweredExpression::Error
                            }
                        }
                    }
                    LoweredExpression::TypeParameterReference { name, .. } => {
                        // TODO: report actual error location
                        self.context.add_error(
                            Offset(0)..Offset(0),
                            format!("Can't instantiate type parameter {name} directly."),
                        );
                        LoweredExpression::Error
                    }
                    LoweredExpression::EnumVariantReference { enum_, variant } => {
                        let (enum_name, type_arguments) = match &enum_ {
                            Type::Named {
                                name,
                                type_arguments,
                            } => (name, type_arguments),
                            Type::Parameter { .. } | Type::Error => unreachable!(),
                        };
                        let (type_parameters, variant_type) =
                            match self.context.hir.type_declarations.get(enum_name).unwrap() {
                                TypeDeclaration::Struct { .. } => unreachable!(),
                                TypeDeclaration::Enum {
                                    type_parameters,
                                    variants,
                                } => (
                                    type_parameters,
                                    variants
                                        .iter()
                                        .find(|(name, _)| name == &variant)
                                        .unwrap()
                                        .1
                                        .as_ref()
                                        .unwrap()
                                        .clone(),
                                ),
                            };
                        let variant_type = variant_type
                            .substitute(&Type::build_environment(type_parameters, type_arguments));
                        let parameter_types = [variant_type];
                        let arguments = lower_arguments(
                            self,
                            call,
                            &call.arguments,
                            parameter_types.as_slice(),
                        );
                        arguments.map_or(LoweredExpression::Error, |arguments| {
                            self.push_lowered(
                                None,
                                ExpressionKind::CreateEnum {
                                    enum_: enum_.clone(),
                                    variant,
                                    value: arguments.first().copied(),
                                },
                                enum_,
                            )
                        })
                    }
                    LoweredExpression::Error => LoweredExpression::Error,
                }
            }
            AstExpression::Navigation(navigation) => {
                let receiver = self.lower_expression_raw(&navigation.receiver, None);

                let Some(key) = navigation.key.value() else {
                    return LoweredExpression::Error;
                };

                match receiver {
                    LoweredExpression::Expression {
                        id: receiver_id,
                        type_: receiver_type,
                    } => match &receiver_type {
                        Type::Named {
                            name,
                            type_arguments,
                        } => {
                            let type_ = &self.context.hir.type_declarations.get(name);
                            if let Some(TypeDeclaration::Struct {
                                type_parameters,
                                fields,
                            }) = type_
                                && let Some((_, field_type)) =
                                    fields.iter().find(|(name, _)| name == &key.string)
                            {
                                return self.push_lowered(
                                    None,
                                    ExpressionKind::StructAccess {
                                        struct_: receiver_id,
                                        field: key.string.clone(),
                                    },
                                    field_type.substitute(&Type::build_environment(
                                        type_parameters,
                                        type_arguments,
                                    )),
                                );
                            }

                            // TODO: merge with global function resolution
                            if let Some(Named::Functions(function_ids)) =
                                self.context.global_identifiers.get(&key.string)
                            {
                                let function_ids = function_ids
                                    .iter()
                                    .map(|id| (*id, &self.context.functions[id]))
                                    .filter(|(_, it)| {
                                        !it.parameters.is_empty()
                                            && it.parameters[0].type_ == receiver_type
                                    })
                                    .map(|(id, _)| id)
                                    .collect::<Box<_>>();
                                if !function_ids.is_empty() {
                                    return LoweredExpression::FunctionReferences {
                                        receiver: Some((receiver_id, receiver_type.clone())),
                                        function_ids,
                                    };
                                }
                            }

                            self.context.add_error(
                                key.span.clone(),
                                format!(
                                    "Value of type `{receiver_type:?}` doesn't have a function or field `{}`",
                                    key.string
                                ),
                            );
                            LoweredExpression::Error
                        }
                        Type::Parameter { name, .. } => {
                            self.context.add_error(
                                key.span.clone(),
                                format!(
                                    "Navigation on value of type parameter type `{name}` is not supported yet."
                                ),
                            );
                            LoweredExpression::Error
                        }
                        Type::Error => todo!(),
                    },
                    LoweredExpression::FunctionReferences { .. } => {
                        self.context.add_error(
                            key.span.clone(),
                            format!("Function doesn't have a field `{}`", key.string),
                        );
                        LoweredExpression::Error
                    }
                    LoweredExpression::NamedTypeReference(type_) => {
                        match self.context.hir.type_declarations.get(&type_).unwrap() {
                            TypeDeclaration::Struct { .. } => {
                                self.context.add_error(
                                    key.span.clone(),
                                    format!(
                                        "Struct type `{type_:?}` doesn't have a field `{}`",
                                        key.string,
                                    ),
                                );
                                LoweredExpression::Error
                            }
                            TypeDeclaration::Enum {
                                type_parameters,
                                variants,
                            } => {
                                if !type_parameters.is_empty() {
                                    todo!();
                                }
                                let type_ = Type::Named {
                                    name: type_.clone(),
                                    type_arguments: Box::default(),
                                };

                                if let Some((_, value_type)) =
                                    variants.iter().find(|(name, _)| name == &key.string)
                                {
                                    if value_type.is_some() {
                                        LoweredExpression::EnumVariantReference {
                                            enum_: type_,
                                            variant: key.string.clone(),
                                        }
                                    } else {
                                        self.push_lowered(
                                            None,
                                            ExpressionKind::CreateEnum {
                                                enum_: type_.clone(),
                                                variant: key.string.clone(),
                                                value: None,
                                            },
                                            type_,
                                        )
                                    }
                                } else {
                                    self.context.add_error(
                                        key.span.clone(),
                                        format!(
                                            "Enum `{type_:?}` doesn't have a variant `{}`",
                                            key.string,
                                        ),
                                    );
                                    LoweredExpression::Error
                                }
                            }
                        }
                    }
                    LoweredExpression::TypeParameterReference { name, .. } => {
                        self.context.add_error(
                            key.span.clone(),
                            format!(
                                "Parameter type `{name:?}` doesn't have a field `{}`",
                                key.string,
                            ),
                        );
                        LoweredExpression::Error
                    }
                    LoweredExpression::EnumVariantReference { .. } => todo!(),
                    LoweredExpression::Error => LoweredExpression::Error,
                }
            }
            AstExpression::Body(AstBody { statements, .. }) => {
                let (id, type_) = self.lower_statements(statements, context_type);
                LoweredExpression::Expression { id, type_ }
            }
            AstExpression::Switch(AstSwitch { value, cases, .. }) => {
                let Some(value) = value.value() else {
                    return LoweredExpression::Error;
                };
                let (value, enum_) = self.lower_expression(value, None);

                let (environment, variants) = match &enum_ {
                    Type::Named {
                        name,
                        type_arguments,
                    } => {
                        match &self.context.hir.type_declarations.get(name) {
                            Some(TypeDeclaration::Struct { .. }) => {
                                // TODO: report actual error location
                                self.context.add_error(
                                    Offset(0)..Offset(0),
                                    format!("Can't switch over struct `{enum_:?}`"),
                                );
                                return LoweredExpression::Error;
                            }
                            Some(TypeDeclaration::Enum {
                                type_parameters,
                                variants,
                            }) => (
                                type_parameters
                                    .iter()
                                    .map(|it| it.id)
                                    .zip_eq(type_arguments.iter().cloned())
                                    .collect::<FxHashMap<_, _>>(),
                                variants.clone(),
                            ),
                            None => {
                                // TODO: report actual error location
                                self.context.add_error(
                                    Offset(0)..Offset(0),
                                    format!("Can't switch over builtin type `{enum_:?}`"),
                                );
                                return LoweredExpression::Error;
                            }
                        }
                    }
                    Type::Parameter { name, .. } => {
                        // TODO: report actual error location
                        self.context.add_error(
                            Offset(0)..Offset(0),
                            format!("Can't switch over type parameter `{name}`"),
                        );
                        return LoweredExpression::Error;
                    }
                    Type::Error => return LoweredExpression::Error,
                };

                let mut variant_names = FxHashSet::default();
                let mut return_type = context_type.cloned();
                let cases = cases
                    .iter()
                    .filter_map(|case| try {
                        let variant = case.variant.value()?.clone();
                        let Some((_, value_type)) =
                            variants.iter().find(|(it, _)| it == &variant.string)
                        else {
                            self.context.add_error(
                                variant.span.clone(),
                                format!("Unknown variant in switch: `{}`", *variant),
                            );
                            return None;
                        };

                        if !variant_names.insert(variant.clone()) {
                            self.context.add_error(
                                variant.span.clone(),
                                format!("Duplicate variant in switch: `{}`", *variant),
                            );
                            return None;
                        }

                        let variant_value = match (value_type, &case.value_name) {
                            (None, None) => None,
                            (None, Some(_)) => {
                                self.context.add_error(
                                    variant.span.clone(),
                                    format!(
                                        "Switch case specifies value name for variant `{}` that doesn't have any value",
                                        *variant
                                    ),
                                );
                                return None;
                            },
                            (Some(_), None) => {
                                self.context.add_error(
                                    variant.span.clone(),
                                    format!(
                                        "Switch case is missing a value name for variant `{}`",
                                        *variant
                                    ),
                                );
                                return None;
                            },
                            (Some(value_type), Some((value_name, _))) => {
                                let value_name = value_name.value()?;
                                Some((value_type.substitute(&environment), value_name.string.clone()))
                            },
                        };

                        let mut value_id = None;
                        let body = self.build_inner(|builder| {
                            value_id = variant_value.map(|(type_,name)| {
                                let id = builder.context.id_generator.generate();
                                builder.push_parameter(Parameter { id, name, type_ });
                                id
                            });
                            if let Some(expression) = case.expression.value() {
                                let (_, new_return_type) = builder.lower_expression(expression, return_type.as_ref());
                                if return_type.is_none() {
                                    return_type = Some(new_return_type);
                                }
                            }
                        });
                        SwitchCase {
                            variant: variant.string,
                            value_id,
                            body,
                        }
                    })
                    .collect();

                // TODO: check for missing variants

                self.push_lowered(
                    None,
                    ExpressionKind::Switch {
                        value,
                        enum_,
                        cases,
                    },
                    return_type.unwrap_or_else(Type::never),
                )
            }
        }
    }

    fn push_lowered(
        &mut self,
        name: impl Into<Option<Box<str>>>,
        kind: ExpressionKind,
        type_: Type,
    ) -> LoweredExpression {
        let id = self.push(name, kind, type_.clone());
        LoweredExpression::Expression { id, type_ }
    }
    fn push_nothing(&mut self) -> Id {
        self.push(
            None,
            ExpressionKind::CreateStruct {
                struct_: Type::nothing(),
                fields: [].into(),
            },
            Type::nothing(),
        )
    }
    fn push_parameter(&mut self, parameter: Parameter) {
        self.local_identifiers
            .push((parameter.name, parameter.id, parameter.type_));
    }
    fn push_error(&mut self) -> Id {
        self.push(None, ExpressionKind::Error, Type::Error)
    }
    fn push(&mut self, name: impl Into<Option<Box<str>>>, kind: ExpressionKind, type_: Type) -> Id {
        let name = name.into();
        let id = self.context.id_generator.generate();
        if let Some(name) = &name {
            self.local_identifiers
                .push(((*name).clone(), id, type_.clone()));
        }
        self.body
            .expressions
            .push((id, name, Expression { kind, type_ }));
        id
    }

    #[must_use]
    fn lookup_local_identifier(&self, name: &str) -> Option<(Id, &Type)> {
        self.local_identifiers
            .iter()
            .rev()
            .find(|(box variable_name, _, _)| variable_name == name)
            .map(|(_, id, type_)| (*id, type_))
    }
}

#[derive(Debug)]
enum LoweredExpression {
    Expression {
        id: Id,
        type_: Type,
    },
    FunctionReferences {
        receiver: Option<(Id, Type)>,
        function_ids: Box<[Id]>,
    },
    NamedTypeReference(Box<str>),
    TypeParameterReference {
        name: Box<str>,
        id: TypeParameterId,
    },
    EnumVariantReference {
        enum_: Type,
        variant: Box<str>,
    },
    Error,
}

struct TypeSolver<'h> {
    type_parameters: &'h [TypeParameter],
    environment: FxHashMap<TypeParameterId, Type>,
}
impl<'h> TypeSolver<'h> {
    #[must_use]
    fn new(type_parameters: &'h [TypeParameter]) -> Self {
        Self {
            type_parameters,
            environment: FxHashMap::default(),
        }
    }

    fn unify(&mut self, argument: &Type, parameter: &Type) -> Result<bool, Box<str>> {
        match (argument, parameter) {
            (Type::Error, _) | (_, Type::Error) => Ok(true),
            (_, Type::Parameter { name, id }) => {
                if let Some(mapped) = self.environment.get(id) {
                    if let Type::Parameter { .. } = mapped {
                        panic!("Type parameters can't depend on each other.")
                    }
                    let mapped = mapped.clone();
                    return self.unify(argument, &mapped);
                }

                assert!(
                    self.type_parameters.iter().any(|it| it.id == *id),
                    "Unresolved type parameter: `{name}`"
                );
                match self.environment.entry(*id) {
                    Entry::Occupied(entry) => {
                        if !Context::is_assignable_to(entry.get(), argument) {
                            return Err(format!("Type parameter {name} gets resolved to different types: `{}` and `{argument}`", entry.get()).into_boxed_str());
                        }
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(argument.clone());
                    }
                }
                Ok(true)
            }
            (
                Type::Named {
                    name: argument_name,
                    type_arguments: argument_type_arguments,
                },
                Type::Named {
                    name: parameter_name,
                    type_arguments: parameter_type_arguments,
                },
            ) => {
                if argument_name != parameter_name
                    || argument_type_arguments.len() != parameter_type_arguments.len()
                {
                    return Ok(false);
                }

                for (argument, parameter) in argument_type_arguments
                    .iter()
                    .zip_eq(parameter_type_arguments.iter())
                {
                    let result = self.unify(argument, parameter)?;
                    if !result {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (Type::Parameter { .. }, Type::Named { .. }) => Ok(true),
        }
    }

    fn finish(self) -> Result<FxHashMap<TypeParameterId, Type>, Box<str>> {
        for type_parameter in self.type_parameters {
            if !self.environment.contains_key(&type_parameter.id) {
                return Err(format!(
                    "The type parameter `{}` can't be resolved to a specific type.",
                    &type_parameter.name
                )
                .into_boxed_str());
            }
        }
        Ok(self.environment)
    }
}
