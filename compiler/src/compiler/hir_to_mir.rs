use super::{
    ast_to_hir::AstToHir,
    cst::CstDb,
    error::CompilerError,
    hir,
    mir::{Body, Expression, Id, Mir},
};
use crate::{
    builtin_functions::BuiltinFunction,
    language_server::utils::LspPositionConversion,
    module::{Module, ModuleKind, Package},
    utils::IdGenerator,
};
use itertools::Itertools;
use std::{collections::HashMap, sync::Arc};

#[salsa::query_group(HirToMirStorage)]
pub trait HirToMir: CstDb + AstToHir + LspPositionConversion {
    fn mir(&self, module: Module, config: TracingConfig) -> Option<Arc<Mir>>;
}

#[derive(PartialEq, Eq, Hash, Debug, Clone, Default)]
pub struct TracingConfig {
    pub register_fuzzables: bool,
    pub trace_calls: bool,
    pub trace_evaluated_expressions: bool,
}

fn mir(db: &dyn HirToMir, module: Module, config: TracingConfig) -> Option<Arc<Mir>> {
    let (hir, _) = db.hir(module.clone())?;
    let hir = (*hir).clone();
    let mir = compile_module(db, module, hir, &config);
    Some(Arc::new(mir))
}

fn compile_module(
    db: &dyn HirToMir,
    module: Module,
    hir: hir::Body,
    config: &TracingConfig,
) -> Mir {
    let mut id_generator = IdGenerator::start_at(0);
    let mut body = Body::new();
    let mut mapping = HashMap::<hir::Id, Id>::new();

    body.push_with_new_id(
        &mut id_generator,
        Expression::ModuleStarts {
            module: module.clone(),
        },
    );

    let needs_function = generate_needs_function(&mut id_generator);
    let needs_function = body.push_with_new_id(&mut id_generator, needs_function);

    let module_hir_id = body.push_with_new_id(
        &mut id_generator,
        Expression::HirId(hir::Id::new(module, vec![])),
    );
    for (id, expression) in hir.expressions {
        compile_expression(
            db,
            &mut id_generator,
            &mut body,
            &mut mapping,
            needs_function,
            module_hir_id,
            &id,
            expression,
            config,
        );
    }

    let return_value = body.return_value();
    body.push_with_new_id(&mut id_generator, Expression::ModuleEnds);
    body.push_with_new_id(&mut id_generator, Expression::Reference(return_value));

    Mir { id_generator, body }
}

/// In the MIR, there's no longer the concept of needs. Instead, HIR IDs are
/// first-class expressions and there's a `panic` expression that takes a HIR
/// ID that's responsible.
///
/// This function generates the `needs` function. Unlike regular functions, it
/// also expects a HIR ID as a normal parameter.
///
/// Here's a high-level pseudocode of the generated `needs` function:
///
/// ```pseudocode
/// needs = { condition reason responsibleForCondition (responsibleForCall) ->
///   isConditionBool = builtinIfElse
///     (builtinEquals condition True)
///     { True }
///     { builtinEquals condition False }
///   builtinIfElse isConditionBool { Nothing } {
///     panic "The condition must be either `True` or `False`." responsibleForCall
///   }
///
///   builtinIfElse (builtinEquals (builtinTypeOf reason) Text) { Nothing} {
///     panic "The `reason` must be a text." responsibleForCall
///   }
///
///   builtinIfElse condition { Nothing } { panic reason responsibleForCondition }
/// }
/// ```
fn generate_needs_function(id_generator: &mut IdGenerator<Id>) -> Expression {
    Expression::build_lambda(id_generator, |body, responsible_for_call| {
        let condition = body.new_parameter();
        let reason = body.new_parameter();
        let responsible_for_condition = body.new_parameter();

        // Common stuff.
        let needs_code = body.push(Expression::HirId(hir::Id::new(
            Module {
                package: Package::Anonymous {
                    url: "$generated".to_string(),
                },
                path: vec![],
                kind: ModuleKind::Code,
            },
            vec!["needs".to_string()],
        )));
        let builtin_equals = body.push(Expression::Builtin(BuiltinFunction::Equals));
        let builtin_if_else = body.push(Expression::Builtin(BuiltinFunction::IfElse));
        let nothing_symbol = body.push(Expression::Symbol("Nothing".to_string()));
        let lambda_returning_nothing = body.push_lambda(|body, _| {
            body.push(Expression::Reference(nothing_symbol));
        });

        // Make sure the condition is a bool.
        let true_symbol = body.push(Expression::Symbol("True".to_string()));
        let false_symbol = body.push(Expression::Symbol("False".to_string()));
        let is_condition_true = body.push(Expression::Call {
            function: builtin_equals,
            arguments: vec![condition, true_symbol],
            responsible: needs_code,
        });
        let is_condition_false = body.push(Expression::Call {
            function: builtin_equals,
            arguments: vec![condition, false_symbol],
            responsible: needs_code,
        });
        let lambda_returning_true = body.push_lambda(|body, _| {
            body.push(Expression::Reference(true_symbol));
        });
        let lambda_returning_whether_condition_is_false = body.push_lambda(|body, _| {
            body.push(Expression::Reference(is_condition_false));
        });
        let is_condition_bool = body.push(Expression::Call {
            function: builtin_if_else,
            arguments: vec![
                is_condition_true,
                lambda_returning_true,
                lambda_returning_whether_condition_is_false,
            ],
            responsible: needs_code,
        });
        let on_invalid_condition = body.push_lambda(|body, _| {
            let panic_reason = body.push(Expression::Text(
                "The `condition` must be either `True` or `False`.".to_string(),
            ));
            body.push(Expression::Panic {
                reason: panic_reason,
                responsible: responsible_for_call,
            });
        });
        body.push(Expression::Call {
            function: builtin_if_else,
            arguments: vec![
                is_condition_bool,
                lambda_returning_nothing,
                on_invalid_condition,
            ],
            responsible: needs_code,
        });

        // Make sure the reason is a text.
        let builtin_type_of = body.push(Expression::Builtin(BuiltinFunction::TypeOf));
        let type_of_reason = body.push(Expression::Call {
            function: builtin_type_of,
            arguments: vec![reason],
            responsible: responsible_for_call,
        });
        let text_symbol = body.push(Expression::Symbol("Text".to_string()));
        let is_reason_text = body.push(Expression::Call {
            function: builtin_equals,
            arguments: vec![type_of_reason, text_symbol],
            responsible: responsible_for_call,
        });
        let on_invalid_reason = body.push_lambda(|body, _| {
            let panic_reason =
                body.push(Expression::Text("The `reason` must be a text.".to_string()));
            body.push(Expression::Panic {
                reason: panic_reason,
                responsible: responsible_for_call,
            });
        });
        body.push(Expression::Call {
            function: builtin_if_else,
            arguments: vec![is_reason_text, lambda_returning_nothing, on_invalid_reason],
            responsible: needs_code,
        });

        // The core logic of the needs.
        let panic_lambda = body.push_lambda(|body, _| {
            body.push(Expression::Panic {
                reason,
                responsible: responsible_for_condition,
            });
        });
        body.push(Expression::Call {
            function: builtin_if_else,
            arguments: vec![condition, lambda_returning_nothing, panic_lambda],
            responsible: needs_code,
        });
    })
}

