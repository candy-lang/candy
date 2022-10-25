use crate::compiler::mir::{Expression, Id, Mir};
use std::collections::{HashMap, HashSet};
use tracing::debug;

impl Mir {
    pub fn tree_shake(&mut self) {
        let mut keep = HashSet::new();
        Self::tree_shake_body(&mut keep, &mut self.expressions, &mut self.body);
    }
    fn tree_shake_body(
        keep: &mut HashSet<Id>,
        expressions: &mut HashMap<Id, Expression>,
        body: &mut Vec<Id>,
    ) {
        // The return value is always needed.
        keep.insert(*body.last().unwrap());

        for id in body.iter().rev() {
            // Definitely keep expressions that are impure or where we don't
            // know if they are pure.
            if !expressions.get(id).unwrap().is_pure() {
                keep.insert(id.clone());
            }

            // A later expression depends on this one.
            if keep.contains(&id) {
                keep.extend(id.referenced_ids(expressions));

                let mut temporary = id.temporarily_get_mut(expressions);
                if let Expression::Lambda { body, .. } = &mut temporary.expression {
                    Self::tree_shake_body(keep, temporary.remaining, body);
                }
            } else {
                debug!("Removing {id} because it's pure but unused.");
            }
        }

        body.retain(|id| keep.contains(id));
    }
}

impl Expression {
    fn is_pure(&self) -> bool {
        match self {
            Expression::Int(_) => true,
            Expression::Text(_) => true,
            Expression::Reference(_) => true,
            Expression::Symbol(_) => true,
            Expression::Struct(_) => true,
            Expression::Lambda { .. } => true,
            Expression::Builtin(_) => true,
            Expression::Responsibility(_) => true,
            Expression::Call { .. } => false,
            Expression::UseModule { .. } => false,
            Expression::Needs { .. } => false,
            Expression::Panic { .. } => false,
            Expression::Error { .. } => false,
        }
    }
}
