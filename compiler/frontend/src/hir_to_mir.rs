use super::{
    ast_to_hir::AstToHir,
    cst::CstDb,
    error::CompilerError,
    hir,
    mir::{BodyBuilder, Expression, Id, Mir},
    tracing::TracingConfig,
};
use crate::{
    builtin_functions::BuiltinFunction,
    module::{Module, ModuleKind},
    position::PositionConversionDb,
    string_to_rcst::ModuleError,
};
use itertools::Itertools;
use linked_hash_map::LinkedHashMap;
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;

#[salsa::query_group(HirToMirStorage)]
pub trait HirToMir: PositionConversionDb + CstDb + AstToHir {
    fn mir(&self, module: Module, tracing: TracingConfig) -> MirResult;
}

pub type MirResult = Result<(Arc<Mir>, Arc<FxHashSet<CompilerError>>), ModuleError>;

#[allow(clippy::needless_pass_by_value)]
fn mir(db: &dyn HirToMir, module: Module, tracing: TracingConfig) -> MirResult {
    let (mir, errors) = match module.kind {
        ModuleKind::Code => {
            let (hir, _) = db.hir(module.clone())?;
            let mut errors = FxHashSet::default();
            let mir = LoweringContext::compile_module(module, &hir, &tracing, &mut errors);
            (mir, errors)
        }
        ModuleKind::Asset => {
            let Some(bytes) = db.get_module_content(module) else {
                return Err(ModuleError::DoesNotExist);
            };
            (
                Mir::build(|body| {
                    let bytes = bytes
                        .iter()
                        .map(|&it| body.push_int(it.into()))
                        .collect_vec();
                    body.push_list(bytes);
                }),
                FxHashSet::default(),
            )
        }
    };
    Ok((Arc::new(mir), Arc::new(errors)))
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
    let needs_id = hir::Id::needs();
    body.push_function(needs_id.clone(), |body, responsible_for_call| {
        let condition = body.new_parameter();
        let reason = body.new_parameter();
        let responsible_for_condition = body.new_parameter();

        // Common stuff.
        let needs_code = body.push_hir_id(needs_id.clone());
        let builtin_equals = body.push_builtin(BuiltinFunction::Equals);
        let nothing_tag = body.push_nothing();

        // Make sure the condition is a bool.
        let true_tag = body.push_bool(true);
        let false_tag = body.push_bool(false);
        let is_condition_true =
            body.push_call(builtin_equals, vec![condition, true_tag], needs_code);
        let is_condition_bool = body.push_if_else(
            &needs_id.child("isConditionTrue"),
            is_condition_true,
            |body| {
                body.push_reference(true_tag);
            },
            |body| {
                body.push_call(builtin_equals, vec![condition, false_tag], needs_code);
            },
            needs_code,
        );
        body.push_if_else(
            &needs_id.child("isConditionBool"),
            is_condition_bool,
            |body| {
                body.push_reference(nothing_tag);
            },
            |body| {
                let panic_reason =
                    body.push_text("The `condition` must be either `True` or `False`.".to_string());
                body.push_panic(panic_reason, responsible_for_call);
            },
            needs_code,
        );

        // Make sure the reason is a text.
        let builtin_type_of = body.push_builtin(BuiltinFunction::TypeOf);
        let type_of_reason = body.push_call(builtin_type_of, vec![reason], responsible_for_call);
        let text_tag = body.push_tag("Text".to_string(), None);
        let is_reason_text = body.push_call(
            builtin_equals,
            vec![type_of_reason, text_tag],
            responsible_for_call,
        );
        body.push_if_else(
            &needs_id.child("isReasonText"),
            is_reason_text,
            |body| {
                body.push_reference(nothing_tag);
            },
            |body| {
                let panic_reason = body.push_text("The `reason` must be a text.".to_string());
                body.push_panic(panic_reason, responsible_for_call);
            },
            needs_code,
        );

        // The core logic of the needs.
        body.push_if_else(
            &needs_id.child("condition"),
            condition,
            |body| {
                body.push_reference(nothing_tag);
            },
            |body| {
                body.push_panic(reason, responsible_for_condition);
            },
            needs_code,
        );
    })
}

