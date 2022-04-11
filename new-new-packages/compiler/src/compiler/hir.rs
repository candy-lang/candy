use std::{
    fmt::{self, Display, Formatter},
    ops::Add,
};

use im::HashMap;
use itertools::Itertools;
use linked_hash_map::LinkedHashMap;

use crate::input::Input;

use super::{ast_to_hir::AstToHir, error::CompilerError};

#[salsa::query_group(HirDbStorage)]
pub trait HirDb: AstToHir {
    fn find_expression(&self, id: Id) -> Option<Expression>;
    fn all_hir_ids(&self, input: Input) -> Option<Vec<Id>>;
}
fn find_expression(db: &dyn HirDb, id: Id) -> Option<Expression> {
    let (hir, _) = db.hir(id.input.clone()).unwrap();
    if id.is_root() {
        return Some(Expression::Body(hir.as_ref().to_owned()));
    }

    hir.find(&id).map(|it| it.to_owned())
}
fn all_hir_ids(db: &dyn HirDb, input: Input) -> Option<Vec<Id>> {
    let (hir, _) = db.hir(input)?;
    let mut ids = vec![];
    hir.collect_all_ids(&mut ids);
    log::info!("all HIR IDs: {:?}", ids);
    Some(ids)
}

impl Expression {
    fn collect_all_ids(&self, ids: &mut Vec<Id>) {
        match self {
            Expression::Int(_) => {}
            Expression::Text(_) => {}
            Expression::Reference(_) => {}
            Expression::Symbol(_) => {}
            Expression::Struct(entries) => {
                for (key_id, value_id) in entries.iter() {
                    ids.push(key_id.to_owned());
                    ids.push(value_id.to_owned());
                }
            }
            Expression::Lambda(Lambda { body, .. }) => {
                // TODO: list parameter IDs?
                // for (index, _) in parameters.iter().enumerate() {
                //     ids.push(first_id.to_owned() + index);
                // }
                body.collect_all_ids(ids);
            }
            Expression::Body(body) => body.collect_all_ids(ids),
            Expression::Call { arguments, .. } => {
                ids.extend(arguments.iter().cloned());
            }
            Expression::Error { .. } => {}
        }
    }
}
impl Body {
    fn collect_all_ids(&self, ids: &mut Vec<Id>) {
        ids.extend(self.expressions.keys().into_iter().cloned());
        for expression in self.expressions.values() {
            expression.collect_all_ids(ids);
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, PartialOrd, Ord)]
pub struct Id {
    pub input: Input,
    pub local: Vec<usize>,
}
impl Id {
    pub fn new(input: Input, local: Vec<usize>) -> Self {
        Self { input, local }
    }

    pub fn is_root(&self) -> bool {
        self.local.is_empty()
    }

    pub fn parent(&self) -> Option<Id> {
        match self.local.len() {
            0 => None,
            _ => Some(Id {
                input: self.input.clone(),
                local: self.local[..self.local.len() - 1].to_vec(),
            }),
        }
    }
}
impl Add<usize> for Id {
    type Output = Self;

