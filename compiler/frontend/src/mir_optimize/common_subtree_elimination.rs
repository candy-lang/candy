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

use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    hir,
    id::{CountableId, IdGenerator},
    mir::{Body, Expression, Id, Mir, VisitorResult},
};
use std::collections::hash_map::Entry;

impl Mir {
    pub fn eliminate_common_subtrees(&mut self) {
        let mut pure_expressions = FxHashMap::default();
        let mut inner_lambdas: FxHashMap<Id, Vec<Id>> = FxHashMap::default();
        let mut additional_lambda_hirs: FxHashMap<Id, FxHashSet<hir::Id>> = FxHashMap::default();

        self.body
            .visit_with_visible(&mut |id, expression, visible, _| {
                if !expression.is_pure() {
                    return;
                }

                let mut normalized = expression.clone();
                normalized.normalize();

                if let Expression::Lambda { body, .. } = expression {
                    inner_lambdas.insert(
                        id,
                        body.all_lambdas().into_iter().map(|(id, _)| id).collect(),
                    );
                }

                let existing_entry = pure_expressions.entry(normalized);
                match existing_entry {
                    Entry::Occupied(id_of_canonical_expression)
                        if visible.contains(*id_of_canonical_expression.get()) =>
                    {
                        let old_expression = std::mem::replace(
                            expression,
                            Expression::Reference(*id_of_canonical_expression.get()),
                        );
                        if let Expression::Lambda {
                            body,
                            original_hirs,
                            ..
                        } = expression
                        {
                            additional_lambda_hirs
                                .entry(*id_of_canonical_expression.get())
                                .or_default()
                                .extend(&mut original_hirs.clone().into_iter());

                            let canonical_child_lambdas =
                                inner_lambdas.get(id_of_canonical_expression.get()).unwrap();
                            for ((_, child_hirs), canonical_child_id) in
                                body.all_lambdas().iter().zip(canonical_child_lambdas)
                            {
                                additional_lambda_hirs
                                    .entry(*canonical_child_id)
                                    .or_default()
                                    .extend(child_hirs.clone());
                            }
                        }
                    }
                    _ => {
                        existing_entry.insert_entry(id);
                    }
                }
            });

        self.body.visit(&mut |id, expression, _| {
            if let Expression::Lambda { original_hirs, .. } = expression && let Some(additional_hirs) = additional_lambda_hirs.remove(&id) {
                original_hirs.extend(additional_hirs);
            }
            VisitorResult::Continue
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
        });
        self.strip_original_hirs();
    }
    fn strip_original_hirs(&mut self) {
        if let Expression::Lambda {
            original_hirs,
            body,
            ..
        } = self
        {
            original_hirs.clear();
            for (_, expression) in body.iter_mut() {
                expression.strip_original_hirs();
            }
        }
    }
}

impl Body {
    fn all_lambdas(&mut self) -> Vec<(Id, FxHashSet<hir::Id>)> {
        let mut ids_and_expressions = vec![];
        self.visit(&mut |id, expression, _| {
            if let Expression::Lambda { original_hirs, .. } = expression {
                ids_and_expressions.push((id, original_hirs.clone()));
            }
            VisitorResult::Continue
        });
        ids_and_expressions
    }
}
