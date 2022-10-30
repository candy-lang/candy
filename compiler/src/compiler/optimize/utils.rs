use crate::compiler::mir::{Body, Expression, Id, Mir, VisibleExpressions};
use std::{collections::HashSet, mem};
use tracing::error;

impl Expression {
    /// All IDs defined inside this expression. For all expressions except
    /// lambdas, this returns an empty vector.
    pub fn defined_ids(&self) -> Vec<Id> {
        let mut defined = vec![];
        self.collect_defined_ids(&mut defined);
        defined
    }
    fn collect_defined_ids(&self, defined: &mut Vec<Id>) {
        match self {
            Expression::Lambda {
                parameters,
                responsible_parameter,
                body,
                ..
            } => {
                defined.extend(parameters);
                defined.push(*responsible_parameter);
                body.collect_defined_ids(defined);
            }
            Expression::Multiple(body) => body.collect_defined_ids(defined),
            _ => {}
        }
    }
}
impl Body {
    pub fn defined_ids(&self) -> Vec<Id> {
        let mut defined = vec![];
        self.collect_defined_ids(&mut defined);
        defined
    }
    fn collect_defined_ids(&self, defined: &mut Vec<Id>) {
        for (id, expression) in self.iter() {
            defined.push(id);
            expression.collect_defined_ids(defined);
        }
    }
}

impl Expression {
    /// All IDs referenced inside this expression. If this is a lambda, this
    /// also includes references to locally defined IDs.
    pub fn referenced_ids(&self) -> Vec<Id> {
        let mut referenced = vec![];
        self.collect_referenced_ids(&mut referenced);
        referenced
    }
    fn collect_referenced_ids(&self, referenced: &mut Vec<Id>) {
        match self {
            Expression::Int(_)
            | Expression::Text(_)
            | Expression::Symbol(_)
            | Expression::Builtin(_)
            | Expression::Responsibility(_) => {}
            Expression::Struct(fields) => {
                for (key, value) in fields {
                    referenced.push(*key);
                    referenced.push(*value);
                }
            }
            Expression::Reference(reference) => referenced.push(*reference),
            Expression::Lambda { body, .. } => body.collect_referenced_ids(referenced),
            Expression::Parameter => {}
            Expression::Call {
                function,
                arguments,
                responsible,
            } => {
                referenced.push(*function);
                referenced.extend(arguments);
                referenced.push(*responsible);
            }
            Expression::UseModule {
                current_module: _,
                relative_path,
                responsible,
            } => {
                referenced.push(*relative_path);
                referenced.push(*responsible);
            }
            Expression::Needs {
                responsible,
                condition,
                reason,
            } => {
                referenced.push(*responsible);
                referenced.push(*condition);
                referenced.push(*reason);
            }
            Expression::Panic {
                reason,
                responsible,
            } => {
                referenced.push(*reason);
                referenced.push(*responsible);
            }
            Expression::Error { child, .. } => {
                if let Some(child) = child {
                    referenced.push(*child);
                }
            }
            Expression::Multiple(body) => body.collect_referenced_ids(referenced),
        }
    }
}
impl Body {
    pub fn referenced_ids(&self) -> Vec<Id> {
        let mut referenced = vec![];
        self.collect_referenced_ids(&mut referenced);
        referenced
    }
    fn collect_referenced_ids(&self, referenced: &mut Vec<Id>) {
        for (_, expression) in self.iter() {
            expression.collect_referenced_ids(referenced);
        }
    }
}

impl Expression {
    pub fn captured_ids(&self) -> Vec<Id> {
        let defined = self.defined_ids().into_iter().collect::<HashSet<_>>();
        let referenced = self.referenced_ids().into_iter().collect::<HashSet<_>>();
        referenced.difference(&defined).copied().collect()
    }
}
impl Body {
    pub fn captured_ids(&self) -> Vec<Id> {
        let defined = self.defined_ids().into_iter().collect::<HashSet<_>>();
        let referenced = self.referenced_ids().into_iter().collect::<HashSet<_>>();
        referenced.difference(&defined).copied().collect()
    }
}

impl Expression {
    pub fn is_pure(&self) -> bool {
        match self {
            Expression::Int(_) => true,
            Expression::Text(_) => true,
            Expression::Reference(_) => true,
            Expression::Symbol(_) => true,
            Expression::Struct(_) => true,
            Expression::Lambda { .. } => true,
            Expression::Parameter => false,
            Expression::Builtin(_) => true,
            Expression::Responsibility(_) => true,
            Expression::Call { .. } => false,
            Expression::UseModule { .. } => false,
            Expression::Needs { .. } => false,
            Expression::Panic { .. } => false,
            Expression::Error { .. } => false,
            Expression::Multiple(body) => body.iter().all(|(_, expr)| expr.is_pure()),
        }
    }
}

impl Id {
    /// Whether the value of this expression is known at compile-time.
    pub fn is_constant(self, visible: &VisibleExpressions) -> bool {
        let expression = visible.get(self);
        match expression {
            Expression::Int(_)
            | Expression::Text(_)
            | Expression::Symbol(_)
            | Expression::Builtin(_)
            | Expression::Responsibility(_) => true,
            Expression::Reference(id) => id.is_constant(visible),
            Expression::Struct(fields) => fields
                .iter()
                .all(|(key, value)| key.is_constant(visible) && value.is_constant(visible)),
            Expression::Lambda { .. } => expression
                .captured_ids()
                .iter()
                .all(|captured| captured.is_constant(visible)),
            Expression::Parameter
            | Expression::Call { .. }
            | Expression::UseModule { .. }
            | Expression::Needs { .. }
            | Expression::Panic { .. }
            | Expression::Error { .. } => false,
            Expression::Multiple(_) => expression
                .captured_ids()
                .iter()
                .all(|captured| captured.is_constant(visible)),
        }
    }

