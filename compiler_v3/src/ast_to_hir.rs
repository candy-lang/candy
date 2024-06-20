use crate::{
    ast::{AstAssignment, AstAssignmentKind, AstExpression, AstStatement, AstString},
    error::CompilerError,
    hir::{Assignment, Body, Expression, Hir, Id, Parameter},
    id::IdGenerator,
    position::Offset,
};
use rustc_hash::FxHashSet;
use std::{ops::Range, path::Path};

pub fn ast_to_hir(path: &Path, ast: &[AstAssignment]) -> (Hir, Vec<CompilerError>) {
    let mut context = Context::new(path);
    context.add_builtin_value("int", Expression::IntType);
    context.add_builtin_value("text", Expression::TextType);
    context.lower_assignments(ast);
    (context.hir, context.errors)
}

#[derive(Debug)]
struct Context<'c> {
    path: &'c Path,
    id_generator: IdGenerator<Id>,
    variables: Vec<(Box<str>, Id)>,
    errors: Vec<CompilerError>,
    hir: Hir,
}
impl<'c> Context<'c> {
    fn new(path: &'c Path) -> Self {
        Self {
            path,
            id_generator: IdGenerator::default(),
            variables: vec![],
            errors: vec![],
            hir: Hir::default(),
        }
    }

    fn add_builtin_value(&mut self, name: impl Into<Box<str>>, expression: Expression) {
        let value = self.expression_to_body(expression);
        self.add_assignment_without_duplicate_name_check(
            name,
            Assignment::Value {
                type_: value.clone(),
                value,
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
                let value = self.expression_to_body(Expression::Error);
                self.add_assignment(
                    name,
                    Assignment::Value {
                        type_: value.clone(),
                        value,
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
                    Assignment::Value {
                        type_: self.lower_expression(type_.as_ref()?.value()?)?,
                        value: self.lower_expression(value.value()?)?,
                    }
                }
            }
            AstAssignmentKind::Function {
                parameters,
                return_type,
                body,
                ..
            } => self.with_scope::<Option<Assignment>>(|context| try {
                let mut parameter_names = FxHashSet::default();
                let parameters = parameters
                    .iter()
                    .map(|parameter| try {
                        let name = parameter.name.identifier.value()?.clone();
                        if !parameter_names.insert(name.clone()) {
                            context.push_error(
                                name.span.clone(),
                                format!("Duplicate parameter name: {}", *name),
                            );
                            return None;
                        }

                        let type_ = context.lower_expression(parameter.type_.as_ref()?.value()?)?;
                        let id = context.id_generator.generate();
                        context.define_variable(name.string.clone(), id);
                        Parameter {
                            id,
                            name: name.string,
                            type_,
                        }
                    })
                    .collect::<Option<Box<[_]>>>()?;

                let return_type = context.lower_expression(return_type.value()?.as_ref())?;

                Assignment::Function {
                    parameters,
                    return_type,
                    body: context.lower_body(body)?,
                }
            }),
        };
        Some((name, assignment))
    }

    fn lower_body(&mut self, body: &[AstStatement]) -> Option<Body> {
        let mut hir_body = Body::default();
        for statement in body {
            match statement {
                AstStatement::Assignment(assignment) => {
                    let name = assignment.name.value()?.identifier.value()?.clone();
                    let expression = match &assignment.kind {
                        AstAssignmentKind::Value { type_, value } => {
                            let type_ = self
                                .lower_expression_into(type_.as_ref()?.value()?, &mut hir_body)?;
                            let value =
                                self.lower_expression_into(value.value()?, &mut hir_body)?;
                            Expression::ValueWithTypeAnnotation { value, type_ }
                        }
                        AstAssignmentKind::Function { .. } => todo!(),
                    };
                    let id = self.add_expression_to_body(expression, &mut hir_body);
                    hir_body.identifiers.push((id, name));
                }
                AstStatement::Expression(expression) => {
                    self.lower_expression_into(expression, &mut hir_body);
                }
            }
        }
        Some(hir_body)
    }

    fn lower_expression(&mut self, expression: &AstExpression) -> Option<Body> {
        let mut body = Body::default();
        self.lower_expression_into(expression, &mut body)?;
        Some(body)
    }
    fn lower_expression_into(&mut self, expression: &AstExpression, body: &mut Body) -> Option<Id> {
        let expression = match expression {
            AstExpression::Identifier(identifier) => {
                let identifier = identifier.identifier.value()?;
                let Some(id) = self.lookup_variable(identifier) else {
                    self.push_error(
                        identifier.span.clone(),
                        format!("Unknown variable: {}", **identifier),
                    );
                    return None;
                };
                Expression::Reference(id)
            }
            AstExpression::Symbol(symbol) => {
                Expression::Symbol(symbol.symbol.value()?.string.clone())
            }
            AstExpression::Int(int) => Expression::Int(*int.value.value()?),
            AstExpression::Text(_) => todo!(),
            AstExpression::Parenthesized(parenthesized) => {
                return self.lower_expression_into(parenthesized.inner.value()?, body);
            }
            AstExpression::Call(call) => {
                let receiver = self.lower_expression_into(&call.receiver, body)?;
                let arguments = call
                    .arguments
                    .iter()
                    .map(|argument| self.lower_expression_into(&argument.value, body))
                    .collect::<Option<Box<[_]>>>()?;
                Expression::Call(receiver, arguments)
            }
            AstExpression::Struct(struct_) => {
                let mut keys = FxHashSet::default();
                let fields = struct_
                    .fields
                    .iter()
                    .map(|field| try {
                        let name = field.key.identifier.value()?;
                        if !keys.insert(name.clone()) {
                            self.push_error(
                                name.span.clone(),
                                format!("Duplicate struct field: {}", **name),
                            );
                            return None;
                        }

                        let value =
                            self.lower_expression_into(field.value.value()?.as_ref(), body)?;
                        (name.string.clone(), value)
                    })
                    .collect::<Option<Box<[_]>>>()?;
                Expression::Struct(fields)
            }
            AstExpression::StructAccess(struct_access) => {
                let struct_ = self.lower_expression_into(struct_access.struct_.as_ref(), body)?;
                let field = struct_access
                    .key
                    .value()?
                    .identifier
                    .value()?
                    .string
                    .clone();
                Expression::StructAccess { struct_, field }
            }
            AstExpression::Lambda(_) => todo!(),
        };
        Some(self.add_expression_to_body(expression, body))
    }

    // Utils
    fn with_scope<T>(&mut self, fun: impl FnOnce(&mut Self) -> T) -> T {
        let scope = self.variables.len();
        let result = fun(self);
        assert!(self.variables.len() >= scope);
        self.variables.truncate(scope);
        result
    }
    fn define_variable(&mut self, name: Box<str>, id: Id) {
        self.variables.push((name, id));
    }
    #[must_use]
    fn lookup_variable(&self, name: &str) -> Option<Id> {
        self.variables
            .iter()
            .rev()
            .find_map(|(box variable_name, id)| {
                if variable_name == name {
                    Some(*id)
                } else {
                    None
                }
            })
    }

    fn expression_to_body(&mut self, expression: Expression) -> Body {
        Body {
            identifiers: vec![],
            expressions: vec![(self.id_generator.generate(), expression)],
        }
    }
    fn add_assignment(&mut self, name: &AstString, assignment: Assignment) {
        if self.hir.assignments.iter().any(|(_, n, _)| n == &**name) {
            self.push_error(
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
        let id = self.id_generator.generate();
        let name = name.into();
        self.define_variable(name.clone(), id);
        self.hir.assignments.push((id, name, assignment));
    }
    fn add_expression_to_body(&mut self, expression: Expression, body: &mut Body) -> Id {
        let id = self.id_generator.generate();
        body.expressions.push((id, expression));
        id
    }

    fn push_error(&mut self, span: Range<Offset>, message: impl Into<String>) {
        self.errors.push(CompilerError {
            path: self.path.to_path_buf(),
            span,
            message: message.into(),
        });
    }
}
