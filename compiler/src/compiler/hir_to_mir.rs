use super::{
    hir,
    mir::{Body, Constant, ConstantPool, Content, Expression, Id, Mir},
};
use crate::utils::IdGenerator;
use linked_hash_map::LinkedHashMap;
use std::collections::HashMap;

impl hir::Body {
    pub fn into_mir(self) -> Mir {
        let mut id_generator = IdGenerator::start_at(0);
        let mut constants = ConstantPool::new();
        let mut hir_to_mir_ids = HashMap::new();

        let body = convert_body(
            &mut id_generator,
            &mut constants,
            &mut hir_to_mir_ids,
            self.expressions,
        );
        Mir {
            id_generator,
            constants,
            body,
        }
    }
}

fn convert_body(
    id_generator: &mut IdGenerator<Id>,
    constants: &mut ConstantPool,
    hir_to_mir_ids: &mut HashMap<hir::Id, Id>,
    body: LinkedHashMap<hir::Id, hir::Expression>,
) -> Body {
    let mut out_body = Body::new();

    for (id, expression) in body {
        let content = match expression.clone() {
            hir::Expression::Int(int) => Content::Constant(Constant::Int(int.into())),
            hir::Expression::Text(text) => Content::Constant(Constant::Text(text)),
            hir::Expression::Symbol(symbol) => Content::Constant(Constant::Symbol(symbol)),
            hir::Expression::Builtin(builtin) => Content::Constant(Constant::Builtin(builtin)),
            hir::Expression::Reference(reference) => {
                let target = hir_to_mir_ids[&reference];
                hir_to_mir_ids.insert(id, target);
                continue;
            }
            hir::Expression::Struct(struct_) => {
                let mapped_fields = struct_
                    .into_iter()
                    .map(|(key, value)| {
                        (hir_to_mir_ids[&key].clone(), hir_to_mir_ids[&value].clone())
                    })
                    .collect();
                Content::Expression(Expression::Struct(mapped_fields))
            }
            hir::Expression::Lambda(lambda) => {
                let mut parameters = vec![];
                for parameter in &lambda.parameters {
                    let id = id_generator.generate();
                    hir_to_mir_ids.insert(parameter.clone(), id);
                    parameters.push(id);
                }
                Content::Expression(Expression::Lambda {
                    parameters,
                    body: convert_body(
                        id_generator,
                        constants,
                        hir_to_mir_ids,
                        lambda.body.expressions,
                    ),
                    fuzzable: lambda.fuzzable,
                })
            }
            hir::Expression::Call {
                function,
                arguments,
            } => Content::Expression(Expression::Call {
                function: hir_to_mir_ids[&function].clone(),
                arguments: arguments
                    .into_iter()
                    .map(|arg| hir_to_mir_ids[&arg].clone())
                    .collect(),
            }),
            hir::Expression::UseModule {
                current_module,
                relative_path,
            } => Content::Expression(Expression::UseModule {
                current_module,
                relative_path: hir_to_mir_ids[&relative_path].clone(),
            }),
            hir::Expression::Needs { condition, reason } => {
                Content::Expression(Expression::Needs {
                    condition: hir_to_mir_ids[&condition].clone(),
                    reason: hir_to_mir_ids[&reason].clone(),
                })
            }
            hir::Expression::Error { child, errors } => Content::Expression(Expression::Error {
                child: child.map(|child| hir_to_mir_ids[&child].clone()),
                errors,
            }),
        };
        let mir_id = match content {
            Content::Constant(constant) => constants.add(id_generator, constant),
            Content::Expression(expression) => out_body.push(id_generator, expression),
        };
        hir_to_mir_ids.insert(id.clone(), mir_id);
    }

    out_body
}
