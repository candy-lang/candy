use crate::compiler::hir::{Body, Expression, Id, Lambda};
use std::collections::{HashMap, HashSet};
use tracing::warn;

impl Body {
    pub fn inline_call(&mut self, call: &Id) -> Result<(), &'static str> {
        let Expression::Call {
            function,
            arguments,
        } = self.find(&call).unwrap() else {
            return Err("Called `inline_call`, but the ID doesn't refer to a call.");
        };

        let Expression::Lambda(Lambda { parameters, body, fuzzable }) = self.find(function).unwrap() else {
            return Err("Tried to inline, but the call's receiver is not a lambda.");
        };

        if arguments.len() != parameters.len() {
            return Err("Number of arguments doesn't match expected parameter count.");
        }

        // TODO: check that the lambda doesn't capture local stuff

        let mut body = body.clone();

        let parameters_to_arguments: HashMap<Id, Id> = parameters
            .iter()
            .zip(arguments.iter())
            .map(|(parameter, argument)| (parameter.clone(), argument.clone()))
            .collect();
        // warn!("Parameters to arguments: {parameters_to_arguments:?}");
        body.replace_ids(&mut |id| {
            if let Some(argument) = parameters_to_arguments.get(id) {
                *id = argument.clone();
            }
            if function.is_same_module_and_any_parent_of(id) {
                let mut internal_id = call.clone();
                internal_id
                    .keys
                    .extend(id.keys.iter().skip(function.keys.len()).cloned());
                *id = internal_id;
            }
        });

        let index = self.ids.iter().position(|it| it == call).unwrap();
        for (i, id) in body.ids.iter().enumerate() {
            self.ids.insert(index + i, id.clone());
            self.expressions
                .insert(id.clone(), body.expressions.get(id).unwrap().clone());
        }
        *self.expressions.get_mut(call).unwrap() = Expression::Reference(body.return_value());

        Ok(())
    }

    pub fn inline_functions_containing_use(&mut self) {
        let mut functions_with_use = HashSet::new();
        for (id, expression) in &self.expressions {
            if let Expression::Lambda(lambda) = expression &&
                lambda.body.expressions.values().any(|expr| matches!(expr, Expression::UseModule { .. })) {
                functions_with_use.insert(id.clone());
            }
        }

        for id in self.ids.clone() {
            if let Expression::Call { function, .. } = self.expressions.get(&id).unwrap() && functions_with_use.contains(function) {
                self.inline_call(&id);
            }
        }
    }
}
