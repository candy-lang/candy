use crate::{
    compiler::mir::{Body, Expression, Id, Mir},
    utils::IdGenerator,
};
use itertools::Itertools;
use std::{cmp::Ordering, collections::HashSet};
use tracing::debug;

impl Mir {
    pub fn lift_constants(&mut self) {
        let mut constants = vec![];
        self.body
            .visit(&mut |id, expression, visible, is_return_value| {
                if expression.is_constant(visible) {
                    if is_return_value && let Expression::Reference(_) = expression {
                        // Returned references shouldn't be lifted. For each of
                        // them, it's guaranteed that no later expression
                        // depends on it (because it's the last in the body) and
                        // if it were to be lifted, we'd have to add a reference
                        // anyway.
                        return;
                    }
                    constants.push((id, expression.clone()));
                }
            });
        debug!(
            "Found constants: {}",
            constants.iter().map(|(id, _)| format!("{id}")).join(", ")
        );

        let constant_ids = constants.iter().map(|(id, _)| *id).collect::<HashSet<_>>();
        self.body.visit_bodies(&mut |body| {
            Self::remove_constants(body, &constant_ids, &mut self.id_generator)
        });
        Self::remove_constants(&mut self.body, &constant_ids, &mut self.id_generator);
        for (_, expression) in &mut constants {
            expression.visit_bodies(&mut |body| {
                Self::remove_constants(body, &constant_ids, &mut self.id_generator);
            })
        }

        constants.sort_by(|(_, a), (_, b)| {
            fn order_score(expr: &Expression) -> u8 {
                match expr {
                    Expression::Responsibility(_) => 0,
                    Expression::Builtin(_) => 1,
                    Expression::Symbol(_) => 2,
                    Expression::Int(_) => 3,
                    Expression::Text(_) => 4,
                    _ => 5,
                }
            }
            match (a, b) {
                (Expression::Responsibility(_), Expression::Responsibility(_)) => Ordering::Equal,
                (Expression::Builtin(_), Expression::Builtin(_)) => Ordering::Equal,
                (Expression::Symbol(a), Expression::Symbol(b)) => a.cmp(b),
                (Expression::Int(a), Expression::Int(b)) => a.cmp(b),
                (Expression::Text(a), Expression::Text(b)) => a.cmp(b),
                _ => order_score(a).cmp(&order_score(b)),
            }
        });
        self.body.insert_at_front(constants);
    }

    fn remove_constants(
        body: &mut Body,
        constant_ids: &HashSet<Id>,
        id_generator: &mut IdGenerator<Id>,
    ) {
        let return_value = body.return_value();
        body.remove_all(&mut |id, _| constant_ids.contains(&id));

        if body.iter().map(|(id, _)| id).last() != Some(return_value) {
            // The return value was removed. Add a reference to the lifted
            // constant.
            body.push(id_generator.generate(), Expression::Reference(return_value));
        }
    }
}
