//! Module stack collapsing removes `ModuleStarts` and `ModuleEnds` expressions
//! without a `Use` in between. Those are guaranteed not to cause cycles:
//! Surrounding expressions for the same module can only be created by the same
//! salsa query, so the import cycle would have been detected right there.
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

impl Mir {
    pub fn cancel_out_module_expressions(&mut self) {
        self.body.visit_bodies(&mut |body| {
            let mut indices_of_expressions_to_eliminate = vec![];

            for ((a_index, (_, a)), (b_index, (_, b))) in body.iter().enumerate().tuple_windows() {
                if matches!(a, Expression::ModuleStarts { .. })
                    && matches!(b, Expression::ModuleEnds)
                {
                    indices_of_expressions_to_eliminate.push(a_index);
                    indices_of_expressions_to_eliminate.push(b_index);
                }
            }

            for (index, (_, expr)) in body.iter_mut().enumerate() {
                if indices_of_expressions_to_eliminate.contains(&index) {
                    *expr = Expression::Symbol("Nothing".to_string());
                }
            }
        });
    }
}
