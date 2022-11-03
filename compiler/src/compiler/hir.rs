use super::{ast_to_hir::AstToHir, error::CompilerError};
use crate::{builtin_functions::BuiltinFunction, module::Module};
use im::HashMap;
use itertools::Itertools;
use linked_hash_map::LinkedHashMap;
use num_bigint::BigUint;
use std::{
    collections::HashSet,
    fmt::{self, Display, Formatter},
    sync::Arc,
};
use tracing::info;

#[salsa::query_group(HirDbStorage)]
pub trait HirDb: AstToHir {
    fn find_expression(&self, id: Id) -> Option<Expression>;
    fn containing_body_of(&self, id: Id) -> Arc<Body>;
    fn all_hir_ids(&self, module: Module) -> Option<Vec<Id>>;
}
fn find_expression(db: &dyn HirDb, id: Id) -> Option<Expression> {
    let (hir, _) = db.hir(id.module.clone()).unwrap();
    if id.is_root() {
        panic!("You can't get the root because that got lowered into multiple IDs.");
    }

    hir.find(&id).map(|it| it.to_owned())
}
fn containing_body_of(db: &dyn HirDb, id: Id) -> Arc<Body> {
    match id.parent() {
        Some(lambda_id) => {
            if lambda_id.is_root() {
                db.hir(id.module).unwrap().0
            } else {
                match db.find_expression(lambda_id).unwrap() {
                    Expression::Lambda(lambda) => Arc::new(lambda.body),
                    _ => panic!("Parent of an expression must be a lambda (or root scope)."),
                }
            }
        }
        None => panic!("The root scope has no parent."),
    }
}
fn all_hir_ids(db: &dyn HirDb, module: Module) -> Option<Vec<Id>> {
    let (hir, _) = db.hir(module)?;
    let mut ids = vec![];
    hir.collect_all_ids(&mut ids);
    info!("All HIR IDs: {ids:?}");
    Some(ids)
}

