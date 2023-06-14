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

use super::pure::PurenessInsights;
use crate::{
    hir,
    id::{CountableId, IdGenerator},
    mir::{Body, Expression, Id, VisitorResult},
};
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::hash_map::Entry;

pub fn eliminate_common_subtrees(body: &mut Body, pureness: &PurenessInsights) {
    let mut pure_expressions = FxHashMap::default();
    let mut inner_functions: FxHashMap<Id, Vec<Id>> = FxHashMap::default();
    let mut additional_function_hirs: FxHashMap<Id, FxHashSet<hir::Id>> = FxHashMap::default();
    let mut updated_references: FxHashMap<Id, Id> = FxHashMap::default();

    for (id, expression) in &mut body.expressions {
        expression.replace_id_references(&mut |id| {
            if let Some(update) = updated_references.get(id) {
                *id = *update;
            }
        });

        if !pureness.is_definition_pure(expression) {
            continue;
        }

        let mut normalized = expression.clone();
        normalized.normalize();

        if let Expression::Function { body, .. } = expression {
            inner_functions.insert(
                *id,
                body.all_functions().into_iter().map(|(id, _)| id).collect(),
            );
        }

        let existing_entry = pure_expressions.entry(normalized);
        match existing_entry {
            Entry::Occupied(canonical_id) => {
                let canonical_id = *canonical_id.get();
                let old_expression =
                    std::mem::replace(expression, Expression::Reference(canonical_id));
                updated_references.insert(*id, canonical_id);

                if let Expression::Function {
                    mut body,
                    original_hirs,
                    ..
                } = old_expression
                {
                    additional_function_hirs
                        .entry(canonical_id)
                        .or_default()
                        .extend(original_hirs);

                    let canonical_child_functions = inner_functions.get(&canonical_id).unwrap();
                    for ((_, child_hirs), canonical_child_id) in body
                        .all_functions()
                        .into_iter()
                        .zip(canonical_child_functions)
                    {
                        additional_function_hirs
                            .entry(*canonical_child_id)
                            .or_default()
                            .extend(child_hirs);
                    }
                }
            }
            _ => {
                existing_entry.insert_entry(*id);
            }
        }
    }
}

impl Expression {
    /// Two functions where local expressions have different IDs are usually not
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
        if let Expression::Function {
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
    fn all_functions(&mut self) -> Vec<(Id, FxHashSet<hir::Id>)> {
        let mut ids_and_expressions = vec![];
        self.visit(&mut |id, expression, _| {
            if let Expression::Function { original_hirs, .. } = expression {
                ids_and_expressions.push((id, original_hirs.clone()));
            }
            VisitorResult::Continue
        });
        ids_and_expressions
    }
}
