use crate::compiler::mir::{Body, Expression, Id, Mir};
use itertools::Itertools;
use std::collections::HashSet;
use tracing::debug;

impl Mir {
    pub fn tree_shake(&mut self) {
        self.body.tree_shake(&mut HashSet::new());
    }
}
impl Body {
    fn tree_shake(&mut self, keep: &mut HashSet<Id>) {
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
                debug!("Removing {id} because it's pure but unused.");
                ids_to_remove.push(id);
            }
        }

        for id in ids_to_remove {
            self.remove(id)
        }
    }
}
