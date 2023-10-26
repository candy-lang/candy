//! Constant lifting refers to lifting constants from functions into surrounding
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
//! [common subtree elimination]: super::common_subtree_elimination

use super::current_expression::Context;
use crate::mir::{Body, Expression, Id};

pub fn lift_constants(context: &mut Context, body: &mut Body) -> Vec<(Id, Expression)> {
    let mut constants = vec![];

    let mut index = 0;
    while index < body.expressions.len() {
        let (id, expression) = &body.expressions[index];
        let id = *id;

        if !context.pureness.is_definition_const(expression) {
            index += 1;
            continue;
        }

        let is_return_value = id == body.return_value();
        if is_return_value && let Expression::Reference(_) = expression {
            // Returned references shouldn't be lifted. If we would lift one,
            // we'd have to add a reference anyway.
            index += 1;
            continue;
        }

        // This is a constant and should be lifted.

        constants.push(body.expressions.remove(index));

        let new_reference_id = if is_return_value {
            // The return value was removed. Add a reference to the lifted
            // constant.
            let id = context.id_generator.generate();
            let expression = Expression::Reference(id);
            context.pureness.visit_optimized(id, &expression);
            body.push(id, expression);
            Some(id)
        } else {
            None
        };
        context
            .data_flow
            .on_constant_lifted(constants.last().unwrap().0, new_reference_id);
    }

    constants
}
