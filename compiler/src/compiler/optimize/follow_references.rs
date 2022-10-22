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

impl Expression {
    fn replace_ids<F: FnMut(&mut Id)>(&mut self, replacer: &mut F) {
        match self {
            Expression::Int(_)
            | Expression::Text(_)
            | Expression::Symbol(_)
            | Expression::Builtin(_) => {}
            Expression::Reference(reference) => replacer(reference),
            Expression::Struct(fields) => {
                *fields = fields
                    .iter()
                    .map(|(key, value)| {
                        let mut key = key.clone();
                        let mut value = value.clone();
                        replacer(&mut key);
                        replacer(&mut value);
                        (key, value)
                    })
                    .collect();
            }
            Expression::Lambda(Lambda { body, .. }) => {
                for id in &body.ids {
                    let expression = body.expressions.get_mut(&id).unwrap();
                    expression.replace_ids::<F>(replacer);
                }
            }
            Expression::Call {
                function,
                arguments,
            } => {
                replacer(function);
                for argument in arguments {
                    replacer(argument);
                }
            }
            Expression::UseModule { relative_path, .. } => replacer(relative_path),
            Expression::Needs { condition, reason } => {
                replacer(condition);
                replacer(reason);
            }
            Expression::Error { child, errors } => {
                if let Some(child) = child {
                    replacer(child);
                }
            }
        }
    }
}
