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
    ColonMissingAfterStructKey,
    NonCommaAfterStructValue,
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
                errors: recovered_errors,
            } => {
                errors.append(&mut recovered_errors.clone());
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
        self.fmt_with_indentation(f, "");
        Ok(())
    }
}
impl Ast {
    fn fmt_with_indentation(&self, f: &mut Formatter<'_>, indentation: &str) -> fmt::Result {
        self.id.fmt(f)?;
        ' '.fmt(f)?;
        let more_indentation = format!("  {}", indentation);
        match &self.kind {
            AstKind::Int(int) => write!(f, "{}", int.0),
            AstKind::Text(text) => write!(f, "\"{}\"", text.0),
            AstKind::Identifier(identifier) => write!(f, "{}", identifier.0),
            AstKind::Symbol(symbol) => write!(f, "{}", symbol.0),
            AstKind::Struct(struct_) => {
                " [".fmt(f)?;
                for (key, value) in &struct_.fields {
                    more_indentation.fmt(f)?;
                    key.fmt_with_indentation(f, &more_indentation)?;
                    ": ".fmt(f)?;
                    value.fmt_with_indentation(f, &more_indentation)?;
                    ",\n".fmt(f)?;
                }
                indentation.fmt(f)?;
                ']'.fmt(f)?;
                Ok(())
            }
            AstKind::Lambda(lambda) => {
                " {".fmt(f)?;
                lambda
                    .parameters
                    .iter()
                    .map(|it| format!("{}", it))
                    .join(", ")
                    .fmt(f)?;
                " -> ".fmt(f)?;
                for ast in &lambda.body {
                    more_indentation.fmt(f)?;
                    ast.fmt(f)?;
                    '\n'.fmt(f)?;
                }
                "}".fmt(f)?;
                Ok(())
            }
            AstKind::Call(call) => {
                call.name.fmt(f)?;
                '\n'.fmt(f)?;
                for argument in &call.arguments {
                    more_indentation.fmt(f)?;
                    argument.fmt_with_indentation(f, indentation);
                }
                Ok(())
            }
            AstKind::Assignment(assignment) => {
                assignment.name.fmt(f)?;
                " =\n".fmt(f)?;
                for ast in &assignment.body {
                    more_indentation.fmt(f)?;
                    ast.fmt_with_indentation(f, &more_indentation);
                    '\n'.fmt(f)?;
                }
                Ok(())
            }
            AstKind::Error { child, errors } => {
                write!(f, "!! errors: {:?}", errors);
                if let Some(child) = child {
                    write!(f, "\n{}", indentation);
                    child.fmt_with_indentation(f, &more_indentation);
                }
                Ok(())
            }
        }
    }
}
impl Display for AstString {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.id, self.value)
    }
}
