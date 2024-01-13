use super::{insights::Insight, static_panics::StaticPanicsOfMir};
use crate::{
    database::Database, features_candy::analyzer::insights::ErrorDiagnostic,
    server::AnalyzerClient, utils::LspPositionConversion,
};
use candy_frontend::{
    ast_to_hir::AstToHir,
    format::{MaxLength, Precedence},
    hir_to_mir::ExecutionTarget,
    mir_optimize::OptimizeMir,
    module::Module,
    tracing::CallTracingMode,
    TracingConfig, TracingMode,
};
use candy_fuzzer::{FuzzablesFinder, Fuzzer, Status};
use candy_vm::{
    byte_code::ByteCode,
    environment::StateAfterRunWithoutHandles,
    heap::{Heap, ToDebugText},
    lir_to_byte_code::compile_byte_code,
    tracer::{evaluated_values::EvaluatedValuesTracer, stack_trace::StackTracer},
    Panic, Vm, VmFinished,
};
use extension_trait::extension_trait;
use itertools::Itertools;
use lsp_types::Diagnostic;
use rand::{prelude::SliceRandom, thread_rng};
use std::rc::Rc;
use tracing::debug;

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
        byte_code: Rc<ByteCode>,
        heap: Heap,
        vm: Vm<Rc<ByteCode>, (StackTracer, EvaluatedValuesTracer)>,
    },
    /// Next, we run the module again to finds fuzzable functions. This time, we
    /// disable tracing of evaluated expressions, but we enable registration of
    /// fuzzable functions. Thus, the found functions to fuzz have the most
    /// efficient byte code possible.
    FindFuzzables {
        static_panics: Vec<Panic>,
        heap_for_constants: Heap,
        stack_tracer: StackTracer,
        /// We need to keep a reference to this byte code for its constant heap
        /// since objects in `evaluated_values` refer to it.
        evaluated_values_byte_code: Rc<ByteCode>,
        evaluated_values: EvaluatedValuesTracer,
        byte_code: Rc<ByteCode>,
        heap: Heap,
        vm: Vm<Rc<ByteCode>, FuzzablesFinder>,
    },
    /// Then, the functions are actually fuzzed.
    Fuzz {
        byte_code: Rc<ByteCode>,
        static_panics: Vec<Panic>,
        heap_for_constants: Heap,
        stack_tracer: StackTracer,
        evaluated_values_byte_code: Rc<ByteCode>,
        evaluated_values: EvaluatedValuesTracer,
        heap_for_fuzzables: Heap,
        fuzzers: Vec<Fuzzer>,
    },
}

impl ModuleAnalyzer {
    pub const fn for_module(module: Module) -> Self {
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
                        ExecutionTarget::Module(self.module.clone()),
                        TracingConfig {
                            register_fuzzables: TracingMode::OnlyCurrent,
                            calls: CallTracingMode::Off,
                            evaluated_expressions: TracingMode::Off,
                        },
                    )
                    .unwrap();
                let mut mir = (*mir).clone();
                let mut static_panics = mir.static_panics();
                static_panics.retain(|panic| panic.responsible.module == self.module);

                let tracing = TracingConfig {
                    register_fuzzables: TracingMode::Off,
                    calls: CallTracingMode::Off,
                    evaluated_expressions: TracingMode::OnlyCurrent,
                };
                let (byte_code, _) =
                    compile_byte_code(db, ExecutionTarget::Module(self.module.clone()), tracing);
                let byte_code = Rc::new(byte_code);

                let mut heap = Heap::default();
                let tracer = (
                    StackTracer::default(),
                    EvaluatedValuesTracer::new(self.module.clone()),
                );
                let vm = Vm::for_module(byte_code.clone(), &mut heap, tracer);

