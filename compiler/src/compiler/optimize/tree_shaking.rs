use crate::compiler::hir::{Body, Expression, Lambda};
use itertools::Itertools;
use std::collections::HashSet;
use tracing::warn;

impl Body {
    pub fn tree_shake(&mut self) {
        let mut keep = HashSet::new();
        keep.insert(self.return_value());

        // TODO: Do this without cloning.
        for id in self.ids.clone().into_iter().rev() {
            // warn!(
            //     "Tree shaking keep list: {}",
            //     keep.iter().map(|id| format!("{id}")).join(", ")
            // );

            if self.expressions.get(&id).unwrap().is_pure() != Some(true) {
                keep.insert(id.clone());
            }

            if keep.contains(&id) {
                let mut child_ids = vec![];
                self.expressions
                    .get(&id)
                    .unwrap()
                    .collect_all_ids(&mut child_ids);
                if format!("{child_ids:?}").contains("✨") {
                    warn!("Sparkles were added to the keep list because {id} needs them.");
                }
                keep.extend(child_ids.into_iter());

                if let Expression::Lambda(lambda) = self.expressions.get_mut(&id).unwrap() {
                    lambda.body.tree_shake();
                }
            } else {
                warn!("{id} will be optmized away");
            }
        }

        self.ids.retain(|id| keep.contains(id));
        self.expressions.retain(|id, _| keep.contains(id));
        self.identifiers.retain(|id, _| keep.contains(id));
    }
}

impl Expression {
    fn is_pure(&self) -> Option<bool> {
        match self {
            Expression::Int(_) => Some(true),
            Expression::Text(_) => Some(true),
            Expression::Reference(_) => Some(true),
            Expression::Symbol(_) => Some(true),
            Expression::Struct(_) => Some(true),
            Expression::Lambda(_) => Some(true),
            Expression::Builtin(_) => Some(true),
            Expression::Call {
                function,
                arguments,
            } => None,
            Expression::UseModule {
                current_module,
                relative_path,
            } => Some(false),
            Expression::Needs { condition, reason } => Some(false),
            Expression::Error { child, errors } => Some(false),
        }
    }
}