struct LoweringContext<'a> {
    mapping: &'a mut FxHashMap<hir::Id, Id>,
    needs_function: Id,
    tracing: &'a TracingConfig,
    ongoing_destructuring: Option<OngoingDestructuring>,
    errors: &'a mut FxHashSet<CompilerError>,
}
#[derive(Clone)]
struct OngoingDestructuring {
    result: Id,

    /// Assignments such as `foo = …` are considered trivial.
    is_trivial: bool,
}

impl<'a> LoweringContext<'a> {
    fn compile_module(
        module: Module,
        hir: &hir::Body,
        tracing: &TracingConfig,
        errors: &mut FxHashSet<CompilerError>,
    ) -> Mir {
        Mir::build(|body| {
            let mut mapping = FxHashMap::default();

            let needs_function = generate_needs_function(body);

            let module_hir_id = body.push_hir_id(hir::Id::new(module, vec![]));
            let mut context = LoweringContext {
                mapping: &mut mapping,
                needs_function,
                tracing,
                ongoing_destructuring: None,
                errors,
            };
            context.compile_expressions(body, module_hir_id, &hir.expressions);
        })
    }

    fn compile_expressions(
        &mut self,
        body: &mut BodyBuilder,
        responsible_for_needs: Id,
        expressions: &LinkedHashMap<hir::Id, hir::Expression>,
    ) {
        for (id, expression) in expressions {
            self.compile_expression(body, responsible_for_needs, id, expression);
        }
    }
    fn compile_expression(
        &mut self,
        body: &mut BodyBuilder,
        responsible_for_needs: Id,
        hir_id: &hir::Id,
        expression: &hir::Expression,
    ) {
        let id = match expression {
            hir::Expression::Int(int) => body.push_int(int.clone().into()),
            hir::Expression::Text(text) => body.push_text(text.clone()),
            hir::Expression::Reference(reference) => body.push_reference(self.mapping[reference]),
            hir::Expression::Symbol(symbol) => body.push_tag(symbol.clone(), None),
            hir::Expression::Builtin(builtin) => body.push_builtin(*builtin),
            hir::Expression::List(items) => {
                body.push_list(items.iter().map(|item| self.mapping[item]).collect())
            }
            hir::Expression::Struct(fields) => {
                let fields = fields
                    .iter()
                    .map(|(key, value)| (self.mapping[key], self.mapping[value]))
                    .collect();
                body.push_struct(fields)
            }
            hir::Expression::Destructure {
                expression,
                pattern,
            } => {
                let responsible = body.push_hir_id(hir_id.clone());
                let expression = self.mapping[expression];

                if let hir::Pattern::NewIdentifier(_) = pattern {
                    // The trivial case: `foo = …`.
                    let result = body.push_reference(expression);
                    self.ongoing_destructuring = Some(OngoingDestructuring {
                        result,
                        is_trivial: true,
                    });
                    result
                } else {
                    let pattern_result = PatternLoweringContext::compile_pattern(
                        body,
                        hir_id.clone(),
                        responsible,
                        expression,
                        pattern,
                    );
                    self.ongoing_destructuring = Some(OngoingDestructuring {
                        result: pattern_result,
                        is_trivial: false,
                    });

                    let nothing = body.push_nothing();
                    let is_match = body.push_is_match(pattern_result, responsible);
                    body.push_if_else(
                        &hir_id.child("isMatch"),
                        is_match,
                        |body| {
                            body.push_reference(nothing);
                        },
                        |body| {
                            let list_get_function = body.push_builtin(BuiltinFunction::ListGet);
                            let one = body.push_int(1.into());
                            let reason = body.push_call(
                                list_get_function,
                                vec![pattern_result, one],
                                responsible,
                            );

                            body.push_panic(reason, responsible);
                        },
                        responsible,
                    )
                }
            }
            hir::Expression::PatternIdentifierReference(identifier_id) => {
                let OngoingDestructuring { result, is_trivial } =
                    self.ongoing_destructuring.clone().unwrap();

                if is_trivial {
                    body.push_reference(result)
                } else {
                    let list_get = body.push_builtin(BuiltinFunction::ListGet);
                    let index = body.push_int((identifier_id.0 + 1).into());
                    let responsible = body.push_hir_id(hir_id.clone());
                    body.push_call(list_get, vec![result, index], responsible)
                }
            }
            hir::Expression::Match { expression, cases } => {
                assert!(!cases.is_empty());

                let responsible_for_match = body.push_hir_id(hir_id.clone());
                let expression = self.mapping[expression];
                self.compile_match(
                    hir_id.clone(),
                    body,
                    expression,
                    cases,
                    responsible_for_needs,
                    responsible_for_match,
                )
            }
            hir::Expression::Function(hir::Function {
                parameters: original_parameters,
                body: original_body,
                kind,
            }) => {
                let function =
                    body.push_function(hir_id.clone(), |function, responsible_parameter| {
                        for original_parameter in original_parameters {
                            let parameter = function.new_parameter();
                            self.mapping.insert(original_parameter.clone(), parameter);
                        }

                        let responsible = if kind.uses_own_responsibility() {
                            responsible_parameter
                        } else {
                            // This is a function with curly braces, so whoever is responsible
                            // for `needs` in the current scope is also responsible for
                            // `needs` in the function.
                            responsible_for_needs
                        };

                        self.compile_expressions(function, responsible, &original_body.expressions);
                    });

                if self.tracing.register_fuzzables.is_enabled() && kind.is_fuzzable() {
                    let hir_definition = body.push(Expression::HirId(hir_id.clone()));
                    body.push(Expression::TraceFoundFuzzableFunction {
                        hir_definition,
                        function,
                    });
                    body.push_reference(function)
                } else {
                    function
                }
            }
            hir::Expression::Call {
                function,
                arguments,
            } => {
                let responsible = body.push_hir_id(hir_id.clone());
                let arguments = arguments
                    .iter()
                    .map(|argument| self.mapping[argument])
                    .collect_vec();

                if self.tracing.calls.is_enabled() {
                    let hir_call = body.push_hir_id(hir_id.clone());
                    body.push(Expression::TraceCallStarts {
                        hir_call,
                        function: self.mapping[function],
                        arguments: arguments.clone(),
                        responsible,
                    });
                }
                let call = body.push_call(self.mapping[function], arguments, responsible);
                if self.tracing.calls.is_enabled() {
                    body.push(Expression::TraceCallEnds { return_value: call });
                    body.push_reference(call)
                } else {
                    call
                }
            }
            hir::Expression::UseModule {
                current_module,
                relative_path,
            } => body.push(Expression::UseModule {
                current_module: current_module.clone(),
                relative_path: self.mapping[relative_path],
                // The `UseModule` expression only exists in the generated `use`
                // function. If a use fails, that's also the fault of the caller.
                // Essentially, the `UseModule` expression works exactly like a
                // `needs`.
                responsible: responsible_for_needs,
            }),
            hir::Expression::Needs { condition, reason } => {
                let responsible = body.push_hir_id(hir_id.clone());
                body.push_call(
                    self.needs_function,
                    vec![
                        self.mapping[condition],
                        self.mapping[reason],
                        responsible_for_needs,
                    ],
                    responsible,
                )
            }
            hir::Expression::Error { errors, .. } => {
                self.errors.extend(errors.clone());
                let responsible = body.push_hir_id(hir_id.clone());
                body.compile_errors(responsible, errors)
            }
        };
        self.mapping.insert(hir_id.clone(), id);

        if self.tracing.evaluated_expressions.is_enabled() {
            let hir_expression = body.push_hir_id(hir_id.clone());
            body.push(Expression::TraceExpressionEvaluated {
                hir_expression,
                value: id,
            });
            body.push_reference(id);
        }
    }

