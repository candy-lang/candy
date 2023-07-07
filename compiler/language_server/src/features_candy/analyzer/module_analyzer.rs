use super::{insights::Insight, static_panics::StaticPanicsOfMir};
use crate::{
    database::Database, features_candy::analyzer::insights::ErrorDiagnostic,
    server::AnalyzerClient, utils::LspPositionConversion,
};
use candy_frontend::{
    ast_to_hir::AstToHir, mir_optimize::OptimizeMir, module::Module, TracingConfig, TracingMode,
};
use candy_fuzzer::{FuzzablesFinder, Fuzzer, Status};
use candy_vm::{
    execution_controller::RunLimitedNumberOfInstructions,
    fiber::{Panic, VmEnded},
    lir::Lir,
    mir_to_lir::compile_lir,
    tracer::{evaluated_values::EvaluatedValuesTracer, stack_trace::StackTracer},
    vm::{self, Vm},
};
use extension_trait::extension_trait;
use itertools::Itertools;
use lsp_types::Diagnostic;
use rand::{prelude::SliceRandom, thread_rng};
use std::rc::Rc;
use tracing::info;

/// A hints finder is responsible for finding hints for a single module.
pub struct ModuleAnalyzer {
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
        lir: Rc<Lir>,
        tracer: FuzzablesFinder,
        vm: Vm<Rc<Lir>, FuzzablesFinder>,
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

impl ModuleAnalyzer {
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

                let (mir, _, _) = db
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
                let lir = Rc::new(lir);

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
                    return State::Fuzz {
                        static_panics,
                        constants_ended,
                        stack_tracer,
                        evaluated_values,
                        fuzzable_finder_ended,
                        fuzzers,
                    };
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

    pub fn insights(&self, db: &Database) -> Vec<Insight> {
        let mut insights = vec![];

        match self.state.as_ref().unwrap() {
            State::Initial => {}
            State::EvaluateConstants { static_panics, .. } => {
                // TODO: Show incremental constant evaluation hints.
                insights.extend(static_panics.to_insights(db, &self.module));
            }
            State::FindFuzzables {
                static_panics,
                evaluated_values,
                ..
            } => {
                insights.extend(static_panics.to_insights(db, &self.module));
                insights.extend(
                    evaluated_values
                        .values()
                        .iter()
                        .flat_map(|(id, value)| Insight::for_value(db, id.clone(), *value)),
                );
            }
            State::Fuzz {
                static_panics,
                evaluated_values,
                fuzzers,
                ..
            } => {
                insights.extend(static_panics.to_insights(db, &self.module));
                insights.extend(
                    evaluated_values
                        .values()
                        .iter()
                        .flat_map(|(id, value)| Insight::for_value(db, id.clone(), *value)),
                );

                for fuzzer in fuzzers {
                    insights.append(&mut Insight::for_fuzzer_status(db, fuzzer));

                    let Status::FoundPanic { input, panic, .. } = fuzzer.status() else {
                        continue;
                    };

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
                    insights.push(Insight::Diagnostic(Diagnostic::error(
                        db.range_to_lsp_range(self.module.clone(), call_span),
                        format!(
                            "For `{} {}`, this call panics: {}",
                            fuzzer.function_id.function_name(),
                            input
                                .arguments
                                .iter()
                                .map(|argument| format!("{argument}"))
                                .join(" "),
                            panic.reason,
                        ),
                    )));
                }
            }
        }

        info!("Insights: {insights:?}");

        insights
    }
}

#[extension_trait]
pub impl StaticPanics for Vec<Panic> {
    fn to_insights(&self, db: &Database, module: &Module) -> Vec<Insight> {
        self.iter()
            .map(|panic| Insight::for_static_panic(db, module.clone(), panic))
            .collect_vec()
    }
}
