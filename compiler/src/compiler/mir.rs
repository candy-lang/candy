use super::hir;
use crate::{
    builtin_functions::BuiltinFunction,
    module::Module,
    utils::{CountableId, IdGenerator},
};
use itertools::Itertools;
use num_bigint::BigInt;
use std::{cmp::Ordering, collections::HashMap, fmt, hash, mem};

#[derive(Clone, PartialEq, Eq)]
pub struct Mir {
    pub id_generator: IdGenerator<Id>,
    pub body: Body,
}

#[derive(Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Id(usize);

#[derive(Clone, PartialEq, Eq, Hash, Default)]
pub struct Body {
    expressions: Vec<(Id, Expression)>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum Expression {
    Int(BigInt),
    Text(String),
    Symbol(String),
    Builtin(BuiltinFunction),
    List(Vec<Id>),
    Struct(Vec<(Id, Id)>),
    Reference(Id),
    /// A HIR ID that can be used to refer to code in the HIR.
    HirId(hir::Id),
    /// In the MIR, responsibilities are explicitly tracked. All lambdas take a
    /// responsible HIR ID as an extra parameter. Based on whether the function
    /// is fuzzable or not, this parameter may be used to dynamically determine
    /// who's at fault if some `needs` is not fulfilled.
    Lambda {
        parameters: Vec<Id>,
        responsible_parameter: Id,
        body: Body,
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
    /// This expression indicates that the code will panic. It's created if the
    /// compiler can statically determine that some expression will always
    /// panic.
    Panic {
        reason: Id,
        responsible: Id,
    },

    /// For convenience when writing optimization passes, this expression allows
    /// storing multiple inner expressions in a single expression. The expansion
    /// back into multiple expressions happens in the [multiple flattening]
    /// optimization.
    ///
    /// [multiple flattening]: super::optimize::multiple_flattening
    Multiple(Body),

    /// Indicates that a module started.
    ///
    /// Unlike the trace instructions below, this expression is not optional –
    /// it needs to always be compiled into the MIR because the `ModuleStarts`
    /// and `ModuleEnds` instructions directly influence the import stack of the
    /// VM and thereby the behavior of the program. Depending on the order of
    /// instructions being executed, an import may succeed, or panic because of
    /// a circular import.
    ///
    /// If there's no `use` between the `ModuleStarts` and `ModuleEnds`
    /// expressions, they can be optimized away.
    ModuleStarts {
        module: Module,
    },
    ModuleEnds,

    TraceCallStarts {
        hir_call: Id,
        function: Id,
        arguments: Vec<Id>,
        responsible: Id,
    },
    TraceCallEnds {
        return_value: Id,
    },
    TraceExpressionEvaluated {
        hir_expression: Id,
        value: Id,
    },
    TraceFoundFuzzableClosure {
        hir_definition: Id,
        closure: Id,
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

impl Expression {
    pub fn nothing() -> Self {
        Expression::Symbol("Nothing".to_string())
    }
}
impl From<bool> for Expression {
    fn from(value: bool) -> Self {
        Expression::Symbol(if value { "True" } else { "False" }.to_string())
    }
}
impl TryInto<bool> for &Expression {
    type Error = ();

    fn try_into(self) -> Result<bool, ()> {
        let Expression::Symbol(symbol) = self else { return Err(()); };
        match symbol.as_str() {
            "True" => Ok(true),
            "False" => Ok(false),
            _ => Err(()),
        }
    }
}

impl Body {
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (Id, &Expression)> {
        self.expressions
            .iter()
            .map(|(id, expression)| (*id, expression))
    }
    pub fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = (Id, &mut Expression)> {
        self.expressions
            .iter_mut()
            .map(|(id, expression)| (*id, expression))
    }
    pub fn into_iter(self) -> impl DoubleEndedIterator<Item = (Id, Expression)> {
        self.expressions.into_iter()
    }
    pub fn return_value(&mut self) -> Id {
        let (id, _) = self.expressions.last().unwrap();
        *id
    }

    pub fn push(&mut self, id: Id, expression: Expression) {
        self.expressions.push((id, expression));
    }
    pub fn push_with_new_id(
        &mut self,
        id_generator: &mut IdGenerator<Id>,
        expression: Expression,
    ) -> Id {
        let id = id_generator.generate();
        self.push(id, expression);
        id
    }
    pub fn insert_at_front(&mut self, expressions: Vec<(Id, Expression)>) {
        let old_expressions = mem::take(&mut self.expressions);
        self.expressions.extend(expressions);
        self.expressions.extend(old_expressions);
    }
    pub fn remove_all<F>(&mut self, mut predicate: F)
    where
        F: FnMut(Id, &Expression) -> bool,
    {
        self.expressions
            .retain(|(id, expression)| !predicate(*id, expression));
    }
    pub fn sort_by<F>(&mut self, predicate: F)
    where
        F: FnMut(&(Id, Expression), &(Id, Expression)) -> Ordering,
    {
        self.expressions.sort_by(predicate);
    }

    /// Flattens all `Expression::Multiple`.
    pub fn flatten_multiples(&mut self) {
        let old_expressions = mem::take(&mut self.expressions);

        for (id, mut expression) in old_expressions.into_iter() {
            if let Expression::Multiple(mut inner_body) = expression {
                inner_body.flatten_multiples();
                let returned_by_inner = inner_body.return_value();
                for (id, expression) in inner_body.expressions {
                    self.expressions.push((id, expression));
                }
                self.expressions
                    .push((id, Expression::Reference(returned_by_inner)));
            } else {
                if let Expression::Lambda { body, .. } = &mut expression {
                    body.flatten_multiples();
                }
                self.expressions.push((id, expression));
            }
        }
    }

    pub fn visit(&mut self, visitor: &mut dyn FnMut(Id, &mut Expression, bool)) {
        let length = self.expressions.len();
        for i in 0..length {
            let (id, expression) = self.expressions.get_mut(i).unwrap();
            Self::visit_expression(*id, expression, i == length - 1, visitor);
        }
    }
    fn visit_expression(
        id: Id,
        expression: &mut Expression,
        is_returned: bool,
        visitor: &mut dyn FnMut(Id, &mut Expression, bool),
    ) {
        if let Expression::Lambda { body, .. } | Expression::Multiple(body) = expression {
            body.visit(visitor);
        }
        visitor(id, expression, is_returned);
    }

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
    pub fn visit_with_visible(
        &mut self,
        visitor: &mut dyn FnMut(Id, &mut Expression, &VisibleExpressions, bool),
    ) {
        self.visit_with_visible_rec(&mut VisibleExpressions::none_visible(), visitor);
    }
    fn visit_with_visible_rec(
        &mut self,
        visible: &mut VisibleExpressions,
        visitor: &mut dyn FnMut(Id, &mut Expression, &VisibleExpressions, bool),
    ) {
        let expressions_in_this_body = self.expressions.iter().map(|(id, _)| *id).collect_vec();
        let length = expressions_in_this_body.len();

        for index in 0..length {
            let (id, mut expression) = mem::replace(
                self.expressions.get_mut(index).unwrap(),
                (Id::from_usize(0), Expression::Parameter),
            );
            let is_returned = index == length - 1;
            Self::visit_expression_with_visible(id, &mut expression, visible, is_returned, visitor);
            visible.insert(id, expression);
        }

        for (index, id) in expressions_in_this_body.iter().enumerate() {
            *self.expressions.get_mut(index).unwrap() =
                (*id, visible.expressions.remove(id).unwrap());
        }
    }
    fn visit_expression_with_visible(
        id: Id,
        expression: &mut Expression,
        visible: &mut VisibleExpressions,
        is_returned: bool,
        visitor: &mut dyn FnMut(Id, &mut Expression, &VisibleExpressions, bool),
    ) {
        if let Expression::Lambda {
            parameters,
            responsible_parameter,
            body,
            ..
        } = expression
        {
            for parameter in parameters.iter() {
                visible.insert(*parameter, Expression::Parameter);
            }
            visible.insert(*responsible_parameter, Expression::Parameter);
            body.visit_with_visible_rec(visible, visitor);
            for parameter in parameters.iter() {
                visible.expressions.remove(parameter);
            }
            visible.expressions.remove(responsible_parameter);
        }
        if let Expression::Multiple(body) = expression {
            body.visit_with_visible_rec(visible, visitor);
        }

        visitor(id, expression, visible, is_returned);
    }

    pub fn visit_bodies(&mut self, visitor: &mut dyn FnMut(&mut Body)) {
        for (_, expression) in self.iter_mut() {
            expression.visit_bodies(visitor);
        }
        visitor(self);
    }
}
impl Expression {
    pub fn visit_bodies(&mut self, visitor: &mut dyn FnMut(&mut Body)) {
        match self {
            Expression::Lambda { body, .. } => body.visit_bodies(visitor),
            Expression::Multiple(body) => body.visit_bodies(visitor),
            _ => {}
        }
    }
}

#[derive(Clone)]
pub struct VisibleExpressions {
    expressions: HashMap<Id, Expression>,
}
impl VisibleExpressions {
    pub fn none_visible() -> Self {
        Self {
            expressions: HashMap::new(),
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

#[allow(clippy::derive_hash_xor_eq)]
impl hash::Hash for Expression {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            Expression::Int(int) => int.hash(state),
            Expression::Text(text) => text.hash(state),
            Expression::Symbol(symbol) => symbol.hash(state),
            Expression::Builtin(builtin) => builtin.hash(state),
            Expression::List(items) => items.hash(state),
            Expression::Struct(fields) => fields.len().hash(state),
            Expression::Reference(id) => id.hash(state),
            Expression::HirId(id) => id.hash(state),
            Expression::Lambda {
                parameters,
                responsible_parameter,
                body,
            } => {
                parameters.hash(state);
                responsible_parameter.hash(state);
                body.hash(state);
            }
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
            Expression::Panic {
                reason,
                responsible,
            } => {
                reason.hash(state);
                responsible.hash(state);
            }
            Expression::Multiple(body) => body.hash(state),
            Expression::ModuleStarts { module } => module.hash(state),
            Expression::ModuleEnds => {}
            Expression::TraceCallStarts {
                hir_call,
                function,
                arguments,
                responsible,
            } => {
                hir_call.hash(state);
                function.hash(state);
                arguments.hash(state);
                responsible.hash(state);
            }
            Expression::TraceCallEnds { return_value } => return_value.hash(state),
            Expression::TraceExpressionEvaluated {
                hir_expression,
                value,
            } => {
                hir_expression.hash(state);
                value.hash(state);
            }
            Expression::TraceFoundFuzzableClosure {
                hir_definition,
                closure,
            } => {
                hir_definition.hash(state);
                closure.hash(state);
            }
        }
    }
}

impl Mir {
    // For now, this is only used in tests.
    #[cfg(test)]
    pub fn build<F: Fn(&mut MirBodyBuilder)>(function: F) -> Self {
        let mut id_generator = IdGenerator::start_at(0);
        let mut builder = MirBodyBuilder::with_generator(&mut id_generator);
        function(&mut builder);
        assert!(builder.parameters.is_empty());
        let body = builder.body;

        Mir { id_generator, body }
    }
}
impl Expression {
    // The builder function takes the builder and the responsible parameter.
    pub fn build_lambda<F: Fn(&mut MirBodyBuilder, Id)>(
        id_generator: &mut IdGenerator<Id>,
        function: F,
    ) -> Self {
        let responsible_parameter = id_generator.generate();
        let mut builder = MirBodyBuilder::with_generator(id_generator);
        function(&mut builder, responsible_parameter);

        Expression::Lambda {
            parameters: builder.parameters,
            responsible_parameter,
            body: builder.body,
        }
    }
}
pub struct MirBodyBuilder<'a> {
    id_generator: &'a mut IdGenerator<Id>,
    parameters: Vec<Id>,
    body: Body,
}
impl<'a> MirBodyBuilder<'a> {
    fn with_generator(id_generator: &'a mut IdGenerator<Id>) -> Self {
        MirBodyBuilder {
            id_generator,
            parameters: vec![],
            body: Body::default(),
        }
    }
    pub fn new_parameter(&mut self) -> Id {
        let id = self.id_generator.generate();
        self.parameters.push(id);
        id
    }
    pub fn push(&mut self, expression: Expression) -> Id {
        self.body.push_with_new_id(self.id_generator, expression)
    }
    pub fn push_lambda<F: Fn(&mut MirBodyBuilder, Id)>(&mut self, function: F) -> Id {
        let lambda = Expression::build_lambda(self.id_generator, function);
        self.push(lambda)
    }
    #[cfg(test)]
    pub fn push_multiple<F: Fn(&mut MirBodyBuilder)>(&mut self, function: F) -> Id {
        let mut builder = MirBodyBuilder::with_generator(self.id_generator);
        function(&mut builder);
        assert!(builder.parameters.is_empty());
        let body = builder.body;
        self.push(Expression::Multiple(body))
    }
}

impl fmt::Display for Mir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.body)
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
impl fmt::Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${}", self.0)
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
            Expression::List(items) => write!(
                f,
                "({})",
                if items.is_empty() {
                    ",".to_string()
                } else {
                    items.iter().map(|item| format!("{item}")).join(", ")
                }
            ),
            Expression::Struct(fields) => write!(
                f,
                "[{}]",
                fields
                    .iter()
                    .map(|(key, value)| format!("{key}: {value}"))
                    .join(", "),
            ),
            Expression::Reference(id) => write!(f, "{id}"),
            Expression::HirId(id) => write!(f, "{id}"),
            Expression::Lambda {
                parameters,
                responsible_parameter,
                body,
            } => write!(
                f,
                "{{ {} ->\n{}\n}}",
                if parameters.is_empty() {
                    format!("(responsible {responsible_parameter})")
                } else {
                    format!(
                        "{} (+ responsible {responsible_parameter})",
                        parameters
                            .iter()
                            .map(|parameter| format!("{parameter}"))
                            .join(" "),
                    )
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
                write!(
                    f,
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
            } => write!(
                f,
                "use {relative_path} (relative to {current_module}; {responsible} is responsible)"
            ),
            Expression::Panic {
                reason,
                responsible,
            } => write!(f, "panicking because {reason} ({responsible} is at fault)"),
            Expression::Multiple(body) => write!(
                f,
                "\n{}",
                format!("{body}")
                    .lines()
                    .map(|line| format!("  {line}"))
                    .join("\n"),
            ),
            Expression::ModuleStarts { module } => write!(f, "module {module} starts"),
            Expression::ModuleEnds => write!(f, "module ends"),
            Expression::TraceCallStarts {
                hir_call,
                function,
                arguments,
                responsible,
            } => {
                write!(f,
                    "trace: start of call of {function} with {} ({responsible} is responsible, code is at {hir_call})",
                    arguments.iter().map(|arg| format!("{arg}")).join(" "),
                )
            }
            Expression::TraceCallEnds { return_value } => {
                write!(f, "trace: end of call with return value {return_value}")
            }
            Expression::TraceExpressionEvaluated {
                hir_expression,
                value,
            } => {
                write!(f, "trace: expression {hir_expression} evaluated to {value}")
            }
            Expression::TraceFoundFuzzableClosure {
                hir_definition,
                closure,
            } => {
                write!(
                    f,
                    "trace: found fuzzable closure {closure}, defined at {hir_definition}"
                )
            }
        }
    }
}
impl fmt::Debug for VisibleExpressions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.expressions
                .keys()
                .sorted()
                .map(|id| format!("{id}"))
                .join(", ")
        )
    }
}
