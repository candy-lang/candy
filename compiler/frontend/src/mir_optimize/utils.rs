use crate::mir::{Body, Expression, Id, VisibleExpressions};
use rustc_hash::FxHashSet;
use std::mem;

impl Expression {
    /// All IDs defined inside this expression. For all expressions except
    /// functions, this returns an empty vector. The IDs are returned in the
    /// order that they are defined in.
    pub fn defined_ids(&self) -> Vec<Id> {
        let mut defined = vec![];
        self.collect_defined_ids(&mut defined);
        defined
    }
    fn collect_defined_ids(&self, defined: &mut Vec<Id>) {
        match self {
            Expression::Function {
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
    /// All IDs referenced inside this expression. If this is a function, this
    /// also includes references to locally defined IDs. IDs are returned in the
    /// order that they are referenced, which means that the vector may contain
    /// the same ID multiple times.
    pub fn referenced_ids(&self) -> FxHashSet<Id> {
        let mut referenced = FxHashSet::default();
        self.collect_referenced_ids(&mut referenced);
        referenced
    }
    fn collect_referenced_ids(&self, referenced: &mut FxHashSet<Id>) {
        match self {
            Expression::Int(_)
            | Expression::Text(_)
            | Expression::Symbol(_)
            | Expression::Builtin(_)
            | Expression::HirId(_) => {}
            Expression::List(items) => {
                referenced.extend(items);
            }
            Expression::Struct(fields) => {
                for (key, value) in fields {
                    referenced.insert(*key);
                    referenced.insert(*value);
                }
            }
            Expression::Reference(reference) => {
                referenced.insert(*reference);
            }
            Expression::Function { body, .. } => body.collect_referenced_ids(referenced),
            Expression::Parameter => {}
            Expression::Call {
                function,
                arguments,
                responsible,
            } => {
                referenced.insert(*function);
                referenced.extend(arguments);
                referenced.insert(*responsible);
            }
            Expression::UseModule {
                current_module: _,
                relative_path,
                responsible,
            } => {
                referenced.insert(*relative_path);
                referenced.insert(*responsible);
            }
            Expression::Panic {
                reason,
                responsible,
            } => {
                referenced.insert(*reason);
                referenced.insert(*responsible);
            }
            Expression::Multiple(body) => body.collect_referenced_ids(referenced),
            Expression::TraceCallStarts {
                hir_call,
                function,
                arguments,
                responsible,
            } => {
                referenced.insert(*hir_call);
                referenced.insert(*function);
                referenced.extend(arguments);
                referenced.insert(*responsible);
            }
            Expression::TraceCallEnds { return_value } => {
                referenced.insert(*return_value);
            }
            Expression::TraceExpressionEvaluated {
                hir_expression,
                value,
            } => {
                referenced.insert(*hir_expression);
                referenced.insert(*value);
            }
            Expression::TraceFoundFuzzableFunction {
                hir_definition,
                function,
            } => {
                referenced.insert(*hir_definition);
                referenced.insert(*function);
            }
        }
    }
}
impl Body {
    fn collect_referenced_ids(&self, referenced: &mut FxHashSet<Id>) {
        for (_, expression) in self.iter() {
            expression.collect_referenced_ids(referenced);
        }
    }
}

impl Expression {
    pub fn captured_ids(&self) -> FxHashSet<Id> {
        let mut ids = self.referenced_ids();
        for id in self.defined_ids() {
            ids.remove(&id);
        }
        ids
    }
}

impl Body {
    pub fn all_ids(&self) -> FxHashSet<Id> {
        let mut ids = self.defined_ids().into_iter().collect::<FxHashSet<_>>();
        self.collect_referenced_ids(&mut ids);
        ids
    }
}

impl Expression {
    pub fn is_pure(&self) -> bool {
        match self {
            Expression::Int(_) => true,
            Expression::Text(_) => true,
            Expression::Reference(_) => true,
            Expression::Symbol(_) => true,
            Expression::List(_) => true,
            Expression::Struct(_) => true,
            Expression::Function { .. } => true,
            Expression::Parameter => false,
            Expression::Builtin(_) => true,
            Expression::HirId(_) => true,
            Expression::Call { .. } => false,
            Expression::UseModule { .. } => false,
            Expression::Panic { .. } => false,
            Expression::Multiple(body) => body.iter().all(|(_, expr)| expr.is_pure()),
            Expression::TraceCallStarts { .. }
            | Expression::TraceCallEnds { .. }
            | Expression::TraceExpressionEvaluated { .. }
            | Expression::TraceFoundFuzzableFunction { .. } => false,
        }
    }

    /// Whether the value of this expression is pure and known at compile-time.
    /// This is useful for moving expressions around without changing the
    /// semantics.
    pub fn is_constant(&self, visible: &VisibleExpressions) -> bool {
        self.is_pure()
            && self
                .captured_ids()
                .iter()
                .all(|captured| visible.get(*captured).is_constant(visible))
    }
}

impl Id {
    pub fn semantically_equals(self, other: Id, visible: &VisibleExpressions) -> Option<bool> {
        if self == other {
            return Some(true);
        }

        let self_expr = visible.get(self);
        let other_expr = visible.get(other);

        if let Expression::Reference(reference) = self_expr {
            return reference.semantically_equals(other, visible);
        }
        if let Expression::Reference(reference) = other_expr {
            return self.semantically_equals(*reference, visible);
        }

        if matches!(self_expr, Expression::Parameter) || matches!(other_expr, Expression::Parameter)
        {
            return None;
        }

        if self_expr == other_expr {
            return Some(true);
        }

        if !self_expr.is_constant(visible) || !other_expr.is_constant(visible) {
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
            | Expression::HirId(_) => {}
            Expression::List(items) => {
                for item in items {
                    replacer(item);
                }
            }
            Expression::Struct(fields) => {
                for (key, value) in fields {
                    replacer(key);
                    replacer(value);
                }
            }
            Expression::Reference(reference) => replacer(reference),
            Expression::Function {
                original_hirs: _,
                parameters,
                responsible_parameter,
                body,
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
            Expression::Panic {
                reason,
                responsible,
            } => {
                replacer(reason);
                replacer(responsible);
            }
            Expression::Multiple(body) => body.replace_id_references(replacer),
            Expression::TraceCallStarts {
                hir_call,
                function,
                arguments,
                responsible,
            } => {
                replacer(hir_call);
                replacer(function);
                for argument in arguments {
                    replacer(argument);
                }
                replacer(responsible);
            }
            Expression::TraceCallEnds { return_value } => {
                replacer(return_value);
            }
            Expression::TraceExpressionEvaluated {
                hir_expression,
                value,
            } => {
                replacer(hir_expression);
                replacer(value);
            }
            Expression::TraceFoundFuzzableFunction {
                hir_definition,
                function,
            } => {
                replacer(hir_definition);
                replacer(function);
            }
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
            Expression::Function {
                original_hirs: _,
                parameters,
                responsible_parameter,
                body,
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
        let body = mem::take(self);
        for (mut id, mut expression) in body.into_iter() {
            replacer(&mut id);
            expression.replace_ids(replacer);
            self.push(id, expression);
        }
    }
}
