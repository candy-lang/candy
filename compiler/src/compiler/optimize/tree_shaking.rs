use crate::{
    builtin_functions::BuiltinFunction,
    compiler::hir::{Body, Expression, Id, Lambda},
};
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use tracing::warn;

impl Body {
    pub fn tree_shake(&mut self) {
        let mut still_needed = HashSet::new();
        still_needed.insert(self.return_value());

        // TODO: Do this without cloning.
        for id in self.ids.clone().into_iter().rev() {
            // warn!(
            //     "Used: {}",
            //     still_needed.iter().map(|id| format!("{id}")).join(", ")
            // );

            if still_needed.contains(&id) {
                let mut child_ids = vec![];
                self.expressions
                    .get(&id)
                    .unwrap()
                    .collect_all_ids(&mut child_ids);
                still_needed.extend(child_ids.into_iter());

                if let Expression::Lambda(Lambda { body, .. }) =
                    self.expressions.get_mut(&id).unwrap()
                {
                    body.tree_shake();
                }
            } else {
                warn!("{id} will be optmized away");
            }
        }

        let used = self
            .ids
            .iter()
            .filter(|id| still_needed.contains(id))
            .cloned()
            .collect_vec();
        self.ids = used;
    }
}
