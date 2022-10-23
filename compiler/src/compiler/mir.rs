use super::{error::CompilerError, hir};
use crate::{
    builtin_functions::BuiltinFunction,
    module::Module,
    utils::{CountableId, IdGenerator},
};
use itertools::Itertools;
use num_bigint::BigInt;
use std::{collections::HashMap, fmt, hash, ops::Index};

#[derive(Clone, PartialEq, Eq)]
pub struct Mir {
    pub id_generator: IdGenerator<Id>,
    pub expressions: HashMap<Id, Expression>,
    pub body: Vec<Id>,
}

#[derive(Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Id(usize);

#[derive(Clone, PartialEq, Eq)]
pub enum Expression {
    Int(BigInt),
    Text(String),
    Symbol(String),
    Builtin(BuiltinFunction),
    Struct(HashMap<Id, Id>),
    Reference(Id),
    Responsibility(hir::Id),
    Lambda {
        parameters: Vec<Id>,
        body: Vec<Id>,
        fuzzable: bool,
    },
    Call {
        function: Id,
        arguments: Vec<Id>,
        responsible: Id,
    },
    UseModule {
        current_module: Module,
        relative_path: Id,
        responsible: Id,
    },
    Needs {
        responsible: Id,
        condition: Id,
        reason: Id,
    },
    Panic {
        reason: Id,
        responsible: Id,
    },
    // TODO: Think about removing this. We should be able to model this using a
    // `Panic` instead. Also think about how the child will be handled.
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

impl hash::Hash for Expression {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            Expression::Int(int) => int.hash(state),
            Expression::Text(text) => text.hash(state),
            Expression::Symbol(symbol) => symbol.hash(state),
            Expression::Builtin(builtin) => builtin.hash(state),
            Expression::Struct(struct_) => struct_.len().hash(state),
            Expression::Reference(id) => id.hash(state),
            Expression::Responsibility(id) => id.hash(state),
            Expression::Lambda { body, .. } => body.hash(state),
            Expression::Call {
                function,
                arguments,
                responsible,
            } => {
                function.hash(state);
                arguments.hash(state);
                responsible.hash(state);
            }
            Expression::UseModule {
                current_module,
                relative_path,
                responsible,
            } => {
                current_module.hash(state);
                relative_path.hash(state);
                responsible.hash(state);
            }
            Expression::Needs {
                responsible,
                condition,
                reason,
            } => {
                responsible.hash(state);
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

impl fmt::Debug for Mir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for id in &self.body {
            let expression = self.expressions.get(id).unwrap();
            writeln!(f, "{id} = {}", expression.format(self))?;
        }
        Ok(())
    }
}
impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${}", self.0)
    }
}
impl Expression {
    fn format(&self, mir: &Mir) -> String {
        match self {
            Expression::Int(int) => format!("{int}"),
            Expression::Text(text) => format!("{text:?}"),
            Expression::Symbol(symbol) => format!("{symbol}"),
            Expression::Builtin(builtin) => format!("builtin{builtin:?}"),
            Expression::Reference(id) => format!("{id}"),
            Expression::Responsibility(id) => format!("{id}"),
            Expression::Struct(fields) => format!(
                "[{}]",
                fields
                    .iter()
                    .map(|(key, value)| format!("{key}: {value}"))
                    .join(", "),
            ),
            Expression::Lambda {
                parameters,
                body,
                fuzzable,
            } => format!(
                "{{ {} -> ({})\n{}\n}}",
                parameters
                    .iter()
                    .map(|parameter| format!("{parameter}"))
                    .join(" "),
                if *fuzzable {
                    "fuzzable"
                } else {
                    "non-fuzzable"
                },
                body.iter()
                    .map(|id| {
                        let expression = mir.expressions.get(id).unwrap();
                        format!("{id} = {}", expression.format(mir))
                    })
                    .join("\n")
                    .lines()
                    .map(|line| format!("  {line}"))
                    .join("\n"),
            ),
            Expression::Call {
                function,
                arguments,
                responsible,
            } => {
                format!(
                    "{function} {} ({responsible} is responsible)",
                    arguments.iter().map(|arg| format!("{arg}")).join(" ")
                )
            }
            Expression::UseModule {
                current_module,
                relative_path,
                responsible,
            } => format!("use {relative_path} (relative to {current_module}; also, {responsible} is responsible)"),
            Expression::Needs {
                responsible,
                condition,
                reason,
            } => {
                format!("needs {condition} {reason} ({responsible} is responsible)")
            }
            Expression::Panic {
                reason,
                responsible,
            } => format!("panicking because {reason} ({responsible} is at fault)"),
            Expression::Error { child, errors } => {
                format!("{}\n{}",
                    format!("{}", if errors.len() == 1 { "error" } else { "errors" }),
                    errors.iter().map(|error| format!("  {error:?}")).join("\n"),
                )
            }
        }
    }
}
