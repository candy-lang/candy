use super::{
    hint::{Hint, HintKind},
    static_panics::{StaticPanicToDiagnostic, StaticPanicsOfMir},
    utils::IdToEndOfLine,
};
use crate::{database::Database, server::AnalyzerClient, utils::LspPositionConversion};
use candy_frontend::{
    ast::{Assignment, AssignmentBody, AstDb, AstKind},
    ast_to_hir::AstToHir,
    hir::{Expression, HirDb},
    mir_optimize::OptimizeMir,
    module::Module,
    TracingConfig, TracingMode,
};
use candy_fuzzer::{FuzzablesFinder, Fuzzer, Status};
use candy_vm::{
    execution_controller::RunLimitedNumberOfInstructions,
    fiber::{EndedReason, Panic, VmEnded},
    lir::Lir,
    mir_to_lir::compile_lir,
    tracer::{
        evaluated_values::EvaluatedValuesTracer,
        stack_trace::{Call, StackTracer},
    },
    vm::{self, Vm},
};
use itertools::Itertools;
use lsp_types::{Diagnostic, DiagnosticSeverity, Range};
use rand::{prelude::SliceRandom, thread_rng};
use std::sync::Arc;

/// A hints finder is responsible for finding hints for a single module.
pub struct HintsFinder {
    module: Module,
    state: Option<State>, // only None during state transition
}
enum State {
    Initial,
    /// First, we run the module with tracing of evaluated expressions enabled.
    /// This enables us to show hints for constants.
    EvaluateConstants {
        static_panics: Vec<Panic>,
        tracer: (StackTracer, EvaluatedValuesTracer),
        vm: Vm<Lir, (StackTracer, EvaluatedValuesTracer)>,
    },
    /// Next, we run the module again to finds fuzzable functions. This time, we
    /// disable tracing of evaluated expressions, but we enable registration of
    /// fuzzable functions. Thus, the found functions to fuzz have the most
    /// efficient LIR possible.
    FindFuzzables {
        static_panics: Vec<Panic>,
        constants_ended: VmEnded,
        stack_tracer: StackTracer,
        evaluated_values: EvaluatedValuesTracer,
        lir: Arc<Lir>,
        tracer: FuzzablesFinder,
        vm: Vm<Arc<Lir>, FuzzablesFinder>,
    },
    /// Then, the functions are actually fuzzed.
    Fuzz {
        static_panics: Vec<Panic>,
        constants_ended: VmEnded,
        stack_tracer: StackTracer,
        evaluated_values: EvaluatedValuesTracer,
        fuzzable_finder_ended: VmEnded,
        fuzzers: Vec<Fuzzer>,
    },
}

impl HintsFinder {
    pub fn for_module(module: Module) -> Self {
        Self {
            module,
            state: Some(State::Initial),
        }
    }
    pub fn module_changed(&mut self) {
        // PERF: Save some incremental state.
        self.state = Some(State::Initial);
    }

