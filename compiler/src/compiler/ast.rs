use super::{error::CompilerError, utils::AdjustCasingOfFirstLetter};
use crate::input::Input;
use itertools::Itertools;
use linked_hash_map::LinkedHashMap;
use std::{
    fmt::{self, Display, Formatter},
    ops::Deref,
};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Id {
    pub input: Input,
    pub local: usize,
}
impl Id {
    pub fn new(input: Input, local: usize) -> Self {
        Self { input, local }
    }
}
impl Display for Id {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "AstId({}:{:?})", self.input, self.local)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Ast {
    pub id: Id,
    pub kind: AstKind,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum AstKind {
    Int(Int),
    Text(Text),
    Identifier(Identifier),
    Symbol(Symbol),
    Struct(Struct),
    StructAccess(StructAccess),
    Lambda(Lambda),
    Call(Call),
    Assignment(Assignment),
    Error {
        /// The child may be set if it still makes sense to continue working
        /// with the error-containing subtree.
        child: Option<Box<Ast>>,
        errors: Vec<CompilerError>,
    },
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Int(pub u64);

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Text(pub AstString);

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Identifier(pub AstString);

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Symbol(pub AstString);

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Struct {
    pub fields: LinkedHashMap<Ast, Ast>,
}
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct StructAccess {
    pub struct_: Box<Ast>,
    pub key: AstString,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Lambda {
    pub parameters: Vec<AstString>,
    pub body: Vec<Ast>,
    pub fuzzable: bool,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Call {
    pub receiver: CallReceiver,
    pub arguments: Vec<Ast>,
}
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum CallReceiver {
    Identifier(AstString),
    StructAccess(StructAccess),
    Call(Box<Call>),
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Assignment {
    pub name: AstString,
    pub is_public: bool,
    pub body: AssignmentBody,
}
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum AssignmentBody {
    Lambda(Lambda),
    Body(Vec<Ast>),
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct AstString {
    pub id: Id,
    pub value: String,
}
impl Deref for AstString {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum AstError {
    UnexpectedPunctuation,
    TextWithoutClosingQuote,
    ParenthesizedWithoutClosingParenthesis,
    CallOfANonIdentifier,
    StructWithNonStructField,
    StructWithoutClosingBrace,
    StructKeyWithoutColon,
    StructValueWithoutComma,
    ExpectedParameter,
    LambdaWithoutClosingCurlyBrace,
}

pub trait CollectErrors {
    fn collect_errors(self, errors: &mut Vec<CompilerError>);
}
impl CollectErrors for Ast {
    fn collect_errors(self, errors: &mut Vec<CompilerError>) {
        match self.kind {
            AstKind::Int(_) => {}
            AstKind::Text(_) => {}
            AstKind::Identifier(_) => {}
            AstKind::Symbol(_) => {}
            AstKind::Struct(struct_) => {
                for (key, value) in struct_.fields {
                    key.collect_errors(errors);
                    value.collect_errors(errors);
                }
            }
            AstKind::StructAccess(struct_access) => {
                struct_access.struct_.collect_errors(errors);
            }
            AstKind::Lambda(lambda) => lambda.body.collect_errors(errors),
            AstKind::Call(call) => call.arguments.collect_errors(errors),
            AstKind::Assignment(assignment) => match assignment.body {
                AssignmentBody::Lambda(lambda) => lambda.body.collect_errors(errors),
                AssignmentBody::Body(body) => {
                    for ast in body {
                        ast.collect_errors(errors)
                    }
                }
            },
            AstKind::Error {
                child,
                errors: mut recovered_errors,
            } => {
                errors.append(&mut recovered_errors);
                child.map(|child| child.collect_errors(errors));
            }
        }
    }
}
impl CollectErrors for Vec<Ast> {
    fn collect_errors(self, errors: &mut Vec<CompilerError>) {
        for ast in self {
            ast.collect_errors(errors);
        }
    }
}

impl Display for Ast {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.id)?;
        match &self.kind {
            AstKind::Int(int) => write!(f, "int {}", int.0),
            AstKind::Text(text) => write!(f, "text \"{}\"", text.0),
            AstKind::Identifier(identifier) => write!(f, "identifier {}", identifier.0),
            AstKind::Symbol(symbol) => write!(f, "symbol {}", symbol.0),
            AstKind::Struct(struct_) => {
                write!(
                    f,
                    "struct [\n{}\n]",
                    struct_
                        .fields
                        .iter()
                        .map(|(key, value)| format!("{key}: {value},"))
                        .join("\n")
                        .lines()
                        .map(|line| format!("  {line}"))
                        .join("\n")
                )
            }
            AstKind::StructAccess(struct_access) => write!(f, "{struct_access}"),
            AstKind::Lambda(lambda) => write!(f, "{}", lambda),
            AstKind::Call(call) => write!(f, "{}", call),
            AstKind::Assignment(assignment) => {
                write!(
                    f,
                    "assignment: {} =\n{}",
                    assignment.name,
                    format!("{}", assignment.body)
                        .lines()
                        .map(|line| format!("  {line}"))
                        .join("\n"),
                )
            }
            AstKind::Error { child, errors } => {
                write!(
                    f,
                    "error:\n{}",
                    errors.iter().map(|error| format!("  {error:?}")).join("\n")
                )?;
                if let Some(child) = child {
                    write!(f, "\n  fallback: {child}")?;
                }
                Ok(())
            }
        }
    }
}
impl Display for AssignmentBody {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            AssignmentBody::Lambda(lambda) => write!(f, "{lambda}"),
            AssignmentBody::Body(body) => {
                write!(f, "{}", body.iter().map(|it| format!("{it}")).join("\n"))
            }
        }
    }
}
impl Display for Lambda {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "lambda ({}) {{ {} ->\n{}\n}}",
            if self.fuzzable {
                "fuzzable"
            } else {
                "non-fuzzable"
            },
            self.parameters.iter().map(|it| format!("{it}")).join(" "),
            self.body
                .iter()
                .map(|it| format!("{it}"))
                .join("\n")
                .lines()
                .map(|line| format!("  {line}"))
                .join("\n")
        )
    }
}
impl Display for StructAccess {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "struct access {}.{}",
            self.struct_,
            self.key.lowercase_first_letter()
        )
    }
}
impl Display for Call {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "call {} with these arguments:\n{}",
            self.receiver,
            self.arguments
                .iter()
                .map(|argument| format!("  {argument}"))
                .join("\n")
        )
    }
}
impl Display for CallReceiver {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CallReceiver::Identifier(identifier) => write!(f, "{}", identifier),
            CallReceiver::StructAccess(struct_access) => write!(f, "{}", struct_access),
            CallReceiver::Call(call) => write!(f, "{}", call),
        }
    }
}
impl Display for AstString {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}@\"{}\"", self.id, self.value)
    }
}
