use super::{error::CompilerError, hir};
use crate::{
    builtin_functions::BuiltinFunction,
    module::Module,
    utils::{CountableId, IdGenerator},
};
use itertools::Itertools;
use num_bigint::BigInt;
use std::{fmt, hash, mem, cmp::Ordering};

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
    Struct(Vec<(Id, Id)>),
    Reference(Id),
    /// In the MIR, responsibilities are explicitly tracked. All lambdas take a
    /// responsibility as an extra parameter. Based on whether the function is
    /// fuzzable or not, this parameter may be used to dynamically determine
    /// who's at fault if some `needs` is not fulfilled.
    Responsibility(hir::Id),
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
        condition: Id,
        reason: Id,
        responsible: Id,
    },
    /// This expression indicates that the code will panic. It's created if the
    /// compiler can statically determine that some expression will always
    /// panic.
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
    /// For convenience when writing optimization passes, this expression allows
    /// storing multiple inner expressions. It's quickly expanded using the
    /// TODO optimization.
    Multiple(Body),
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
    pub fn push_with_new_id(&mut self, id_generator: &mut IdGenerator<Id>, expression: Expression) -> Id {
        let id = id_generator.generate();
        self.push(id, expression);
        id
    }
    pub fn insert_at_front(&mut self, expressions: Vec<(Id, Expression)>) {
        let old_expressions = mem::take(&mut self.expressions);
        self.expressions.extend(expressions);
        self.expressions.extend(old_expressions);
    }
    pub fn remove_all<F>(&mut self, mut predicate: F) where F: FnMut(Id, &Expression) -> bool {
        self.expressions.retain(|(id, expression)| !predicate(*id, expression));
    }
    pub fn sort_by<F>(&mut self, predicate: F) where F: FnMut(&(Id, Expression), &(Id, Expression)) -> Ordering {
        self.expressions.sort_by(predicate);
    }

    pub fn return_value(&mut self) -> Id {
        let (id, _) =self.expressions.iter_mut().last().unwrap();
        *id
    }

    /// Flattens all `Expression::Multiple`.
    pub fn flatten_multiples(&mut self) {
        let old_expressions = mem::take(&mut self.expressions);

        for (id, expression) in old_expressions.into_iter() {
            if let Expression::Multiple(mut inner_body) = expression {
                inner_body.flatten_multiples();
                let returned_by_inner = inner_body.return_value();
                for (id, expression) in inner_body.expressions {
                    self.expressions.push((id, expression));
                }
                self.expressions.push((id, Expression::Reference(returned_by_inner)));
            } else {
                self.expressions.push((id, expression));
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (Id, &Expression)> {
        self.expressions.iter().map(|(id, expression)| (*id, expression))
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Id, &mut Expression)> {
        self.expressions.iter_mut().map(|(id, expression)| (*id, expression))
    }
    pub fn into_iter(self) -> impl Iterator<Item = (Id, Expression)> {
        self.expressions.into_iter()
    }
}


impl Body {
    /// Calls the visitor for each contained expression, even expressions in
    /// lambdas or multiples.
    /// 
    /// The visitor is called in inside-out order, so if the body contains a
    /// lambda, the visitor is first called for its body expressions and only
    /// then for the lambda expression itself.
    /// 
    /// The visitor takes the ID of the current expression as well as the
    /// expression itself. It also takes `VisibleExpressions`, which allows it
    /// to inspect all expressions currently in scope. Finally, the visitor also
    /// receives whether the current expression is returned from the surrounding
    /// body.
    pub fn visit(&mut self, visitor: &mut dyn FnMut(Id, &mut Expression, &VisibleExpressions, bool)) {
        self.visit_with_visible(VisibleExpressions::none_visible(), visitor);
    }
    fn visit_with_visible(&mut self, mut visible: VisibleExpressions, visitor: &mut dyn FnMut(Id, &mut Expression, &VisibleExpressions, bool)) {
        let length = self.expressions.len();
        for i in 0..length {
            let (id, mut expression) = self.expressions.remove(i);
            Self::visit_expression(id, &mut expression, visible.clone(), i == length - 1, visitor);
            self.expressions.insert(i, (id, expression.clone()));
            visible.insert(id, expression);
        }
    }

    fn visit_expression(id: Id, expression: &mut Expression, visible: VisibleExpressions, is_returned: bool, visitor: &mut dyn FnMut(Id, &mut Expression, &VisibleExpressions, bool)) {
        if let Expression::Lambda { parameters, responsible_parameter, body, .. } = expression {
            let mut inner_visible = visible.clone();
            for parameter in parameters {
                inner_visible.insert(*parameter, Expression::Parameter);
            }
            inner_visible.insert(*responsible_parameter, Expression::Parameter);
            body.visit_with_visible(inner_visible, visitor);
        }
        if let Expression::Multiple(body) = expression {
            body.visit_with_visible(visible.clone(), visitor);
        }

        visitor(id, expression, &visible, is_returned);
    }

    pub fn visit_bodies(&mut self, visitor: &mut dyn FnMut(&mut Body)) {
        visitor(self);
        for (_, expression) in self.iter_mut() {
            expression.visit_bodies(visitor);
        }
    }
}
impl Expression {
    pub fn visit_bodies(&mut self, visitor: &mut dyn FnMut(&mut Body)) {
        match self {
            Expression::Lambda { body, .. } => body.visit_bodies(visitor),
            Expression::Multiple(body) => body.visit_bodies(visitor),
            _ => {},
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
    pub fn contains(&self, id: Id) -> bool {
        self.expressions.contains_key(&id)
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
            Expression::Error { errors, .. } => errors.hash(state),
            Expression::Multiple(body) => body.hash(state),
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
            Expression::Multiple(body) => write!(f,
                "\n{}",
                format!("{body}")
                    .lines()
                    .map(|line| format!("  {line}"))
                    .join("\n"),
            ),
        }
    }
}
impl fmt::Debug for VisibleExpressions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.expressions.keys().sorted().map(|id| format!("{id}")).join(", "))
    }
}
