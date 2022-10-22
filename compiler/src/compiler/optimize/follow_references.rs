use crate::{
    builtin_functions::BuiltinFunction,
    compiler::hir::{Body, Expression, Id, Lambda},
};
use std::collections::HashMap;
use tracing::warn;

impl Body {
    pub fn follow_references(&mut self) {
        let mut replacements = HashMap::<Id, Id>::new();

        for id in self.ids.clone() {
            if let Expression::Reference(reference) = self.expressions.get(&id).unwrap() {
                let resolved = replacements
                    .get(reference)
                    .cloned()
                    .unwrap_or_else(|| reference.clone());
                replacements.insert(id, resolved);
            } else {
                let expression = self.expressions.get_mut(&id).unwrap();
                expression.replace_ids(&mut |id| {
                    if let Some(replacement) = replacements.get(id) {
                        *id = replacement.clone();
                    }
                });
                if let Expression::Lambda(lambda) = expression {
                    lambda.body.follow_references();
                }
            }
        }
    }
}
