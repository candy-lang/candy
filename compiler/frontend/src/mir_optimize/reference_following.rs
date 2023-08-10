//! Reference following avoids reference expressions by replacing their usages
//! with original referenced value.
//!
//! Here's a before-and-after example:
//!
//! ```mir
//! $0 = Foo               |  $0 = Foo
//! $1 = $0                |  $1 = $0
//! $2 = call ... with $1  |  $2 = call ... with $0
//! ```
//!
//! This is useful for [constant folding], which tests for specific expression
//! types. For example, to constant-fold a `builtinIntAdd`, it tests whether
//! both arguments are an `Expression::Int`. An `Expression::Reference` prevents
//! that optimization.
//!
//! [constant folding]: super::constant_folding

use super::current_expression::{Context, CurrentExpression};
use crate::mir::{Body, Expression};

pub fn follow_references(context: &mut Context, expression: &mut CurrentExpression) {
    expression.replace_id_references(&mut |id| {
        if context.visible.contains(*id) && let Expression::Reference(referenced) = context.visible.get(*id) {
            *id = *referenced;
        }
    });
}

pub fn remove_redundant_return_references(body: &mut Body) {
    while let [.., (second_last_id, _), (_, Expression::Reference(referenced))] = &body.expressions[..]
        && referenced == second_last_id {
        body.expressions.pop();
    }
}
