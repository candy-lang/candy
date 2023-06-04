use super::{
    hint::{Hint, HintKind},
    utils::IdToEndOfLine,
};
use crate::{
    database::Database,
    utils::{error_into_diagnostic, LspPositionConversion},
};
use candy_frontend::{
    ast::{Assignment, AssignmentBody, AstDb, AstKind},
    ast_to_hir::AstToHir,
    cst::CstDb,
    error::CompilerError,
    hir::{CollectErrors, Expression, HirDb},
    mir_optimize::OptimizeMir,
    module::Module,
    rich_ir::ToRichIr,
    TracingConfig, TracingMode,
};
use candy_fuzzer::{FuzzablesFinder, Fuzzer, Status};
use candy_vm::{
    execution_controller::RunLimitedNumberOfInstructions,
    fiber::{EndedReason, VmEnded},
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
use std::{future::Future, sync::Arc};

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
        errors: Vec<CompilerError>,
        tracer: (StackTracer, EvaluatedValuesTracer),
        vm: Vm<Lir, (StackTracer, EvaluatedValuesTracer)>,
    },
    /// Next, we run the module again to finds fuzzable functions. This time, we
    /// disable tracing of evaluated expressions, but we enable registration of
    /// fuzzable functions. Thus, the found functions to fuzz have the most
    /// efficient LIR possible.
    FindFuzzables {
        errors: Vec<CompilerError>,
        constants_ended: VmEnded,
        stack_tracer: StackTracer,
        evaluated_values: EvaluatedValuesTracer,
        lir: Arc<Lir>,
        tracer: FuzzablesFinder,
        vm: Vm<Arc<Lir>, FuzzablesFinder>,
    },
    /// Then, the functions are actually fuzzed.
    Fuzz {
        errors: Vec<CompilerError>,
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

    pub async fn run<F: Future<Output = ()>>(
        &mut self,
        db: &(impl CstDb + OptimizeMir),
        set_status: impl Fn(Option<String>) -> F,
    ) {
        let state = self.state.take().unwrap();
        let state = self.update_state(db, set_status, state).await;
        self.state = Some(state);
    }
    async fn update_state<F: Future<Output = ()>>(
        &self,
        db: &(impl CstDb + OptimizeMir),
        set_status: impl Fn(Option<String>) -> F,
        state: State,
    ) -> State {
        match state {
            State::Initial => {
                set_status(Some(format!("Compiling {}", self.module.to_rich_ir()))).await;

                let (hir, _) = db.hir(self.module.clone()).unwrap();
                let mut errors = vec![];
                hir.collect_errors(&mut errors);

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

                State::EvaluateConstants { errors, tracer, vm }
            }
            State::EvaluateConstants {
                errors,
                mut tracer,
                mut vm,
            } => {
                set_status(Some(format!("Evaluating {}", self.module.to_rich_ir()))).await;

                vm.run(&mut RunLimitedNumberOfInstructions::new(500), &mut tracer);
                if !matches!(vm.status(), vm::Status::Done | vm::Status::Panicked(_)) {
                    return State::EvaluateConstants { errors, tracer, vm };
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
                    errors,
                    constants_ended,
                    stack_tracer,
                    evaluated_values,
                    lir,
                    tracer,
                    vm,
                }
            }
            State::FindFuzzables {
                errors,
                constants_ended,
                stack_tracer,
                evaluated_values,
                lir,
                mut tracer,
                mut vm,
            } => {
                set_status(Some(format!("Evaluating {}", self.module.to_rich_ir()))).await;

                vm.run(&mut RunLimitedNumberOfInstructions::new(500), &mut tracer);
                if !matches!(vm.status(), vm::Status::Done | vm::Status::Panicked(_)) {
                    return State::FindFuzzables {
                        errors,
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
                    errors,
                    constants_ended,
                    stack_tracer,
                    evaluated_values,
                    fuzzable_finder_ended,
                    fuzzers,
                }
            }
            State::Fuzz {
                errors,
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
                    set_status(None).await;
                    return State::Fuzz { errors, constants_ended, stack_tracer, evaluated_values, fuzzable_finder_ended, fuzzers };
                };

                set_status(Some(format!("Fuzzing {}", fuzzer.function_id))).await;

                fuzzer.run(&mut RunLimitedNumberOfInstructions::new(500));

                State::Fuzz {
                    errors,
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
            State::EvaluateConstants { errors, .. } | State::FindFuzzables { errors, .. } => {
                // TODO: Show incremental constant evaluation hints.
                diagnostics.extend(
                    errors
                        .iter()
                        .map(|it| error_into_diagnostic(db, module.clone(), it)),
                );
            }
            State::Fuzz {
                errors,
                constants_ended,
                stack_tracer,
                evaluated_values,
                fuzzers,
                ..
            } => {
                diagnostics.extend(
                    errors
                        .iter()
                        .map(|it| error_into_diagnostic(db, module.clone(), it)),
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

        for hint in &mut hints {
            hint.ensure_leading_spaces_visible();
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
