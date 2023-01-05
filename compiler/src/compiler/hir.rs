use crate::{
    builtin_functions::BuiltinFunction,
    module::{Module, ModuleKind, Package},
    utils::CountableId,
};

use super::{ast_to_hir::AstToHir, error::CompilerError};
use itertools::Itertools;
use linked_hash_map::LinkedHashMap;
use num_bigint::BigUint;
use std::{
    collections::HashMap,
    fmt::{self, Display, Formatter},
    hash,
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
            Expression::Destructure { expression, .. } => ids.push(expression.to_owned()),
            Expression::PatternIdentifierReference { destructuring, .. } => {
                ids.push(destructuring.to_owned())
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
                ids.push(condition.clone());
                ids.push(reason.clone());
            }
            Expression::Error { .. } => {}
        }
    }
}
impl Body {
    fn collect_all_ids(&self, ids: &mut Vec<Id>) {
        ids.extend(self.expressions.keys().cloned());
        for expression in self.expressions.values() {
            expression.collect_all_ids(ids);
        }
    }
}

#[derive(PartialEq, Eq, Clone, Hash, PartialOrd, Ord)]
pub struct Id {
    pub module: Module,
    pub keys: Vec<String>,
}
impl Id {
    pub fn new(module: Module, keys: Vec<String>) -> Self {
        Self { module, keys }
    }

    /// An ID that can be used to blame the tooling. For example, when calling
    /// the `main` function, we want to be able to blame the platform for
    /// passing a wrong environment.
    fn tooling(name: String) -> Self {
        Self {
            module: Module {
                package: Package::Tooling(name),
                path: vec![],
                kind: ModuleKind::Code,
            },
            keys: vec![],
        }
    }
    /// Refers to the platform (non-Candy code).
    pub fn platform() -> Self {
        Self::tooling("platform".to_string())
    }
    pub fn fuzzer() -> Self {
        Self::tooling("fuzzer".to_string())
    }
    /// A dummy ID that is guaranteed to never be responsible for a panic.
    pub fn dummy() -> Self {
        Self::tooling("dummy".to_string())
    }
    /// TODO: Currently, when a higher-order function calls a closure passed as
    /// a parameter, that's registered as a normal call instruction, making the
    /// callsite in the higher-order function responsible for the successful
    /// fulfillment of the passed function's `needs`. We probably want to change
    /// how that works so that the caller of the higher-order function is at
    /// fault when passing a panicking function. After we did that, we should be
    /// able to remove this ID.
    pub fn complicated_responsibility() -> Self {
        Self::tooling("complicated-responsibility".to_string())
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
impl fmt::Debug for Id {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "HirId({}:{})", self.module, self.keys.iter().join(":"))
    }
}
impl Display for Id {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Expression {
    Int(BigUint),
    Text(String),
    Reference(Id),
    Symbol(String),
    List(Vec<Id>),
    Struct(HashMap<Id, Id>),
    Destructure {
        expression: Id,
        pattern: Pattern,
    },
    PatternIdentifierReference {
        destructuring: Id,
        identifier_id: PatternIdentifierId,
    },
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
        condition: Id,
        reason: Id,
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
#[allow(clippy::derive_hash_xor_eq)]
impl hash::Hash for Expression {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PatternIdentifierId(pub usize);
impl CountableId for PatternIdentifierId {
    fn from_usize(id: usize) -> Self {
        Self(id)
    }
    fn to_usize(&self) -> usize {
        self.0
    }
}
impl fmt::Debug for PatternIdentifierId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pattern_identifier_{:x}", self.0)
    }
}
impl fmt::Display for PatternIdentifierId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "p${}", self.0)
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Pattern {
    NewIdentifier(PatternIdentifierId),
    Int(BigUint),
    Text(String),
    Symbol(String),
    List(Vec<Pattern>),
    // Keys may not contain `NewIdentifier`.
    Struct(HashMap<Pattern, Pattern>),
    Error {
        child: Option<Box<Pattern>>,
        errors: Vec<CompilerError>,
    },
}
#[allow(clippy::derive_hash_xor_eq)]
impl hash::Hash for Pattern {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Lambda {
    pub parameters: Vec<Id>,
    pub body: Body,
    pub fuzzable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Body {
    pub expressions: LinkedHashMap<Id, Expression>,
    pub identifiers: HashMap<Id, String>,
}
#[allow(clippy::derive_hash_xor_eq)]
impl hash::Hash for Body {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.expressions.hash(state);
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
            Expression::Destructure {
                expression,
                pattern,
            } => write!(f, "destructure {expression} into {pattern}"),
            Expression::PatternIdentifierReference {
                destructuring,
                identifier_id,
            } => write!(
                f,
                "get destructured p${} from {destructuring}",
                identifier_id.0
            ),
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
impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Pattern::Int(int) => write!(f, "{int}"),
            Pattern::Text(text) => write!(f, "\"{text:?}\""),
            Pattern::NewIdentifier(reference) => write!(f, "{reference}"),
            Pattern::Symbol(symbol) => write!(f, "{symbol}"),
            Pattern::List(items) => {
                write!(
                    f,
                    "({})",
                    match items.as_slice() {
                        [] => ",".to_owned(),
                        [item] => format!("{item},"),
                        items => items.iter().map(|item| format!("{item}")).join(", "),
                    },
                )
            }
            Pattern::Struct(entries) => {
                write!(
                    f,
                    "[{}]",
                    entries
                        .iter()
                        .map(|(key, value)| format!("{key}: {value}"))
                        .join(", "),
                )
            }
            Pattern::Error { child, errors } => {
                write!(f, "{}", if errors.len() == 1 { "error" } else { "errors" })?;
                for error in errors {
                    write!(f, "\n  {error}")?;
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
        for (id, expression) in &self.expressions {
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
            Expression::Destructure { .. } => None,
            Expression::PatternIdentifierReference { .. } => None,
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
    pub fn find(&self, id: &Id) -> Option<&Expression> {
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
            | Expression::PatternIdentifierReference { .. }
            | Expression::Builtin(_)
            | Expression::Call { .. }
            | Expression::UseModule { .. }
            | Expression::Needs { .. } => {}
            Expression::Lambda(lambda) => lambda.body.collect_errors(errors),
            Expression::Destructure { pattern, .. } => pattern.collect_errors(errors),
            Expression::Error {
                errors: the_errors, ..
            } => {
                errors.append(&mut the_errors.clone());
            }
        }
    }
}
impl CollectErrors for Pattern {
    fn collect_errors(&self, errors: &mut Vec<CompilerError>) {
        match self {
            Pattern::NewIdentifier(_)
            | Pattern::Int(_)
            | Pattern::Text(_)
            | Pattern::Symbol(_)
            | Pattern::List(_)
            | Pattern::Struct(_) => {}
            Pattern::Error {
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
