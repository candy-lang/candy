use crate::{
    ast::{
        AstAssignment, AstAssignmentFunction, AstAssignmentKind, AstAssignmentValue, AstExpression,
        AstStatement, AstTextPart,
    },
    error::CompilerError,
    hir::{Body, BuiltinFunction, Definition, Expression, Hir, Id, Parameter, Type},
    id::IdGenerator,
    position::Offset,
    utils::HashMapExtension,
};
use itertools::Itertools;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{
    collections::{hash_map::Entry, BTreeSet},
    mem,
    ops::Range,
    path::Path,
};
use strum::VariantArray;

pub fn ast_to_hir(path: &Path, ast: &[AstAssignment]) -> (Hir, Vec<CompilerError>) {
    let mut context = Context::new(path);
    context.add_builtin_value("type", Expression::Type(Type::Type), Type::Type);
    context.add_builtin_value("symbol", Expression::Type(Type::Symbol), Type::Type);
    context.add_builtin_value("int", Expression::Type(Type::Int), Type::Type);
    context.add_builtin_value("text", Expression::Type(Type::Text), Type::Type);
    context.add_builtin_functions();

    context.catalog_assignments(ast);
    context.lower_assignments();

    let mut identifiers: FxHashMap<_, _> = context
        .global_identifiers
        .iter()
        .flat_map(|(name, named)| match named {
            Named::Value(id) => vec![(*id, name.clone())],
            Named::Functions(functions) => {
                functions.iter().map(|id| (*id, name.clone())).collect_vec()
            }
        })
        .collect();

    let mut hir = Hir::default();
    for (id, definition) in mem::take(&mut context.definitions) {
        let identifier = identifiers.remove(&id).unwrap();
        match definition {
            TempDefinition::Value(ValueDefinition { definition, .. }) => {
                let (type_, value) = definition.expect("Missing definition");
                hir.assignments.push((
                    id,
                    identifier,
                    Definition::Value {
                        type_,
                        value: value.unwrap(),
                    },
                ));
            }
            TempDefinition::Function(FunctionDefinition {
                signature_and_body, ..
            }) => {
                let (parameters, return_type, body) =
                    signature_and_body.expect("Missing signature and body");
                hir.assignments.push((
                    id,
                    identifier.clone(),
                    Definition::Function {
                        parameters,
                        return_type,
                        body: body.expect("Missing body"),
                    },
                ));
            }
        }
    }

    let main_function = hir.assignments.iter().find(|(_, box n, _)| n == "main");
    if let Some((_, _, assignment)) = main_function {
        match assignment.clone() {
            Definition::Function {
                parameters,
                return_type,
                ..
            } => {
                if !parameters.is_empty() {
                    // TODO: report actual error location
                    context.add_error(
                        Offset(0)..Offset(0),
                        "Main function must not have parameters",
                    );
                }
                if !matches!(return_type, Type::Int | Type::Error) {
                    // TODO: report actual error location
                    context.add_error(Offset(0)..Offset(0), "Main function must return an int");
                }
            }
            Definition::Value { value, type_ } => {
                // TODO: report actual error location
                context.add_error(Offset(0)..Offset(0), "`main` function must be a function");
            }
        }
    } else {
        context.add_error(Offset(0)..Offset(0), "Program is missing a main function");
    }

    (hir, context.errors)
}