    fn add(self, rhs: usize) -> Self::Output {
        let mut local = self.local[..self.local.len() - 1].to_vec();
        local.push(self.local.last().unwrap() + rhs);
        Id {
            input: self.input,
            local: local,
        }
    }
}
impl Display for Id {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "HirId({}:{})",
            self.input,
            self.local.iter().map(|id| format!("{}", id)).join(":")
        )
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Expression {
    Int(u64),
    Text(String),
    Reference(Id),
    Symbol(String),
    Struct(HashMap<Id, Id>),
    Lambda(Lambda),
    Body(Body),
    Call {
        function: Id,
        arguments: Vec<Id>,
    },
    Error {
        child: Option<Id>,
        errors: Vec<CompilerError>,
    },
}
impl Expression {
    pub fn nothing() -> Self {
        Expression::Symbol("Nothing".to_owned())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Lambda {
    pub first_id: Id,
    pub parameters: Vec<String>,
    pub body: Body,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Body {
    pub expressions: LinkedHashMap<Id, Expression>,
    pub identifiers: HashMap<Id, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum HirError {
    UnknownReference { symbol: String },
    UnknownFunction { name: String },
}

impl Body {
    pub fn new() -> Self {
        Self {
            expressions: LinkedHashMap::new(),
            identifiers: HashMap::new(),
        }
    }
    pub fn push(&mut self, id: Id, expression: Expression, identifier: Option<String>) {
        self.expressions.insert(id.to_owned(), expression);
        if let Some(identifier) = identifier {
            self.identifiers.insert(id, identifier);
        }
    }
    pub fn out_id(&self) -> &Id {
        self.expressions.keys().last().unwrap()
    }
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expression::Int(int) => write!(f, "int {}", int),
            Expression::Text(text) => write!(f, "text {:?}", text),
            Expression::Reference(reference) => write!(f, "reference {}", reference),
            Expression::Symbol(symbol) => write!(f, "symbol {}", symbol),
            Expression::Struct(entries) => {
                write!(
                    f,
                    "struct [{}]",
                    entries
                        .iter()
                        .map(|(key, value)| format!("{}: {}", key, value))
                        .join(", "),
                )
            }
            Expression::Lambda(lambda) => {
                write!(
                    f,
                    "lambda {{{}\n}}",
                    lambda
                        .to_string()
                        .lines()
                        .map(|line| format!("  {}", line))
                        .join("\n"),
                )
            }
            Expression::Body(body) => {
                write!(
                    f,
                    "body {{\n{}\n}}",
                    body.to_string()
                        .lines()
                        .map(|line| format!("  {}", line))
                        .join("\n"),
                )
            }
            Expression::Call {
                function,
                arguments,
            } => {
                assert!(arguments.len() > 0, "A call needs to have arguments.");
                write!(
                    f,
                    "call {} with arguments {}",
                    function,
                    arguments.iter().join(", ")
                )
            }
            Expression::Error { child, errors } => {
                write!(f, "Error {:?}", errors)?;
                if let Some(child) = child {
                    write!(f, "\nfallback: {}", child)?;
                }
                Ok(())
            }
        }
    }
}
impl fmt::Display for Lambda {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} parameters\n", self.parameters.len())?;
        write!(f, "{}", self.body)?;
        Ok(())
    }
}
impl fmt::Display for Body {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (id, expression) in self.expressions.iter() {
            write!(f, "{} = {}\n", id, expression)?;
        }
        Ok(())
    }
}

impl Expression {
    fn find(&self, id: &Id) -> Option<&Self> {
        match self {
            Expression::Int { .. } => None,
            Expression::Text { .. } => None,
            Expression::Reference { .. } => None,
            Expression::Symbol { .. } => None,
            Expression::Struct(_) => None,
            Expression::Lambda(Lambda { body, .. }) => body.find(id),
            Expression::Body(body) => body.find(id),
            Expression::Call { .. } => None,
            Expression::Error { .. } => None,
        }
    }
}
impl Body {
    fn find(&self, id: &Id) -> Option<&Expression> {
        if let Some(expression) = self.expressions.get(id) {
            Some(expression)
        } else {
            self.expressions
                .iter()
                .filter(|(key, _)| key <= &id)
                .max_by_key(|(key, _)| key.local.to_owned())?
                .1
                .find(id)
        }
    }
}

pub trait CollectErrors {
    fn collect_errors(&self, errors: &mut Vec<CompilerError>);
}
impl CollectErrors for Expression {
    fn collect_errors(&self, errors: &mut Vec<CompilerError>) {
        match self {
            Expression::Int(_)
            | Expression::Text(_)
            | Expression::Reference(_)
            | Expression::Symbol(_)
            | Expression::Struct(_)
            | Expression::Call { .. } => {}
            Expression::Lambda(lambda) => lambda.body.collect_errors(errors),
            Expression::Body(body) => body.collect_errors(errors),
            Expression::Error {
                errors: the_errors, ..
            } => {
                errors.append(&mut the_errors.clone());
            }
        }
    }
}
impl CollectErrors for Body {
    fn collect_errors(&self, errors: &mut Vec<CompilerError>) {
        for (_id, ast) in &self.expressions {
            ast.collect_errors(errors);
        }
    }
}
