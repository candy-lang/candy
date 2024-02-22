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
use itertools::Itertools;

const NAME: &str = "Constant Lifting";

pub fn lift_constants(context: &mut Context, expression: &mut CurrentExpression) {
    let Expression::Function { body, .. } = expression.get_mut_carefully() else {
        return;
    };

    let return_value = body.return_value();
    let mut new_return_reference_target = None;
    let constants = body
        .expressions
        .extract_if(|(id, expression)| {
            let id = *id;

            if !context.pureness.is_definition_const(expression) {
                return false;
            }

            let is_return_value = id == return_value;
            if is_return_value && let Expression::Reference(_) = expression {
                // Returned references shouldn't be lifted. If we would lift one,
                // we'd have to add a reference anyway.
                return false;
            }

            // This is a constant and should be lifted.

            if is_return_value {
                // The return value was removed. Add a reference to the lifted
                // constant.
                new_return_reference_target = Some(id);
            }
            true
        })
        .collect_vec();

    if let Some(new_return_reference_target) = new_return_reference_target {
        body.push(
            context.id_generator.generate(),
            Expression::Reference(new_return_reference_target),
        );
    }

    expression.prepend_optimized(NAME, context.visible, constants);
}
