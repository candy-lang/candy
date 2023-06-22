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
//! TODO: Have a separate constant heap directly in the LIR, so that
//! instructions such as `Instruction::CreateInt` are never actually executed at
//! runtime.
//!
//! [common subtree elimination]: super::common_subtree_elimination

use super::current_expression::{Context, CurrentExpression};
use crate::mir::Expression;

pub fn lift_constants(context: &mut Context, expression: &mut CurrentExpression) {
    let Expression::Function { body, .. } = &mut **expression else { return; };

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

        if is_return_value {
            // The return value was removed. Add a reference to the lifted
            // constant.
            body.push(context.id_generator.generate(), Expression::Reference(id));
        }
    }

    expression.prepend_optimized(context.visible, constants);
}
