//! Module stack collapsing removes `ModuleStarts` and `ModuleEnds` expressions
//! without a `Use` in between. Those are guaranteed not to cause cycles: Nested
//! expressions for the same module can only be created by the same salsa query,
//! so the import cycle would have been detected right there.
//!
//! Here's a before-and-after example of module expressions cancelling out:
//!
//! ```mir
//! # before:
//! $0 = moduleStarts "some module"
//! $1 = moduleEnds
//!
//! # after:
//! ```

use crate::compiler::mir::{Expression, Mir};
use itertools::Itertools;
use rustc_hash::FxHashSet;

impl Mir {
    pub fn cancel_out_module_expressions(&mut self) {
        self.body.visit_bodies(&mut |body| {
            let mut indices_of_expressions_to_eliminate = FxHashSet::default();

            for ((a_index, (_, a)), (b_index, (_, b))) in body.iter().enumerate().tuple_windows() {
                if matches!(a, Expression::ModuleStarts { .. })
                    && matches!(b, Expression::ModuleEnds)
                {
                    indices_of_expressions_to_eliminate.insert(a_index);
                    indices_of_expressions_to_eliminate.insert(b_index);
                }
            }

            for (index, (_, expr)) in body.iter_mut().enumerate() {
                if indices_of_expressions_to_eliminate.contains(&index) {
                    *expr = Expression::nothing();
                }
            }
        });
    }

    pub fn remove_all_module_expressions_if_no_use_exists(&mut self) {
        let mut contains_use = false;
        self.body.visit(&mut |_, expression, _| {
            if matches!(expression, Expression::UseModule { .. }) {
                contains_use = true;
            }
        });

        if !contains_use {
            self.body.visit(&mut |_, expression, _| {
                if matches!(
                    expression,
                    Expression::ModuleStarts { .. } | Expression::ModuleEnds,
                ) {
                    *expression = Expression::nothing();
                }
            });
        }
    }
}
