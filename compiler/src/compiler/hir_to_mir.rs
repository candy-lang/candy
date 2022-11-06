use super::{
    ast_to_hir::AstToHir,
    cst::CstDb,
    hir,
    mir::{Body, Expression, Id, Mir},
};
use crate::{module::Module, utils::IdGenerator};
use itertools::Itertools;
use std::{collections::HashMap, sync::Arc};

#[salsa::query_group(HirToMirStorage)]
pub trait HirToMir: CstDb + AstToHir {
    fn mir(&self, module: Module, config: MirConfig) -> Option<Arc<Mir>>;
}

#[derive(PartialEq, Eq, Hash, Debug, Clone, Default)]
pub struct MirConfig {
    register_fuzzables: bool,
    trace_calls: bool,
    trace_evaluated_expressions: bool,
}

fn mir(db: &dyn HirToMir, module: Module, config: MirConfig) -> Option<Arc<Mir>> {
    let (hir, _) = db.hir(module.clone())?;
    let hir = (*hir).clone();
    let mir = compile_module(module, hir, &config);
    Some(Arc::new(mir))
}

fn compile_module(module: Module, hir: hir::Body, config: &MirConfig) -> Mir {
    let mut id_generator = IdGenerator::start_at(0);
    let mut body = Body::new();
    let mut mapping = HashMap::<hir::Id, Id>::new();

    let module_responsibility = {
        let id = id_generator.generate();
        body.push(id, Expression::Responsibility(hir::Id::new(module, vec![])));
        id
    };
    for (id, expression) in hir.expressions {
        compile_expression(
            &mut id_generator,
            &mut body,
            &mut mapping,
            module_responsibility,
            &id,
            expression,
            config,
        );
    }

    Mir { id_generator, body }
}

fn compile_expression(
    id_generator: &mut IdGenerator<Id>,
    body: &mut Body,
    mapping: &mut HashMap<hir::Id, Id>,
    responsible_for_needs: Id,
    hir_id: &hir::Id,
    expression: hir::Expression,
    config: &MirConfig,
) {
    let expression = match expression {
        hir::Expression::Int(int) => Expression::Int(int.into()),
        hir::Expression::Text(text) => Expression::Text(text),
        hir::Expression::Reference(reference) => Expression::Reference(mapping[&reference]),
        hir::Expression::Symbol(symbol) => Expression::Symbol(symbol),
        hir::Expression::Struct(fields) => Expression::Struct(
            fields
                .iter()
                .map(|(key, value)| (mapping[key], mapping[value]))
                .collect(),
        ),
        hir::Expression::Lambda(hir::Lambda {
            parameters: original_parameters,
            body: original_body,
            fuzzable,
        }) => {
            let mut parameters = vec![];
            let responsible_parameter: Id = id_generator.generate();
            let mut lambda_body = Body::new();

            for original_parameter in original_parameters {
                let parameter = id_generator.generate();
                parameters.push(parameter);
                mapping.insert(original_parameter, parameter);
            }

            let responsible = if fuzzable {
                responsible_parameter
            } else {
                // This is a lambda with curly braces, so whoever is responsible
                // for `needs` in the current scope is also responsible for
                // `needs` in the lambda.
                responsible_for_needs
            };

            for (id, expression) in original_body.expressions {
                compile_expression(
                    id_generator,
                    &mut lambda_body,
                    mapping,
                    responsible,
                    &id,
                    expression,
                    config,
                );
            }

            let lambda = body.push_with_new_id(
                id_generator,
                Expression::Lambda {
                    parameters,
                    responsible_parameter,
                    body: lambda_body,
                    fuzzable,
                },
            );
            if config.register_fuzzables && fuzzable {
                let hir_definition =
                    body.push_with_new_id(id_generator, Expression::Responsibility(hir_id.clone()));
                body.push_with_new_id(
                    id_generator,
                    Expression::TraceFoundFuzzableClosure {
                        hir_definition,
                        closure: lambda,
                    },
                );
            }
            Expression::Reference(lambda)
        }
        hir::Expression::Builtin(builtin) => Expression::Builtin(builtin),
        hir::Expression::Call {
            function,
            arguments,
        } => {
            let responsible =
                body.push_with_new_id(id_generator, Expression::Responsibility(hir_id.clone()));
            let arguments = arguments
                .iter()
                .map(|argument| mapping[argument])
                .collect_vec();

            if config.trace_calls {
                let hir_call =
                    body.push_with_new_id(id_generator, Expression::Responsibility(hir_id.clone()));
                body.push_with_new_id(
                    id_generator,
                    Expression::TraceCallStarts {
                        hir_call,
                        function: mapping[&function],
                        arguments: arguments.clone(),
                        responsible,
                    },
                );
            }
            let call = body.push_with_new_id(
                id_generator,
                Expression::Call {
                    function: mapping[&function],
                    arguments,
                    responsible,
                },
            );
            if config.trace_calls {
                body.push_with_new_id(
                    id_generator,
                    Expression::TraceCallEnds { return_value: call },
                );
            }
            Expression::Reference(call)
        }
        hir::Expression::UseModule {
            current_module,
            relative_path,
        } => Expression::UseModule {
            current_module,
            relative_path: mapping[&relative_path],
            // The `UseModule` expression only exists in the generated `use`
            // function. If a use fails, that's also the fault of the caller.
            // Essentially, the `UseModule` expression works exactly like a
            // `needs`.
            responsible: responsible_for_needs,
        },
        hir::Expression::Needs { condition, reason } => {
            let responsible =
                body.push_with_new_id(id_generator, Expression::Responsibility(hir_id.clone()));
            Expression::Needs {
                condition: mapping[&condition],
                reason: mapping[&reason],
                responsible,
                responsible_for_condition: responsible_for_needs,
            }
        }
        hir::Expression::Error { child, errors } => Expression::Error {
            child: child.map(|child| mapping[&child]),
            errors,
        },
    };

    let id = body.push_with_new_id(id_generator, expression);
    mapping.insert(hir_id.clone(), id);

    if config.trace_evaluated_expressions {
        let hir_expression =
            body.push_with_new_id(id_generator, Expression::Responsibility(hir_id.clone()));
        body.push_with_new_id(
            id_generator,
            Expression::TraceExpressionEvaluated {
                hir_expression,
                value: id,
            },
        );
    }
}
