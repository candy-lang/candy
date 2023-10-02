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

        // Leading dups
        let mut reference_count_adjustments = self.get_combined_reference_count_adjustments();
        for id in self
            .captured_ids()
            .chain(self.parameter_ids())
            .chain([self.responsible_parameter_id()])
        {
            new_body.maybe_dup(&mut reference_count_adjustments, id, &id_mapping);
        }

        // Determine returned expression
        let return_expression_id =
            if let Expression::Reference(id) = self.expressions().last().unwrap()
                && *id == self.ids_and_expressions().rev()
                    .skip(1)
                    .find(|(_, expression)| !matches!(expression, Expression::Dup {..} | Expression::Drop(_)))
                    .unwrap()
                    .0 {
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
            new_body.maybe_dup(&mut reference_count_adjustments, old_id, &id_mapping);
        }

        // All drops
        for (old_id, amount) in reference_count_adjustments
            .into_iter()
            .sorted_by_key(|(id, _)| *id)
        {
            let id = self.get_new_id(&id_mapping, old_id);
            match amount {
                -1 => {
                    new_body.push(Expression::Drop(id));
                }
                0 => {}
                _ => panic!("Unexpected reference count adjustment for {id}: {amount}"),
            }
        }

        // Returned expression
        let mut new_expression = self.expression(return_expression_id).unwrap().clone();
        new_expression.replace_ids(|id| self.get_new_id(&id_mapping, id));
        new_body.push(new_expression);

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
                id: self.get_new_id(id_mapping, old_id),
                amount: (*entry.get()).try_into().unwrap(),
            });
            entry.remove();
        }
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
