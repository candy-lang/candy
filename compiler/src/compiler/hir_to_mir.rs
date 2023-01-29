use super::{
    ast_to_hir::AstToHir,
    cst::CstDb,
    error::CompilerError,
    hir,
    mir::{BodyBuilder, Expression, Id, LambdaBodyBuilder, Mir},
    tracing::TracingConfig,
};
use crate::{
    builtin_functions::BuiltinFunction,
    language_server::utils::LspPositionConversion,
    module::{Module, ModuleKind, Package},
    utils::IdGenerator,
};
use itertools::Itertools;
use rustc_hash::FxHashMap;
use std::sync::Arc;

#[salsa::query_group(HirToMirStorage)]
pub trait HirToMir: CstDb + AstToHir + LspPositionConversion {
    fn mir(&self, module: Module, tracing: TracingConfig) -> Option<Arc<Mir>>;
}

fn mir(db: &dyn HirToMir, module: Module, tracing: TracingConfig) -> Option<Arc<Mir>> {
    let mir = match module.kind {
        ModuleKind::Code => {
            let (hir, _) = db.hir(module.clone())?;
            compile_module(db, module, &hir, &tracing)
        }
        ModuleKind::Asset => {
            let bytes = db.get_module_content(module)?;
            Mir::build(|body| {
                let bytes = bytes
                    .iter()
                    .map(|&it| body.push(Expression::Int(it.into())))
                    .collect_vec();
                body.push(Expression::List(bytes));
            })
        }
    };
    Some(Arc::new(mir))
}

