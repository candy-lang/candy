//! Constant lifting refers to lifting constants from lambdas into surrounding
//! scopes.
//!
//! Here's a before-and-after example:
//!
//! ```mir
//!                             |  $2 = Foo
//!                             |  $5 = Foo
//! $0 = { ($1 responsible) ->  |  $0 = { ($1 responsible) ->
//!   $2 = Foo                  |
//!   ...                       |    ...
//! }                           |  }
//! $3 = { ($4 responsible) ->  |  $3 = { ($4 responsible) ->
//!   $5 = Foo                  |
//!   ...                       |    ...
//! }                           |  }
//! ```
//!
//! This enables more effective [common subtree elimination] and is especially
//! important for avoiding an exponential code blowup when importing modules.
//!
//! When the lifted constant is the last in a body (aka it's the body's return
//! value), a reference expression is inserted in its place.
//!
//! ```mir
//!                             |  $2 = Foo
//! $0 = { ($1 responsible) ->  |  $0 = { ($1 responsible) ->
//!   $2 = Foo                  |    $3 = $2
//! }                           |  }
//! ```
//!
//! TODO: Don't sort constant here, but in a cleanup pass instead.
//! TODO: Have a separate constant heap directly in the LIR, so that
//! instructions such as `Instruction::CreateInt` are never actually executed at
//! runtime.
//!
//! [common subtree elimination]: super::common_subtree_elimination

use crate::{
    compiler::mir::{Body, Expression, Id, Mir},
    utils::IdGenerator,
};
use std::collections::HashSet;

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

        self.body.insert_at_front(constants);
    }

    fn remove_constants(
        body: &mut Body,
        constant_ids: &HashSet<Id>,
        id_generator: &mut IdGenerator<Id>,
    ) {
        let return_value = body.return_value();
        body.remove_all(|id, _| constant_ids.contains(&id));

        if body.iter().map(|(id, _)| id).last() != Some(return_value) {
            // The return value was removed. Add a reference to the lifted
            // constant.
            body.push(id_generator.generate(), Expression::Reference(return_value));
        }
    }
}
