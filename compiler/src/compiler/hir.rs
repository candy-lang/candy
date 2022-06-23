use super::{ast_to_hir::AstToHir, error::CompilerError};
use crate::{builtin_functions::BuiltinFunction, input::Input};
use im::HashMap;
use itertools::Itertools;
use linked_hash_map::LinkedHashMap;
use std::fmt::{self, Display, Formatter};

#[salsa::query_group(HirDbStorage)]
pub trait HirDb: AstToHir {
    fn find_expression(&self, id: Id) -> Option<Expression>;
    fn all_hir_ids(&self, input: Input) -> Option<Vec<Id>>;
}
fn find_expression(db: &dyn HirDb, id: Id) -> Option<Expression> {
    let (hir, _) = db.hir(id.input.clone()).unwrap();
    if id.is_root() {
        panic!("You can't get the root because that got lowered into multiple IDs.");
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
            Expression::Call { arguments, .. } => {
                ids.extend(arguments.iter().cloned());
            }
            Expression::Builtin(builtin) => {}
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
    pub keys: Vec<String>,
}
impl Id {
    pub fn new(input: Input, keys: Vec<String>) -> Self {
        Self { input, keys }
    }

    pub fn is_root(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn parent(&self) -> Option<Id> {
        match self.keys.len() {
            0 => None,
            _ => Some(Id {
                input: self.input.clone(),
                keys: self.keys[..self.keys.len() - 1].to_vec(),
            }),
        }
    }
}
impl Display for Id {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "HirId({}:{})", self.input, self.keys.iter().join(":"))
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
    Call {
        function: Id,
        arguments: Vec<Id>,
    },
    Builtin(BuiltinFunction),
    Error {
        child: Option<Id>,
        errors: Vec<CompilerError>,
    },
}
impl Expression {
    pub fn nothing() -> Self {
        Expression::Symbol("Nothing".to_string())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Lambda {
    pub parameters: Vec<Id>,
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
                    "struct [\n{}\n]",
                    entries
                        .iter()
                        .map(|(key, value)| format!("  {}: {},", key, value))
                        .join("\n"),
                )
            }
            Expression::Lambda(lambda) => {
                write!(
                    f,
                    "lambda {{ {}\n}}",
                    lambda
                        .to_string()
                        .lines()
                        .enumerate()
                        .map(|(i, line)| format!("{}{}", if i == 0 { "" } else { "  " }, line))
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
                    "call {} with these arguments:\n{}",
                    function,
                    arguments
                        .iter()
                        .map(|argument| format!("  {}", argument))
                        .join("\n")
                )
            }
            Expression::Builtin(builtin) => {
                write!(f, "builtin{:?}", builtin)
            }
            Expression::Error { child, errors } => {
                write!(f, "error")?;
                for error in errors {
                    write!(f, "\n  {:?}", error)?;
                }
                if let Some(child) = child {
                    write!(f, "\n  fallback: {}", child)?;
                }
                Ok(())
            }
        }
    }
}
impl fmt::Display for Lambda {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ->\n",
            self.parameters
                .iter()
                .map(|parameter| format!("{}", parameter))
                .join(" "),
        )?;
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
            Expression::Call { .. } => None,
            Expression::Builtin(_) => None,
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
                .filter(|(it, _)| it <= &id)
                .max_by_key(|(id, _)| id.keys.to_owned())?
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
            | Expression::Call { .. }
            | Expression::Builtin(_) => {}
            Expression::Lambda(lambda) => lambda.body.collect_errors(errors),
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
