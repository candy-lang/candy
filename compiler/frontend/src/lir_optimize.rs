use crate::{
    lir::{Bodies, Body, Expression, Id, Lir},
    mir_to_lir::{LirResult, MirToLir},
    module::Module,
    utils::HashMapExtension,
    TracingConfig,
};
use itertools::Itertools;
use rustc_hash::FxHashMap;
use std::{collections::hash_map::Entry, sync::Arc};

#[salsa::query_group(OptimizeLirStorage)]
pub trait OptimizeLir: MirToLir {
    fn optimized_lir(&self, module: Module, tracing: TracingConfig) -> LirResult;
}

#[allow(clippy::needless_pass_by_value)]
fn optimized_lir(db: &dyn OptimizeLir, module: Module, tracing: TracingConfig) -> LirResult {
    let (lir, errors) = db.lir(module, tracing)?;

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

        let mut reference_count_adjustments = self.get_combined_reference_count_adjustments();
        for id in self
            .captured_ids()
            .chain(self.parameter_ids())
            .chain([self.responsible_parameter_id()])
        {
            new_body.maybe_dup(&mut reference_count_adjustments, id, &id_mapping);
        }

        for (old_id, old_expression) in self.ids_and_expressions() {
            if matches!(old_expression, Expression::Dup { .. } | Expression::Drop(_)) {
                continue;
            }

            let mut new_expression = old_expression.clone();
            new_expression.replace_ids(|id| id_mapping.get(&id).copied().unwrap_or(id));
            let id = new_body.push(new_expression);
            id_mapping.force_insert(old_id, id);
            new_body.maybe_dup(&mut reference_count_adjustments, old_id, &id_mapping);
        }

        for (id, amount) in reference_count_adjustments
            .into_iter()
            .sorted_by_key(|(id, _)| *id)
        {
            match amount {
                -1 => {
                    new_body.push(Expression::Drop(id));
                }
                0 => {}
                _ => panic!("Unexpected reference count adjustment for {id}: {amount}"),
            }
        }

        new_body
    }
    fn maybe_dup(
        &mut self,
        reference_count_adjustments: &mut FxHashMap<Id, isize>,
        old_id: Id,
        id_mapping: &FxHashMap<Id, Id>,
    ) {
        if let Entry::Occupied(entry) = reference_count_adjustments.entry(old_id)
            && *entry.get() > 0 {
            self.push(Expression::Dup {
                id: id_mapping.get(&old_id).copied().unwrap_or(old_id),
                amount: (*entry.get()).try_into().unwrap(),
            });
            entry.remove();
        }
    }

    /// The sum of dups minus drops per Id.
    fn get_combined_reference_count_adjustments(&self) -> FxHashMap<Id, isize> {
        let mut adjustments: FxHashMap<Id, isize> = FxHashMap::default();
        for expression in self.expressions() {
            match expression {
                Expression::Dup { id, amount } => {
                    *adjustments.entry(*id).or_default() += isize::try_from(*amount).unwrap();
                }
                Expression::Drop(id) => *adjustments.entry(*id).or_default() -= 1,
                _ => {}
            }
        }
        adjustments
    }
}
