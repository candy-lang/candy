use crate::{
    hir_coverage::HirCoverage,
    input::Input,
    input_pool::{InputPool, Score},
    lir_coverage::LirCoverage,
    runner::{HirCoverageTracer, RunResult, Runner},
    utils::collect_symbols_in_heap,
    values::InputGeneration,
};
use candy_frontend::hir::Id;
use candy_vm::{
    execution_controller::ExecutionController,
    fiber::Panic,
    heap::{Data, Function, Heap},
    lir::Lir,
};
use std::rc::Rc;
use tracing::debug;

pub struct Fuzzer {
    lir: Rc<Lir>,
    pub function_heap: Heap,
    pub function: Function,
    pub function_id: Id,
    pool: InputPool,
    status: Option<Status>, // only `None` during transitions
    hir_coverage: HirCoverage,
}

// TODO: Decrease enum variant sizes and size differences
#[allow(clippy::large_enum_variant)]
pub enum Status {
    StillFuzzing {
        lir_coverage: LirCoverage,
        runner: Runner<Rc<Lir>>,
    },
    // TODO: In the future, also add a state for trying to simplify the input.
    FoundPanic {
        input: Input,
        panic: Panic,
        tracer: HirCoverageTracer,
    },
}

impl Fuzzer {
    pub fn new(lir: Rc<Lir>, function: Function, function_id: Id) -> Self {
        let mut heap = Heap::default();
        let function: Function = Data::from(function.clone_to_heap(&mut heap))
            .try_into()
            .unwrap();

        // PERF: Avoid collecting the symbols into a hash set of owned strings that we then copy again.
        let pool = InputPool::new(function.argument_count(), &collect_symbols_in_heap(&heap));
        let runner = Runner::new(lir.clone(), function, pool.generate_new_input());

        let num_instructions = lir.instructions.len();
        Self {
            lir,
            function_heap: heap,
            function,
            function_id,
            pool,
            status: Some(Status::StillFuzzing {
                lir_coverage: LirCoverage::none(num_instructions),
                runner,
            }),
            hir_coverage: HirCoverage::none(),
        }
    }

    pub fn lir(&self) -> Rc<Lir> {
        self.lir.clone()
    }

    pub fn status(&self) -> &Status {
        self.status.as_ref().unwrap()
    }
    pub fn into_status(self) -> Status {
        self.status.unwrap()
    }

    pub fn input_pool(&self) -> &InputPool {
        &self.pool
    }

    pub fn hir_coverage(&self) -> &HirCoverage {
        &self.hir_coverage
    }

    pub fn run(&mut self, execution_controller: &mut impl ExecutionController<HirCoverageTracer>) {
        let mut status = self.status.take().unwrap();
        while matches!(status, Status::StillFuzzing { .. })
            && execution_controller.should_continue_running()
        {
            status = match status {
                Status::StillFuzzing {
                    lir_coverage: total_coverage,
                    runner,
                } => self.continue_fuzzing(execution_controller, total_coverage, runner),
                // We already found some arguments that caused the function to panic,
                // so there's nothing more to do.
                Status::FoundPanic {
                    input,
                    panic,
                    tracer,
                } => Status::FoundPanic {
                    input,
                    panic,
                    tracer,
                },
            };
        }
        self.status = Some(status);
    }

    fn continue_fuzzing(
        &mut self,
        execution_controller: &mut impl ExecutionController<HirCoverageTracer>,
        total_coverage: LirCoverage,
        mut runner: Runner<Rc<Lir>>,
    ) -> Status {
        runner.run(execution_controller);
        let Some(result) = runner.result else {
            return Status::StillFuzzing {
                lir_coverage: total_coverage,
                runner,
            };
        };

        let call_string = format!(
            "`{} {}`",
            self.function_id
                .keys
                .last()
                .map(|function_name| function_name.to_string())
                .unwrap_or_else(|| "{…}".to_string()),
            runner.input,
        );
        debug!("{}", result.to_string(&call_string));

        for id in runner.hir_coverage.read().unwrap().all_ids() {
            self.hir_coverage.add(id.clone());
        }

        match result {
            RunResult::Timeout => self.create_new_fuzzing_case(total_coverage),
            RunResult::Done { .. } | RunResult::NeedsUnfulfilled { .. } => {
                let function_range = self.lir.range_of_function(&self.function_id);
                let function_coverage = total_coverage.in_range(&function_range);

                // We favor small inputs with good code coverage.
                let score = {
                    let complexity = runner.input.complexity() as Score;
                    let new_function_coverage = runner.lir_coverage.in_range(&function_range);
                    let score: Score = (1.5 * runner.num_instructions as f64)
                        + (0.1 * new_function_coverage.improvement_on(&function_coverage) as f64)
                        - 0.4 * complexity;
                    score.clamp(0.1, Score::MAX)
                };
                self.pool.add(runner.input, result, score);
                self.create_new_fuzzing_case(&total_coverage + &runner.lir_coverage)
            }
            RunResult::Panicked(panic) => Status::FoundPanic {
                input: runner.input,
                panic,
                tracer: runner.tracer,
            },
        }
    }
    fn create_new_fuzzing_case(&self, total_coverage: LirCoverage) -> Status {
        let runner = Runner::new(
            self.lir.clone(),
            self.function,
            self.pool.generate_new_input(),
        );
        Status::StillFuzzing {
            lir_coverage: total_coverage,
            runner,
        }
    }
}
