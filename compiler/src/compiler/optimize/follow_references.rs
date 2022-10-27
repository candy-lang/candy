use crate::compiler::mir::{Expression, Id, Mir};
use std::collections::HashMap;
use tracing::debug;

impl Mir {
    pub fn follow_references(&mut self) {
        let mut replacements = HashMap::<Id, Id>::new();

        self.body.visit(&mut |_, id, expression| {
            expression.replace_id_references(&mut |id| {
                if let Some(&replacement) = replacements.get(id) {
                    debug!("Replacing reference to {id} with {replacement}.");
                    *id = replacement;
                }
            });
            if let Expression::Reference(reference) = &expression {
                replacements.insert(id, *reference);
            }
        });
    }
}
