use crate::compiler::mir::{Expression, Id, Mir};
use core::fmt;
use std::{
    collections::{HashMap, HashSet},
    ops::Add,
};

impl Id {
    /// All IDs defined inside the expression of this ID. For all expressions
    /// except lambdas, this only returns the single ID itself.
    pub fn defined_ids(self, expressions: &HashMap<Id, Expression>) -> Vec<Id> {
        let mut defined = vec![];
        self.collect_defined_ids(expressions, &mut defined);
        defined
    }
    fn collect_defined_ids(self, expressions: &HashMap<Id, Expression>, defined: &mut Vec<Id>) {
        defined.push(self);
        if let Expression::Lambda {
            parameters,
            responsible_parameter,
            body,
            ..
        } = expressions.get(&self).unwrap()
        {
            defined.extend(parameters);
            defined.push(*responsible_parameter);
            for id in body {
                id.collect_defined_ids(expressions, defined);
            }
        }
    }

    /// All IDs referenced inside the expression of this ID.
    pub fn referenced_ids(self, expressions: &HashMap<Id, Expression>) -> Vec<Id> {
        let mut referenced = vec![];
        self.collect_referenced_ids(expressions, &mut referenced);
        referenced
    }
    fn collect_referenced_ids(
        self,
        expressions: &HashMap<Id, Expression>,
        referenced: &mut Vec<Id>,
    ) {
        match expressions.get(&self).unwrap() {
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
            Expression::Lambda { body, .. } => {
                for id in body {
                    id.collect_referenced_ids(expressions, referenced);
                }
            }
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
                relative_path,
                responsible,
                ..
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
            Expression::Error { child, errors } => {
                if let Some(child) = child {
                    child.collect_referenced_ids(expressions, referenced);
                }
            }
        }
    }

    pub fn captured_ids(&self, expressions: &HashMap<Id, Expression>) -> Vec<Id> {
        let defined = self
            .defined_ids(expressions)
            .into_iter()
            .collect::<HashSet<_>>();
        let referenced = self
            .referenced_ids(expressions)
            .into_iter()
            .collect::<HashSet<_>>();
        referenced.difference(&defined).copied().collect()
    }

    pub fn is_constant(self, expressions: &HashMap<Id, Expression>) -> bool {
        match expressions.get(&self).unwrap() {
            Expression::Int(_)
            | Expression::Text(_)
            | Expression::Symbol(_)
            | Expression::Builtin(_)
            | Expression::Responsibility(_) => true,
            Expression::Reference(id) => id.is_constant(expressions),
            Expression::Struct(fields) => fields
                .iter()
                .all(|(key, value)| key.is_constant(expressions) && value.is_constant(expressions)),
            Expression::Lambda { .. } => self
                .captured_ids(expressions)
                .iter()
                .all(|id| id.is_constant(expressions)),
            Expression::Call { .. }
            | Expression::UseModule { .. }
            | Expression::Needs { .. }
            | Expression::Panic { .. }
            | Expression::Error { .. } => false,
        }
    }

    pub fn semantically_equals(
        self,
        other: Id,
        expressions: &HashMap<Id, Expression>,
    ) -> Option<bool> {
        if self == other {
            return Some(true);
        }
        let self_expr = expressions.get(&self).unwrap();
        let other_expr = expressions.get(&other).unwrap();
        if let Expression::Reference(reference) = self_expr {
            return reference.semantically_equals(other, expressions);
        }
        if let Expression::Reference(reference) = other_expr {
            return self.semantically_equals(*reference, expressions);
        }

        if self_expr == other_expr {
            return Some(true);
        }

        if !self.is_constant(expressions) || !other.is_constant(expressions) {
            return None;
        }

        Some(false)
    }
}

impl Id {
    /// When traversing the expressions, we sometimes want to mutably borrow
    /// some part of an outer expression (for example, a lambda's body) while
    /// still traversing over inner expressions. The invariant that makes it
    /// safe to have both a mutably borrowed outer expression as well as
    /// mutably borrowed inner expressions is that expressions never reference
    /// later expressions. Rust doesn't know about this invariant, so using this
    /// method, you can temporarily mutably borrow an expression while
    /// continuing to use the rest of the expressions.
    ///
    /// Internally, this just temporarily removes an expression from the map and
    /// then adds it again when the wrapper is dropped.
    pub fn temporarily_get_mut<'a>(
        self,
        expressions: &'a mut HashMap<Id, Expression>,
    ) -> TemporaryExpression<'a> {
        let expression = expressions.remove(&self).unwrap();
        TemporaryExpression {
            id: self,
            expression,
            remaining: expressions,
        }
    }
}
pub struct TemporaryExpression<'a> {
    id: Id,
    pub expression: Expression,
    pub remaining: &'a mut HashMap<Id, Expression>,
}
impl<'a> Drop for TemporaryExpression<'a> {
    fn drop(&mut self) {
        self.remaining.insert(self.id, self.expression.clone());
    }
}

impl Id {
    pub fn replace_id_references<F: FnMut(&mut Id)>(
        self,
        expressions: &mut HashMap<Id, Expression>,
        replacer: &mut F,
    ) {
        let mut temporary = self.temporarily_get_mut(expressions);
        match &mut temporary.expression {
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
                ..
            } => {
                for parameter in parameters {
                    replacer(parameter);
                }
                replacer(responsible_parameter);
                for id in body {
                    id.replace_id_references(temporary.remaining, replacer);
                }
            }
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
            Expression::UseModule { relative_path, .. } => replacer(relative_path),
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
        }
    }
}
