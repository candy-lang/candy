use crate::{
    ast::{AstAssignment, AstAssignmentKind, AstExpression, AstStatement, AstString, AstTextPart},
    error::CompilerError,
    hir::{Assignment, Body, BuiltinFunction, Expression, Hir, Id, Parameter, Type},
    id::IdGenerator,
    position::Offset,
};
use rustc_hash::FxHashSet;
use std::{ops::Range, path::Path};
use strum::VariantArray;

pub fn ast_to_hir(path: &Path, ast: &[AstAssignment]) -> (Hir, Vec<CompilerError>) {
    let mut context = Context::new(path);
    context.add_builtin_value("type", Expression::Type(Type::Type), Type::Type);
    context.add_builtin_value("symbol", Expression::Type(Type::Symbol), Type::Type);
    context.add_builtin_value("int", Expression::Type(Type::Int), Type::Type);
    context.add_builtin_value("text", Expression::Type(Type::Text), Type::Type);
    context.add_builtin_functions();
    context.lower_assignments(ast);

    let main_function = context
        .hir
        .assignments
        .iter()
        .find(|(_, box n, _)| n == "main");
    if let Some((_, _, assignment)) = main_function {
        match assignment.clone() {
            Assignment::Function {
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
            Assignment::Value { value, type_ } => {
                // TODO: report actual error location
                context.add_error(Offset(0)..Offset(0), "`main` function must be a function");
            }
        }
    } else {
        context.add_error(Offset(0)..Offset(0), "Program is missing a main function");
    }

    (context.hir, context.errors)
}

#[derive(Debug)]
struct Context<'c> {
    path: &'c Path,
    id_generator: IdGenerator<Id>,
    identifiers: Vec<(Box<str>, Id, Type)>,
    errors: Vec<CompilerError>,
    hir: Hir,
}
impl<'c> Context<'c> {
    fn new(path: &'c Path) -> Self {
        Self {
            path,
            id_generator: IdGenerator::start_at(BuiltinFunction::VARIANTS.len()),
            identifiers: vec![],
            errors: vec![],
            hir: Hir::default(),
        }
    }

    fn add_builtin_value(
        &mut self,
        name: impl Into<Box<str>>,
        expression: Expression,
        type_: Type,
    ) {
        self.add_assignment_without_duplicate_name_check(
            name,
            Assignment::Value {
                value: expression,
                type_,
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
        let type_ = Type::Function {
            parameter_types: parameters.iter().map(|it| it.type_.clone()).collect(),
            return_type: Box::new(return_type.clone()),
        };
        self.identifiers.push((name.into(), id, type_));
        let assignment = Assignment::Function {
            parameters,
            return_type,
            body: Body::Builtin(builtin_function),
        };
        self.hir.assignments.push((id, name.into(), assignment));
    }

    fn lower_assignments(&mut self, assignments: &[AstAssignment]) {
        for assignment in assignments {
            let Some((name, assignment)) = self.lower_assignment(assignment) else {
                continue;
            };

            if let Some(assignment) = assignment {
                self.add_assignment(name, assignment);
            } else {
                self.add_assignment(
                    name,
                    Assignment::Value {
                        value: Expression::Error,
                        type_: Type::Error,
                    },
                );
            }
        }
    }
    fn lower_assignment<'a>(
        &mut self,
        assignment: &'a AstAssignment,
    ) -> Option<(&'a AstString, Option<Assignment>)> {
        let name = assignment.name.value()?.identifier.value()?;
        let assignment = match &assignment.kind {
            AstAssignmentKind::Value { type_, value } => {
                try {
                    // TODO: lower written type
                    let (value, type_) = self.lower_expression(value.value()?)?;
                    Assignment::Value { value, type_ }
                }
            }
            AstAssignmentKind::Function {
                parameters, body, ..
            } => {
                self.with_scope(|context| try {
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

                    // TODO: lower written return type
                    // let return_type = context.lower_expression(return_type.value()?.as_ref())?;

                    let (body, return_type) = context.lower_body(body);
                    Assignment::Function {
                        parameters: [].into(),
                        return_type,
                        body,
                    }
                })
            }
        };
        Some((name, assignment))
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
                self.identifiers.push((name.clone(), id, type_.clone()));
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
                let Some((id, type_)) = self.lookup_identifier(identifier) else {
                    self.add_error(identifier.span.clone(), format!("Unknown variable: {name}"));
                    return None;
                };

                (Expression::Reference(id), type_.clone())
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
        let scope = self.identifiers.len();
        let result = fun(self);
        assert!(self.identifiers.len() >= scope);
        self.identifiers.truncate(scope);
        result
    }
    #[must_use]
    fn lookup_identifier(&self, name: &str) -> Option<(Id, &Type)> {
        self.identifiers
            .iter()
            .rev()
            .find(|(box variable_name, _, _)| variable_name == name)
            .map(|(_, id, type_)| (*id, type_))
    }

    fn add_assignment(&mut self, name: &AstString, assignment: Assignment) {
        if self.hir.assignments.iter().any(|(_, n, _)| n == &**name) {
            self.add_error(
                name.span.clone(),
                format!("Duplicate assignment: {}", **name),
            );
            return;
        }

        self.add_assignment_without_duplicate_name_check(name.string.clone(), assignment);
    }
    fn add_assignment_without_duplicate_name_check(
        &mut self,
        name: impl Into<Box<str>>,
        assignment: Assignment,
    ) {
        let name = name.into();
        let id = self.id_generator.generate();
        let type_ = match &assignment {
            Assignment::Value { type_, .. } => type_.clone(),
            Assignment::Function {
                parameters,
                return_type,
                ..
            } => Type::Function {
                parameter_types: parameters.iter().map(|it| it.type_.clone()).collect(),
                return_type: Box::new(return_type.clone()),
            },
        };
        self.identifiers.push((name.clone(), id, type_));
        self.hir.assignments.push((id, name, assignment));
    }

    fn add_error(&mut self, span: Range<Offset>, message: impl Into<String>) {
        self.errors.push(CompilerError {
            path: self.path.to_path_buf(),
            span,
            message: message.into(),
        });
    }
}
