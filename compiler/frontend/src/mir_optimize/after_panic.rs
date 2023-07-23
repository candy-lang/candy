//! Removes expressions after a panic in a body. They can never be reached.

use crate::mir::{Body, Expression};

pub fn remove_expressions_after_panic(body: &mut Body) {
    let Some(panic_index) = body
        .expressions
        .iter()
        .position(|(_, expression)| matches!(expression, Expression::Panic { .. }))
    else {
        return;
    };

    body.expressions.drain((panic_index + 1)..);
}