fn compile_module(
    db: &dyn HirToMir,
    module: Module,
    hir: &hir::Body,
    tracing: &TracingConfig,
) -> Mir {
    Mir::build(|body| {
        let mut mapping = FxHashMap::default();

        body.push(Expression::ModuleStarts {
            module: module.clone(),
        });

        let needs_function = generate_needs_function(body);

        let module_hir_id = body.push(Expression::HirId(hir::Id::new(module, vec![])));
        let mut pattern_identifier_ids = FxHashMap::default();
        for (id, expression) in &hir.expressions {
            compile_expression(
                db,
                body,
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

        let return_value = body.current_return_value();
        body.push(Expression::ModuleEnds);
        body.push(Expression::Reference(return_value));
    })
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
fn generate_needs_function(body: &mut BodyBuilder) -> Id {
    body.push_lambda(|body, responsible_for_call| {
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
    body: &mut BodyBuilder,
    mapping: &mut FxHashMap<hir::Id, Id>,
    pattern_identifier_ids: &mut FxHashMap<hir::Id, FxHashMap<hir::PatternIdentifierId, Id>>,
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
            let responsible = body.push(Expression::HirId(hir_id.clone()));
            let expression = mapping[expression];

            let (pattern_result, identifier_ids) =
                PatternLoweringContext::compile_pattern(db, body, responsible, expression, pattern);

            // TODO: use pattern_result

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
        hir::Expression::Match { expression, cases } => {
            let expression = mapping[expression];
        }
        hir::Expression::Lambda(hir::Lambda {
            parameters: original_parameters,
            body: original_body,
            fuzzable,
        }) => {
            let lambda = body.push_lambda(|lambda, responsible_parameter| {
                for original_parameter in original_parameters {
                    let parameter = lambda.new_parameter();
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
                        lambda,
                        mapping,
                        pattern_identifier_ids,
                        needs_function,
                        responsible,
                        id,
                        expression,
                        tracing,
                    );
                }
            });

            if tracing.register_fuzzables.is_enabled() && *fuzzable {
                let hir_definition = body.push(Expression::HirId(hir_id.clone()));
                body.push(Expression::TraceFoundFuzzableClosure {
                    hir_definition,
                    closure: lambda,
                });
            }

            Expression::Reference(lambda)
        }
        hir::Expression::Call {
            function,
            arguments,
        } => {
            let responsible = body.push(Expression::HirId(hir_id.clone()));
            let arguments = arguments
                .iter()
                .map(|argument| mapping[argument])
                .collect_vec();

            if tracing.calls.is_enabled() {
                let hir_call = body.push(Expression::HirId(hir_id.clone()));
                body.push(Expression::TraceCallStarts {
                    hir_call,
                    function: mapping[function],
                    arguments: arguments.clone(),
                    responsible,
                });
            }
            let call = body.push(Expression::Call {
                function: mapping[function],
                arguments,
                responsible,
            });
            if tracing.calls.is_enabled() {
                body.push(Expression::TraceCallEnds { return_value: call });
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
            let responsible = body.push(Expression::HirId(hir_id.clone()));
            Expression::Call {
                function: needs_function,
                arguments: vec![mapping[condition], mapping[reason], responsible_for_needs],
                responsible,
            }
        }
        hir::Expression::Error { errors, .. } => {
            let responsible = body.push(Expression::HirId(hir_id.clone()));
            compile_errors(db, body, responsible, errors)
        }
    };

    let id = body.push(expression);
    mapping.insert(hir_id.clone(), id);

    if tracing.evaluated_expressions.is_enabled() {
        let hir_expression = body.push(Expression::HirId(hir_id.clone()));
        body.push(Expression::TraceExpressionEvaluated {
            hir_expression,
            value: id,
        });
        body.push(Expression::Reference(id));
    }
}

struct PatternLoweringContext<'a> {
    db: &'a dyn HirToMir,
    body: &'a mut BodyBuilder,
    responsible: Id,
}
impl<'a> PatternLoweringContext<'a> {
    fn compile_pattern(
        db: &'a dyn HirToMir,
        body: &'a mut BodyBuilder,
        responsible: Id,
        expression: Id,
        pattern: &hir::Pattern,
    ) -> Id {
        let mut context = PatternLoweringContext {
            db,
            body,
            responsible,
        };
        context.compile(expression, pattern)
    }

    fn compile(&mut self, expression: Id, pattern: &hir::Pattern) -> Id {
        match pattern {
            hir::Pattern::NewIdentifier(identifier_id) => self.push_match(vec![expression]),
            hir::Pattern::Int(int) => {
                self.compile_exact_value(expression, Expression::Int(int.to_owned().into()))
            }
            hir::Pattern::Text(text) => {
                self.compile_exact_value(expression, Expression::Text(text.to_owned()))
            }
            hir::Pattern::Symbol(symbol) => {
                self.compile_exact_value(expression, Expression::Symbol(symbol.to_owned()))
            }
            hir::Pattern::List(list) => {
                // Check that it's a list.
                self.compile_verify_type(expression, "List".to_string());

                // Check that the length is correct.
                let builtin_list_length = self
                    .body
                    .push(Expression::Builtin(BuiltinFunction::ListLength));
                let actual_length = self.body.push(Expression::Call {
                    function: builtin_list_length,
                    arguments: vec![expression],
                    responsible: self.responsible,
                });
                self.compile_equals(
                    Expression::Int(list.len().into()),
                    actual_length,
                    |body, _expected, actual| {
                        vec![
                            body.push(Expression::Text(format!(
                                "Expected {} {}, got ",
                                list.len(),
                                if list.len() == 1 { "item" } else { "items" },
                            ))),
                            actual,
                            body.push(Expression::Text(".".to_string())),
                        ]
                    },
                );

                // Destructure the elements.
                let builtin_list_get = self
                    .body
                    .push(Expression::Builtin(BuiltinFunction::ListGet));
                for (index, pattern) in list.iter().enumerate() {
                    let index = self.body.push(Expression::Int(index.into()));
                    let item = self.body.push(Expression::Call {
                        function: builtin_list_get,
                        arguments: vec![expression, index],
                        responsible: self.responsible,
                    });
                    self.compile_destructure(item, pattern);
                }
            }
            hir::Pattern::Struct(struct_) => {
                // Check that it's a struct.
                self.compile_verify_type(expression, "Struct".to_string());

                // Destructure the elements.
                let builtin_struct_get = self
                    .body
                    .push(Expression::Builtin(BuiltinFunction::StructGet));
                for (key_pattern, value_pattern) in struct_ {
                    let key = self.compile_pattern_to_key_expression(key_pattern);
                    let item = self.body.push(Expression::Call {
                        function: builtin_struct_get,
                        arguments: vec![expression, key],
                        responsible: self.responsible,
                    });
                    self.compile_destructure(item, value_pattern);
                }
            }
            hir::Pattern::Error { child, errors } => {
                compile_errors(self.db, self.body, self.responsible, errors);
                if let Some(child) = child {
                    // We still have to populate `identifier_ids`.
                    self.compile_destructure(expression, child);
                }
            }
        }
    }
    fn compile_exact_value(&mut self, expression: Id, expected_value: Expression) -> Id {
        self.compile_equals(expected_value, expression, |body, expected, actual| {
            vec![
                body.push(Expression::Text("Expected `".to_string())),
                expected,
                body.push(Expression::Text("`, got `".to_string())),
                actual,
                body.push(Expression::Text("`.".to_string())),
            ]
        })
    }
    fn compile_verify_type(&mut self, expression: Id, expected_type: String) -> Id {
        let builtin_type_of = self.body.push(Expression::Builtin(BuiltinFunction::TypeOf));
        let type_ = self.body.push(Expression::Call {
            function: builtin_type_of,
            arguments: vec![expression],
            responsible: self.responsible,
        });

        self.compile_equals(
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
        )
    }
    fn compile_equals<R>(
        &mut self,
        expected_value: Expression,
        actual_value: Id,
        reason_factory: R,
    ) -> Id
    where
        R: FnOnce(&mut BodyBuilder, Id, Id) -> Vec<Id>,
    {
        let builtin_equals = self.body.push(Expression::Builtin(BuiltinFunction::Equals));
        let expected_value = self.body.push(expected_value);
        let equals = self.body.push(Expression::Call {
            function: builtin_equals,
            arguments: vec![expected_value, actual_value],
            responsible: self.responsible,
        });

        let on_match = self.body.push_lambda(|body, _| {
            self.push_match(vec![]);
        });

        let on_no_match = self.body.push_lambda(|body, _| {
            let to_debug_text = body.push(Expression::Builtin(BuiltinFunction::ToDebugText));

            let expected_as_text = body.push(Expression::Call {
                function: to_debug_text,
                arguments: vec![expected_value],
                responsible: self.responsible,
            });

            let actual_as_text = body.push(Expression::Call {
                function: to_debug_text,
                arguments: vec![actual_value],
                responsible: self.responsible,
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
                        responsible: self.responsible,
                    })
                })
                .unwrap();

            self.push_no_match(reason);
        });

        let builtin_if_else = self.body.push(Expression::Builtin(BuiltinFunction::IfElse));
        self.body.push(Expression::Call {
            function: builtin_if_else,
            arguments: vec![equals, on_match, on_no_match],
            responsible: self.responsible,
        })
    }
    fn compile_pattern_to_key_expression(&mut self, pattern: &hir::Pattern) -> Id {
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
                compile_errors(self.db, &mut self.body, self.responsible, errors)
            }
        };
        self.body.push(expression)
    }

    fn push_match(&mut self, mut captured_identifiers: Vec<Id>) -> Id {
        // TODO: Return `Match (…)` instead of `(Match, …)` when we have tags.
        let match_ = self.body.push(Expression::Symbol("Match".to_string()));
        captured_identifiers.insert(0, match_);
        self.body.push(Expression::List(captured_identifiers))
    }
    fn push_no_match(&mut self, reason_text: Id) -> Id {
        // TODO: Return `NoMatch reasonAsText` instead of `(NoMatch, reasonAsText)` when we have tags.
        let no_match = self.body.push(Expression::Symbol("NoMatch".to_string()));
        self.body
            .push(Expression::List(vec![no_match, reason_text]))
    }
}

// Errors

#[must_use]
fn compile_errors(
    db: &dyn HirToMir,
    body: &mut BodyBuilder,
    responsible: Id,
    errors: &Vec<CompilerError>,
) -> Expression {
    let reason = body.push(Expression::Text(if errors.len() == 1 {
        format!(
            "The code still contains an error: {}",
            errors.iter().next().unwrap().format_nicely(db),
        )
    } else {
        format!(
            "The code still contains errors:\n{}",
            errors
                .iter()
                .map(|error| format!("- {}", error.format_nicely(db)))
                .join("\n"),
        )
    }));
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
