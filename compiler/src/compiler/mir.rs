use itertools::Itertools;
use num_bigint::BigInt;

use super::error::CompilerError;
use crate::{
    builtin_functions::BuiltinFunction,
    module::Module,
    utils::{CountableId, IdGenerator},
};
use std::{collections::HashMap, fmt, hash, ops::Index};

pub struct Mir {
    pub id_generator: IdGenerator<Id>,
    pub constants: ConstantPool,
    pub body: Body,
}

#[derive(Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Id(usize);

#[derive(Clone, PartialEq, Eq)]
pub struct Body {
    pub expressions: HashMap<Id, Expression>,
    pub ids: Vec<Id>,
}

pub struct ConstantPool {
    id_to_constant: HashMap<Id, Constant>,
    constant_to_id: HashMap<Constant, Id>,
}

#[derive(PartialEq, Eq, Clone)]
pub enum Constant {
    Int(BigInt),
    Text(String),
    Symbol(String),
    Struct(HashMap<Id, Id>),
    Closure {
        captured: Vec<Id>,
        parameters: Vec<Id>,
        body: Body,
        fuzzable: bool,
    },
    Builtin(BuiltinFunction),
}

#[derive(Clone, PartialEq, Eq)]
pub enum Expression {
    Struct(HashMap<Id, Id>),
    Lambda {
        parameters: Vec<Id>,
        body: Body,
        fuzzable: bool,
    },
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
    Panic {
        reason: Id,
        responsible: Option<Id>,
    },
    Error {
        child: Option<Id>,
        errors: Vec<CompilerError>,
    },
}

impl CountableId for Id {
    fn from_usize(id: usize) -> Self {
        Self(id)
    }

    fn to_usize(&self) -> usize {
        self.0
    }
}

impl Body {
    pub fn new() -> Self {
        Self {
            expressions: Default::default(),
            ids: vec![],
        }
    }
    pub fn push(&mut self, id_generator: &mut IdGenerator<Id>, expression: Expression) -> Id {
        let id = id_generator.generate();
        self.expressions.insert(id, expression);
        self.ids.push(id);
        id
    }
}

impl ConstantPool {
    pub fn new() -> Self {
        Self {
            id_to_constant: Default::default(),
            constant_to_id: Default::default(),
        }
    }
    pub fn add(&mut self, id_generator: &mut IdGenerator<Id>, constant: Constant) -> Id {
        if let Some(id) = self.constant_to_id.get(&constant) {
            return *id;
        }

        let id = id_generator.generate();
        self.id_to_constant.insert(id, constant.clone());
        self.constant_to_id.insert(constant, id);
        id
    }
    pub fn contains(&self, id: &Id) -> bool {
        self.id_to_constant.contains_key(&id)
    }
}

pub enum Content {
    Constant(Constant),
    Expression(Expression),
}
impl Mir {
    pub fn get(&mut self, id: Id) -> Content {
        match self.constants.id_to_constant.get(&id) {
            Some(constant) => Content::Constant(constant.clone()),
            None => Content::Expression(self.body.expressions.get(&id).unwrap().clone()),
        }
    }
    pub fn replace_with_constant(&mut self, id: &Id, constant: Constant) {
        self.body.expressions.remove(id);
        let index = self.body.ids.iter().position(|it| it == id).unwrap();
        self.body.ids.remove(index);
        self.constants.add(&mut self.id_generator, constant);
    }
}

impl hash::Hash for Constant {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            Constant::Int(int) => int.hash(state),
            Constant::Text(text) => text.hash(state),
            Constant::Symbol(symbol) => symbol.hash(state),
            Constant::Struct(struct_) => struct_.len().hash(state),
            Constant::Closure { body, .. } => body.expressions.len().hash(state),
            Constant::Builtin(builtin) => builtin.hash(state),
        }
    }
}
impl hash::Hash for Expression {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            Expression::Struct(struct_) => struct_.len().hash(state),
            Expression::Lambda { body, .. } => body.expressions.len().hash(state),
            Expression::Call {
                function,
                arguments,
            } => {
                function.hash(state);
                arguments.hash(state);
            }
            Expression::UseModule {
                current_module,
                relative_path,
            } => {
                current_module.hash(state);
                relative_path.hash(state);
            }
            Expression::Needs { condition, reason } => {
                condition.hash(state);
                reason.hash(state);
            }
            Expression::Panic {
                reason,
                responsible,
            } => {
                reason.hash(state);
                responsible.hash(state);
            }
            Expression::Error { child, errors } => {
                errors.hash(state);
            }
        }
    }
}

impl fmt::Display for Mir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}\n{}", self.constants, self.body)
    }
}
impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${}", self.0)
    }
}
impl fmt::Display for ConstantPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut all_ids = self.id_to_constant.keys().copied().collect_vec();
        all_ids.sort();
        for id in &all_ids {
            writeln!(f, "{id}: {}", self.id_to_constant[&id])?;
        }
        Ok(())
    }
}
impl fmt::Display for Constant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Constant::Int(int) => int.fmt(f),
            Constant::Text(text) => write!(f, "{text:?}"),
            Constant::Symbol(symbol) => write!(f, "{symbol}"),
            Constant::Struct(struct_) => write!(
                f,
                "[{}]",
                struct_
                    .iter()
                    .map(|(key, value)| format!("{key}: {value}"))
                    .join(", ")
            ),
            Constant::Closure {
                captured,
                parameters,
                body,
                fuzzable,
            } => write!(f, "{{…}}"),
            Constant::Builtin(builtin) => write!(f, "builtin{builtin:?}"),
        }
    }
}
impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            Expression::Lambda {
                parameters,
                body,
                fuzzable,
            } => {
                write!(
                    f,
                    "lambda ({}) {{ {}\n}}",
                    if *fuzzable {
                        "fuzzable"
                    } else {
                        "non-fuzzable"
                    },
                    format!(
                        "{} ->\n{}",
                        parameters
                            .iter()
                            .map(|parameter| format!("{parameter}"))
                            .join(" "),
                        body,
                    )
                    .to_string()
                    .lines()
                    .enumerate()
                    .map(|(i, line)| format!("{}{line}", if i == 0 { "" } else { "  " }))
                    .join("\n"),
                )
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
                write!(f, "needs {condition} because {reason}")
            }
            Expression::Panic {
                reason,
                responsible,
            } => write!(
                f,
                "panic{} because {}",
                if let Some(responsible) = responsible {
                    format!("{responsible}")
                } else {
                    "".to_string()
                },
                reason
            ),
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
impl fmt::Display for Body {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for id in &self.ids {
            writeln!(f, "{id} = {}", self.expressions.get(&id).unwrap())?;
        }
        Ok(())
    }
}
