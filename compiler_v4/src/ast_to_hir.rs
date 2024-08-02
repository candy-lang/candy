use crate::{
    ast::{
        Ast, AstArgument, AstAssignment, AstBody, AstDeclaration, AstEnum, AstExpression,
        AstFunction, AstNamedType, AstParameter, AstStatement, AstStruct, AstTextPart, AstType,
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
use rustc_hash::{FxHashMap, FxHashSet};
use std::{collections::hash_map::Entry, ops::Range, path::Path};
use strum::VariantArray;

pub fn ast_to_hir(path: &Path, ast: &Ast) -> (Hir, Vec<CompilerError>) {
    let mut context = Context::new(path, ast);
    context.add_builtin_functions();
    context.lower_declarations();

    if let Some(named) = context.global_identifiers.get("main") {
        match named.clone() {
            Named::Assignment(_) => {
                // TODO: report actual error location
                context.add_error(Offset(0)..Offset(0), "`main` must be a function");
            }
            Named::Functions(ids) => {
                assert!(!ids.is_empty());

                let ValueDeclaration::Function(function) = context
                    .value_declarations
                    .get(ids.first().unwrap())
                    .unwrap()
                else {
                    unreachable!();
                };

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
struct Context<'c> {
    path: &'c Path,
    ast: &'c Ast,
    id_generator: IdGenerator<Id>,
    value_declarations: FxHashMap<Id, ValueDeclaration<'c>>,
    global_identifiers: FxHashMap<Box<str>, Named>,
    errors: Vec<CompilerError>,
    hir: Hir,
}
#[derive(Debug)]
enum Named {
    Assignment(Id),
    Functions(Vec<Id>),
}
#[derive(Debug)]
enum ValueDeclaration<'a> {
    Assignment(AssignmentDeclaration<'a>),
    Function(FunctionDeclaration<'a>),
}
#[derive(Debug)]
struct AssignmentDeclaration<'a> {
    ast: &'a AstAssignment,
    type_: Type,
    body: Option<Body>,
}
#[derive(Debug)]
struct FunctionDeclaration<'a> {
    ast: Option<&'a AstFunction>,
    parameters: Box<[Parameter]>,
    return_type: Type,
    body: Option<BodyOrBuiltin>,
}

impl<'c> Context<'c> {
    fn new(path: &'c Path, ast: &'c Ast) -> Self {
        Self {
            path,
            ast,
            id_generator: IdGenerator::start_at(BuiltinFunction::VARIANTS.len()),
            value_declarations: FxHashMap::default(),
            global_identifiers: FxHashMap::default(),
            errors: vec![],
            hir: Hir::default(),
        }
    }

    fn into_hir(mut self) -> (Hir, Vec<CompilerError>) {
        let mut assignments = vec![];
        let mut functions = vec![];
        for (name, named) in self.global_identifiers {
            let ids = match named {
                Named::Assignment(id) => vec![id],
                Named::Functions(ids) => ids,
            };
            for id in ids {
                let declaration = self.value_declarations.remove(&id).unwrap();
                match declaration {
                    ValueDeclaration::Assignment(AssignmentDeclaration { type_, body, .. }) => {
                        assignments.push((
                            id,
                            name.clone(),
                            Assignment {
                                type_,
                                body: body.unwrap(),
                            },
                        ))
                    }
                    ValueDeclaration::Function(FunctionDeclaration {
                        parameters,
                        return_type,
                        body,
                        ..
                    }) => functions.push((
                        id,
                        name.clone(),
                        Function {
                            parameters,
                            return_type,
                            body: body.unwrap(),
                        },
                    )),
                }
            }
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
        self.value_declarations.force_insert(
            id,
            ValueDeclaration::Function(FunctionDeclaration {
                ast: None,
                parameters,
                return_type,
                body: Some(BodyOrBuiltin::Builtin(builtin_function)),
            }),
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

    fn lower_struct(&mut self, struct_type: &'c AstStruct) {
        let Some(name) = struct_type.name.value() else {
            return;
        };

        let fields = struct_type
            .fields
            .iter()
            .filter_map(|field| {
                let Some(name) = field.name.value() else {
                    return None;
                };

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
    fn lower_enum(&mut self, enum_type: &'c AstEnum) {
        let Some(name) = enum_type.name.value() else {
            return;
        };

        let variants = enum_type
            .variants
            .iter()
            .filter_map(|variant| {
                let Some(name) = variant.name.value() else {
                    return None;
                };

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

    fn lower_assignment_signature(&mut self, assignment: &'c AstAssignment) -> Option<Id> {
        let Some(name) = assignment.name.value() else {
            return None;
        };

        let id = self.id_generator.generate();
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
        self.value_declarations.force_insert(
            id,
            ValueDeclaration::Assignment(AssignmentDeclaration {
                ast: assignment,
                type_,
                body: None,
            }),
        );
        Some(id)
    }
    fn lower_assignment(&mut self, id: Id) {
        let declaration = self.value_declarations.get(&id).unwrap();
        let ValueDeclaration::Assignment(declaration) = declaration else {
            unreachable!();
        };
        let value = declaration.ast.value.clone();
        let type_ = declaration.type_.clone();

        let hir_body = self.build_body(|builder| {
            if let Some(value) = value.value() {
                builder.lower_expression(value, Some(&type_));
            } else {
                builder.push_error();
            }
        });

        match self.value_declarations.get_mut(&id).unwrap() {
            ValueDeclaration::Assignment(AssignmentDeclaration { body, .. }) => {
                *body = Some(hir_body);
            }
            ValueDeclaration::Function(_) => unreachable!(),
        }
    }

    fn lower_function_signature(&mut self, function: &'c AstFunction) -> Option<Id> {
        let Some(name) = function.name.value() else {
            return None;
        };

        let id = self.id_generator.generate();
        let parameters = self.lower_parameters(&function.parameters);
        let return_type = function
            .return_type
            .as_ref()
            .map_or_else(|| Type::nothing(), |it| self.lower_type(it));
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
        self.value_declarations.force_insert(
            id,
            ValueDeclaration::Function(FunctionDeclaration {
                ast: Some(function),
                parameters,
                return_type,
                body: None,
            }),
        );
        Some(id)
    }
    fn lower_parameters(&mut self, parameters: &'c [AstParameter]) -> Box<[Parameter]> {
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
        let declaration = self.value_declarations.get(&id).unwrap();
        let ValueDeclaration::Function(declaration) = declaration else {
            unreachable!();
        };
        let body = declaration.ast.unwrap().body.clone();
        let parameters = declaration.parameters.clone();
        let return_type = declaration.return_type.clone();

        let hir_body = self.build_body(|builder| {
            for parameter in parameters.iter() {
                builder.push_parameter(parameter.name.clone(), parameter.type_.clone());
            }

            builder.lower_statements(&body, Some(&return_type));
        });

        match self.value_declarations.get_mut(&id).unwrap() {
            ValueDeclaration::Assignment(_) => unreachable!(),
            ValueDeclaration::Function(FunctionDeclaration { body, .. }) => {
                *body = Some(BodyOrBuiltin::Body(hir_body));
            }
        };
    }

    fn is_assignable_to(from: &Type, to: &Type) -> bool {
        match (from, to) {
            (Type::Error, _) | (_, Type::Error) => true,
            (Type::Named(from_name), Type::Named(to_name)) => from_name == to_name,
        }
    }

    fn build_body(&mut self, fun: impl FnOnce(&mut BodyBuilder)) -> Body {
        let mut builder = BodyBuilder::new(self);
        fun(&mut builder);
        builder.body
    }

    fn add_error(&mut self, span: Range<Offset>, message: impl Into<String>) {
        self.errors.push(CompilerError {
            path: self.path.to_path_buf(),
            span,
            message: message.into(),
        });
    }
}

struct BodyBuilder<'c0, 'c1> {
    context: &'c0 mut Context<'c1>,
    local_identifiers: Vec<(Box<str>, Id, Type)>,
    body: Body,
}
impl<'c0, 'c1> BodyBuilder<'c0, 'c1> {
    #[must_use]
    fn new(context: &'c0 mut Context<'c1>) -> Self {
        Self {
            context,
            local_identifiers: vec![],
            body: Body::default(),
        }
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
            LoweredExpression::FunctionReference(_) => {
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
            LoweredExpression::EnumVariantReference { .. } => {
                // TODO: report actual error location
                self.context
                    .add_error(Offset(0)..Offset(0), "Enum variant must be instantiated.");
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
                            let type_ = match self.context.value_declarations.get(&id).unwrap() {
                                ValueDeclaration::Assignment(AssignmentDeclaration {
                                    type_,
                                    ..
                                }) => type_.clone(),
                                ValueDeclaration::Function(_) => unreachable!(),
                            };
                            self.push_lowered(None, ExpressionKind::Reference(id), type_.clone())
                        }
                        Named::Functions(functions) => {
                            assert!(!functions.is_empty());
                            if functions.len() > 1 {
                                self.context.add_error(
                                    identifier.span.clone(),
                                    "Function overloads are not yet supported",
                                );
                                LoweredExpression::Error
                            } else {
                                LoweredExpression::FunctionReference(functions[0])
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
                                receiver: BuiltinFunction::TextConcat.id(),
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
                let receiver = self.lower_expression_raw(&call.receiver, None);

                fn lower_arguments(
                    builder: &mut BodyBuilder,
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
                        // TODO: report actual error location
                        builder.context.add_error(
                            Offset(0)..Offset(0),
                            format!(
                                "Expected {} argument(s), got {}.",
                                parameter_types.len(),
                                arguments.len(),
                            ),
                        );
                        None
                    }
                }

                match receiver {
                    LoweredExpression::Expression { .. } => {
                        // TODO: report actual error location
                        self.context
                            .add_error(Offset(0)..Offset(0), "Cannot call this type");
                        LoweredExpression::Error
                    }
                    LoweredExpression::FunctionReference(function_id) => {
                        match self.context.value_declarations.get(&function_id).unwrap() {
                            ValueDeclaration::Assignment(_) => unreachable!(),
                            ValueDeclaration::Function(FunctionDeclaration {
                                parameters,
                                return_type,
                                ..
                            }) => {
                                let parameter_types =
                                    parameters.iter().map(|it| it.type_.clone()).collect_vec();
                                let return_type = return_type.clone();

                                let arguments =
                                    lower_arguments(self, &call.arguments, &parameter_types);
                                if let Some(arguments) = arguments {
                                    self.push_lowered(
                                        None,
                                        ExpressionKind::Call {
                                            receiver: function_id,
                                            arguments,
                                        },
                                        return_type,
                                    )
                                } else {
                                    LoweredExpression::Error
                                }
                            }
                        }
                    }
                    LoweredExpression::TypeReference(type_) => match &type_ {
                        Type::Named(type_name) => {
                            match self.context.hir.type_declarations.get(type_name).unwrap() {
                                TypeDeclaration::Struct { fields } => {
                                    let fields = lower_arguments(
                                        self,
                                        &call.arguments,
                                        &fields
                                            .iter()
                                            .map(|(_, type_)| type_.clone())
                                            .collect_vec(),
                                    );
                                    if let Some(fields) = fields {
                                        self.push_lowered(
                                            None,
                                            ExpressionKind::CreateStruct {
                                                struct_: type_.clone(),
                                                fields,
                                            },
                                            type_,
                                        )
                                    } else {
                                        LoweredExpression::Error
                                    }
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
                        let parameter_types = if let Some(variant_type) = variant_type {
                            vec![variant_type]
                        } else {
                            vec![]
                        };
                        let arguments = lower_arguments(self, &call.arguments, &parameter_types);
                        if let Some(arguments) = arguments {
                            self.push_lowered(
                                None,
                                ExpressionKind::CreateEnum {
                                    enum_: enum_.clone(),
                                    variant,
                                    value: arguments.first().copied(),
                                },
                                enum_,
                            )
                        } else {
                            LoweredExpression::Error
                        }
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
                    LoweredExpression::Expression { id, type_ } => match &type_ {
                        Type::Named(type_name) => {
                            match self.context.hir.type_declarations.get(type_name).unwrap() {
                                TypeDeclaration::Struct { fields } => {
                                    if let Some((_, field_type)) =
                                        fields.iter().find(|(name, _)| name == &key.string)
                                    {
                                        self.push_lowered(
                                            None,
                                            ExpressionKind::StructAccess {
                                                struct_: id,
                                                field: key.string.clone(),
                                            },
                                            field_type.clone(),
                                        )
                                    } else {
                                        self.context.add_error(
                                            key.span.clone(),
                                            format!(
                                                "Struct `{type_:?}` doesn't have a field `{}`",
                                                key.string
                                            ),
                                        );
                                        LoweredExpression::Error
                                    }
                                }
                                TypeDeclaration::Enum { .. } => {
                                    self.context.add_error(
                                        key.span.clone(),
                                        format!(
                                            "Enum `{type_:?}` doesn't have a field `{}`",
                                            key.string
                                        ),
                                    );
                                    LoweredExpression::Error
                                }
                            }
                        }
                        Type::Error => todo!(),
                    },
                    LoweredExpression::FunctionReference(_) => {
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
                                    if variants.iter().any(|(name, _)| name == &key.string) {
                                        LoweredExpression::EnumVariantReference {
                                            enum_: type_,
                                            variant: key.string.clone(),
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
    fn push_parameter(&mut self, name: Box<str>, type_: Type) -> Id {
        let id = self.context.id_generator.generate();
        self.local_identifiers.push((name, id, type_.clone()));
        id
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

    fn with_scope<T>(&mut self, fun: impl FnOnce(&mut Self) -> T) -> T {
        let scope = self.local_identifiers.len();
        let result = fun(self);
        assert!(self.local_identifiers.len() >= scope);
        self.local_identifiers.truncate(scope);
        result
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
    Expression { id: Id, type_: Type },
    FunctionReference(Id),
    TypeReference(Type),
    EnumVariantReference { enum_: Type, variant: Box<str> },
    Error,
}
