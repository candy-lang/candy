use std::collections::HashMap;

use crate::{
    compiler::{
        hir_to_mir::HirToMir,
        mir::{Expression, Id, Mir},
    },
    database::Database,
    module::{Module, UsePath},
    utils::IdGenerator,
};
use itertools::Itertools;
use tracing::{debug, warn};

impl Mir {
    pub fn fold_modules(&mut self, db: &Database, import_chain: &[Module]) {
        Self::fold_inner_modules(
            db,
            import_chain,
            &mut self.id_generator,
            &mut self.expressions,
            &mut self.body,
        );
    }
    fn fold_inner_modules(
        db: &Database,
        import_chain: &[Module],
        id_generator: &mut IdGenerator<Id>,
        expressions: &mut HashMap<Id, Expression>,
        body: &mut Vec<Id>,
    ) {
        for id in body.clone() {
            let mut temporary = id.temporarily_get_mut(expressions);
            let expressions = &mut temporary.remaining;
            match &mut temporary.expression {
                Expression::Lambda { body, .. } => {
                    Self::fold_inner_modules(db, import_chain, id_generator, expressions, body);
                }
                Expression::UseModule {
                    current_module,
                    relative_path,
                    responsible,
                } => {
                    let use_id = id;
                    let Some(Expression::Text(path)) = expressions.get(&relative_path) else {
                        warn!("use called with non-constant text");
                        return;
                    };
                    let Ok(path) = UsePath::parse(&path) else {
                        warn!("use called with an invalid path");
                        return;
                    };
                    let Ok(module_to_import) = path.resolve_relative_to(current_module.clone()) else {
                        warn!("use called with an invalid path");
                        return;
                    };
                    if import_chain.contains(&module_to_import) {
                        warn!("circular import");
                        return;
                    }

                    let mir = db.mir(module_to_import.clone()).unwrap();
                    let mut mir = (*mir).clone();
                    let import_chain = {
                        let mut chain = vec![];
                        chain.extend(import_chain.iter().cloned());
                        chain.push(module_to_import);
                        chain
                    };
                    mir.optimize_obvious(db, &import_chain);

                    let mapping: HashMap<Id, Id> = mir
                        .body
                        .iter()
                        .flat_map(|id| id.defined_ids(&mir.expressions))
                        .map(|id| (id, id_generator.generate()))
                        .collect();
                    let ids_to_insert = mir
                        .body
                        .iter()
                        .map(|id| {
                            id.deep_clone_to_other_mir(&mapping, &mir.expressions, expressions)
                        })
                        .collect_vec();
                    let index = body.iter().position(|it| *it == use_id).unwrap();
                    for (i, id) in ids_to_insert.iter().enumerate() {
                        body.insert(index + i, *id);
                    }
                    let return_value = *ids_to_insert.last().unwrap();
                    expressions.insert(use_id, Expression::Reference(return_value));
                }
                _ => {}
            }
        }
    }
}

impl Id {
    fn deep_clone_to_other_mir(
        self,
        mapping: &HashMap<Id, Id>,
        from_expressions: &HashMap<Id, Expression>,
        to_expressions: &mut HashMap<Id, Expression>,
    ) -> Id {
        // debug!(
        //     "Deeply cloning {self} to other MIR with mapping {}.",
        //     mapping
        //         .iter()
        //         .map(|(key, value)| format!("{key}: {value}"))
        //         .join(", ")
        // );
        let expression = match from_expressions.get(&self).unwrap().clone() {
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
                    .map(|id| id.deep_clone_to_other_mir(mapping, from_expressions, to_expressions))
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
        let id = mapping[&self];
        to_expressions.insert(id, expression);
        id
    }
}
