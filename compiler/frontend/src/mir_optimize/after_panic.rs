//! Removes expressions after a panic in a body. They can never be reached.

use super::pure::PurenessInsights;
use crate::mir::Body;

pub fn remove_expressions_after_panic(body: &mut Body, pureness: &mut PurenessInsights) {
    let Some(panic_index) = body
        .expressions
        .iter()
        .position(|(_, expression)| expression.is_panic())
    else {
        return;
    };

    for (removed, expression) in body.expressions.drain((panic_index + 1)..) {
        pureness.on_remove(removed);
        for id in expression.defined_ids() {
            pureness.on_remove(id);
        }
    }
}
