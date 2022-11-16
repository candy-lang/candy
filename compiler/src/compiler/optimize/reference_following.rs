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

use crate::compiler::mir::{Expression, Id, Mir};
use std::collections::HashMap;

impl Mir {
    pub fn follow_references(&mut self) {
        let mut replacements = HashMap::<Id, Id>::new();

        self.body.visit(&mut |id, expression, _| {
            if let Expression::Reference(reference) = &expression {
                let replacement = *replacements.get(reference).unwrap_or(reference);
                replacements.insert(id, replacement);
            }
        });
        self.body.visit(&mut |_, expression, _| {
            expression.replace_id_references(&mut |id| {
                if let Some(&replacement) = replacements.get(id) {
                    *id = replacement;
                }
            });
        });
    }
}
