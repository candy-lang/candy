//! Tree shaking removes unused pure expressions.
//!
//! Here's a before-and-after example:
//!
//! ```mir
//! $0 = 4                 |  $0 = 4
//! $1 = Foo               |
//! $2 = $0                |
//! $3 = call ... with $0  |  $3 = call ... with $0
//! ```
//!
//! This is useful because other optimization passes such as [constant folding]
//! cause some expressions to be no longer needed.
//!
//! [constant folding]: super::constant_folding

use crate::compiler::mir::{Body, Expression, Id, Mir};
use itertools::Itertools;
use rustc_hash::FxHashSet;

impl Mir {
    pub fn tree_shake(&mut self) {
        self.body.tree_shake(&mut FxHashSet::default());
    }
}
impl Body {
    fn tree_shake(&mut self, keep: &mut FxHashSet<Id>) {
        let body = self.iter_mut().collect_vec();
        let mut ids_to_remove = vec![];

        let return_value_id = body.last().unwrap().0;
        keep.insert(return_value_id);

        for (id, expression) in body.into_iter().rev() {
            if !expression.is_pure() || keep.contains(&id) {
                keep.insert(id);
                keep.extend(expression.referenced_ids());

                if let Expression::Lambda { body, .. } = expression {
                    body.tree_shake(keep);
                }
            } else {
                ids_to_remove.push(id);
            }
        }

        self.remove_all(|id, _| ids_to_remove.contains(&id));
    }
}
