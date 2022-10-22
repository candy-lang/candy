use core::fmt;
use std::ops::Add;

use crate::compiler::hir::{Body, Expression, Id, Lambda};

impl Expression {
    pub fn replace_ids<F: FnMut(&mut Id)>(&mut self, replacer: &mut F) {
        match self {
            Expression::Int(_)
            | Expression::Text(_)
            | Expression::Symbol(_)
            | Expression::Builtin(_) => {}
            Expression::Reference(reference) => replacer(reference),
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
            Expression::Lambda(Lambda { body, .. }) => {
                for id in &body.ids {
                    let expression = body.expressions.get_mut(&id).unwrap();
                    expression.replace_ids::<F>(replacer);
                }
            }
            Expression::Call {
                function,
                arguments,
            } => {
                replacer(function);
                for argument in arguments {
                    replacer(argument);
                }
            }
            Expression::UseModule { relative_path, .. } => replacer(relative_path),
            Expression::Needs { condition, reason } => {
                replacer(condition);
                replacer(reason);
            }
            Expression::Error { child, errors } => {
                if let Some(child) = child {
                    replacer(child);
                }
            }
        }
    }
}

impl Body {
    pub fn replace_ids<F: FnMut(&mut Id)>(&mut self, replacer: &mut F) {
        for id in &mut self.ids {
            let mut expression = self.expressions.remove(&id).unwrap();
            let identifier = self.identifiers.remove(&id);

            replacer(id);
            expression.replace_ids(replacer);

            self.expressions.insert(id.clone(), expression);
            if let Some(identifier) = identifier {
                self.identifiers.insert(id.clone(), identifier);
            }
        }
    }

    pub fn is_constant(&self, id: &Id) -> bool {
        match self.expressions.get(id).unwrap() {
            Expression::Int(_)
            | Expression::Text(_)
            | Expression::Symbol(_)
            | Expression::Builtin(_) => true,
            Expression::Reference(id) => self.is_constant(id),
            Expression::Struct(fields) => fields
                .iter()
                .all(|(key, value)| self.is_constant(key) && self.is_constant(value)),
            Expression::Lambda(lambda) => lambda
                .captured_ids(id)
                .iter()
                .all(|id| self.is_constant(id)),
            Expression::Call { .. }
            | Expression::UseModule { .. }
            | Expression::Needs { .. }
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

        match (
            self.expressions.get(a).unwrap(),
            self.expressions.get(b).unwrap(),
        ) {
            (Expression::Int(a), Expression::Int(b)) => Some(a == b),
            (Expression::Text(a), Expression::Text(b)) => Some(a == b),
            (Expression::Symbol(a), Expression::Symbol(b)) => Some(a == b),
            (Expression::Struct(a), Expression::Struct(b)) => {
                // TODO
                todo!()
            }
            // Also consider lambdas equal where only some IDs are named
            // differently.
            (Expression::Lambda(a), Expression::Lambda(b)) => Some(a == b),
            (Expression::Builtin(a), Expression::Builtin(b)) => Some(a == b),
            (Expression::Call { .. }, _)
            | (Expression::UseModule { .. }, _)
            | (Expression::Needs { .. }, _)
            | (Expression::Error { .. }, _)
            | (_, Expression::Call { .. })
            | (_, Expression::UseModule { .. })
            | (_, Expression::Needs { .. })
            | (_, Expression::Error { .. }) => None,
            (_, _) => Some(false),
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

pub struct Complexity {
    is_self_contained: bool,
    expressions: usize,
}

impl Complexity {
    fn none() -> Self {
        Self {
            is_self_contained: true,
            expressions: 0,
        }
    }
    fn single() -> Self {
        Self {
            is_self_contained: true,
            expressions: 1,
        }
    }
}
impl Add for Complexity {
    type Output = Complexity;

    fn add(self, other: Self) -> Self::Output {
        Complexity {
            is_self_contained: self.is_self_contained && other.is_self_contained,
            expressions: self.expressions + other.expressions,
        }
    }
}
impl fmt::Display for Complexity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}, {} expressions",
            if self.is_self_contained {
                "self-contained"
            } else {
                "still contains `use`"
            },
            self.expressions
        )
    }
}

impl Body {
    pub fn complexity(&self) -> Complexity {
        let mut complexity = Complexity::none();
        for (_, expression) in &self.expressions {
            complexity = complexity + expression.complexity();
        }
        complexity
    }
}
impl Expression {
    fn complexity(&self) -> Complexity {
        match self {
            Expression::Int(_) => Complexity::single(),
            Expression::Text(_) => Complexity::single(),
            Expression::Reference(_) => Complexity::single(),
            Expression::Symbol(_) => Complexity::single(),
            Expression::Struct(_) => Complexity::single(),
            Expression::Lambda(lambda) => Complexity::single() + lambda.body.complexity(),
            Expression::Builtin(_) => Complexity::single(),
            Expression::Call { .. } => Complexity::single(),
            Expression::UseModule { .. } => Complexity {
                is_self_contained: false,
                expressions: 1,
            },
            Expression::Needs { .. } => Complexity::single(),
            Expression::Error { .. } => Complexity::single(),
        }
    }
}
