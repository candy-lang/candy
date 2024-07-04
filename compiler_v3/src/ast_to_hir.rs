use crate::{
    ast::{AstAssignment, AstAssignmentKind, AstExpression, AstStatement, AstString, AstTextPart},
    error::CompilerError,
    hir::{Body, BuiltinFunction, Definition, Expression, Hir, Id, Parameter, Type},
    id::IdGenerator,
    position::Offset,
    utils::HashMapExtension,
};
use rustc_hash::{FxHashMap, FxHashSet};
use std::{collections::hash_map::Entry, mem, ops::Range, path::Path};
use strum::VariantArray;

pub fn ast_to_hir(path: &Path, ast: &[AstAssignment]) -> (Hir, Vec<CompilerError>) {
    let mut context = Context::new(path);
    context.add_builtin_value("type", Expression::Type(Type::Type), Type::Type);
    context.add_builtin_value("symbol", Expression::Type(Type::Symbol), Type::Type);
    context.add_builtin_value("int", Expression::Type(Type::Int), Type::Type);
    context.add_builtin_value("text", Expression::Type(Type::Text), Type::Type);
    context.add_builtin_functions();
    context.lower_assignments(ast);

    let mut hir = Hir::default();
    for (identifier, named) in mem::take(&mut context.definitions) {
        match named {
            Named::Value { id, definition } => {
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
            Named::Functions(functions) => {
                for (id, signature_and_body) in functions {
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
    definitions: FxHashMap<Box<str>, Named>,
    local_identifiers: Vec<(Box<str>, Id, Option<Definition>, Type)>,
    errors: Vec<CompilerError>,
}
#[derive(Debug)]
enum Named {
    Value {
        id: Id,
        definition: Option<(Type, Option<Expression>)>,
    },
    Functions(Vec<(Id, Option<(Box<[Parameter]>, Type, Option<Body>)>)>),
}
impl<'c> Context<'c> {
    fn new(path: &'c Path) -> Self {
        Self {
            path,
            id_generator: IdGenerator::start_at(BuiltinFunction::VARIANTS.len()),
            definitions: FxHashMap::default(),
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
        self.definitions.force_insert(
            name.into(),
            Named::Value {
                id: self.id_generator.generate(),
                definition: Some((type_, Some(expression))),
            },
        );
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
            name.into(),
            Named::Functions(vec![(
                id,
                Some((
                    parameters,
                    return_type,
                    Some(Body::Builtin(builtin_function)),
                )),
            )]),
        );
    }

    fn lower_assignments(&mut self, assignments: &[AstAssignment]) {
        for assignment in assignments {
            self.lower_assignment(assignment);
        }
    }
    fn lower_assignment(&mut self, assignment: &AstAssignment) {
        let Some(name) = assignment.name.value().and_then(|it| it.identifier.value()) else {
            return;
        };

        let old_local_assignments = mem::take(&mut self.local_identifiers);

        match &assignment.kind {
            AstAssignmentKind::Value { type_, value } => 'case: {
                match self.definitions.entry(name.string.clone()) {
                    Entry::Occupied(entry) => match entry.get() {
                        Named::Functions(_) => {
                            self.add_error(name.span.clone(), "A top-level value can't have the same name as a top-level function.");
                            break 'case;
                        }
                        Named::Value { .. } => {
                            self.add_error(
                                name.span.clone(),
                                "Two top-level values can't have the same name.",
                            );
                            break 'case;
                        }
                    },
                    Entry::Vacant(entry) => {
                        entry.insert(Named::Value {
                            id: self.id_generator.generate(),
                            definition: None,
                        });
                    }
                }

                let explicit_type = type_.as_ref().and_then(|type_| type_.value()).map(|type_| {
                    let (type_value, type_type) = self
                        .lower_expression(type_)
                        .unwrap_or((Expression::Type(Type::Error), Type::Type));
                    if !matches!(type_type, Type::Type | Type::Error) {
                        self.add_error(name.span.clone(), "Assignment's type must be a type");
                    }
                    self.evaluate_expression_to_type(&type_value)
                });
                if let Some(explicit_type) = explicit_type.as_ref() {
                    match self.definitions.get_mut(&name.string).unwrap() {
                        Named::Value { definition, .. } => {
                            *definition = Some((explicit_type.clone(), None));
                        }
                        Named::Functions(_) => unreachable!(),
                    }
                }

                let (value, value_type) = value
                    .value()
                    .and_then(|it| self.lower_expression(it))
                    .unwrap_or((Expression::Error, Type::Error));
                // TODO: check `value_type` is assignable to `explicit_type`
                match self.definitions.get_mut(&name.string).unwrap() {
                    Named::Value { definition, .. } => {
                        *definition = Some((explicit_type.unwrap_or(value_type), Some(value)));
                    }
                    Named::Functions(_) => unreachable!(),
                }
            }
            AstAssignmentKind::Function {
                parameters,
                return_type,
                body,
                ..
            } => 'case: {
                let index = match self.definitions.entry(name.string.clone()) {
                    Entry::Occupied(mut entry) => match entry.get_mut() {
                        Named::Functions(functions) => {
                            let index = functions.len();
                            functions.push((self.id_generator.generate(), None));
                            index
                        }
                        Named::Value { .. } => {
                            self.add_error(name.span.clone(), "A top-level function can't have the same name as a top-level value.");
                            break 'case;
                        }
                    },
                    Entry::Vacant(entry) => {
                        entry.insert(Named::Functions(vec![(self.id_generator.generate(), None)]));
                        0
                    }
                };

                self.with_scope(|context| {
                    // TODO: lower parameter types
                    assert!(parameters.is_empty());
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

                    let return_type = return_type
                        .value()
                        .and_then(|it| {
                            context
                                .lower_expression(it)
                                .map(|(value, _)| context.evaluate_expression_to_type(&value))
                        })
                        .unwrap_or(Type::Error);
                    match context.definitions.get_mut(&name.string).unwrap() {
                        Named::Value { .. } => unreachable!(),
                        Named::Functions(functions) => {
                            functions[index].1 = Some(([].into(), return_type.clone(), None));
                        }
                    }

                    let (body, _) = context.lower_body(body);
                    // TODO: check body's return type is assignable to `return_type`
                    match context.definitions.get_mut(&name.string).unwrap() {
                        Named::Value { .. } => unreachable!(),
                        Named::Functions(functions) => {
                            functions[index].1 = Some(([].into(), return_type, Some(body)));
                        }
                    };
                });
            }
        }

        self.local_identifiers = old_local_assignments;
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
                } else {
                    for definition in self.definitions.values() {
                        match definition {
                            Named::Value {
                                id: inner_id,
                                definition,
                            } => {
                                if id != inner_id {
                                    continue;
                                }

                                if let Some(value) =
                                    definition.as_ref().and_then(|(_, value)| value.as_ref())
                                {
                                    return self.evaluate_expression_to_type(&value.clone());
                                }
                                // TODO: report actual error location
                                self.add_error(
                                    Offset(0)..Offset(0),
                                    "Recursion while resolving type",
                                );
                                return Type::Error;
                            }
                            Named::Functions(functions) => {
                                if functions.iter().any(|(i, _)| i == id) {
                                    // TODO: report actual error location
                                    self.add_error(
                                        Offset(0)..Offset(0),
                                        "Function reference is not a valid type",
                                    );
                                    return Type::Error;
                                }
                                continue;
                            }
                        }
                    }
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
                        AstAssignmentKind::Value { value, type_: _ } => {
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
                } else if let Some(named) = self.definitions.get(name) {
                    match named {
                        Named::Value { id, definition } => {
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
                                let (id, signature_and_body) = &functions[0];
                                if let Some((parameter_types, return_type, _)) = signature_and_body
                                {
                                    (
                                        Expression::Reference(*id),
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