    pub fn semantically_equals(self, other: Id, visible: &VisibleExpressions) -> Option<bool> {
        if self == other {
            return Some(true);
        }

        let self_expr = visible.get(self);
        let other_expr = visible.get(other);

        if matches!(self_expr, Expression::Parameter) || matches!(other_expr, Expression::Parameter)
        {
            return None;
        }
        if let Expression::Reference(reference) = self_expr {
            return reference.semantically_equals(other, visible);
        }
        if let Expression::Reference(reference) = other_expr {
            return self.semantically_equals(*reference, visible);
        }

        if self_expr == other_expr {
            return Some(true);
        }

        if !self.is_constant(visible) || !other.is_constant(visible) {
            return None;
        }

        Some(false)
    }
}

impl Expression {
    /// Replaces all referenced IDs. Does *not* replace IDs that are defined in
    /// this expression.
    pub fn replace_id_references<F: FnMut(&mut Id)>(&mut self, replacer: &mut F) {
        match self {
            Expression::Int(_)
            | Expression::Text(_)
            | Expression::Symbol(_)
            | Expression::Builtin(_)
            | Expression::Responsibility(_) => {}
            Expression::Struct(fields) => {
                *fields = fields
                    .iter()
                    .map(|(key, value)| {
                        let mut key = key.clone();
                        let mut value = value.clone();
                        replacer(&mut key);
                        replacer(&mut value);
                        (key, value)
                    })
                    .collect();
            }
            Expression::Reference(reference) => replacer(reference),
            Expression::Lambda {
                parameters,
                responsible_parameter,
                body,
                fuzzable: _,
            } => {
                for parameter in parameters {
                    replacer(parameter);
                }
                replacer(responsible_parameter);
                body.replace_id_references(replacer);
            }
            Expression::Parameter => {}
            Expression::Call {
                function,
                arguments,
                responsible,
            } => {
                replacer(function);
                for argument in arguments {
                    replacer(argument);
                }
                replacer(responsible);
            }
            Expression::UseModule {
                current_module: _,
                relative_path,
                responsible,
            } => {
                replacer(relative_path);
                replacer(responsible);
            }
            Expression::Needs {
                responsible,
                condition,
                reason,
            } => {
                replacer(responsible);
                replacer(condition);
                replacer(reason);
            }
            Expression::Panic {
                reason,
                responsible,
            } => {
                replacer(reason);
                replacer(responsible);
            }
            Expression::Error { child, .. } => {
                if let Some(child) = child {
                    replacer(child);
                }
            }
            Expression::Multiple(body) => body.replace_id_references(replacer),
        }
    }
}
impl Body {
    pub fn replace_id_references<F: FnMut(&mut Id)>(&mut self, replacer: &mut F) {
        for (_, expression) in self.iter_mut() {
            expression.replace_id_references(replacer);
        }
    }
}

impl Expression {
    /// Replaces all IDs in this expression using the replacer, including
    /// definitions.
    pub fn replace_ids<F: FnMut(&mut Id)>(&mut self, replacer: &mut F) {
        match self {
            Expression::Lambda {
                parameters,
                responsible_parameter,
                body,
                fuzzable: _,
            } => {
                for parameter in parameters {
                    replacer(parameter);
                }
                replacer(responsible_parameter);
                body.replace_ids(replacer);
            }
            Expression::Multiple(body) => body.replace_ids(replacer),
            // All other expressions don't define IDs and instead only contain
            // references. Thus, the function above does the job.
            _ => self.replace_id_references(replacer),
        }
    }
}
impl Body {
    pub fn replace_ids<F: FnMut(&mut Id)>(&mut self, replacer: &mut F) {
        let body = mem::replace(self, Body::new());
        for (mut id, mut expression) in body.into_iter() {
            replacer(&mut id);
            expression.replace_ids(replacer);
            self.push(id, expression);
        }
    }
}

impl Mir {
    pub fn validate(&self) {
        self.validate_body(&self.body, &mut HashSet::new(), im::HashSet::new());
    }
    fn validate_body(
        &self,
        body: &Body,
        defined_ids: &mut HashSet<Id>,
        mut visible: im::HashSet<Id>,
    ) {
        if body.iter().next().is_none() {
            error!("A body of a lambda is empty! Lambdas should have at least a return value.");
            error!("This is the MIR:\n{self:?}");
            panic!("Mir is invalid!");
        }
        for (id, expression) in body.iter() {
            for captured in expression.captured_ids() {
                if !visible.contains(&captured) {
                    error!("Mir is invalid! {id} captures {captured}, but that's not visible.");
                    error!("This is the MIR:\n{self:?}");
                    panic!("Mir is invalid!");
                }
            }
            if let Expression::Lambda {
                parameters,
                responsible_parameter,
                body,
                fuzzable: _,
            } = expression
            {
                let mut inner_visible = visible.clone();
                inner_visible.extend(parameters.iter().copied());
                inner_visible.insert(*responsible_parameter);
                self.validate_body(&body, defined_ids, inner_visible);
            }
            if let Expression::Multiple(body) = expression {
                self.validate_body(&body, defined_ids, visible.clone());
            }

            if defined_ids.contains(&id) {
                error!("ID {id} exists twice.");
                error!("This is the MIR:\n{self:?}");
                panic!("Mir is invalid!");
            }
            defined_ids.insert(id);

            visible.insert(id);
        }
    }
}
