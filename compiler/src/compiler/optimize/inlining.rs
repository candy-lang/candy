use crate::{
    compiler::mir::{Expression, Id, Mir},
    utils::IdGenerator,
};
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use tracing::debug;

impl Mir {
    pub fn inline_call(
        call: Id,
        surrounding_body: &mut Vec<Id>,
        id_generator: &mut IdGenerator<Id>,
        expressions: &mut HashMap<Id, Expression>,
    ) -> Result<(), &'static str> {
        let Expression::Call {
            function,
            arguments,
            responsible: responsible_argument,
        } = expressions.get(&call).unwrap().clone() else {
            return Err("Called `inline_call`, but the ID doesn't refer to a call.");
        };
        let Expression::Lambda {
            parameters,
            responsible_parameter,
            body,
            fuzzable: _,
        } = expressions.get(&function).unwrap().clone() else {
            return Err("Tried to inline, but the call's receiver is not a lambda.");
        };
        if arguments.len() != parameters.len() {
            return Err("Number of arguments doesn't match expected parameter count.");
        }
        if !function.captured_ids(expressions).is_empty() {
            return Err("Lambda captures variables.");
        }

        let mut mapping: HashMap<Id, Id> = parameters
            .iter()
            .zip(arguments.iter())
            .map(|(parameter, argument)| (*parameter, *argument))
            .chain([(responsible_parameter, responsible_argument)])
            .collect();
        let ids_to_insert = body
            .iter()
            .map(|id| id.deep_copy(id_generator, expressions, &mut mapping))
            .collect_vec();
        debug!(
            "Id mapping: {}",
            mapping
                .iter()
                .map(|(par, arg)| format!("{par}: {arg}"))
                .join(", ")
        );
        let index = surrounding_body.iter().position(|it| *it == call).unwrap();
        for (i, id) in ids_to_insert.iter().enumerate() {
            surrounding_body.insert(index + i, *id);
        }
        let return_value = *ids_to_insert.last().unwrap();
        *expressions.get_mut(&call).unwrap() = Expression::Reference(return_value);

        Ok(())
    }

    pub fn inline_functions_containing_use(&mut self) {
        let mut functions_with_use = HashSet::new();
        for id in &self.body {
            if let Expression::Lambda { body, .. } = self.expressions.get(id).unwrap() &&
                body.iter().any(|id| matches!(self.expressions.get(id).unwrap(), Expression::UseModule { .. })) {
                functions_with_use.insert(*id);
            }
        }
        for id in self.body.clone() {
            if let Expression::Call { function, .. } = self.expressions.get(&id).unwrap() && functions_with_use.contains(&function) {
                Self::inline_call(id, &mut self.body, &mut self.id_generator, &mut self.expressions);
            }
        }
    }
}

impl Id {
    fn deep_copy(
        self,
        id_generator: &mut IdGenerator<Id>,
        expressions: &mut HashMap<Id, Expression>,
        mapping: &mut HashMap<Id, Id>,
    ) -> Id {
        let expression = match expressions.get(&self).unwrap().clone() {
            Expression::Int(int) => Expression::Int(int),
            Expression::Text(text) => Expression::Text(text),
            Expression::Symbol(symbol) => Expression::Symbol(symbol),
            Expression::Builtin(builtin) => Expression::Builtin(builtin),
            Expression::Struct(fields) => Expression::Struct(
                fields
                    .iter()
                    .map(|(key, value)| (mapping[key], mapping[value]))
                    .collect(),
            ),
            Expression::Reference(reference) => Expression::Reference(mapping[&reference]),
            Expression::Responsibility(responsibility) => {
                Expression::Responsibility(responsibility)
            }
            Expression::Lambda {
                parameters,
                responsible_parameter,
                body,
                fuzzable,
            } => Expression::Lambda {
                parameters: parameters
                    .iter()
                    .map(|parameter| mapping[parameter])
                    .collect(),
                responsible_parameter: mapping[&responsible_parameter],
                body: body
                    .iter()
                    .map(|id| id.deep_copy(id_generator, expressions, mapping))
                    .collect(),
                fuzzable,
            },
            Expression::Call {
                function,
                arguments,
                responsible,
            } => Expression::Call {
                function: mapping[&function],
                arguments: arguments.iter().map(|argument| mapping[argument]).collect(),
                responsible: mapping[&responsible],
            },
            Expression::UseModule {
                current_module,
                relative_path,
                responsible,
            } => Expression::UseModule {
                current_module: current_module.clone(),
                relative_path: mapping[&relative_path],
                responsible: mapping[&responsible],
            },
            Expression::Needs {
                responsible,
                condition,
                reason,
            } => Expression::Needs {
                responsible: mapping[&responsible],
                condition: mapping[&condition],
                reason: mapping[&reason],
            },
            Expression::Panic {
                reason,
                responsible,
            } => Expression::Panic {
                reason: mapping[&reason],
                responsible: mapping[&responsible],
            },
            Expression::Error { child, errors } => Expression::Error {
                child: child.map(|child| mapping[&child]),
                errors: errors.clone(),
            },
        };
        let id = id_generator.generate();
        mapping.insert(self, id);
        expressions.insert(id, expression);
        id
    }
}
