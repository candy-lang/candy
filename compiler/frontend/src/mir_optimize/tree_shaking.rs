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

use super::pure::PurenessInsights;
use crate::mir::Body;
use itertools::Itertools;
use rustc_hash::FxHashSet;

pub fn tree_shake(body: &mut Body, pureness: &mut PurenessInsights) {
    let expressions = body.iter().collect_vec();
    let mut keep = FxHashSet::default();
    let mut ids_to_remove = FxHashSet::default();

    let return_value_id = expressions.last().unwrap().0;
    keep.insert(return_value_id);

    for (id, expression) in expressions.into_iter().rev() {
        if keep.remove(&id) || !pureness.is_definition_pure(expression) {
            keep.extend(expression.referenced_ids());
        } else {
            ids_to_remove.insert(id);
        }
    }

    for (id, expression) in body.remove_all(|id, _| ids_to_remove.contains(&id)) {
        pureness.on_remove(id);
        for id in expression.defined_ids() {
            pureness.on_remove(id);
        }
    }
}
