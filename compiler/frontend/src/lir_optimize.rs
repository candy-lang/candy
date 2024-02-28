use crate::{
    hir_to_mir::ExecutionTarget,
    lir::{Bodies, Body, Expression, Id, Lir},
    mir_to_lir::{LirResult, MirToLir},
    utils::{HashMapExtension, HashSetExtension},
    TracingConfig,
};
use itertools::Itertools;
use rustc_hash::{FxHashMap, FxHashSet};
use std::{collections::hash_map::Entry, sync::Arc};

#[salsa::query_group(OptimizeLirStorage)]
pub trait OptimizeLir: MirToLir {
    fn optimized_lir(&self, target: ExecutionTarget, tracing: TracingConfig) -> LirResult;
}

#[allow(clippy::needless_pass_by_value)]
fn optimized_lir(
    db: &dyn OptimizeLir,
    target: ExecutionTarget,
    tracing: TracingConfig,
) -> LirResult {
    let (lir, errors) = db.lir(target, tracing)?;

    let mut bodies = Bodies::default();
    for (id, body) in lir.bodies().ids_and_bodies() {
        let new_id = bodies.push(body.optimize());
        assert_eq!(id, new_id);
    }

    let optimized_lir = Lir::new(lir.constants().clone(), bodies);
    Ok((Arc::new(optimized_lir), errors))
}

impl Body {
    fn optimize(&self) -> Self {
        let mut new_body = Self::new(
            self.original_hirs().clone(),
            self.captured_count(),
            self.parameter_count(),
        );
        let mut id_mapping = FxHashMap::default();

        // Leading dups
        let (mut to_dup, to_drop) = self.get_combined_reference_count_adjustments();
        for id in self
            .captured_ids()
            .chain(self.parameter_ids())
            .chain([self.responsible_parameter_id()])
        {
            new_body.maybe_dup(&mut to_dup, id, &id_mapping);
        }

        // Determine the returned expression. We'll insert it after all the
        // drops to avoid having to create a reference to it after those drops.
        let return_expression_id = if let Expression::Reference(id) =
            self.expressions().last().unwrap()
            && self
                .ids_and_expressions()
                .rev()
                .skip(1)
                .find(|(_, expression)| {
                    !matches!(expression, Expression::Dup { .. } | Expression::Drop(_))
                })
                .map(|(id, _)| id)
                == Some(*id)
        {
            // The last expression is a reference to the last expression
            // before all drops. We can remove it because we move all drops
            // before that last expression.
            *id
        } else {
            self.last_expression_id().unwrap()
        };

        // All expressions except the returned one
        for (old_id, old_expression) in self.ids_and_expressions() {
            if matches!(old_expression, Expression::Dup { .. } | Expression::Drop(_)) {
                continue;
            }
            if old_id == return_expression_id {
                // After the expression whose value is returned, the can only be
                // drop expressions and maybe a reference to this return value.
                break;
            }

            let mut new_expression = old_expression.clone();
            new_expression.replace_ids(|id| self.get_new_id(&id_mapping, id));
            let id = new_body.push(new_expression);
            id_mapping.force_insert(old_id, id);
            new_body.maybe_dup(&mut to_dup, old_id, &id_mapping);
        }
        assert!(to_dup.is_empty());

        // All drops
        for old_id in to_drop.into_iter().sorted() {
            new_body.push(Expression::Drop(self.get_new_id(&id_mapping, old_id)));
        }

        // Returned expression
        let mut new_expression = self.expression(return_expression_id).unwrap().clone();
        new_expression.replace_ids(|id| self.get_new_id(&id_mapping, id));
        new_body.push(new_expression);

        new_body
    }
    fn maybe_dup(
        &mut self,
        to_dup: &mut FxHashMap<Id, usize>,
        old_id: Id,
        id_mapping: &FxHashMap<Id, Id>,
    ) {
        let Some(amount) = to_dup.remove(&old_id) else {
            return;
        };
        assert!(amount > 0);
        let id = self.get_new_id(id_mapping, old_id);
        self.push(Expression::Dup { id, amount });
    }

    fn get_new_id(&self, id_mapping: &FxHashMap<Id, Id>, id: Id) -> Id {
        if id <= self.responsible_parameter_id() {
            // Captured variables, parameters, and the responsible parameter
            // keep their ID.
            id
        } else {
            id_mapping[&id]
        }
    }

    /// The sum of dups minus drops per ID.
    fn get_combined_reference_count_adjustments(&self) -> (FxHashMap<Id, usize>, FxHashSet<Id>) {
        let mut to_dup: FxHashMap<Id, usize> = FxHashMap::default();
        let mut to_drop: FxHashSet<Id> = FxHashSet::default();
        for expression in self.expressions() {
            match expression {
                Expression::Dup { id, amount } => {
                    *to_dup.entry(*id).or_default() += *amount;
                }
                Expression::Drop(id) => match to_dup.entry(*id) {
                    Entry::Occupied(mut entry) => {
                        if *entry.get() == 1 {
                            entry.remove();
                        } else {
                            *entry.get_mut() -= 1;
                        }
                    }
                    Entry::Vacant(_) => to_drop.force_insert(*id),
                },
                _ => {}
            }
        }
        (to_dup, to_drop)
    }
}