#[derive(Debug)]
struct Context<'c> {
    path: &'c Path,
    id_generator: IdGenerator<Id>,
    definitions: FxHashMap<Id, TempDefinition<'c>>,
    definitions_to_lower: BTreeSet<Id>,
    global_identifiers: FxHashMap<Box<str>, Named>,
    local_identifiers: Vec<(Box<str>, Id, Option<Definition>, Type)>,
    errors: Vec<CompilerError>,
}
#[derive(Debug)]
enum Named {
    Value(Id),
    Functions(Vec<Id>),
}
#[derive(Debug)]
enum TempDefinition<'a> {
    Value(ValueDefinition<'a>),
    Function(FunctionDefinition<'a>),
}
#[derive(Debug)]
struct ValueDefinition<'a> {
    identifier_span: Range<Offset>,
    ast: Option<&'a AstAssignmentValue>,
    definition: Option<(Type, Option<Expression>)>,
}
#[derive(Debug)]
struct FunctionDefinition<'a> {
    identifier_span: Range<Offset>,
    ast: Option<&'a AstAssignmentFunction>,
    signature_and_body: Option<(Box<[Parameter]>, Type, Option<Body>)>,
}
impl<'c> Context<'c> {
    fn new(path: &'c Path) -> Self {
        Self {
            path,
            id_generator: IdGenerator::start_at(BuiltinFunction::VARIANTS.len()),
            definitions: FxHashMap::default(),
            definitions_to_lower: BTreeSet::default(),
            global_identifiers: FxHashMap::default(),
            local_identifiers: vec![],
            errors: vec![],
        }
    }

