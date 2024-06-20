use crate::{
    ast::{AstAssignment, AstAssignmentKind, AstExpression, AstStatement, AstString, AstTextPart},
    error::CompilerError,
    hir::{Assignment, Body, Expression, Hir, Type},
    position::Offset,
};
use rustc_hash::FxHashSet;
use std::{ops::Range, path::Path};

pub fn ast_to_hir(path: &Path, ast: &[AstAssignment]) -> (Hir, Vec<CompilerError>) {
    let mut context = Context::new(path);
    context.add_builtin_value("type", Expression::Type(Type::Type), Type::Type);
    context.add_builtin_value("symbol", Expression::Type(Type::Symbol), Type::Type);
    context.add_builtin_value("int", Expression::Type(Type::Int), Type::Type);
    context.add_builtin_value("text", Expression::Type(Type::Text), Type::Type);
    context.lower_assignments(ast);
    (context.hir, context.errors)
}

#[derive(Debug)]
struct Context<'c> {
    path: &'c Path,
    identifiers: Vec<(Box<str>, Identifier, Type)>,
    errors: Vec<CompilerError>,
    hir: Hir,
}
#[derive(Debug)]
enum Identifier {
    Variable { value: Expression },
    Parameter,
}
impl<'c> Context<'c> {
    fn new(path: &'c Path) -> Self {
        Self {
            path,
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
                parameters,
                return_type,
                body,
                ..
            } => {
                // self.with_scope(|context| try {
                //     let mut parameter_names = FxHashSet::default();
                //     let parameters = parameters
                //         .iter()
                //         .map(|parameter| try {
                //             let name = parameter.name.identifier.value()?.clone();
                //             if !parameter_names.insert(name.clone()) {
                //                 context.add_error(
                //                     name.span.clone(),
                //                     format!("Duplicate parameter name: {}", *name),
                //                 );
                //                 return None;
                //             }

                //             todo!();

                //             // let type_ = context.lower_expression(parameter.type_.as_ref()?.value()?)?;
                //             // let id = context.id_generator.generate();
                //             // context.define_variable(name.string.clone(), id);
                //             // Parameter {
                //             //     name: name.string,
                //             //     type_,
                //             // }
                //         })
                //         .collect::<Option<Box<[_]>>>()?;

                //     let return_type = context.lower_expression(return_type.value()?.as_ref())?;

                //     // Assignment::Function {
                //     //     parameters,
                //     //     return_type,
                //     //     body: context.lower_body(body)?,
                //     // }
                // });
                todo!()
            }
        };
        Some((name, assignment))
    }

    fn lower_body(&mut self, body: &[AstStatement]) -> Option<Body> {
        let mut hir_body = Body::default();
        for statement in body {
            match statement {
                AstStatement::Assignment(assignment) => {
                    let name = assignment.name.value()?.identifier.value()?.clone();
                    let (expression, type_) = match &assignment.kind {
                        AstAssignmentKind::Value { value, type_: _ } => {
                            // TODO: lower written type
                            let (value, type_) = self.lower_expression(value.value()?)?;
                            (
                                Expression::ValueWithTypeAnnotation {
                                    value: Box::new(value),
                                    type_: type_.clone(),
                                },
                                type_,
                            )
                        }
                        AstAssignmentKind::Function { .. } => todo!(),
                    };
                    self.add_expression_to_body(&mut hir_body, name.string, expression, type_);
                }
                AstStatement::Expression(expression) => {
                    let (expression, type_) = self.lower_expression(expression)?;
                    self.add_expression_to_body(&mut hir_body, None, expression, type_);
                }
            }
        }
        Some(hir_body)
    }

    fn lower_expression(&mut self, expression: &AstExpression) -> Option<(Expression, Type)> {
        let (expression, type_) = match expression {
            AstExpression::Identifier(identifier) => {
                let identifier = identifier.identifier.value()?;
                let name = &identifier.string;
                let Some((identifier, type_)) = self.lookup_identifier(identifier) else {
                    self.add_error(identifier.span.clone(), format!("Unknown variable: {name}"));
                    return None;
                };

                (
                    match identifier {
                        Identifier::Variable { value } => value.clone(),
                        Identifier::Parameter => Expression::ParameterReference(name.clone()),
                    },
                    type_.clone(),
                )
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
                            AstTextPart::Interpolation { expression, .. } => Expression::Call {
                                receiver: Box::new(Expression::BuiltinToDebugText),
                                arguments: [self.lower_expression(expression.value()?)?.0].into(),
                            },
                        }
                    })
                    .reduce(|lhs, rhs| match (lhs, rhs) {
                        (Some(lhs), Some(rhs)) => Some(Expression::Call {
                            receiver: Box::new(Expression::BuiltinTextConcat),
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
    fn define_identifier(&mut self, name: Box<str>, identifier: Identifier, type_: Type) {
        self.identifiers.push((name, identifier, type_));
    }
    #[must_use]
    fn lookup_identifier(&self, name: &str) -> Option<(&Identifier, &Type)> {
        self.identifiers
            .iter()
            .rev()
            .find(|(box variable_name, _, _)| variable_name == name)
            .map(|(_, identifier, type_)| (identifier, type_))
    }

    fn add_assignment(&mut self, name: &AstString, assignment: Assignment) {
        if self.hir.assignments.iter().any(|(n, _)| n == &**name) {
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
        let (identifier, type_) = match &assignment {
            Assignment::Value { value, type_ } => (
                Identifier::Variable {
                    value: value.clone(),
                },
                type_.clone(),
            ),
            Assignment::Function {
                parameters,
                return_type,
                body,
            } => todo!(),
        };
        self.define_identifier(name.clone(), identifier, type_);
        self.hir.assignments.push((name, assignment));
    }
    fn add_expression_to_body(
        &mut self,
        body: &mut Body,
        name: impl Into<Option<Box<str>>>,
        expression: Expression,
        type_: Type,
    ) {
        body.expressions.push((name.into(), expression, type_));
    }

    fn add_error(&mut self, span: Range<Offset>, message: impl Into<String>) {
        self.errors.push(CompilerError {
            path: self.path.to_path_buf(),
            span,
            message: message.into(),
        });
    }
}