                State::EvaluateConstants {
                    static_panics,
                    byte_code,
                    heap,
                    vm,
                }
            }
            State::EvaluateConstants {
                static_panics,
                byte_code,
                heap: mut heap_for_constants,
                vm,
            } => {
                client
                    .update_status(Some(format!("Evaluating {}", self.module)))
                    .await;

                let tracer = match vm.run_n_without_handles(&mut heap_for_constants, 500) {
                    StateAfterRunWithoutHandles::Running(vm) => {
                        return State::EvaluateConstants {
                            static_panics,
                            byte_code,
                            heap: heap_for_constants,
                            vm,
                        }
                    }
                    StateAfterRunWithoutHandles::Finished(VmFinished { tracer, .. }) => tracer,
                };
                let (stack_tracer, evaluated_values) = tracer;

                let tracing = TracingConfig {
                    register_fuzzables: TracingMode::OnlyCurrent,
                    calls: CallTracingMode::Off,
                    evaluated_expressions: TracingMode::Off,
                };
                let (fuzzing_byte_code, _) =
                    compile_byte_code(db, ExecutionTarget::Module(self.module.clone()), tracing);
                let fuzzing_byte_code = Rc::new(fuzzing_byte_code);

                let mut heap = Heap::default();
                let vm = Vm::for_module(
                    fuzzing_byte_code.clone(),
                    &mut heap,
                    FuzzablesFinder::default(),
                );
                State::FindFuzzables {
                    static_panics,
                    heap_for_constants,
                    stack_tracer,
                    evaluated_values_byte_code: byte_code,
                    evaluated_values,
                    byte_code: fuzzing_byte_code,
                    heap,
                    vm,
                }
            }
            State::FindFuzzables {
                static_panics,
                heap_for_constants,
                stack_tracer,
                evaluated_values_byte_code,
                evaluated_values,
                byte_code,
                mut heap,
                vm,
            } => {
                client
                    .update_status(Some(format!("Evaluating {}", self.module)))
                    .await;

                let (heap, tracer) = match vm.run_n_without_handles(&mut heap, 500) {
                    StateAfterRunWithoutHandles::Running(vm) => {
                        return State::FindFuzzables {
                            static_panics,
                            heap_for_constants,
                            stack_tracer,
                            evaluated_values_byte_code,
                            evaluated_values,
                            byte_code,
                            heap,
                            vm,
                        }
                    }
                    StateAfterRunWithoutHandles::Finished(VmFinished { tracer, .. }) => {
                        (heap, tracer)
                    }
                };

                let fuzzers = tracer
                    .fuzzables
                    .iter()
                    .map(|(id, function)| Fuzzer::new(byte_code.clone(), *function, id.clone()))
                    .collect();
                State::Fuzz {
                    byte_code,
                    static_panics,
                    heap_for_constants,
                    stack_tracer,
                    evaluated_values_byte_code,
                    evaluated_values,
                    heap_for_fuzzables: heap,
                    fuzzers,
                }
            }
            State::Fuzz {
                byte_code,
                static_panics,
                heap_for_constants,
                stack_tracer,
                evaluated_values_byte_code,
                evaluated_values,
                heap_for_fuzzables,
                mut fuzzers,
            } => {
                let mut running_fuzzers = fuzzers
                    .iter_mut()
                    .filter(|fuzzer| matches!(fuzzer.status(), Status::StillFuzzing { .. }))
                    .collect_vec();
                let Some(fuzzer) = running_fuzzers.choose_mut(&mut thread_rng()) else {
                    client.update_status(None).await;
                    return State::Fuzz {
                        byte_code,
                        static_panics,
                        heap_for_constants,
                        stack_tracer,
                        evaluated_values_byte_code,
                        evaluated_values,
                        heap_for_fuzzables,
                        fuzzers,
                    };
                };

                client
                    .update_status(Some(format!("Fuzzing {}", fuzzer.function_id)))
                    .await;

                fuzzer.run(500);

                State::Fuzz {
                    byte_code,
                    static_panics,
                    heap_for_constants,
                    stack_tracer,
                    evaluated_values_byte_code,
                    evaluated_values,
                    heap_for_fuzzables,
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
                        .filter_map(|(id, value)| Insight::for_value(db, id.clone(), *value)),
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
                        .filter_map(|(id, value)| Insight::for_value(db, id.clone(), *value)),
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
                    assert!(
                        db.hir_to_cst_id(&id).is_some(),
                        "It looks like the generated code {} is at fault for a panic.",
                        panic.responsible,
                    );

                    // TODO: In the future, re-run only the failing case with
                    // tracing enabled and also show the arguments to the failing
                    // function in the hint.
                    let call_span = db
                        .hir_id_to_display_span(&panic.responsible)
                        .unwrap_or_else(|| panic!("Couldn't find the span for {panic:?}."));
                    insights.push(Insight::Diagnostic(Diagnostic::error(
                        db.range_to_lsp_range(self.module.clone(), call_span),
                        format!(
                            "For `{} {}`, this call panics: {}",
                            fuzzer.function_id.function_name(),
                            input
                                .arguments()
                                .iter()
                                .map(|it| it.to_debug_text(Precedence::High, MaxLength::Unlimited))
                                .join(" "),
                            panic.reason,
                        ),
                    )));
                }
            }
        }

        debug!("Insights: {insights:?}");

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