// Nothing to see here.
#[allow(clippy::too_many_arguments)]
fn compile_expression(
    db: &dyn HirToMir,
    id_generator: &mut IdGenerator<Id>,
    body: &mut Body,
    mapping: &mut HashMap<hir::Id, Id>,
    needs_function: Id,
    responsible_for_needs: Id,
    hir_id: &hir::Id,
    expression: hir::Expression,
    config: &TracingConfig,
) {
    let expression = match expression {
        hir::Expression::Int(int) => Expression::Int(int.into()),
        hir::Expression::Text(text) => Expression::Text(text),
        hir::Expression::Reference(reference) => Expression::Reference(mapping[&reference]),
        hir::Expression::Symbol(symbol) => Expression::Symbol(symbol),
        hir::Expression::Builtin(builtin) => Expression::Builtin(builtin),
        hir::Expression::List(items) => {
            Expression::List(items.iter().map(|item| mapping[item]).collect())
        }
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
                    db,
                    id_generator,
                    &mut lambda_body,
                    mapping,
                    needs_function,
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
                },
            );
            if config.register_fuzzables && fuzzable {
                let hir_definition =
                    body.push_with_new_id(id_generator, Expression::HirId(hir_id.clone()));
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
        hir::Expression::Call {
            function,
            arguments,
        } => {
            let responsible =
                body.push_with_new_id(id_generator, Expression::HirId(hir_id.clone()));
            let arguments = arguments
                .iter()
                .map(|argument| mapping[argument])
                .collect_vec();

            if config.trace_calls {
                let hir_call =
                    body.push_with_new_id(id_generator, Expression::HirId(hir_id.clone()));
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
                body.push_with_new_id(id_generator, Expression::HirId(hir_id.clone()));
            Expression::Call {
                function: needs_function,
                arguments: vec![mapping[&condition], mapping[&reason], responsible_for_needs],
                responsible,
            }
        }
        hir::Expression::Error { errors, .. } => {
            let reason = body.push_with_new_id(
                id_generator,
                Expression::Text(if errors.len() == 1 {
                    format!(
                        "The code still contains an error: {}",
                        errors.into_iter().next().unwrap().format_nicely(db)
                    )
                } else {
                    format!(
                        "The code still contains errors:\n{}",
                        errors
                            .into_iter()
                            .map(|error| format!("- {}", error.format_nicely(db)))
                            .join("\n"),
                    )
                }),
            );
            let responsible =
                body.push_with_new_id(id_generator, Expression::HirId(hir_id.clone()));
            Expression::Panic {
                reason,
                responsible,
            }
        }
    };

    let id = body.push_with_new_id(id_generator, expression);
    mapping.insert(hir_id.clone(), id);

    if config.trace_evaluated_expressions {
        let hir_expression = body.push_with_new_id(id_generator, Expression::HirId(hir_id.clone()));
        body.push_with_new_id(
            id_generator,
            Expression::TraceExpressionEvaluated {
                hir_expression,
                value: id,
            },
        );
    }
}

impl CompilerError {
    fn format_nicely(&self, db: &dyn HirToMir) -> String {
        let (start_line, start_col) = db.offset_to_lsp(self.module.clone(), self.span.start);
        let (end_line, end_col) = db.offset_to_lsp(self.module.clone(), self.span.end);

        format!(
            "{}, {}:{} – {}:{}: {}",
            self.module, start_line, start_col, end_line, end_col, self.payload
        )
    }
}
