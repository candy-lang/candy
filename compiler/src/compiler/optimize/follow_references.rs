use crate::compiler::mir::{Expression, Id, Mir};
use std::collections::HashMap;
use tracing::{debug, warn};

impl Mir {
    pub fn follow_references(&mut self) {
        let mut replacements = HashMap::<Id, Id>::new();
        Self::follow_inner_references(&mut self.expressions, &self.body, &mut replacements);
    }

    fn follow_inner_references(
        expressions: &mut HashMap<Id, Expression>,
        body: &[Id],
        replacements: &mut HashMap<Id, Id>,
    ) {
        for id in body {
            id.replace_id_references(expressions, &mut |id| {
                if let Some(replacement) = replacements.get(id) {
                    debug!("Replacing reference to {id} with {replacement}.");
                    *id = *replacement;
                }
            });
            match expressions.get(&id).unwrap().clone() {
                Expression::Reference(reference) => {
                    let resolved = replacements.get(&reference).cloned().unwrap_or(reference);
                    replacements.insert(*id, resolved);
                }
                Expression::Lambda { body, .. } => {
                    Self::follow_inner_references(expressions, &body, replacements);
                }
                _ => {}
            }
        }
    }
}