    fn compile_match(
        &mut self,
        hir_id: hir::Id,
        body: &mut BodyBuilder,
        expression: Id,
        cases: &[(hir::Pattern, hir::Body)],
        responsible_for_needs: Id,
        responsible_for_match: Id,
    ) -> Id {
        self.compile_match_rec(
            hir_id,
            body,
            expression,
            cases,
            responsible_for_needs,
            responsible_for_match,
            vec![],
            0,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn compile_match_rec(
        &mut self,
        hir_id: hir::Id,
        body: &mut BodyBuilder,
        expression: Id,
        cases: &[(hir::Pattern, hir::Body)],
        responsible_for_needs: Id,
        responsible_for_match: Id,
        mut no_match_reasons: Vec<Id>,
        case_index: usize,
    ) -> Id {
        match cases {
            [] => {
                let reason = body.push_text("No case matched the given expression.".to_string());
                // TODO: concat reasons
                body.push_panic(reason, responsible_for_match)
            }
            [(case_pattern, case_body), rest @ ..] => {
                let pattern_result = PatternLoweringContext::compile_pattern(
                    body,
                    hir_id.clone(),
                    responsible_for_match,
                    expression,
                    case_pattern,
                );

                let is_match = body.push_is_match(pattern_result, responsible_for_match);

                let case_id = hir_id.child(format!("case-{case_index}"));
                let builtin_if_else = body.push_builtin(BuiltinFunction::IfElse);
                let then_function = body.push_function(case_id.child("matched"), |body, _| {
                    self.ongoing_destructuring = Some(OngoingDestructuring {
                        result: pattern_result,
                        is_trivial: false,
                    });
                    self.compile_expressions(body, responsible_for_needs, &case_body.expressions);
                });
                let else_function = body.push_function(case_id.child("didNotMatch"), |body, _| {
                    let list_get_function = body.push_builtin(BuiltinFunction::ListGet);
                    let one = body.push_int(1.into());
                    let reason = body.push_call(
                        list_get_function,
                        vec![pattern_result, one],
                        responsible_for_match,
                    );
                    no_match_reasons.push(reason);

                    self.compile_match_rec(
                        hir_id,
                        body,
                        expression,
                        rest,
                        responsible_for_needs,
                        responsible_for_match,
                        no_match_reasons,
                        case_index + 1,
                    );
                });
                body.push_call(
                    builtin_if_else,
                    vec![is_match, then_function, else_function],
                    responsible_for_match,
                )
            }
        }
    }
}

struct PatternLoweringContext {
    hir_id: hir::Id,
    match_tag: Id,
    no_match_tag: Id,
    responsible: Id,
}
impl PatternLoweringContext {
    /// Checks a pattern and returns an expression of type
    /// `(Match, variable0, …, variableN) | (NoMatch, reasonText)`.
    fn compile_pattern(
        body: &mut BodyBuilder,
        hir_id: hir::Id,
        responsible: Id,
        expression: Id,
        pattern: &hir::Pattern,
    ) -> Id {
        let match_tag = body.push_match_tag();
        let no_match_tag = body.push_no_match_tag();
        let context = Self {
            hir_id,
            match_tag,
            no_match_tag,
            responsible,
        };
        context.compile(body, expression, pattern)
    }

    fn compile(&self, body: &mut BodyBuilder, expression: Id, pattern: &hir::Pattern) -> Id {
        match pattern {
            hir::Pattern::NewIdentifier(_) => self.push_match(body, vec![expression]),
            hir::Pattern::Int(int) => {
                let expected = body.push_int(int.clone().into());
                self.compile_exact_value(body, expression, expected)
            }
            hir::Pattern::Text(text) => {
                let expected = body.push_text(text.clone());
                self.compile_exact_value(body, expression, expected)
            }
            hir::Pattern::Tag { symbol, value } => {
                self.compile_verify_type_condition(body, expression, "Tag".to_string(), |body| {
                    let builtin_tag_without_value =
                        body.push_builtin(BuiltinFunction::TagWithoutValue);
                    let actual_symbol = body.push_call(
                        builtin_tag_without_value,
                        vec![expression],
                        self.responsible,
                    );
                    let expected_symbol = body.push_tag(symbol.clone(), None);
                    self.compile_equals(body, expected_symbol, actual_symbol, |body| {
                        let builtin_tag_has_value = body.push_builtin(BuiltinFunction::TagHasValue);
                        let actual_has_value = body.push_call(builtin_tag_has_value, vec![expression], self.responsible);
                        let expected_has_value = body.push_bool(value.is_some());
                        self.compile_equals(body, expected_has_value, actual_has_value, |body| {
                            if let Some(value) = value {
                                let builtin_tag_get_value = body.push_builtin(BuiltinFunction::TagGetValue);
                                let actual_value = body.push_call(builtin_tag_get_value, vec![expression], self.responsible);
                                self.compile(body, actual_value, value);
                            } else {
                                self.push_match(body, vec![]);
                            }
                        }, |body, _, _| {
                            if value.is_some() {
                                vec![
                                    body.push_text("Expected tag to have a value, but it doesn't have any.".to_string()),
                                ]
                            } else {
                                let builtin_tag_get_value = body.push_builtin(BuiltinFunction::TagGetValue);
                                let actual_value = body.push_call(builtin_tag_get_value, vec![expression], self.responsible);
                                let builtin_to_debug_text = body.push_builtin(BuiltinFunction::ToDebugText);
                                let actual_value_text = body.push_call(builtin_to_debug_text, vec![actual_value], self.responsible);
                                vec![
                                    body.push_text("Expected tag to not have a value, but it has one: `".to_string()),
                                    actual_value_text,
                                    body.push_text("`.".to_string()),
                                ]
                            }
                        });

                    }, |body, expected, actual| {
                        vec![
                            body.push_text(
                                "Expected ".to_string()
                            ),
                            expected,
                            body.push_text(", got ".to_string()),
                            actual,
                            body.push_text(".".to_string()),
                        ]
                    });
                })
            }
            hir::Pattern::List(list) => {
                // Check that it's a list.
                self.compile_verify_type_condition(body, expression, "List".to_string(), |body| {
                    // Check that the length is correct.
                    let expected = body.push_int(list.len().into());
                    let builtin_list_length = body.push_builtin(BuiltinFunction::ListLength);
                    let actual_length =
                        body.push_call(builtin_list_length, vec![expression], self.responsible);
                    self.compile_equals(
                        body,
                        expected,
                        actual_length,
                        |body| {
                            // Destructure the items.
                            let builtin_list_get = body.push_builtin(BuiltinFunction::ListGet);
                            let condition_builders = list
                                .iter()
                                .enumerate()
                                .map(|(index, item_pattern)| {
                                    move |body: &mut BodyBuilder| {
                                        let index = body.push_int(index.into());
                                        let item = body.push_call(
                                            builtin_list_get,
                                            vec![expression, index],
                                            self.responsible,
                                        );
                                        let result = self.compile(body, item, item_pattern);
                                        (result, item_pattern.captured_identifier_count())
                                    }
                                })
                                .collect_vec();
                            self.compile_match_conjunction(body, condition_builders);
                        },
                        |body, _expected, actual| {
                            vec![
                                body.push_text(format!(
                                    "Expected {} {}, got ",
                                    list.len(),
                                    if list.len() == 1 { "item" } else { "items" },
                                )),
                                actual,
                                body.push_text(".".to_string()),
                            ]
                        },
                    );
                })
            }
            hir::Pattern::Struct(struct_) => {
                // Check that it's a struct.
                self.compile_verify_type_condition(body, expression, "Struct".to_string(), |body| {
                    // Destructure the entries.
                    let builtin_struct_has_key = body.push_builtin(BuiltinFunction::StructHasKey);
                    let builtin_struct_get = body.push_builtin(BuiltinFunction::StructGet);
                    let condition_builders = struct_
                        .iter()
                        .map(|(key_pattern, value_pattern)| {
                            |body: &mut BodyBuilder| {
                                let key = self.compile_pattern_to_key_expression(body, key_pattern);
                                let has_key = body.push_call(
                                    builtin_struct_has_key,
                                    vec![expression, key],
                                    self.responsible,
                                );

                                let result = body.push_if_else(
                                    &self.hir_id.child("hasKey"),
                                    has_key,
                                    |body| {
                                        let value = body.push_call(
                                            builtin_struct_get,
                                            vec![expression, key],
                                            self.responsible,
                                        );
                                        self.compile(body, value, value_pattern);
                                    },
                                    |body| {
                                        let to_debug_text =
                                            body.push_builtin(BuiltinFunction::ToDebugText);

                                        let key_as_text = body.push_call(
                                            to_debug_text,
                                            vec![key],
                                            self.responsible,
                                        );

                                        let struct_as_text = body.push_call(
                                            to_debug_text,
                                            vec![expression],
                                            self.responsible,
                                        );

                                        let reason_parts = vec![
                                            body.push_text(
                                                "Struct doesn't contain key `".to_string(),
                                            ),
                                            key_as_text,
                                            body.push_text("`: `".to_string()),
                                            struct_as_text,
                                            body.push_text("`.".to_string()),
                                        ];
                                        let reason_text =
                                            self.push_text_concatenate(body, reason_parts);
                                        self.push_no_match(body, reason_text);
                                    },
                                    self.responsible,
                                );
                                (result, value_pattern.captured_identifier_count())
                            }
                        })
                        .collect_vec();
                    self.compile_match_conjunction(body, condition_builders);
                })
            }
            hir::Pattern::Or(patterns) => {
                let [first_pattern, rest_patterns @ ..] = patterns.as_slice() else {
                    panic!("Or pattern must contain at least two patterns.");
                };

                let mut result = self.compile(body, expression, first_pattern);

                let captured_identifiers_order = first_pattern.captured_identifiers();
                let list_get_function = body.push_builtin(BuiltinFunction::ListGet);
                let nothing = body.push_nothing();

                for pattern in rest_patterns {
                    let is_match = body.push_is_match(result, self.responsible);
                    result = body.push_if_else(
                        &self.hir_id.child("isMatch"),
                        is_match,
                        |body| {
                            let captured_identifiers = pattern.captured_identifiers();
                            if captured_identifiers == captured_identifiers_order {
                                body.push_reference(result);
                                return;
                            }

                            let captured_identifiers = captured_identifiers_order
                                .iter()
                                .map(|identifier_id| {
                                    let index = captured_identifiers
                                        .iter()
                                        .position(|it| it == identifier_id);
                                    let Some(index) = index else { return body.push_reference(nothing); };

                                    let index = body.push_int((1 + index).into());
                                    body.push_call(list_get_function, vec![result, index], self.responsible)
                                })
                                .collect();
                            self.push_match(body, captured_identifiers);
                        },
                        |body| {
                            self.compile(body, expression, pattern);
                        },
                        self.responsible,
                    );
                }
                result
            }
            hir::Pattern::Error { errors } => {
                body.compile_errors(self.responsible, errors)
            }
        }
    }

    fn compile_exact_value(
        &self,
        body: &mut BodyBuilder,
        expression: Id,
        expected_value: Id,
    ) -> Id {
        self.compile_equals(
            body,
            expected_value,
            expression,
            |body| {
                self.push_match(body, vec![]);
            },
            |body, expected, actual| {
                vec![
                    body.push_text("Expected `".to_string()),
                    expected,
                    body.push_text("`, got `".to_string()),
                    actual,
                    body.push_text("`.".to_string()),
                ]
            },
        )
    }
    fn compile_verify_type_condition<T>(
        &self,
        body: &mut BodyBuilder,
        expression: Id,
        expected_type: String,
        then_builder: T,
    ) -> Id
    where
        T: FnOnce(&mut BodyBuilder),
    {
        let expected_type = body.push_tag(expected_type, None);
        let builtin_type_of = body.push_builtin(BuiltinFunction::TypeOf);
        let type_ = body.push_call(builtin_type_of, vec![expression], self.responsible);
        self.compile_equals(
            body,
            expected_type,
            type_,
            then_builder,
            |body, expected, actual| {
                vec![
                    body.push_text("Expected a ".to_string()),
                    expected,
                    body.push_text(", got `".to_string()),
                    actual,
                    body.push_text("`.".to_string()),
                ]
            },
        )
    }

    fn compile_equals<T, E>(
        &self,
        body: &mut BodyBuilder,
        expected: Id,
        actual: Id,
        then_builder: T,
        reason_factory: E,
    ) -> Id
    where
        T: FnOnce(&mut BodyBuilder),
        E: FnOnce(&mut BodyBuilder, Id, Id) -> Vec<Id>,
    {
        let builtin_equals = body.push_builtin(BuiltinFunction::Equals);
        let equals = body.push_call(builtin_equals, vec![expected, actual], self.responsible);

        body.push_if_else(
            &self.hir_id.child("equals"),
            equals,
            then_builder,
            |body| {
                let to_debug_text = body.push_builtin(BuiltinFunction::ToDebugText);
                let expected_as_text =
                    body.push_call(to_debug_text, vec![expected], self.responsible);
                let actual_as_text = body.push_call(to_debug_text, vec![actual], self.responsible);
                let reason_parts = reason_factory(body, expected_as_text, actual_as_text);
                let reason = self.push_text_concatenate(body, reason_parts);
                self.push_no_match(body, reason);
            },
            self.responsible,
        )
    }
    fn compile_pattern_to_key_expression(
        &self,
        body: &mut BodyBuilder,
        pattern: &hir::Pattern,
    ) -> Id {
        match pattern {
            hir::Pattern::NewIdentifier(_) => {
                panic!("New identifiers can't be used in this part of a pattern.")
            }
            hir::Pattern::Int(int) => body.push_int(int.clone().into()),
            hir::Pattern::Text(text) => body.push_text(text.clone()),
            hir::Pattern::Tag { symbol, value } => {
                let value = value
                    .as_ref()
                    .map(|value| self.compile_pattern_to_key_expression(body, value));
                body.push_tag(symbol.to_string(), value)
            }
            hir::Pattern::List(_) => panic!("Lists can't be used in this part of a pattern."),
            hir::Pattern::Struct(_) => panic!("Structs can't be used in this part of a pattern."),
            hir::Pattern::Or(_) => panic!("Or-patterns can't be used in this part of a pattern."),
            hir::Pattern::Error { errors, .. } => body.compile_errors(self.responsible, errors),
        }
    }

    fn compile_match_conjunction<F>(&self, body: &mut BodyBuilder, condition_builders: Vec<F>) -> Id
    where
        F: FnMut(&mut BodyBuilder) -> (Id, usize),
    {
        self.compile_match_conjunction_rec(body, condition_builders, vec![])
    }
    fn compile_match_conjunction_rec<F>(
        &self,
        body: &mut BodyBuilder,
        mut condition_builders: Vec<F>,
        mut captured_identifiers: Vec<Id>,
    ) -> Id
    where
        F: FnMut(&mut BodyBuilder) -> (Id, usize),
    {
        if condition_builders.is_empty() {
            return self.push_match(body, captured_identifiers);
        };

        let mut condition_builder = condition_builders.remove(0);
        let (return_value, captured_identifier_count) = condition_builder(body);

        let is_match = body.push_is_match(return_value, self.responsible);
        body.push_if_else(
            &self.hir_id.child("isMatch"),
            is_match,
            |body| {
                let list_get_function = body.push_builtin(BuiltinFunction::ListGet);
                for index in 0..captured_identifier_count {
                    let index = body.push_int((index + 1).into());
                    let captured_identifier = body.push_call(
                        list_get_function,
                        vec![return_value, index],
                        self.responsible,
                    );
                    captured_identifiers.push(captured_identifier);
                }
                self.compile_match_conjunction_rec(body, condition_builders, captured_identifiers);
            },
            |body| {
                body.push_reference(return_value);
            },
            self.responsible,
        )
    }

    fn push_text_concatenate(&self, body: &mut BodyBuilder, parts: Vec<Id>) -> Id {
        assert!(!parts.is_empty());

        let builtin_text_concatenate = body.push_builtin(BuiltinFunction::TextConcatenate);
        parts
            .into_iter()
            .reduce(|left, right| {
                body.push_call(
                    builtin_text_concatenate,
                    vec![left, right],
                    self.responsible,
                )
            })
            .unwrap()
    }

    fn push_match(&self, body: &mut BodyBuilder, mut captured_identifiers: Vec<Id>) -> Id {
        captured_identifiers.insert(0, self.match_tag);
        body.push_list(captured_identifiers)
    }
    fn push_no_match(&self, body: &mut BodyBuilder, reason_text: Id) -> Id {
        let no_match_tag = self.no_match_tag;
        body.push_list(vec![no_match_tag, reason_text])
    }
}

impl BodyBuilder {
    fn push_match_tag(&mut self) -> Id {
        self.push_tag("Match".to_string(), None)
    }
    fn push_no_match_tag(&mut self) -> Id {
        self.push_tag("NoMatch".to_string(), None)
    }

    /// Compiles to code taking a `(Match, …)` or `(NoMatch, …)` and returning a
    /// boolean.
    fn push_is_match(&mut self, match_or_no_match: Id, responsible: Id) -> Id {
        let list_get_function = self.push_builtin(BuiltinFunction::ListGet);
        let zero = self.push_int(0.into());
        let match_or_no_match_tag = self.push_call(
            list_get_function,
            vec![match_or_no_match, zero],
            responsible,
        );

        let equals_function = self.push_builtin(BuiltinFunction::Equals);
        let match_tag = self.push_match_tag();
        self.push_call(
            equals_function,
            vec![match_or_no_match_tag, match_tag],
            responsible,
        )
    }

    fn compile_errors(&mut self, responsible: Id, errors: &[CompilerError]) -> Id {
        let reason = errors
            .iter()
            .map(|error| format!("{}", error.payload))
            .join("\n");
        let reason = self.push_text(reason);
        self.push_panic(reason, responsible)
    }
}
