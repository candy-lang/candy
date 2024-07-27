use crate::{error::CompilerError, position::Offset};
use derive_more::Deref;
use std::{ops::Range, path::PathBuf};

#[derive(Clone, Debug, Deref, Eq, Hash, PartialEq)]
pub struct AstString {
    #[deref]
    pub string: Box<str>,
    pub file: PathBuf,
    pub span: Range<Offset>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AstResult<T> {
    value: Option<T>,
    errors: Vec<AstError>,
}
impl<T> AstResult<T> {
    #[must_use]
    pub const fn ok(value: T) -> Self {
        Self {
            value: Some(value),
            errors: vec![],
        }
    }
    #[must_use]
    pub fn error(value: impl Into<Option<T>>, error: AstError) -> Self {
        Self::errors(value, vec![error])
    }
    #[must_use]
    pub fn errors(value: impl Into<Option<T>>, errors: Vec<AstError>) -> Self {
        Self {
            value: value.into(),
            errors,
        }
    }

    #[must_use]
    pub const fn value(&self) -> Option<&T> {
        self.value.as_ref()
    }

    #[must_use]
    pub fn map<U>(self, op: impl FnOnce(T) -> U) -> AstResult<U> {
        AstResult {
            value: self.value.map(op),
            errors: self.errors,
        }
    }
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AstError {
    pub unparsable_input: AstString,
    pub error: String,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum AstStatement {
    Expression(AstExpression),
    Assignment(AstAssignment),
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum AstExpression {
    Identifier(AstIdentifier),
    Symbol(AstSymbol),
    Int(AstInt),
    Text(AstText),
    Parenthesized(AstParenthesized),
    Call(AstCall),
    Struct(AstStruct),
    StructAccess(AstStructAccess),
    Lambda(AstLambda),
    Or(AstOr),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AstIdentifier {
    pub identifier: AstResult<AstString>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AstSymbol {
    pub symbol: AstResult<AstString>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AstInt {
    pub value: AstResult<i64>,
    pub string: AstString,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AstText {
    pub parts: Vec<AstTextPart>,
    pub closing_double_quote_error: Option<AstError>,
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum AstTextPart {
    Text(Box<str>),
    Interpolation {
        expression: AstResult<Box<AstExpression>>,
        closing_curly_brace_error: Option<AstError>,
    },
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AstParenthesized {
    pub inner: AstResult<Box<AstExpression>>,
    pub closing_parenthesis_error: Option<AstError>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AstCall {
    pub receiver: Box<AstExpression>,
    pub arguments: Vec<AstCallArgument>,
    pub closing_parenthesis_error: Option<AstError>,
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AstCallArgument {
    pub value: AstExpression,
    pub comma_error: Option<AstError>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AstStruct {
    pub fields: Vec<AstStructField>,
    pub closing_bracket_error: Option<AstError>,
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AstStructField {
    pub key: AstIdentifier,
    pub colon_error: Option<AstError>,
    pub value: AstResult<Box<AstExpression>>,
    pub comma_error: Option<AstError>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AstStructAccess {
    pub struct_: Box<AstExpression>,
    pub key: AstResult<AstIdentifier>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AstLambda {
    pub parameters: Vec<AstParameter>,
    pub body: Vec<AstStatement>,
    pub closing_curly_brace_error: Option<AstError>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AstOr {
    pub left: Box<AstExpression>,
    pub right: AstResult<Box<AstExpression>>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AstParameter {
    pub name: AstIdentifier,
    pub type_: Option<AstResult<AstExpression>>,
    pub comma_error: Option<AstError>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AstAssignment {
    pub name: AstResult<AstIdentifier>,
    pub assignment_sign_error: Option<AstError>,
    pub is_public: bool,
    pub kind: AstAssignmentKind,
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum AstAssignmentKind {
    Value(AstAssignmentValue),
    Function(AstAssignmentFunction),
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AstAssignmentValue {
    pub type_: Option<AstResult<Box<AstExpression>>>,
    pub value: AstResult<Box<AstExpression>>,
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AstAssignmentFunction {
    pub parameters: Vec<AstParameter>,
    pub closing_parenthesis_error: Option<AstError>,
    pub return_type: Option<AstResult<Box<AstExpression>>>,
    pub opening_curly_brace_error: Option<AstError>,
    pub body: Vec<AstStatement>,
    pub closing_curly_brace_error: Option<AstError>,
}

pub trait CollectAstErrors {
    fn collect_errors(&self) -> Vec<CompilerError> {
        let mut errors = vec![];
        self.collect_errors_to(&mut errors);
        errors
    }
    fn collect_errors_to(&self, errors: &mut Vec<CompilerError>);
}
#[allow(clippy::mismatching_type_param_order)]
impl<A: CollectAstErrors> CollectAstErrors for Vec<A> {
    fn collect_errors_to(&self, errors: &mut Vec<CompilerError>) {
        for cst in self {
            cst.collect_errors_to(errors);
        }
    }
}
impl<A: CollectAstErrors> CollectAstErrors for Option<A> {
    fn collect_errors_to(&self, errors: &mut Vec<CompilerError>) {
        if let Some(cst) = self {
            cst.collect_errors_to(errors);
        }
    }
}
#[allow(clippy::mismatching_type_param_order)]
impl<A: CollectAstErrors> CollectAstErrors for Box<A> {
    fn collect_errors_to(&self, errors: &mut Vec<CompilerError>) {
        self.as_ref().collect_errors_to(errors);
    }
}
impl<A0: CollectAstErrors, A1: CollectAstErrors> CollectAstErrors for (A0, A1) {
    fn collect_errors_to(&self, errors: &mut Vec<CompilerError>) {
        self.0.collect_errors_to(errors);
        self.1.collect_errors_to(errors);
    }
}
impl<A: CollectAstErrors> CollectAstErrors for AstResult<A> {
    fn collect_errors_to(&self, errors: &mut Vec<CompilerError>) {
        self.errors.collect_errors_to(errors);
    }
}
impl CollectAstErrors for AstError {
    fn collect_errors_to(&self, errors: &mut Vec<CompilerError>) {
        errors.push(CompilerError {
            path: self.unparsable_input.file.clone(),
            span: self.unparsable_input.span.clone(),
            message: self.error.clone(),
        });
    }
}
impl CollectAstErrors for AstIdentifier {
    fn collect_errors_to(&self, errors: &mut Vec<CompilerError>) {
        self.identifier.collect_errors_to(errors);
    }
}
impl CollectAstErrors for AstString {
    fn collect_errors_to(&self, _errors: &mut Vec<CompilerError>) {}
}
impl CollectAstErrors for i64 {
    fn collect_errors_to(&self, _errors: &mut Vec<CompilerError>) {}
}

impl CollectAstErrors for AstStatement {
    fn collect_errors_to(&self, errors: &mut Vec<CompilerError>) {
        match &self {
            Self::Expression(expression) => expression.collect_errors_to(errors),
            Self::Assignment(assignment) => assignment.collect_errors_to(errors),
        }
    }
}
impl CollectAstErrors for AstExpression {
    fn collect_errors_to(&self, errors: &mut Vec<CompilerError>) {
        match &self {
            Self::Identifier(AstIdentifier { identifier }) => identifier.collect_errors_to(errors),
            Self::Symbol(AstSymbol { symbol }) => symbol.collect_errors_to(errors),
            Self::Int(AstInt { value, string }) => {
                value.collect_errors_to(errors);
                string.collect_errors_to(errors);
            }
            Self::Text(AstText {
                parts,
                closing_double_quote_error,
            }) => {
                for part in parts {
                    match part {
                        AstTextPart::Text(_) => {}
                        AstTextPart::Interpolation {
                            expression,
                            closing_curly_brace_error,
                        } => {
                            expression.collect_errors_to(errors);
                            closing_curly_brace_error.collect_errors_to(errors);
                        }
                    }
                }
                closing_double_quote_error.collect_errors_to(errors);
            }
            Self::Parenthesized(AstParenthesized {
                inner,
                closing_parenthesis_error,
            }) => {
                inner.collect_errors_to(errors);
                closing_parenthesis_error.collect_errors_to(errors);
            }
            Self::Call(AstCall {
                receiver,
                arguments,
                closing_parenthesis_error,
            }) => {
                receiver.collect_errors_to(errors);
                for argument in arguments {
                    argument.value.collect_errors_to(errors);
                    argument.comma_error.collect_errors_to(errors);
                }
                closing_parenthesis_error.collect_errors_to(errors);
            }
            Self::Struct(AstStruct {
                fields,
                closing_bracket_error,
            }) => {
                for field in fields {
                    field.key.collect_errors_to(errors);
                    field.colon_error.collect_errors_to(errors);
                    field.value.collect_errors_to(errors);
                    field.comma_error.collect_errors_to(errors);
                }
                closing_bracket_error.collect_errors_to(errors);
            }
            Self::StructAccess(AstStructAccess { struct_, key }) => {
                struct_.collect_errors_to(errors);
                key.collect_errors_to(errors);
            }
            Self::Lambda(AstLambda {
                parameters,
                body,
                closing_curly_brace_error,
            }) => {
                for parameter in parameters {
                    parameter.name.collect_errors_to(errors);
                    parameter.type_.collect_errors_to(errors);
                    parameter.comma_error.collect_errors_to(errors);
                }
                for statement in body {
                    statement.collect_errors_to(errors);
                }
                closing_curly_brace_error.collect_errors_to(errors);
            }
            Self::Or(AstOr { left, right }) => {
                left.collect_errors_to(errors);
                right.collect_errors_to(errors);
            }
        }
    }
}
impl CollectAstErrors for AstAssignment {
    fn collect_errors_to(&self, errors: &mut Vec<CompilerError>) {
        self.name.collect_errors_to(errors);
        self.assignment_sign_error.collect_errors_to(errors);
        match &self.kind {
            AstAssignmentKind::Value(AstAssignmentValue { type_, value }) => {
                type_.collect_errors_to(errors);
                value.collect_errors_to(errors);
            }
            AstAssignmentKind::Function(AstAssignmentFunction {
                parameters,
                closing_parenthesis_error,
                return_type,
                opening_curly_brace_error,
                body,
                closing_curly_brace_error,
            }) => {
                for parameter in parameters {
                    parameter.name.collect_errors_to(errors);
                    parameter.type_.collect_errors_to(errors);
                    parameter.comma_error.collect_errors_to(errors);
                }
                closing_parenthesis_error.collect_errors_to(errors);
                return_type.collect_errors_to(errors);
                opening_curly_brace_error.collect_errors_to(errors);
                for statement in body {
                    statement.collect_errors_to(errors);
                }
                closing_curly_brace_error.collect_errors_to(errors);
            }
        }
    }
}
