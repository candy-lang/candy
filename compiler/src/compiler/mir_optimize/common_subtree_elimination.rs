//! Common subtree elimination deduplicates pure expressions that yield the same
//! value.
//!
//! Here's a before-and-after example:
//!
//! ```mir
//! $0 = builtinIntAdd       |  $0 = builtinIntAdd
//! $1 = 2                   |  $1 = 2
//! $2 = 2                   |  $2 = $1
//! $3 = call $0 with $1 $2  |  $3 = call $0 with $1 $2
//! ```
//!
//! This is especially effective after [constant lifting] because lots of
//! constants are in the same scope. This optimization is also a necessity to
//! avoid exponential code blowup when importing modules – after
//! [module folding], a lot of duplicate functions exist.
//!
//! [constant lifting]: super::constant_lifting
//! [module folding]: super::module_folding

use rustc_hash::FxHashMap;

use crate::{
    compiler::mir::{Expression, Id, Mir},
    utils::{CountableId, IdGenerator},
};
use std::collections::hash_map::Entry;

impl Mir {
    pub fn eliminate_common_subtrees(&mut self) {
        let mut pure_expressions = FxHashMap::default();

        self.body
            .visit_with_visible(&mut |id, expression, visible, _| {
                if !expression.is_pure() {
                    return;
                }

                let mut normalized = expression.clone();
                normalized.normalize();

                let existing_entry = pure_expressions.entry(normalized);
                match existing_entry {
                    Entry::Occupied(id_of_same_expression)
                        if visible.contains(*id_of_same_expression.get()) =>
                    {
                        *expression = Expression::Reference(*id_of_same_expression.get());
                    }
                    _ => {
                        existing_entry.insert_entry(id);
                    }
                }
            });
    }
}

impl Expression {
    /// Two lambdas where local expressions have different IDs are usually not
    /// considered equal. This method normalizes expressions by replacing all
    /// locally defined IDs.
    fn normalize(&mut self) {
        let mut generator = IdGenerator::start_at(
            self.captured_ids()
                .into_iter()
                .max()
                .map(|id| id.to_usize() + 1)
                .unwrap_or(0),
        );
        let mapping: FxHashMap<Id, Id> = self
            .defined_ids()
            .into_iter()
            .map(|id| (id, generator.generate()))
            .collect();

        self.replace_ids(&mut |id| {
            if let Some(replacement) = mapping.get(id) {
                *id = *replacement;
            }
        })
    }
}
