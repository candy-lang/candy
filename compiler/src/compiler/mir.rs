use super::{error::CompilerError, hir};
use crate::{
    builtin_functions::BuiltinFunction,
    module::Module,
    utils::{CountableId, IdGenerator},
};
use itertools::Itertools;
use num_bigint::BigInt;
use std::{collections::HashMap, fmt, hash};

#[derive(Clone, PartialEq, Eq)]
pub struct Mir {
    pub id_generator: IdGenerator<Id>,
    pub body: Body,
}

#[derive(Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Id(usize);

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Body {
    expressions: Vec<(Id, Expression)>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum Expression {
    Int(BigInt),
    Text(String),
    Symbol(String),
    Builtin(BuiltinFunction),
    Struct(HashMap<Id, Id>),
    Reference(Id),
    Responsibility(hir::Id),
    /// In the MIR, lambdas take one extra parameter: The responsibility. Based
    /// on whether the function is fuzzable or not, this parameter may be used
    /// to dynamically determine who's at fault if some `needs` is not
    /// fulfilled.
    Lambda {
        parameters: Vec<Id>,
        responsible_parameter: Id,
        body: Body,
        fuzzable: bool,
    },
    /// This expression is never contained in an actual MIR body, but when
    /// dealing with expressions, its easier to not special-case IDs referring
    /// to parameters.
    Parameter,
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

impl Body {
    pub fn new() -> Self {
        Self {
            expressions: vec![],
        }
    }
    pub fn push(&mut self, id: Id, expression: Expression) {
        self.expressions.push((id, expression));
    }
    pub fn get(&self, id: Id) -> &Expression {
        self.expressions.iter()
            .find(|(key, _)| *key == id)
            .map(|(_, expression)| expression)
            .unwrap_or(&Expression::Parameter)
    }
    pub fn remove(&mut self, id: Id) {
        let index = self.expressions.iter().position(|(key, _)| *key == id).unwrap();
        self.expressions.remove(index);
    }
    pub fn insert(&mut self, index: usize, id: Id, expression: Expression) {
        self.expressions.insert(index, (id, expression));
    }
    pub fn insert_multiple(&mut self, index: usize, mut body: Body) {
        for (i, (id, expression)) in body.expressions.into_iter().enumerate() {
            self.expressions.insert(index + i, (id, expression));
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = (Id, &Expression)> {
        self.expressions.iter().map(|(id, expression)| (*id, expression))
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Id, &mut Expression)> {
        self.expressions.iter_mut().map(|(id, expression)| (*id, expression))
    }
    pub fn visit(&mut self, visitor: &mut dyn FnMut(&VisibleExpressions, Id, &mut Expression) -> ()) {
        self.visit_body(VisibleExpressions::none_visible(), visitor)
    }
    fn visit_body(&mut self, mut visible: VisibleExpressions, visitor: &mut dyn FnMut(&VisibleExpressions, Id, &mut Expression) -> ()) {
        let length = self.expressions.len();
        for i in 0..length {
            let (id, mut expression) = self.expressions.remove(i);
            visitor(&visible, id, &mut expression);

            if let Expression::Lambda { parameters, responsible_parameter, body, .. } = &mut expression {
                let mut inner_visible = visible.clone();
                for parameter in parameters {
                    inner_visible.insert(*parameter, Expression::Parameter);
                }
                inner_visible.insert(*responsible_parameter, Expression::Parameter);
                body.visit_body(inner_visible, visitor);
            }

            self.expressions.insert(i, (id, expression.clone()));
            visible.insert(id, expression);
        }

    }
}
#[derive(Clone)]
pub struct VisibleExpressions {
    expressions: im::HashMap<Id, Expression>,
}
impl VisibleExpressions {
    pub fn none_visible() -> Self {
        Self {
            expressions: im::HashMap::new(),
        }
    }
    pub fn insert(&mut self, id: Id, expression: Expression) {
        self.expressions.insert(id, expression);
    }
    pub fn get(&self, id: Id) -> &Expression {
        self.expressions.get(&id).unwrap()
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
            Expression::Parameter => {}
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
            Expression::Error { errors, .. } => {
                errors.hash(state);
            }
        }
    }
}

impl fmt::Debug for Mir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.body)
    }
}
impl fmt::Display for Body {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (id, expression) in &self.expressions {
            writeln!(f, "{id} = {expression:?}")?;
        }
        Ok(())
    }
}
impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${}", self.0)
    }
}
impl fmt::Debug for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expression::Int(int) => write!(f, "{int}"),
            Expression::Text(text) => write!(f, "{text:?}"),
            Expression::Symbol(symbol) => write!(f, "{symbol}"),
            Expression::Builtin(builtin) => write!(f, "builtin{builtin:?}"),
            Expression::Reference(id) => write!(f, "{id}"),
            Expression::Responsibility(id) => write!(f, "{id}"),
            Expression::Struct(fields) => write!(f, 
                "[{}]",
                fields
                    .iter()
                    .map(|(key, value)| format!("{key}: {value}"))
                    .join(", "),
            ),
            
            Expression::Lambda {
                parameters,
                responsible_parameter,
                body,
                fuzzable,
            } => write!(f,
                "{{ {} (+ responsible {responsible_parameter}) -> ({})\n{}\n}}",
                parameters
                    .iter()
                    .map(|parameter| format!("{parameter}"))
                    .join(" "),
                if *fuzzable {
                    "fuzzable"
                } else {
                    "non-fuzzable"
                },
                format!("{body}")
                    .lines()
                    .map(|line| format!("  {line}"))
                    .join("\n"),
            ),
            Expression::Parameter => write!(f, "parameter"),
            Expression::Call {
                function,
                arguments,
                responsible,
            } => {
                write!(f, 
                    "call {function} with {} ({responsible} is responsible)",
                    if arguments.is_empty() {
                        "no arguments".to_string()
                    } else {
                        arguments.iter().map(|arg| format!("{arg}")).join(" ")
                    }
                )
            }
            Expression::UseModule {
                current_module,
                relative_path,
                responsible,
            } => write!(f, "use {relative_path} (relative to {current_module}; also, {responsible} is responsible)"),
            Expression::Needs {
                responsible,
                condition,
                reason,
            } => {
                write!(f, "needs {condition} {reason} ({responsible} is responsible)")
            }
            Expression::Panic {
                reason,
                responsible,
            } => write!(f, "panicking because {reason} ({responsible} is at fault)"),
            Expression::Error { errors, .. } => {
                write!(f, "{}\n{}",
                    format!("{}", if errors.len() == 1 { "error" } else { "errors" }),
                    errors.iter().map(|error| format!("  {error:?}")).join("\n"),
                )
            }
        }
    }
}