    fn add_builtin_value(
        &mut self,
        name: impl Into<Box<str>>,
        expression: Expression,
        type_: Type,
    ) {
        let id = self.id_generator.generate();
        self.definitions.force_insert(
            id,
            TempDefinition::Value(ValueDefinition {
                identifier_span: Offset(0)..Offset(0),
                ast: None,
                definition: Some((type_, Some(expression))),
            }),
        );
        self.global_identifiers
            .force_insert(name.into(), Named::Value(id));
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
                        type_: Type::Int,
                    },
                    Parameter {
                        id: b_id,
                        name: "b".into(),
                        type_: Type::Int,
                    },
                ],
                Type::Int,
            );
        }
        {
            // TODO: Return `Nothing`
            let message_id = self.id_generator.generate();
            self.add_builtin_function(
                BuiltinFunction::Print,
                [Parameter {
                    id: message_id,
                    name: "message".into(),
                    type_: Type::Text,
                }],
                Type::Int,
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
                        type_: Type::Text,
                    },
                    Parameter {
                        id: b_id,
                        name: "b".into(),
                        type_: Type::Text,
                    },
                ],
                Type::Text,
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
        self.definitions.force_insert(
            id,
            TempDefinition::Function(FunctionDefinition {
                identifier_span: Offset(0)..Offset(0),
                ast: None,
                signature_and_body: Some((
                    parameters,
                    return_type,
                    Some(Body::Builtin(builtin_function)),
                )),
            }),
        );
        self.global_identifiers
            .force_insert(name.into(), Named::Functions(vec![id]));
    }

    fn catalog_assignments(&mut self, assignments: &'c [AstAssignment]) {
        for assignment in assignments {
            self.catalog_assignment(assignment);
        }
    }
    fn catalog_assignment(&mut self, assignment: &'c AstAssignment) {
        let Some(name) = assignment.name.value().and_then(|it| it.identifier.value()) else {
            return;
        };

        let id = self.id_generator.generate();
        let definition = match &assignment.kind {
            AstAssignmentKind::Value(ast) => {
                match self.global_identifiers.entry(name.string.clone()) {
                    Entry::Occupied(entry) => match entry.get() {
                        Named::Functions(_) => {
                            self.add_error(name.span.clone(), "A top-level value can't have the same name as a top-level function.");
                            return;
                        }
                        Named::Value { .. } => {
                            self.add_error(
                                name.span.clone(),
                                "Two top-level values can't have the same name.",
                            );
                            return;
                        }
                    },
                    Entry::Vacant(entry) => {
                        entry.insert(Named::Value(id));
                    }
                }
                TempDefinition::Value(ValueDefinition {
                    identifier_span: name.span.clone(),
                    ast: Some(ast),
                    definition: None,
                })
            }
            AstAssignmentKind::Function(ast) => {
                let function = FunctionDefinition {
                    identifier_span: name.span.clone(),
                    ast: Some(ast),
                    signature_and_body: None,
                };
                match self.global_identifiers.entry(name.string.clone()) {
                    Entry::Occupied(mut entry) => match entry.get_mut() {
                        Named::Functions(functions) => {
                            functions.push(id);
                        }
                        Named::Value { .. } => {
                            self.add_error(name.span.clone(), "A top-level function can't have the same name as a top-level value.");
                            return;
                        }
                    },
                    Entry::Vacant(entry) => {
                        entry.insert(Named::Functions(vec![id]));
                    }
                }
                TempDefinition::Function(function)
            }
        };
        self.definitions.force_insert(id, definition);
        self.definitions_to_lower.insert(id);
    }
    fn lower_assignments(&mut self) {
        while let Some(id) = self.definitions_to_lower.pop_last() {
            let old_local_assignments = mem::take(&mut self.local_identifiers);

            let definition = self.definitions.get(&id).unwrap();
            match definition {
                TempDefinition::Value(ValueDefinition {
                    identifier_span,
                    ast,
                    definition,
                }) => {
                    if let Some((_, Some(_))) = definition {
                        continue;
                    }

                    let identifier_span = identifier_span.clone();
                    let ast = ast.unwrap();

                    let explicit_type =
                        ast.type_
                            .as_ref()
                            .and_then(|type_| type_.value())
                            .map(|type_| {
                                let (type_value, type_type) = self
                                    .lower_expression(type_)
                                    .unwrap_or((Expression::Type(Type::Error), Type::Type));
                                if !matches!(type_type, Type::Type | Type::Error) {
                                    self.add_error(
                                        identifier_span,
                                        "Assignment's type must be a type",
                                    );
                                }
                                self.evaluate_expression_to_type(&type_value)
                            });
                    if let Some(explicit_type) = explicit_type.as_ref() {
                        match self.definitions.get_mut(&id).unwrap() {
                            TempDefinition::Value(ValueDefinition { definition, .. }) => {
                                *definition = Some((explicit_type.clone(), None));
                            }
                            TempDefinition::Function(_) => unreachable!(),
                        }
                    }

                    let (value, value_type) = ast
                        .value
                        .value()
                        .and_then(|it| self.lower_expression(it))
                        .unwrap_or((Expression::Error, Type::Error));
                    // TODO: check `value_type` is assignable to `explicit_type`
                    match self.definitions.get_mut(&id).unwrap() {
                        TempDefinition::Value(ValueDefinition { definition, .. }) => {
                            *definition = Some((explicit_type.unwrap_or(value_type), Some(value)));
                        }
                        TempDefinition::Function(_) => unreachable!(),
                    }
                }
                TempDefinition::Function(FunctionDefinition {
                    identifier_span,
                    ast,
                    signature_and_body,
                }) => 'case: {
                    let ast = ast.unwrap();
                    self.with_scope(|context| {
                        // TODO: lower parameter types
                        assert!(ast.parameters.is_empty());
                        // let mut parameter_names = FxHashSet::default();
                        // let parameters = parameters
                        //     .iter()
                        //     .map(|parameter| try {
                        //         let name = parameter.name.identifier.value()?.clone();
                        //         if !parameter_names.insert(name.clone()) {
                        //             context.add_error(
                        //                 name.span.clone(),
                        //                 format!("Duplicate parameter name: {}", *name),
                        //             );
                        //             return None;
                        //         }

                        //         todo!();

                        //         // let type_ = context.lower_expression(parameter.type_.as_ref()?.value()?)?;
                        //         // let id = context.id_generator.generate();
                        //         // context.define_variable(name.string.clone(), id);
                        //         // Parameter {
                        //         //     name: name.string,
                        //         //     type_,
                        //         // }
                        //     })
                        //     .collect::<Option<Box<[_]>>>()?;

                        let return_type = ast
                            .return_type
                            .value()
                            .and_then(|it| {
                                context
                                    .lower_expression(it)
                                    .map(|(value, _)| context.evaluate_expression_to_type(&value))
                            })
                            .unwrap_or(Type::Error);
                        match context.definitions.get_mut(&id).unwrap() {
                            TempDefinition::Value(_) => unreachable!(),
                            TempDefinition::Function(FunctionDefinition {
                                signature_and_body,
                                ..
                            }) => {
                                *signature_and_body = Some(([].into(), return_type.clone(), None));
                            }
                        }

                        let (body, _) = context.lower_body(&ast.body);
                        // TODO: check body's return type is assignable to `return_type`
                        match context.definitions.get_mut(&id).unwrap() {
                            TempDefinition::Value(_) => unreachable!(),
                            TempDefinition::Function(FunctionDefinition {
                                signature_and_body,
                                ..
                            }) => {
                                *signature_and_body = Some(([].into(), return_type, Some(body)));
                            }
                        };
                    });
                }
            }

            self.local_identifiers = old_local_assignments;
        }
    }

    fn evaluate_expression_to_type(&mut self, expression: &Expression) -> Type {
        match expression {
            Expression::Symbol(_) => todo!(),
            Expression::Int(_) => {
                // TODO: report actual error location
                self.add_error(Offset(0)..Offset(0), "Expected a type, not an int");
                Type::Error
            }
            Expression::Text(_) => {
                // TODO: report actual error location
                self.add_error(Offset(0)..Offset(0), "Expected a type, not a text");
                Type::Error
            }
            Expression::Struct(_) => todo!(),
            Expression::StructAccess { struct_, field } => todo!(),
            Expression::ValueWithTypeAnnotation { value, type_ } => todo!(),
            Expression::Lambda { parameters, body } => todo!(),
            Expression::Reference(id) => {
                if let Some(assignment) = self
                    .local_identifiers
                    .iter()
                    .find(|(_, i, _, _)| i == id)
                    .map(|(_, _, assignment, _)| {
                        assignment.clone().expect("TODO: ID belongs to a parameter")
                    })
                {
                    match assignment {
                        Definition::Value { value, .. } => self.evaluate_expression_to_type(&value),
                        Definition::Function {
                            parameters,
                            return_type,
                            body,
                        } => todo!(),
                    }
                } else if let Some(definition) = self.definitions.get(id) {
                    match definition {
                        TempDefinition::Value(ValueDefinition { definition, .. }) => {
                            let Some(value) =
                                definition.as_ref().and_then(|(_, value)| value.as_ref())
                            else {
                                // TODO: report actual error location
                                self.add_error(
                                    Offset(0)..Offset(0),
                                    "Recursion while resolving type",
                                );
                                return Type::Error;
                            };
                            self.evaluate_expression_to_type(&value.clone())
                        }
                        TempDefinition::Function(_) => {
                            // TODO: report actual error location
                            self.add_error(
                                Offset(0)..Offset(0),
                                "Function reference is not a valid type",
                            );
                            return Type::Error;
                        }
                    }
                } else {
                    unreachable!("ID not found");
                }
            }
            Expression::Call {
                receiver,
                arguments,
            } => todo!(),
            Expression::Type(type_) => type_.clone(),
            Expression::Error => Type::Error,
        }
    }

    fn lower_body(&mut self, body: &[AstStatement]) -> (Body, Type) {
        let mut expressions = vec![];
        for statement in body {
            let (name, expression, type_) = match statement {
                AstStatement::Assignment(assignment) => {
                    let Some(name) = assignment
                        .name
                        .value()
                        .and_then(|it| it.identifier.value())
                        .cloned()
                    else {
                        continue;
                    };

                    let (expression, type_) = match &assignment.kind {
                        AstAssignmentKind::Value(AstAssignmentValue { value, type_: _ }) => {
                            // TODO: lower written type
                            if let Some((value, type_)) =
                                value.value().and_then(|it| self.lower_expression(it))
                            {
                                (
                                    Expression::ValueWithTypeAnnotation {
                                        value: Box::new(value),
                                        type_: type_.clone(),
                                    },
                                    type_,
                                )
                            } else {
                                (Expression::Error, Type::Error)
                            }
                        }
                        AstAssignmentKind::Function { .. } => todo!(),
                    };
                    (Some(name.string), expression, type_)
                }
                AstStatement::Expression(expression) => {
                    let Some((expression, type_)) = self.lower_expression(expression) else {
                        continue;
                    };

                    (None, expression, type_)
                }
            };

            let id = self.id_generator.generate();
            if let Some(name) = &name {
                self.local_identifiers.push((
                    name.clone(),
                    id,
                    Some(Definition::Value {
                        value: expression.clone(),
                        type_: type_.clone(),
                    }),
                    type_.clone(),
                ));
            }
            expressions.push((id, name, expression, type_));
        }

        if expressions.is_empty() {
            // TODO: report actual error location
            self.add_error(Offset(0)..Offset(0), "Body must not be empty");
        }
        let return_type = expressions
            .last()
            .map_or(Type::Error, |(_, _, _, type_)| type_.clone());
        (Body::Written { expressions }, return_type)
    }

    fn lower_expression(&mut self, expression: &AstExpression) -> Option<(Expression, Type)> {
        let (expression, type_) = match expression {
            AstExpression::Identifier(identifier) => {
                let identifier = identifier.identifier.value()?;
                let name = &identifier.string;
                if let Some((id, type_)) = self.lookup_local_identifier(identifier) {
                    (Expression::Reference(id), type_.clone())
                } else if let Some(named) = self.global_identifiers.get(name) {
                    match named {
                        Named::Value(id) => {
                            let definition = match self.definitions.get(id).unwrap() {
                                TempDefinition::Value(ValueDefinition { definition, .. }) => {
                                    definition
                                }
                                TempDefinition::Function(_) => unreachable!(),
                            };

                            let id = *id;
                            let type_ = if let Some((type_, _)) = definition {
                                type_.clone()
                            } else {
                                self.add_error(
                                    identifier.span.clone(),
                                    "Missing type in recursion",
                                );
                                Type::Error
                            };
                            (Expression::Reference(id), type_)
                        }
                        Named::Functions(functions) => {
                            assert!(!functions.is_empty());
                            if functions.len() > 1 {
                                self.add_error(
                                    identifier.span.clone(),
                                    "Function overloads are not yet supported",
                                );
                                (Expression::Error, Type::Error)
                            } else {
                                let id = functions[0];
                                let signature_and_body = match self.definitions.get(&id).unwrap() {
                                    TempDefinition::Value(_) => unreachable!(),
                                    TempDefinition::Function(FunctionDefinition {
                                        signature_and_body,
                                        ..
                                    }) => signature_and_body,
                                };

                                if let Some((parameter_types, return_type, _)) = signature_and_body
                                {
                                    (
                                        Expression::Reference(id),
                                        Type::Function {
                                            parameter_types: parameter_types
                                                .iter()
                                                .map(|it| it.type_.clone())
                                                .collect(),
                                            return_type: Box::new(return_type.clone()),
                                        },
                                    )
                                } else {
                                    self.add_error(
                                        identifier.span.clone(),
                                        "Missing function signature in recursion",
                                    );
                                    (Expression::Error, Type::Error)
                                }
                            }
                        }
                    }
                } else {
                    self.add_error(identifier.span.clone(), format!("Unknown variable: {name}"));
                    return None;
                }
            }
            AstExpression::Symbol(symbol) => (
                Expression::Symbol(symbol.symbol.value()?.string.clone()),
                Type::Symbol,
            ),
            AstExpression::Int(int) => (Expression::Int(*int.value.value()?), Type::Int),
            AstExpression::Text(text) => {
                let text = text
                    .parts
                    .iter()
                    .map::<Option<Expression>, _>(|it| try {
                        match it {
                            AstTextPart::Text(text) => Expression::Text(text.clone()),
                            AstTextPart::Interpolation { expression, .. } => {
                                let (value, type_) = self.lower_expression(expression.value()?)?;
                                if type_ != Type::Text {
                                    // TODO: report actual error location
                                    self.add_error(
                                        Offset(0)..Offset(0),
                                        "Interpolated expression must be text",
                                    );
                                    return None;
                                }
                                value
                            }
                        }
                    })
                    .reduce(|lhs, rhs| match (lhs, rhs) {
                        (Some(lhs), Some(rhs)) => Some(Expression::Call {
                            receiver: Box::new(Expression::Reference(
                                BuiltinFunction::TextConcat.id(),
                            )),
                            arguments: [lhs, rhs].into(),
                        }),
                        _ => None,
                    })??;
                (text, Type::Text)
            }
            AstExpression::Parenthesized(parenthesized) => {
                return self.lower_expression(parenthesized.inner.value()?);
            }
            AstExpression::Call(call) => {
                let (receiver, receiver_type) = self.lower_expression(&call.receiver)?;
                let arguments = call
                    .arguments
                    .iter()
                    .map(|argument| self.lower_expression(&argument.value))
                    .collect::<Option<Box<[_]>>>()?;
                let type_ = match receiver_type {
                    Type::Type | Type::Symbol | Type::Int | Type::Text | Type::Struct(_) => {
                        // TODO: report actual error location
                        self.add_error(Offset(0)..Offset(0), "Cannot call this type");
                        return None;
                    }
                    Type::Function {
                        box return_type, ..
                    } => return_type,
                    Type::Error => Type::Error,
                };
                (
                    Expression::Call {
                        receiver: Box::new(receiver),
                        arguments: arguments.iter().map(|(value, _)| value.clone()).collect(),
                    },
                    type_,
                )
            }
            AstExpression::Struct(struct_) => {
                let mut keys = FxHashSet::default();
                let fields = struct_
                    .fields
                    .iter()
                    .map(|field| try {
                        let name = field.key.identifier.value()?;
                        if !keys.insert(name.clone()) {
                            self.add_error(
                                name.span.clone(),
                                format!("Duplicate struct field: {}", **name),
                            );
                            return None;
                        }

                        let (value, type_) =
                            self.lower_expression(field.value.value()?.as_ref())?;
                        (name.string.clone(), value, type_)
                    })
                    .collect::<Option<Vec<(Box<str>, Expression, Type)>>>()?;
                let type_ = if fields.iter().any(|(_, _, type_)| type_ == &Type::Type) {
                    Type::Type
                } else {
                    Type::Struct(
                        fields
                            .iter()
                            .map(|(name, _, type_)| (name.clone(), type_.clone()))
                            .collect(),
                    )
                };
                let fields = fields
                    .into_iter()
                    .map(|(name, value, _)| (name, value))
                    .collect();
                (Expression::Struct(fields), type_)
            }
            AstExpression::StructAccess(struct_access) => {
                let (struct_, struct_type) =
                    self.lower_expression(struct_access.struct_.as_ref())?;
                let field = struct_access
                    .key
                    .value()?
                    .identifier
                    .value()?
                    .string
                    .clone();
                let type_ = match struct_type {
                    Type::Type | Type::Symbol | Type::Int | Type::Text | Type::Function { .. } => {
                        // TODO: report actual error location
                        self.add_error(Offset(0)..Offset(0), "Receiver is not a struct");
                        Type::Error
                    }
                    Type::Struct(struct_type) => {
                        struct_type
                            .iter()
                            .find_map(|(name, type_)| {
                                if name == &field {
                                    Some(type_.clone())
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_else(|| {
                                // TODO: report actual error location
                                self.add_error(
                                    Offset(0)..Offset(0),
                                    format!("Struct does not have field `{field}`"),
                                );
                                Type::Error
                            })
                    }
                    Type::Error => todo!(),
                };
                (
                    Expression::StructAccess {
                        struct_: Box::new(struct_),
                        field,
                    },
                    type_,
                )
            }
            AstExpression::Lambda(_) => todo!(),
        };
        Some((expression, type_))
    }

    // Utils
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
            .find(|(box variable_name, _, _, _)| variable_name == name)
            .map(|(_, id, _, type_)| (*id, type_))
    }

    fn add_error(&mut self, span: Range<Offset>, message: impl Into<String>) {
        self.errors.push(CompilerError {
            path: self.path.to_path_buf(),
            span,
            message: message.into(),
        });
    }
}
