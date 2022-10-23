use crate::compiler::mir::{Expression, Id, Mir};
use core::fmt;
use std::{collections::HashSet, ops::Add};

impl Expression {
    pub fn replace_ids<F: FnMut(&mut Id)>(&mut self, replacer: &mut F) {
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
                ..
            } => {
                for parameter in parameters {
                    replacer(parameter);
                }
                replacer(responsible_parameter);
                for id in body {
                    replacer(id);
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

impl Mir {
    pub fn defined_ids(&self, id: &Id) -> Vec<Id> {
        let mut defined = vec![];
        self.collect_defined_ids(id, &mut defined);
        defined
    }
    fn collect_defined_ids(&self, id: &Id, defined: &mut Vec<Id>) {
        defined.push(id.clone());
        if let Expression::Lambda {
            parameters,
            responsible_parameter,
            body,
            ..
        } = self.expressions.get(id).unwrap()
        {
            defined.extend(parameters);
            defined.push(responsible_parameter.clone());
            for id in body {
                self.collect_defined_ids(id, defined);
            }
        }
    }

    pub fn referenced_ids(&self, id: &Id) -> Vec<Id> {
        let mut referenced = vec![];
        self.collect_referenced_ids(id, &mut referenced);
        referenced
    }
    fn collect_referenced_ids(&self, id: &Id, referenced: &mut Vec<Id>) {
        match self.expressions.get(id).unwrap() {
            Expression::Int(_)
            | Expression::Text(_)
            | Expression::Symbol(_)
            | Expression::Builtin(_)
            | Expression::Responsibility(_) => {}
            Expression::Struct(fields) => {
                for (key, value) in fields {
                    referenced.push(key.clone());
                    referenced.push(value.clone());
                }
            }
            Expression::Reference(reference) => referenced.push(reference.clone()),
            Expression::Lambda { body, .. } => {
                for id in body {
                    self.collect_referenced_ids(id, referenced);
                }
            }
            Expression::Call {
                function,
                arguments,
                responsible,
            } => {
                referenced.push(function.clone());
                referenced.extend(arguments);
                referenced.push(responsible.clone());
            }
            Expression::UseModule {
                relative_path,
                responsible,
                ..
            } => {
                referenced.push(relative_path.clone());
                referenced.push(responsible.clone());
            }
            Expression::Needs {
                responsible,
                condition,
                reason,
            } => {
                referenced.push(responsible.clone());
                referenced.push(condition.clone());
                referenced.push(reason.clone());
            }
            Expression::Panic {
                reason,
                responsible,
            } => {
                referenced.push(reason.clone());
                referenced.push(responsible.clone());
            }
            Expression::Error { child, errors } => {
                if let Some(child) = child {
                    self.collect_referenced_ids(child, referenced);
                }
            }
        }
    }

    pub fn captured_ids(&self, lambda: &Id) -> Vec<Id> {
        let defined: HashSet<Id> = self.defined_ids(lambda).into_iter().collect();
        let referenced: HashSet<Id> = self.referenced_ids(lambda).into_iter().collect();
        referenced.difference(&defined).copied().collect()
    }

    pub fn is_constant(&self, id: &Id) -> bool {
        match self.expressions.get(id).unwrap() {
            Expression::Int(_)
            | Expression::Text(_)
            | Expression::Symbol(_)
            | Expression::Builtin(_)
            | Expression::Responsibility(_) => true,
            Expression::Reference(id) => self.is_constant(id),
            Expression::Struct(fields) => fields
                .iter()
                .all(|(key, value)| self.is_constant(key) && self.is_constant(value)),
            Expression::Lambda { .. } => {
                self.captured_ids(id).iter().all(|id| self.is_constant(id))
            }
            Expression::Call { .. }
            | Expression::UseModule { .. }
            | Expression::Needs { .. }
            | Expression::Panic { .. }
            | Expression::Error { .. } => false,
        }
    }

    pub fn equals(&self, a: &Id, b: &Id) -> Option<bool> {
        if a == b {
            return Some(true);
        }
        let a_expr = self.expressions.get(a).unwrap();
        let b_expr = self.expressions.get(a).unwrap();
        if let Expression::Reference(reference) = a_expr {
            return self.equals(reference, b);
        }
        if let Expression::Reference(reference) = b_expr {
            return self.equals(a, reference);
        }

        if a_expr == b_expr {
            return Some(true);
        }

        if !self.is_constant(a) || !self.is_constant(b) {
            return None;
        }

        Some(false)
    }

    pub fn replace_ids<F: FnMut(&mut Id)>(&mut self, replacer: &mut F) {
        for id in &mut self.body {
            let mut expression = self.expressions.remove(id).unwrap();

            replacer(id);
            expression.replace_ids(replacer);

            self.expressions.insert(id.clone(), expression);
        }
    }

    // Replaces a range of the statements with some other statements. Updates
    // all later references into the range using the `reference_replacements`.
    //
    // # Example
    //
    // Given this code:
    //
    // ```txt
    // HirId(0) = symbol :
    // HirId(1) = int 1
    // HirId(2) = int 2
    // HirId(3) = builtinAdd 1 2
    // HirId(4) = builtinPrint 3
    // HirId(5) = symbol :foo
    // HirId(6) = builtinPrint 5
    // ```
    //
    // Calling `replace_range(HirId(1), 3, [number 3], {3, 1})` turns it into this:
    //
    // ```txt
    // HirId(0) = symbol :
    // HirId(1) = number 3
    // HirId(2) = primitive_print 1
    // HirId(3) = symbol :foo
    // HirId(4) = primitive_print 3
    // ```
    // pub fn replace_range(
    //     &mut self,
    //     start: Id,
    //     length: usize,
    //     replacement: Vec<Statement>,
    //     reference_replacements: HashMap<Id, Id>,
    // ) {
    //     debug!(
    //         "Optimizer: Replacing {} len {} with {:?}. Replacements: {:?}",
    //         start, length, replacement, reference_replacements
    //     );
    //     let mut statements = vec![];

    //     let start = start;
    //     let end = start + length as u32;
    //     let start_index = start as usize - self.in_ as usize - 1;
    //     let end_index = end as usize - self.in_ as usize - 1;

    //     // The statements before the replaced part stay the same.
    //     for statement in &self.statements[0..start_index] {
    //         statements.push(statement.clone());
    //     }

    //     // The replaced part gets ignored, we use the replacement instead.
    //     for statement in &replacement {
    //         statements.push(statement.clone());
    //     }

    //     // The statements after that need to get their IDs replaced. IDs that
    //     // reference statements before the replaced ones stay the same. IDs that
    //     // reference into the replaced range get replaced according to the
    //     // `reference_replacements`. IDs that reference statements after the
    //     // replaced range get shifted – the replacement may have a different
    //     // length than the replaced statements.
    //     let shift = replacement.len() as isize - length as isize;
    //     let transform = |id| {
    //         if id < start {
    //             id
    //         } else if id >= end {
    //             (id as isize + shift) as u32
    //         } else {
    //             *reference_replacements.get(&id).expect(&format!(
    //                 "Reference to ID {} in replaced range with no replacement.",
    //                 id
    //             ))
    //         }
    //     };
    //     for statement in &mut self.statements[end_index as usize..] {
    //         let mut statement = statement.clone();
    //         statement.replace_ids(&transform);
    //         statements.push(statement);
    //     }

    //     self.statements = statements;
    //     self.in_ = transform(self.in_);
    //     self.out = transform(self.out);

    //     debug!("Now the HIR is this: {}", self);
    // }

    // pub fn replace_ids<F: Fn(Id) -> Id>(&mut self, transform: &F) {
    //     self.in_ = transform(self.in_);
    //     self.out = transform(self.out);
    //     for (_, statement) in self.iter_mut() {
    //         statement.replace_ids(transform);
    //     }
    // }
}
