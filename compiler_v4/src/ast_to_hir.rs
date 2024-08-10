use crate::{
    ast::{
        Ast, AstArgument, AstAssignment, AstBody, AstCall, AstDeclaration, AstEnum, AstExpression,
        AstFunction, AstNamedType, AstParameter, AstStatement, AstStruct, AstSwitch, AstTextPart,
        AstType,
    },
    error::CompilerError,
    hir::{
        Assignment, Body, BodyOrBuiltin, BuiltinFunction, Expression, ExpressionKind, Function,
        Hir, Id, Parameter, Type, TypeDeclaration,
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
use std::{collections::hash_map::Entry, ops::Range, path::Path};
use strum::VariantArray;

pub fn ast_to_hir(path: &Path, ast: &Ast) -> (Hir, Vec<CompilerError>) {
    let mut context = Context::new(path, ast);
    context.add_builtin_functions();
    context.lower_declarations();

    if let Some(named) = context.global_identifiers.get("main") {
        match named {
            Named::Assignment(_) => {
                // TODO: report actual error location
                context.add_error(Offset(0)..Offset(0), "`main` must be a function");
            }
            Named::Functions(ids) => {
                assert!(!ids.is_empty());

                let function = context.functions.get(ids.first().unwrap()).unwrap();
                let parameters_are_empty = function.parameters.is_empty();
                let return_type = function.return_type.clone();
                if ids.len() > 1 {
                    // TODO: report actual error location
                    context.add_error(Offset(0)..Offset(0), "Main function may not be overloaded");
                } else {
                    if !parameters_are_empty {
                        // TODO: report actual error location
                        context.add_error(
                            Offset(0)..Offset(0),
                            "Main function must not have parameters",
                        );
                    }
                    if return_type != Type::Error && return_type != Type::int() {
                        // TODO: report actual error location
                        context.add_error(Offset(0)..Offset(0), "Main function must return an Int");
                    }
                }
            }
        }
    } else {
        context.add_error(Offset(0)..Offset(0), "Program is missing a main function");
    }

    context.into_hir()
}

#[derive(Debug)]
struct Context<'a> {
    path: &'a Path,
    ast: &'a Ast,
    id_generator: IdGenerator<Id>,
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
    parameters: Box<[Parameter]>,
    return_type: Type,
    body: Option<BodyOrBuiltin>,
}
impl<'a> FunctionDeclaration<'a> {
    fn signature_to_string(&self) -> String {
        format!(
            "{}({})",
            self.name,
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
            global_identifiers: FxHashMap::default(),
            assignments: FxHashMap::default(),
            assignment_dependency_graph: DiGraph::new(),
            functions: FxHashMap::default(),
            errors: vec![],
            hir: Hir::default(),
        }
    }

    fn into_hir(mut self) -> (Hir, Vec<CompilerError>) {
        match toposort(&self.assignment_dependency_graph, None) {
            Ok(order) => {
                self.hir.assignment_initialization_order = order
                    .iter()
                    .map(|it| *self.assignment_dependency_graph.node_weight(*it).unwrap())
                    .collect();
            }
            Err(cycle) => {
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
                            parameters,
                            return_type,
                            body,
                            ..
                        } = self.functions.remove(&id).unwrap();
                        functions.push((
                            id,
                            name.clone(),
                            Function {
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

    fn add_builtin_functions(&mut self) {
        {
            let a_id = self.id_generator.generate();
            let b_id = self.id_generator.generate();
            self.add_builtin_function(
                BuiltinFunction::IntAdd,
                [
                    Parameter {
                        id: a_id,
                        name: "a".into(),
                        type_: Type::int(),
                    },
                    Parameter {
                        id: b_id,
                        name: "b".into(),
                        type_: Type::int(),
                    },
                ],
                Type::int(),
            );
        }
        {
            let a_id = self.id_generator.generate();
            let b_id = self.id_generator.generate();
            self.add_builtin_function(
                BuiltinFunction::IntCompareTo,
                [
                    Parameter {
                        id: a_id,
                        name: "a".into(),
                        type_: Type::int(),
                    },
                    Parameter {
                        id: b_id,
                        name: "b".into(),
                        type_: Type::int(),
                    },
                ],
                Type::Named("Ordering".into()),
            );
        }
        {
            let a_id = self.id_generator.generate();
            let b_id = self.id_generator.generate();
            self.add_builtin_function(
                BuiltinFunction::IntSubtract,
                [
                    Parameter {
                        id: a_id,
                        name: "a".into(),
                        type_: Type::int(),
                    },
                    Parameter {
                        id: b_id,
                        name: "b".into(),
                        type_: Type::int(),
                    },
                ],
                Type::int(),
            );
        }
        {
            let int_id = self.id_generator.generate();
            self.add_builtin_function(
                BuiltinFunction::IntToText,
                [Parameter {
                    id: int_id,
                    name: "int".into(),
                    type_: Type::int(),
                }],
                Type::text(),
            );
        }
        {
            let message_id = self.id_generator.generate();
            self.add_builtin_function(
                BuiltinFunction::Print,
                [Parameter {
                    id: message_id,
                    name: "message".into(),
                    type_: Type::text(),
                }],
                Type::nothing(),
            );
        }
        {
            let a_id = self.id_generator.generate();
            let b_id = self.id_generator.generate();
            self.add_builtin_function(
                BuiltinFunction::TextConcat,
                [
                    Parameter {
                        id: a_id,
                        name: "a".into(),
                        type_: Type::text(),
                    },
                    Parameter {
                        id: b_id,
                        name: "b".into(),
                        type_: Type::text(),
                    },
                ],
                Type::text(),
            );
        }
    }
    fn add_builtin_function(
        &mut self,
        builtin_function: BuiltinFunction,
        parameters: impl Into<Box<[Parameter]>>,
        return_type: Type,
    ) {
        let name = builtin_function.as_ref();
        let parameters = parameters.into();
        let id = builtin_function.id();
        self.functions.force_insert(
            id,
            FunctionDeclaration {
                ast: None,
                name: name.into(),
                parameters,
                return_type,
                body: Some(BodyOrBuiltin::Builtin(builtin_function)),
            },
        );
        self.global_identifiers
            .force_insert(name.into(), Named::Functions(vec![id]));
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

        let fields = struct_type
            .fields
            .iter()
            .filter_map(|field| {
                let name = field.name.value()?;

                let type_ = self.lower_type(field.type_.value());
                Some((name.string.clone(), type_))
            })
            .collect();

        match self.hir.type_declarations.entry(name.string.clone()) {
            Entry::Occupied(_) => {
                self.add_error(name.span.clone(), "Duplicate type name");
            }
            Entry::Vacant(entry) => {
                entry.insert(TypeDeclaration::Struct { fields });
            }
        };
    }
    fn lower_enum(&mut self, enum_type: &'a AstEnum) {
        let Some(name) = enum_type.name.value() else {
            return;
        };

        let variants = enum_type
            .variants
            .iter()
            .filter_map(|variant| {
                let name = variant.name.value()?;

                let type_ = variant.type_.as_ref().map(|it| self.lower_type(it.value()));
                Some((name.string.clone(), type_))
            })
            .collect();

        match self.hir.type_declarations.entry(name.string.clone()) {
            Entry::Occupied(_) => {
                self.add_error(name.span.clone(), "Duplicate type name");
            }
            Entry::Vacant(entry) => {
                entry.insert(TypeDeclaration::Enum { variants });
            }
        };
    }

    fn lower_type(&mut self, type_: impl Into<Option<&AstType>>) -> Type {
        match type_.into() {
            Some(AstType::Named(AstNamedType { name })) => {
                let Some(name) = name.value() else {
                    return Type::Error;
                };

                if &*name.string == "Int" {
                    return Type::int();
                }
                if &*name.string == "Text" {
                    return Type::text();
                }

                if self.ast.iter().any(|it| {
                    matches!(
                        it,
                        AstDeclaration::Struct(AstStruct { name:it_name, .. })
                            | AstDeclaration::Enum(AstEnum { name:it_name, .. })
                        if it_name.value().map(|it| &it.string) == Some(&name.string)
                    )
                }) {
                    Type::Named(name.string.clone())
                } else {
                    self.add_error(name.span.clone(), format!("Unknown type: `{}`", **name));
                    Type::Error
                }
            }
            None => Type::Error,
        }
    }

    fn lower_assignment_signature(&mut self, assignment: &'a AstAssignment) -> Option<Id> {
        let name = assignment.name.value()?;

        let id = self.id_generator.generate();
        // TODO: infer type
        let type_ = assignment
            .type_
            .as_ref()
            .map_or(Type::Error, |it| self.lower_type(it.value()));

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

        let (hir_body, global_assignment_dependencies) = BodyBuilder::build(self, |builder| {
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
        let parameters = self.lower_parameters(&function.parameters);
        let return_type = function
            .return_type
            .as_ref()
            .map_or_else(Type::nothing, |it| self.lower_type(it));
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
                parameters,
                return_type,
                body: None,
            },
        );
        Some(id)
    }
    fn lower_parameters(&mut self, parameters: &'a [AstParameter]) -> Box<[Parameter]> {
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

                let type_ = self.lower_type(parameter.type_.value());

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
        let declaration = self.functions.get(&id).unwrap();
        let body = declaration.ast.unwrap().body.clone();
        let parameters = declaration.parameters.clone();
        let return_type = declaration.return_type.clone();

        let (hir_body, _) = BodyBuilder::build(self, |builder| {
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
            (Type::Named(from_name), Type::Named(to_name)) => from_name == to_name,
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
    local_identifiers: Vec<(Box<str>, Id, Type)>,
    body: Body,
}
impl<'c, 'a> BodyBuilder<'c, 'a> {
    #[must_use]
    fn build(
        context: &'c mut Context<'a>,
        fun: impl FnOnce(&mut BodyBuilder),
    ) -> (Body, FxHashSet<Id>) {
        let mut builder = Self {
            context,
            global_assignment_dependencies: FxHashSet::default(),
            local_identifiers: vec![],
            body: Body::default(),
        };
        fun(&mut builder);
        (builder.body, builder.global_assignment_dependencies)
    }
    #[must_use]
    fn build_inner(&mut self, fun: impl FnOnce(&mut BodyBuilder)) -> Body {
        BodyBuilder::build(self.context, |builder| {
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
                        .map(|it| self.context.lower_type(it.value()));

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
            LoweredExpression::TypeReference(_) => {
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
                } else if self.context.hir.type_declarations.get(name).is_some() {
                    LoweredExpression::TypeReference(Type::Named(name.clone()))
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
                    arguments: &[AstArgument],
                    parameter_types: &[Type],
                ) -> Option<Box<[Id]>> {
                    let arguments = arguments
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
                                call.opening_parenthesis_span.clone()
                            } else {
                                call.arguments[parameter_types.len()].span.start
                                    ..call.arguments.last().unwrap().span.end
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

                        let arguments = receiver
                            .into_iter()
                            .chain(
                                call.arguments
                                    .iter()
                                    .map(|argument| self.lower_expression(&argument.value, None)),
                            )
                            .collect::<Box<_>>();

                        let (parameter_count_matches, parameter_count_mismatches) = function_ids
                            .iter()
                            .map(|id| (*id, &self.context.functions[id]))
                            .partition::<Vec<_>, _>(|(_, it)| {
                                it.parameters.len() == arguments.len()
                            });

                        if parameter_count_matches.is_empty() {
                            self.context.add_error(
                                call.opening_parenthesis_span.clone(),
                                format!(
                                    "No overload accepts exactly {} {}:\n{}",
                                    arguments.len(),
                                    if arguments.len() == 1 {
                                        "argument"
                                    } else {
                                        "arguments"
                                    },
                                    parameter_count_mismatches
                                        .iter()
                                        .map(|(_, it)| it.signature_to_string())
                                        .join("\n"),
                                ),
                            );
                            return LoweredExpression::Error;
                        }

                        let argument_types = arguments
                            .iter()
                            .map(|(_, type_)| type_.clone())
                            .collect::<Box<_>>();
                        let (matches, mismatches) = parameter_count_matches
                            .iter()
                            .partition::<Vec<(Id, &FunctionDeclaration)>, _>(|(_, function)| {
                                function
                                    .parameters
                                    .iter()
                                    .zip_eq(argument_types.iter())
                                    .all(|(parameter, argument_type)| {
                                        &parameter.type_ == argument_type
                                    })
                            });

                        if matches.is_empty() {
                            self.context.add_error(
                                call.opening_parenthesis_span.clone(),
                                format!(
                                    "No matching function found for:\n  {}\n{} with the same number of parameters:{}",
                                    FunctionDeclaration::call_signature_to_string(mismatches.first().unwrap().1.name.as_ref(), argument_types.as_ref()),
                                    if mismatches.len() == 1 {
                                        "This is the candidate function"
                                    }else {
                                    "These are candidate functions"},
                                    mismatches
                                        .iter()
                                        .map(|(_, it)| format!("\n• {}", it.signature_to_string()))
                                        .join(""),
                                ),
                            );
                            return LoweredExpression::Error;
                        } else if matches.len() > 1 {
                            self.context.add_error(
                                call.opening_parenthesis_span.clone(),
                                format!(
                                    "Multiple matching function found for:\n  {}\nThese are candidate functions:{}",
                                    FunctionDeclaration::call_signature_to_string(matches.first().unwrap().1.name.as_ref(), argument_types.as_ref()),
                                    matches
                                        .iter()
                                        .map(|(_,it)| format!("\n• {}", it.signature_to_string()))
                                        .join(""),
                                ),
                            );
                            return LoweredExpression::Error;
                        }

                        let (function, signature) = matches[0];
                        self.push_lowered(
                            None,
                            ExpressionKind::Call {
                                function,
                                arguments: arguments.iter().map(|(id, _)| *id).collect(),
                            },
                            signature.return_type.clone(),
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
                    LoweredExpression::TypeReference(type_) => match &type_ {
                        Type::Named(type_name) => {
                            match self.context.hir.type_declarations.get(type_name).unwrap() {
                                TypeDeclaration::Struct { fields } => {
                                    let fields = lower_arguments(
                                        self,
                                        call,
                                        &call.arguments,
                                        &fields
                                            .iter()
                                            .map(|(_, type_)| type_.clone())
                                            .collect_vec(),
                                    );
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
                                TypeDeclaration::Enum { .. } => {
                                    // TODO: report actual error location
                                    self.context.add_error(
                                        Offset(0)..Offset(0),
                                        "Enum variant is missing.",
                                    );
                                    LoweredExpression::Error
                                }
                            }
                        }
                        Type::Error => todo!(),
                    },
                    LoweredExpression::EnumVariantReference { enum_, variant } => {
                        let enum_name = match &enum_ {
                            Type::Named(name) => name,
                            Type::Error => unreachable!(),
                        };
                        let variant_type =
                            match self.context.hir.type_declarations.get(enum_name).unwrap() {
                                TypeDeclaration::Struct { .. } => unreachable!(),
                                TypeDeclaration::Enum { variants } => variants
                                    .iter()
                                    .find(|(name, _)| name == &variant)
                                    .unwrap()
                                    .1
                                    .clone(),
                            };
                        let parameter_types = [variant_type.unwrap()];
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
                        Type::Named(type_name) => {
                            let type_ = &self.context.hir.type_declarations.get(type_name);
                            if let Some(TypeDeclaration::Struct { fields }) = type_
                                && let Some((_, field_type)) =
                                    fields.iter().find(|(name, _)| name == &key.string)
                            {
                                return self.push_lowered(
                                    None,
                                    ExpressionKind::StructAccess {
                                        struct_: receiver_id,
                                        field: key.string.clone(),
                                    },
                                    field_type.clone(),
                                );
                            }

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
                        Type::Error => todo!(),
                    },
                    LoweredExpression::FunctionReferences { .. } => {
                        self.context.add_error(
                            key.span.clone(),
                            format!("Function doesn't have a field `{}`", key.string),
                        );
                        LoweredExpression::Error
                    }
                    LoweredExpression::TypeReference(type_) => match &type_ {
                        Type::Named(type_name) => {
                            match self.context.hir.type_declarations.get(type_name).unwrap() {
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
                                TypeDeclaration::Enum { variants } => {
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
                                                type_.clone(),
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
                        Type::Error => todo!(),
                    },
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

                let variants = match &enum_ {
                    Type::Named(type_name) => {
                        match &self.context.hir.type_declarations[type_name] {
                            TypeDeclaration::Struct { .. } => {
                                // TODO: report actual error location
                                self.context.add_error(
                                    Offset(0)..Offset(0),
                                    format!("Can't switch over struct `{enum_:?}`"),
                                );
                                return LoweredExpression::Error;
                            }
                            TypeDeclaration::Enum { variants } => variants.clone(),
                        }
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
                                Some((value_type.clone(),value_name.string.clone()))
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
                        (variant.string, value_id, body)
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
    TypeReference(Type),
    EnumVariantReference {
        enum_: Type,
        variant: Box<str>,
    },
    Error,
}
