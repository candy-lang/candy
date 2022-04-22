use std::{
    fmt::{self, Display, Formatter},
    ops::Deref,
};

use itertools::Itertools;
use linked_hash_map::LinkedHashMap;

use crate::input::Input;

use super::error::CompilerError;

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
pub struct Lambda {
    pub parameters: Vec<AstString>,
    pub body: Vec<Ast>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Call {
    pub name: AstString,
    pub arguments: Vec<Ast>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Assignment {
    pub name: AstString,
    pub body: Vec<Ast>,
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
            AstKind::Lambda(lambda) => lambda.body.collect_errors(errors),
            AstKind::Call(call) => call.arguments.collect_errors(errors),
            AstKind::Assignment(assignment) => assignment.body.collect_errors(errors),
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
                        .map(|(key, value)| format!("{}: {},", key, value))
                        .join("\n")
                        .lines()
                        .map(|line| format!("  {}", line))
                        .join("\n")
                )
            }
            AstKind::Lambda(lambda) => {
                write!(
                    f,
                    "lambda {{ {} ->\n{}\n}}",
                    lambda
                        .parameters
                        .iter()
                        .map(|it| format!("{}", it))
                        .join(" "),
                    lambda
                        .body
                        .iter()
                        .map(|it| format!("{}", it))
                        .join("\n")
                        .lines()
                        .map(|line| format!("  {}", line))
                        .join("\n")
                )
            }
            AstKind::Call(call) => {
                write!(
                    f,
                    "call {} with these arguments:\n{}",
                    call.name,
                    call.arguments
                        .iter()
                        .map(|argument| format!("  {}", argument))
                        .join("\n")
                )
            }
            AstKind::Assignment(assignment) => {
                write!(
                    f,
                    "assignment: {} =\n{}",
                    assignment.name,
                    assignment
                        .body
                        .iter()
                        .map(|it| format!("{}", it))
                        .join("\n")
                        .lines()
                        .map(|line| format!("  {}", line))
                        .join("\n")
                )
            }
            AstKind::Error { child, errors } => {
                write!(
                    f,
                    "error:\n{}",
                    errors
                        .iter()
                        .map(|error| format!("  {:?}", error))
                        .join("\n")
                )?;
                if let Some(child) = child {
                    write!(f, "\n  fallback: {}", child)?;
                }
                Ok(())
            }
        }
    }
}
impl Display for AstString {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}@\"{}\"", self.id, self.value)
    }
}