    pub async fn run(&mut self, db: &Database, client: &AnalyzerClient) {
        let state = self.state.take().unwrap();
        let state = self.update_state(db, client, state).await;
        self.state = Some(state);
    }
    async fn update_state(&self, db: &Database, client: &AnalyzerClient, state: State) -> State {
        match state {
            State::Initial => {
                client
                    .update_status(Some(format!("Compiling {}", self.module)))
                    .await;

                let (mir, _, _, _) = db
                    .optimized_mir(
                        self.module.clone(),
                        TracingConfig {
                            register_fuzzables: TracingMode::OnlyCurrent,
                            calls: TracingMode::Off,
                            evaluated_expressions: TracingMode::Off,
                        },
                    )
                    .unwrap();
                let mut mir = (*mir).clone();
                let mut static_panics = mir.static_panics();
                static_panics.retain(|panic| panic.responsible.module == self.module);

                let tracing = TracingConfig {
                    register_fuzzables: TracingMode::Off,
                    calls: TracingMode::Off,
                    evaluated_expressions: TracingMode::OnlyCurrent,
                };
                let (lir, _) = compile_lir(db, self.module.clone(), tracing);

                let mut tracer = (
                    StackTracer::default(),
                    EvaluatedValuesTracer::new(self.module.clone()),
                );
                let vm = Vm::for_module(lir, &mut tracer);

                State::EvaluateConstants {
                    static_panics,
                    tracer,
                    vm,
                }
            }
            State::EvaluateConstants {
                static_panics,
                mut tracer,
                mut vm,
            } => {
                client
                    .update_status(Some(format!("Evaluating {}", self.module)))
                    .await;

                vm.run(&mut RunLimitedNumberOfInstructions::new(500), &mut tracer);
                if !matches!(vm.status(), vm::Status::Done | vm::Status::Panicked(_)) {
                    return State::EvaluateConstants {
                        static_panics,
                        tracer,
                        vm,
                    };
                }

                let constants_ended = vm.tear_down(&mut tracer);
                let (stack_tracer, evaluated_values) = tracer;

                let tracing = TracingConfig {
                    register_fuzzables: TracingMode::OnlyCurrent,
                    calls: TracingMode::Off,
                    evaluated_expressions: TracingMode::Off,
                };
                let (lir, _) = compile_lir(db, self.module.clone(), tracing);
                let lir = Arc::new(lir);

                let mut tracer = FuzzablesFinder::default();
                let vm = Vm::for_module(lir.clone(), &mut tracer);
                State::FindFuzzables {
                    static_panics,
                    constants_ended,
                    stack_tracer,
                    evaluated_values,
                    lir,
                    tracer,
                    vm,
                }
            }
            State::FindFuzzables {
                static_panics,
                constants_ended,
                stack_tracer,
                evaluated_values,
                lir,
                mut tracer,
                mut vm,
            } => {
                client
                    .update_status(Some(format!("Evaluating {}", self.module)))
                    .await;

                vm.run(&mut RunLimitedNumberOfInstructions::new(500), &mut tracer);
                if !matches!(vm.status(), vm::Status::Done | vm::Status::Panicked(_)) {
                    return State::FindFuzzables {
                        static_panics,
                        constants_ended,
                        stack_tracer,
                        evaluated_values,
                        lir,
                        tracer,
                        vm,
                    };
                }

                let fuzzable_finder_ended = vm.tear_down(&mut tracer);
                let fuzzers = tracer
                    .into_fuzzables()
                    .iter()
                    .map(|(id, function)| Fuzzer::new(lir.clone(), *function, id.clone()))
                    .collect();
                State::Fuzz {
                    static_panics,
                    constants_ended,
                    stack_tracer,
                    evaluated_values,
                    fuzzable_finder_ended,
                    fuzzers,
                }
            }
            State::Fuzz {
                static_panics,
                constants_ended,
                stack_tracer,
                evaluated_values,
                fuzzable_finder_ended,
                mut fuzzers,
            } => {
                let mut running_fuzzers = fuzzers
                    .iter_mut()
                    .filter(|fuzzer| matches!(fuzzer.status(), Status::StillFuzzing { .. }))
                    .collect_vec();
                let Some(fuzzer) = running_fuzzers.choose_mut(&mut thread_rng()) else {
                    client.update_status(None).await;
                    return State::Fuzz { static_panics, constants_ended, stack_tracer, evaluated_values, fuzzable_finder_ended, fuzzers };
                };

                client
                    .update_status(Some(format!("Fuzzing {}", fuzzer.function_id)))
                    .await;

                fuzzer.run(&mut RunLimitedNumberOfInstructions::new(500));

                State::Fuzz {
                    static_panics,
                    constants_ended,
                    stack_tracer,
                    evaluated_values,
                    fuzzable_finder_ended,
                    fuzzers,
                }
            }
        }
    }

