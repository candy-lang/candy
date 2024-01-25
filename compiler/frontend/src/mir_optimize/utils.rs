use super::pure::PurenessInsights;
use crate::mir::{Body, Expression, Id, VisibleExpressions};
use itertools::Itertools;
use rustc_hash::FxHashSet;

impl Expression {
    /// All IDs defined inside this expression. For all expressions except
    /// functions, this returns an empty vector. The IDs are returned in the
    /// order that they are defined in.
    #[must_use]
    pub fn defined_ids(&self) -> Vec<Id> {
        let mut defined = vec![];
        self.collect_defined_ids(&mut defined);
        defined
    }
    fn collect_defined_ids(&self, defined: &mut Vec<Id>) {
        if let Self::Function {
            parameters, body, ..
        } = self
        {
            defined.extend(parameters);
            body.collect_defined_ids(defined);
        }
    }
}
impl Body {
    #[must_use]
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
    // PERF: Maybe change this to accept a closure instead of collecting them to an `FxHashSet`
    #[must_use]
    pub fn referenced_ids(&self) -> FxHashSet<Id> {
        let mut referenced = FxHashSet::default();
        self.collect_referenced_ids(&mut referenced);
        referenced
    }
    fn collect_referenced_ids(&self, referenced: &mut FxHashSet<Id>) {
        match self {
            Self::Int(_) | Self::Text(_) | Self::Builtin(_) | Self::HirId(_) => {}
            Self::Tag { value, .. } => {
                if let Some(value) = value {
                    referenced.insert(*value);
                }
            }
            Self::List(items) => {
                referenced.extend(items);
            }
            Self::Struct(fields) => {
                for (key, value) in fields {
                    referenced.insert(*key);
                    referenced.insert(*value);
                }
            }
            Self::Reference(reference) => {
                referenced.insert(*reference);
            }
            Self::Function { body, .. } => body.collect_referenced_ids(referenced),
            Self::Parameter => {}
            Self::Call {
                function,
                arguments,
            } => {
                referenced.insert(*function);
                referenced.extend(arguments);
            }
            Self::UseModule {
                current_module: _,
                relative_path,
                responsible,
            } => {
                referenced.insert(*relative_path);
                referenced.insert(*responsible);
            }
            Self::Panic {
                reason,
                responsible,
            } => {
                referenced.insert(*reason);
                referenced.insert(*responsible);
            }
            Self::TraceCallStarts {
                hir_call,
                function,
                arguments,
            }
            | Self::TraceTailCall {
                hir_call,
                function,
                arguments,
            } => {
                referenced.insert(*hir_call);
                referenced.insert(*function);
                referenced.extend(arguments);
            }
            Self::TraceCallEnds { return_value } => {
                referenced.insert(*return_value);
            }
            Self::TraceExpressionEvaluated {
                hir_expression,
                value,
            } => {
                referenced.insert(*hir_expression);
                referenced.insert(*value);
            }
            Self::TraceFoundFuzzableFunction {
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
    #[must_use]
    pub fn captured_ids(&self) -> FxHashSet<Id> {
        let mut ids = self.referenced_ids();
        for id in self.defined_ids() {
            ids.remove(&id);
        }
        ids
    }
}

impl Body {
    #[must_use]
    pub fn all_ids(&self) -> FxHashSet<Id> {
        let mut ids = self.defined_ids().into_iter().collect();
        self.collect_referenced_ids(&mut ids);
        ids
    }
}

impl Id {
    #[must_use]
    pub fn semantically_equals(
        self,
        other: Self,
        visible: &VisibleExpressions,
        pureness: &PurenessInsights,
    ) -> Option<bool> {
        if self == other {
            return Some(true);
        }

        let self_expr = visible.get(self);
        let other_expr = visible.get(other);

        if let Expression::Reference(reference) = self_expr {
            return reference.semantically_equals(other, visible, pureness);
        }
        if let Expression::Reference(reference) = other_expr {
            return self.semantically_equals(*reference, visible, pureness);
        }

        if self_expr.is_parameter() || other_expr.is_parameter() {
            return None;
        }

        if !pureness.is_definition_const(self_expr) || !pureness.is_definition_const(other_expr) {
            return None;
        }

        match (self_expr, other_expr) {
            (Expression::Int(a), Expression::Int(b)) => Some(a == b),
            (Expression::Text(a), Expression::Text(b)) => Some(a == b),
            (
                Expression::Tag {
                    symbol: symbol_a,
                    value: value_a,
                },
                Expression::Tag {
                    symbol: symbol_b,
                    value: value_b,
                },
            ) => {
                if symbol_a != symbol_b || value_a.is_some() != value_b.is_some() {
                    return Some(false);
                }
                if let (Some(a), Some(b)) = (value_a, value_b) {
                    return a.semantically_equals(*b, visible, pureness);
                }
                Some(true)
            }
            (Expression::Builtin(a), Expression::Builtin(b)) => Some(a == b),
            (Expression::List(a), Expression::List(b)) => {
                if a.len() != b.len() {
                    return Some(false);
                }
                for (a, b) in a.iter().zip_eq(b) {
                    if !a.semantically_equals(*b, visible, pureness)? {
                        return Some(false);
                    }
                }
                Some(true)
            }
            (Expression::Struct(a), Expression::Struct(b)) => {
                if a.len() != b.len() {
                    return Some(false);
                }
                // TODO: Match keys and compare values.
                None
            }
            // Expressions have different types.
            (
                Expression::Int(_)
                | Expression::Text(_)
                | Expression::Tag { .. }
                | Expression::Builtin(_)
                | Expression::List(_)
                | Expression::Struct(_),
                Expression::Int(_)
                | Expression::Text(_)
                | Expression::Tag { .. }
                | Expression::Builtin(_)
                | Expression::List(_)
                | Expression::Struct(_),
            ) => Some(false),
            _ => None,
        }
    }
}

impl Expression {
    /// Replaces all referenced IDs. Does *not* replace IDs that are defined in
    /// this expression.
    pub fn replace_id_references(&mut self, replacer: &mut impl FnMut(&mut Id)) {
        match self {
            Self::Int(_) | Self::Text(_) | Self::Builtin(_) | Self::HirId(_) => {}
            Self::Tag { value, .. } => {
                if let Some(value) = value {
                    replacer(value);
                }
            }
            Self::List(items) => {
                for item in items {
                    replacer(item);
                }
            }
            Self::Struct(fields) => {
                for (key, value) in fields {
                    replacer(key);
                    replacer(value);
                }
            }
            Self::Reference(reference) => replacer(reference),
            Self::Function {
                original_hirs: _,
                parameters,
                body,
            } => {
                for parameter in parameters {
                    replacer(parameter);
                }
                body.replace_id_references(replacer);
            }
            Self::Parameter => {}
            Self::Call {
                function,
                arguments,
            } => {
                replacer(function);
                for argument in arguments {
                    replacer(argument);
                }
            }
            Self::UseModule {
                current_module: _,
                relative_path,
                responsible,
            } => {
                replacer(relative_path);
                replacer(responsible);
            }
            Self::Panic {
                reason,
                responsible,
            } => {
                replacer(reason);
                replacer(responsible);
            }
            Self::TraceCallStarts {
                hir_call,
                function,
                arguments,
            }
            | Self::TraceTailCall {
                hir_call,
                function,
                arguments,
            } => {
                replacer(hir_call);
                replacer(function);
                for argument in arguments {
                    replacer(argument);
                }
            }
            Self::TraceCallEnds { return_value } => {
                replacer(return_value);
            }
            Self::TraceExpressionEvaluated {
                hir_expression,
                value,
            } => {
                replacer(hir_expression);
                replacer(value);
            }
            Self::TraceFoundFuzzableFunction {
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
    pub fn replace_id_references(&mut self, replacer: &mut impl FnMut(&mut Id)) {
        for (_, expression) in self.iter_mut() {
            expression.replace_id_references(replacer);
        }
    }
}

impl Expression {
    /// Replaces all IDs in this expression using the replacer, including
    /// definitions.
    pub fn replace_ids(&mut self, replacer: &mut impl FnMut(&mut Id)) {
        match self {
            Self::Function {
                original_hirs: _,
                parameters,
                body,
            } => {
                for parameter in parameters {
                    replacer(parameter);
                }
                body.replace_ids(replacer);
            }
            // All other expressions don't define IDs and instead only contain
            // references. Thus, the function above does the job.
            _ => self.replace_id_references(replacer),
        }
    }
}
impl Body {
    pub fn replace_ids(&mut self, replacer: &mut impl FnMut(&mut Id)) {
        for (id, expression) in &mut self.expressions {
            replacer(id);
            expression.replace_ids(replacer);
        }
    }
}
