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
//! types. For example, to constant-fold a `builtinIntAdd', it tests whether
//! both arguments are an `Expression::Int`. An `Expression::Reference` prevents
//! that optimization.
//!
//! [constant folding]: super::constant_folding

use crate::mir::{Body, Expression, VisibleExpressions};

pub fn follow_references(expression: &mut Expression, visible: &VisibleExpressions) {
    expression.replace_id_references(&mut |id| {
        if visible.contains(*id) && let Expression::Reference(referenced) = visible.get(*id) {
            *id = *referenced;
        }
    });
}

pub fn remove_redundant_return_references(body: &mut Body) {
    while body.expressions.len() > 1 {
        let mut from_back = body.iter_mut().rev();
        let (_, last_expression) = from_back.next().unwrap();
        let (second_last_id, _) = from_back.next().unwrap();

        if let Expression::Reference(referenced) = last_expression && *referenced == second_last_id {
            drop(from_back);
            body.expressions.pop();
        } else {
            break;
        }
    }
}