    pub fn hints(&self, db: &Database, module: &Module) -> (Vec<Hint>, Vec<Diagnostic>) {
        let mut hints = vec![];
        let mut diagnostics = vec![];

        match self.state.as_ref().unwrap() {
            State::Initial => {}
            State::EvaluateConstants { static_panics, .. }
            | State::FindFuzzables { static_panics, .. } => {
                // TODO: Show incremental constant evaluation hints.
                diagnostics.extend(
                    static_panics
                        .iter()
                        .map(|panic| panic.to_diagnostic(db, module)),
                );
            }
            State::Fuzz {
                static_panics,
                constants_ended,
                stack_tracer,
                evaluated_values,
                fuzzers,
                ..
            } => {
                diagnostics.extend(
                    static_panics
                        .iter()
                        .map(|panic| panic.to_diagnostic(db, module)),
                );

                // TODO: Think about how to highlight the responsible piece of code.
                if let EndedReason::Panicked(panic) = &constants_ended.reason
                    && let Some(hint) = panic_hint(db, module.clone(), stack_tracer, &panic.reason)
                {
                    hints.push(hint);
                }

                for (id, value) in evaluated_values.values() {
                    let Some(hir) = db.find_expression(id.clone()) else { continue; };
                    match hir {
                        Expression::Reference(_) => {
                            // Could be an assignment.
                            let Some(ast_id) = db.hir_to_ast_id(id.clone()) else { continue; };
                            let Some(ast) = db.find_ast(ast_id) else { continue; };
                            let AstKind::Assignment(Assignment { body, .. }) = &ast.kind else { continue; };
                            let creates_hint = match body {
                                AssignmentBody::Function { .. } => true,
                                AssignmentBody::Body { pattern, .. } => {
                                    matches!(pattern.kind, AstKind::Identifier(_))
                                }
                            };
                            if !creates_hint {
                                continue;
                            }

                            hints.push(Hint::like_comment(
                                HintKind::Value,
                                value.to_string(),
                                db.id_to_end_of_line(id.clone()).unwrap(),
                            ));
                        }
                        Expression::PatternIdentifierReference { .. } => {
                            let body = db.containing_body_of(id.clone());
                            let name = body.identifiers.get(id).unwrap();
                            hints.push(Hint::like_comment(
                                HintKind::Value,
                                format!("{name} = {value}"),
                                db.id_to_end_of_line(id.clone()).unwrap(),
                            ));
                        }
                        _ => {}
                    }
                }

                for fuzzer in fuzzers {
                    let Status::FoundPanic {
                        input,
                        panic,
                        ..
                    } = fuzzer.status() else { continue; };

                    let id = fuzzer.function_id.clone();
                    if !id.is_same_module_and_any_parent_of(&panic.responsible) {
                        // The function panics internally for an input, but it's
                        // the fault of another function that's called
                        // internally.
                        // TODO: The fuzz case should instead be highlighted in
                        // the used function directly. We don't do that right
                        // now because we assume the fuzzer will find the panic
                        // when fuzzing the faulty function, but we should save
                        // the panicking case (or something like that) in the
                        // future.
                        continue;
                    }
                    if db.hir_to_cst_id(id.clone()).is_none() {
                        panic!(
                            "It looks like the generated code {} is at fault for a panic.",
                            panic.responsible,
                        );
                    }

                    // TODO: In the future, re-run only the failing case with
                    // tracing enabled and also show the arguments to the failing
                    // function in the hint.
                    let call_span = db
                        .hir_id_to_display_span(panic.responsible.clone())
                        .unwrap();
                    diagnostics.push(Diagnostic {
                        range: Range::new(
                            db.offset_to_lsp_position(module.clone(), call_span.start),
                            db.offset_to_lsp_position(module.clone(), call_span.end),
                        ),
                        severity: Some(DiagnosticSeverity::ERROR),
                        code: None,
                        code_description: None,
                        source: None,
                        message: format!(
                            "For `{} {}`, this call panics: {}",
                            fuzzer.function_id.keys.last().unwrap(),
                            input
                                .arguments
                                .iter()
                                .map(|argument| format!("{argument}"))
                                .join(" "),
                            panic.reason
                        ),
                        related_information: None,
                        tags: None,
                        data: None,
                    });
                }
            }
        }

        (hints, diagnostics)
    }
}

fn panic_hint(db: &Database, module: Module, tracer: &StackTracer, reason: &str) -> Option<Hint> {
    // We want to show the hint at the last call site still inside the current
    // module. If there is no call site in this module, then the panic results
    // from a compiler error in a previous stage which is already reported.
    let stack = tracer.panic_chain().unwrap();

    // Find the last call in this module.
    let (
        Call {
            callee, arguments, ..
        },
        call_site,
    ) = stack
        .iter()
        .map(|call| (call, call.call_site.get().to_owned()))
        .find(|(_, call_site)| {
            // Make sure the entry comes from the same file and is not generated
            // code.
            call_site.module == module && db.hir_to_cst_id(call_site.to_owned()).is_some()
        })?;

    let call_info = format!(
        "{callee} {}",
        arguments.iter().map(|it| it.to_string()).join(" "),
    );

    Some(Hint::like_comment(
        HintKind::Panic,
        format!("Calling `{call_info}` panics: {reason}"),
        db.id_to_end_of_line(call_site)?,
    ))
}
