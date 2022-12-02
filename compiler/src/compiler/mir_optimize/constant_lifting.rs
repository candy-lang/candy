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
        // Expressions in the top level should not be lifted as that would just
        // mean moving some constants and then creating references to them in
        // the original places.
        let top_level_ids = self.body.iter().map(|(id, _)| id).collect::<HashSet<_>>();

        let mut constants = vec![];
        let mut constant_ids = HashSet::new();

        self.body.visit(&mut |id, expression, is_return_value| {
            if top_level_ids.contains(&id) {
                return;
            }
            let is_constant = expression.is_pure()
                && expression
                    .captured_ids()
                    .iter()
                    .all(|captured| constant_ids.contains(captured));
            if !is_constant {
                return;
            }
            if is_return_value && let Expression::Reference(_) = expression {
                // Returned references shouldn't be lifted. If we would lift
                // one, we'd have to add a reference anyway.
                return;
            }
            constants.push((id, expression.clone()));
            constant_ids.insert(id);
        });

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

        if constant_ids.contains(&return_value) {
            // The return value was removed. Add a reference to the lifted
            // constant.
            body.push(id_generator.generate(), Expression::Reference(return_value));
        }
    }
}