impl Expression {
    pub fn collect_all_ids(&self, ids: &mut Vec<Id>) {
        match self {
            Expression::Int(_) => {}
            Expression::Text(_) => {}
            Expression::Reference(id) => {
                ids.push(id.clone());
            }
            Expression::Symbol(_) => {}
            Expression::List(items) => {
                ids.extend(items.iter().cloned());
            }
            Expression::Struct(entries) => {
                for (key_id, value_id) in entries.iter() {
                    ids.push(key_id.to_owned());
                    ids.push(value_id.to_owned());
                }
            }
            Expression::Lambda(Lambda {
                parameters, body, ..
            }) => {
                for parameter in parameters {
                    ids.push(parameter.clone());
                }
                body.collect_all_ids(ids);
            }
            Expression::Call {
                function,
                arguments,
            } => {
                ids.push(function.clone());
                ids.extend(arguments.iter().cloned());
            }
            Expression::UseModule { relative_path, .. } => {
                ids.push(relative_path.clone());
            }
            Expression::Builtin(_) => {}
            Expression::Needs { condition, reason } => {
                ids.push(*condition.clone());
                ids.push(*reason.clone());
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
    pub module: Module,
    pub keys: Vec<String>,
}
impl Id {
    pub fn new(module: Module, keys: Vec<String>) -> Self {
        Self { module, keys }
    }

    pub fn is_root(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn parent(&self) -> Option<Id> {
        match self.keys.len() {
            0 => None,
            _ => Some(Id {
                module: self.module.clone(),
                keys: self.keys[..self.keys.len() - 1].to_vec(),
            }),
        }
    }

    pub fn is_same_module_and_any_parent_of(&self, other: &Self) -> bool {
        self.module == other.module
            && self.keys.len() < other.keys.len()
            && self.keys.iter().zip(&other.keys).all(|(a, b)| a == b)
    }
}
impl Display for Id {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "HirId({}:{})", self.module, self.keys.iter().join(":"))
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Expression {
    Int(BigUint),
    Text(String),
    Reference(Id),
    Symbol(String),
    List(Vec<Id>),
    Struct(HashMap<Id, Id>),
    Lambda(Lambda),
    Builtin(BuiltinFunction),
    Call {
        function: Id,
        arguments: Vec<Id>,
    },
    UseModule {
        current_module: Module,
        relative_path: Id,
    },
    Needs {
        condition: Box<Id>,
        reason: Box<Id>,
    },
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
    pub fuzzable: bool,
}
impl Lambda {
    pub fn captured_ids(&self, my_id: &Id) -> Vec<Id> {
        let mut captured = vec![];
        self.body.collect_all_ids(&mut captured);
        captured
            .into_iter()
            .filter(|potentially_captured_id| {
                !my_id.is_same_module_and_any_parent_of(potentially_captured_id)
            })
            .collect::<HashSet<_>>()
            .into_iter()
            .collect_vec()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Body {
    pub expressions: LinkedHashMap<Id, Expression>,
    pub identifiers: HashMap<Id, String>,
}
impl Body {
    #[allow(dead_code)]
    pub fn return_value(&self) -> Id {
        self.expressions
            .keys()
            .last()
            .expect("no expressions")
            .clone()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum HirError {
    NeedsWithWrongNumberOfArguments { num_args: usize },
    PublicAssignmentInNotTopLevel,
    PublicAssignmentWithSameName { name: String },
    UnknownReference { name: String },
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
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expression::Int(int) => write!(f, "int {int}"),
            Expression::Text(text) => write!(f, "text {text:?}"),
            Expression::Reference(reference) => write!(f, "reference {reference}"),
            Expression::Symbol(symbol) => write!(f, "symbol {symbol}"),
            Expression::List(items) => {
                write!(
                    f,
                    "list (\n{}\n)",
                    items.iter().map(|item| format!("  {item},")).join("\n"),
                )
            }
            Expression::Struct(entries) => {
                write!(
                    f,
                    "struct [\n{}\n]",
                    entries
                        .iter()
                        .map(|(key, value)| format!("  {key}: {value},"))
                        .join("\n"),
                )
            }
            Expression::Lambda(lambda) => {
                write!(
                    f,
                    "lambda ({}) {{ {}\n}}",
                    if lambda.fuzzable {
                        "fuzzable"
                    } else {
                        "non-fuzzable"
                    },
                    lambda
                        .to_string()
                        .lines()
                        .enumerate()
                        .map(|(i, line)| format!("{}{line}", if i == 0 { "" } else { "  " }))
                        .join("\n"),
                )
            }
            Expression::Builtin(builtin) => {
                write!(f, "builtin{builtin:?}")
            }
            Expression::Call {
                function,
                arguments,
            } => {
                assert!(!arguments.is_empty(), "A call needs to have arguments.");
                write!(
                    f,
                    "call {function} with these arguments:\n{}",
                    arguments
                        .iter()
                        .map(|argument| format!("  {argument}"))
                        .join("\n")
                )
            }
            Expression::UseModule {
                current_module,
                relative_path,
            } => write!(
                f,
                "use module {} relative to {}",
                relative_path, current_module
            ),
            Expression::Needs { condition, reason } => {
                write!(f, "needs {condition} with reason {reason}")
            }
            Expression::Error { child, errors } => {
                write!(f, "{}", if errors.len() == 1 { "error" } else { "errors" })?;
                for error in errors {
                    write!(f, "\n  {error:?}")?;
                }
                if let Some(child) = child {
                    write!(f, "\n  fallback: {child}")?;
                }
                Ok(())
            }
        }
    }
}
impl fmt::Display for Lambda {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{} ->",
            self.parameters
                .iter()
                .map(|parameter| format!("{parameter}"))
                .join(" "),
        )?;
        write!(f, "{}", self.body)?;
        Ok(())
    }
}
impl fmt::Display for Body {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (id, expression) in self.expressions.iter() {
            writeln!(f, "{id} = {expression}")?;
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
            Expression::List(_) => None,
            Expression::Struct(_) => None,
            Expression::Lambda(Lambda { body, .. }) => body.find(id),
            Expression::Builtin(_) => None,
            Expression::Call { .. } => None,
            Expression::UseModule { .. } => None,
            Expression::Needs { .. } => None,
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
            | Expression::List(_)
            | Expression::Struct(_)
            | Expression::Builtin(_)
            | Expression::Call { .. }
            | Expression::UseModule { .. }
            | Expression::Needs { .. } => {}
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
