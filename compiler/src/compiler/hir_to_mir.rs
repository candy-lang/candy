use super::{
    ast_to_hir::AstToHir,
    cst::CstDb,
    error::CompilerError,
    hir,
    mir::{Body, Expression, Id, Mir, MirBodyBuilder},
    tracing::TracingConfig,
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
    fn mir(&self, module: Module, tracing: TracingConfig) -> Option<Arc<Mir>>;
}

fn mir(db: &dyn HirToMir, module: Module, tracing: TracingConfig) -> Option<Arc<Mir>> {
    let (hir, _) = db.hir(module.clone())?;
    let mir = compile_module(db, module, &hir, &tracing);
    Some(Arc::new(mir))
}

fn compile_module(
    db: &dyn HirToMir,
    module: Module,
    hir: &hir::Body,
    tracing: &TracingConfig,
) -> Mir {
    let mut id_generator = IdGenerator::start_at(0);
    let mut body = Body::default();
    let mut mapping = HashMap::new();

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
    let mut pattern_identifier_ids = HashMap::new();
    for (id, expression) in &hir.expressions {
        compile_expression(
            db,
            &mut id_generator,
            &mut body,
            &mut mapping,
            &mut pattern_identifier_ids,
            needs_function,
            module_hir_id,
            id,
            expression,
            tracing,
        );
    }
    assert!(pattern_identifier_ids.is_empty());

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
///     builtinEquals condition True
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
        let nothing_symbol = body.push(Expression::nothing());
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
    pattern_identifier_ids: &mut HashMap<hir::Id, HashMap<hir::PatternIdentifierId, Id>>,
    needs_function: Id,
    responsible_for_needs: Id,
    hir_id: &hir::Id,
    expression: &hir::Expression,
    tracing: &TracingConfig,
) {
    let expression = match expression {
        hir::Expression::Int(int) => Expression::Int(int.clone().into()),
        hir::Expression::Text(text) => Expression::Text(text.clone()),
        hir::Expression::Reference(reference) => Expression::Reference(mapping[reference]),
        hir::Expression::Symbol(symbol) => Expression::Symbol(symbol.clone()),
        hir::Expression::Builtin(builtin) => Expression::Builtin(*builtin),
        hir::Expression::List(items) => {
            Expression::List(items.iter().map(|item| mapping[item]).collect())
        }
        hir::Expression::Struct(fields) => Expression::Struct(
            fields
                .iter()
                .map(|(key, value)| (mapping[key], mapping[value]))
                .collect(),
        ),
        hir::Expression::Destructure {
            expression,
            pattern,
        } => {
            let responsible =
                body.push_with_new_id(id_generator, Expression::HirId(hir_id.clone()));
            let expression = mapping[expression];
            let mut identifier_ids = HashMap::new();
            compile_destructure(
                db,
                id_generator,
                body,
                &mut identifier_ids,
                responsible,
                expression,
                pattern,
            );

            let existing_entry = pattern_identifier_ids.insert(hir_id.to_owned(), identifier_ids);
            assert!(existing_entry.is_none());

            Expression::Reference(expression)
        }
        hir::Expression::PatternIdentifierReference {
            destructuring,
            identifier_id,
        } => {
            let identifier_ids = pattern_identifier_ids
                .get_mut(destructuring)
                .unwrap_or_else(|| {
                    panic!("Destructure expression is missing for destructuring {destructuring}.")
                });
            let id = identifier_ids.remove(identifier_id).unwrap_or_else(|| panic!(
                    "Pattern identifier {identifier_id} is missing for destructuring {destructuring}.",
                ));
            if identifier_ids.is_empty() {
                pattern_identifier_ids.remove(destructuring).unwrap();
            }
            Expression::Reference(id)
        }
        hir::Expression::Lambda(hir::Lambda {
            parameters: original_parameters,
            body: original_body,
            fuzzable,
        }) => {
            let mut parameters = vec![];
            let responsible_parameter: Id = id_generator.generate();
            let mut lambda_body = Body::default();

            for original_parameter in original_parameters {
                let parameter = id_generator.generate();
                parameters.push(parameter);
                mapping.insert(original_parameter.clone(), parameter);
            }

            let responsible = if *fuzzable {
                responsible_parameter
            } else {
                // This is a lambda with curly braces, so whoever is responsible
                // for `needs` in the current scope is also responsible for
                // `needs` in the lambda.
                responsible_for_needs
            };

            for (id, expression) in &original_body.expressions {
                compile_expression(
                    db,
                    id_generator,
                    &mut lambda_body,
                    mapping,
                    pattern_identifier_ids,
                    needs_function,
                    responsible,
                    id,
                    expression,
                    tracing,
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
            if tracing.register_fuzzables.is_enabled() && *fuzzable {
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

            if tracing.calls.is_enabled() {
                let hir_call =
                    body.push_with_new_id(id_generator, Expression::HirId(hir_id.clone()));
                body.push_with_new_id(
                    id_generator,
                    Expression::TraceCallStarts {
                        hir_call,
                        function: mapping[function],
                        arguments: arguments.clone(),
                        responsible,
                    },
                );
            }
            let call = body.push_with_new_id(
                id_generator,
                Expression::Call {
                    function: mapping[function],
                    arguments,
                    responsible,
                },
            );
            if tracing.calls.is_enabled() {
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
            current_module: current_module.clone(),
            relative_path: mapping[relative_path],
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
                arguments: vec![mapping[condition], mapping[reason], responsible_for_needs],
                responsible,
            }
        }
        hir::Expression::Error { errors, .. } => {
            let responsible =
                body.push_with_new_id(id_generator, Expression::HirId(hir_id.clone()));
            compile_errors(db, id_generator, body, responsible, errors)
        }
    };

    let id = body.push_with_new_id(id_generator, expression);
    mapping.insert(hir_id.clone(), id);

    if tracing.evaluated_expressions.is_enabled() {
        let hir_expression = body.push_with_new_id(id_generator, Expression::HirId(hir_id.clone()));
        body.push_with_new_id(
            id_generator,
            Expression::TraceExpressionEvaluated {
                hir_expression,
                value: id,
            },
        );
        body.push_with_new_id(id_generator, Expression::Reference(id));
    }
}

// Destructuring
fn compile_destructure(
    db: &dyn HirToMir,
    id_generator: &mut IdGenerator<Id>,
    body: &mut Body,
    identifier_ids: &mut HashMap<hir::PatternIdentifierId, Id>,
    responsible: Id,
    expression: Id,
    pattern: &hir::Pattern,
) {
    match pattern {
        hir::Pattern::NewIdentifier(identifier_id) => {
            let existing_entry = identifier_ids.insert(identifier_id.to_owned(), expression);
            assert!(existing_entry.is_none());
        }
        hir::Pattern::Int(int) => compile_destructure_exact_value(
            id_generator,
            body,
            responsible,
            expression,
            Expression::Int(int.to_owned().into()),
        ),
        hir::Pattern::Text(text) => compile_destructure_exact_value(
            id_generator,
            body,
            responsible,
            expression,
            Expression::Text(text.to_owned()),
        ),
        hir::Pattern::Symbol(symbol) => compile_destructure_exact_value(
            id_generator,
            body,
            responsible,
            expression,
            Expression::Symbol(symbol.to_owned()),
        ),
        hir::Pattern::List(list) => {
            // Check that it's a list.
            compile_destructure_verify_type(
                id_generator,
                body,
                responsible,
                expression,
                "List".to_string(),
            );

            // Check that the length is correct.
            let builtin_list_length = body.push_with_new_id(
                id_generator,
                Expression::Builtin(BuiltinFunction::ListLength),
            );
            let actual_length = body.push_with_new_id(
                id_generator,
                Expression::Call {
                    function: builtin_list_length,
                    arguments: vec![expression],
                    responsible,
                },
            );
            push_equals_or_panic(
                id_generator,
                body,
                responsible,
                Expression::Int(list.len().into()),
                actual_length,
                |body, _expected, actual| {
                    vec![
                        body.push(Expression::Text(format!(
                            "Expected {} items, got ",
                            list.len(),
                        ))),
                        actual,
                        body.push(Expression::Text(".".to_string())),
                    ]
                },
            );

            // Destructure the elements.
            let builtin_list_get =
                body.push_with_new_id(id_generator, Expression::Builtin(BuiltinFunction::ListGet));
            for (index, pattern) in list.iter().enumerate() {
                let index = body.push_with_new_id(id_generator, Expression::Int(index.into()));
                let item = body.push_with_new_id(
                    id_generator,
                    Expression::Call {
                        function: builtin_list_get,
                        arguments: vec![expression, index],
                        responsible,
                    },
                );
                compile_destructure(
                    db,
                    id_generator,
                    body,
                    identifier_ids,
                    responsible,
                    item,
                    pattern,
                );
            }
        }
        hir::Pattern::Struct(struct_) => {
            // Check that it's a struct.
            compile_destructure_verify_type(
                id_generator,
                body,
                responsible,
                expression,
                "Struct".to_string(),
            );

            // Destructure the elements.
            let builtin_struct_get = body.push_with_new_id(
                id_generator,
                Expression::Builtin(BuiltinFunction::StructGet),
            );
            for (key_pattern, value_pattern) in struct_ {
                let key = compile_pattern_to_key_expression(
                    db,
                    id_generator,
                    body,
                    responsible,
                    key_pattern,
                );
                let item = body.push_with_new_id(
                    id_generator,
                    Expression::Call {
                        function: builtin_struct_get,
                        arguments: vec![expression, key],
                        responsible,
                    },
                );
                compile_destructure(
                    db,
                    id_generator,
                    body,
                    identifier_ids,
                    responsible,
                    item,
                    value_pattern,
                );
            }
        }
        hir::Pattern::Error { child, errors } => {
            compile_errors(db, id_generator, body, responsible, errors);
            if let Some(child) = child {
                // We still have to populate `identifier_ids`.
                compile_destructure(
                    db,
                    id_generator,
                    body,
                    identifier_ids,
                    responsible,
                    expression,
                    child,
                );
            }
        }
    }
}
fn compile_destructure_exact_value(
    id_generator: &mut IdGenerator<Id>,
    body: &mut Body,
    responsible: Id,
    expression: Id,
    expected_value: Expression,
) {
    push_equals_or_panic(
        id_generator,
        body,
        responsible,
        expected_value,
        expression,
        |body, expected, actual| {
            vec![
                body.push(Expression::Text("Expected `".to_string())),
                expected,
                body.push(Expression::Text("`, got `".to_string())),
                actual,
                body.push(Expression::Text("`.".to_string())),
            ]
        },
    );
}
fn compile_destructure_verify_type(
    id_generator: &mut IdGenerator<Id>,
    body: &mut Body,
    responsible: Id,
    expression: Id,
    expected_type: String,
) {
    let builtin_type_of =
        body.push_with_new_id(id_generator, Expression::Builtin(BuiltinFunction::TypeOf));
    let type_ = body.push_with_new_id(
        id_generator,
        Expression::Call {
            function: builtin_type_of,
            arguments: vec![expression],
            responsible,
        },
    );

    push_equals_or_panic(
        id_generator,
        body,
        responsible,
        Expression::Symbol(expected_type),
        type_,
        |body, expected, actual| {
            vec![
                body.push(Expression::Text("Expected a ".to_string())),
                expected,
                body.push(Expression::Text(", got `".to_string())),
                actual,
                body.push(Expression::Text("`.".to_string())),
            ]
        },
    );
}
fn push_equals_or_panic<R>(
    id_generator: &mut IdGenerator<Id>,
    body: &mut Body,
    responsible: Id,
    expected_value: Expression,
    actual_value: Id,
    reason_factory: R,
) -> Id
where
    R: Fn(&mut MirBodyBuilder, Id, Id) -> Vec<Id>,
{
    let builtin_equals =
        body.push_with_new_id(id_generator, Expression::Builtin(BuiltinFunction::Equals));
    let expected_value = body.push_with_new_id(id_generator, expected_value);
    let equals = body.push_with_new_id(
        id_generator,
        Expression::Call {
            function: builtin_equals,
            arguments: vec![expected_value, actual_value],
            responsible,
        },
    );

    let empty_lambda = push_empty_lambda(id_generator, body);

    let on_wrong_value = Expression::build_lambda(id_generator, |body, _| {
        let to_debug_text = body.push(Expression::Builtin(BuiltinFunction::ToDebugText));

        let expected_as_text = body.push(Expression::Call {
            function: to_debug_text,
            arguments: vec![expected_value],
            responsible,
        });

        let actual_as_text = body.push(Expression::Call {
            function: to_debug_text,
            arguments: vec![actual_value],
            responsible,
        });

        let builtin_text_concatenate =
            body.push(Expression::Builtin(BuiltinFunction::TextConcatenate));
        let reason_parts = reason_factory(body, expected_as_text, actual_as_text);
        let reason = reason_parts
            .into_iter()
            .reduce(|left, right| {
                body.push(Expression::Call {
                    function: builtin_text_concatenate,
                    arguments: vec![left, right],
                    responsible,
                })
            })
            .unwrap();

        body.push(Expression::Panic {
            reason,
            responsible,
        });
    });
    let on_wrong_value = body.push_with_new_id(id_generator, on_wrong_value);

    let builtin_if_else =
        body.push_with_new_id(id_generator, Expression::Builtin(BuiltinFunction::IfElse));
    body.push_with_new_id(
        id_generator,
        Expression::Call {
            function: builtin_if_else,
            arguments: vec![equals, empty_lambda, on_wrong_value],
            responsible,
        },
    )
}
fn push_empty_lambda(id_generator: &mut IdGenerator<Id>, body: &mut Body) -> Id {
    let nothing = body.push_with_new_id(id_generator, Expression::nothing());
    let lambda = Expression::build_lambda(id_generator, |body, _| {
        body.push(Expression::Reference(nothing));
    });
    body.push_with_new_id(id_generator, lambda)
}
fn compile_pattern_to_key_expression(
    db: &dyn HirToMir,
    id_generator: &mut IdGenerator<Id>,
    body: &mut Body,
    responsible: Id,
    pattern: &hir::Pattern,
) -> Id {
    let expression = match pattern {
        hir::Pattern::NewIdentifier(_) => {
            panic!("New identifiers can't be used in this part of a pattern.")
        }
        hir::Pattern::Int(int) => Expression::Int(int.to_owned().into()),
        hir::Pattern::Text(text) => Expression::Text(text.to_owned()),
        hir::Pattern::Symbol(symbol) => Expression::Symbol(symbol.to_owned()),
        hir::Pattern::List(_) => panic!("Lists can't be used in this part of a pattern."),
        hir::Pattern::Struct(_) => panic!("Structs can't be used in this part of a pattern."),
        hir::Pattern::Error { errors, .. } => {
            compile_errors(db, id_generator, body, responsible, errors)
        }
    };
    body.push_with_new_id(id_generator, expression)
}

// Errors

fn compile_errors(
    db: &dyn HirToMir,
    id_generator: &mut IdGenerator<Id>,
    body: &mut Body,
    responsible: Id,
    errors: &Vec<CompilerError>,
) -> Expression {
    let reason = body.push_with_new_id(
        id_generator,
        Expression::Text(if errors.len() == 1 {
            format!(
                "The code still contains an error: {}",
                errors.iter().next().unwrap().format_nicely(db)
            )
        } else {
            format!(
                "The code still contains errors:\n{}",
                errors
                    .iter()
                    .map(|error| format!("- {}", error.format_nicely(db)))
                    .join("\n"),
            )
        }),
    );
    Expression::Panic {
        reason,
        responsible,
    }
}

impl CompilerError {
    fn format_nicely(&self, db: &dyn HirToMir) -> String {
        let (start_line, start_col) = db.offset_to_lsp(self.module.clone(), self.span.start);
        let (end_line, end_col) = db.offset_to_lsp(self.module.clone(), self.span.end);

        format!(
            "{}:{}:{} – {}:{}: {}",
            self.module, start_line, start_col, end_line, end_col, self.payload
        )
    }
}
