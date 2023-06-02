use crate::{
    coverage::Coverage,
    input::Input,
    input_pool::{InputPool, Score},
    runner::{RunResult, Runner},
    utils::collect_symbols_in_heap,
    values::complexity_of_input,
};
use candy_frontend::hir::Id;
use candy_vm::{
    context::ExecutionController,
    fiber::Panic,
    heap::{Data, Function, Heap},
    lir::Lir,
    tracer::stack_trace::StackTracer,
};
use std::sync::Arc;
use tracing::debug;

pub struct Fuzzer {
    lir: Arc<Lir>,
    pub function_heap: Heap,
    pub function: Function,
    pub function_id: Id,
    status: Option<Status>, // only `None` during transitions
}

// TODO: Decrease enum variant sizes and size differences
#[allow(clippy::large_enum_variant)]
pub enum Status {
    StillFuzzing {
        pool: InputPool,
        total_coverage: Coverage,
        runner: Runner<Arc<Lir>>,
    },
    // TODO: In the future, also add a state for trying to simplify the input.
    FoundPanic {
        input: Input,
        panic: Panic,
        tracer: StackTracer,
    },
    TotalCoverageButNoPanic,
}

impl Fuzzer {
    pub fn new(lir: Arc<Lir>, function: Function, function_id: Id) -> Self {
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
            status: Some(Status::StillFuzzing {
                pool,
                total_coverage: Coverage::none(num_instructions),
                runner,
            }),
        }
    }

    pub fn status(&self) -> &Status {
        self.status.as_ref().unwrap()
    }
    pub fn into_status(self) -> Status {
        self.status.unwrap()
    }

    pub fn run(&mut self, execution_controller: &mut impl ExecutionController) {
        let mut status = self.status.take().unwrap();
        while matches!(status, Status::StillFuzzing { .. })
            && execution_controller.should_continue_running()
        {
            status = match status {
                Status::StillFuzzing {
                    pool,
                    total_coverage,
                    runner,
                } => self.continue_fuzzing(execution_controller, pool, total_coverage, runner),
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
                Status::TotalCoverageButNoPanic => Status::TotalCoverageButNoPanic,
            };
        }
        self.status = Some(status);
    }

    fn continue_fuzzing(
        &self,
        execution_controller: &mut impl ExecutionController,
        mut pool: InputPool,
        total_coverage: Coverage,
        mut runner: Runner<Arc<Lir>>,
    ) -> Status {
        runner.run(execution_controller);
        let Some(result) = runner.result else {
            return Status::StillFuzzing { pool, total_coverage, runner };
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
        match result {
            RunResult::Timeout => self.create_new_fuzzing_case(pool, total_coverage),
            RunResult::Done { .. } | RunResult::NeedsUnfulfilled { .. } => {
                let function_range = self.lir.range_of_function(&self.function_id);
                let function_coverage = total_coverage.in_range(&function_range);

                if function_coverage.relative_coverage() == 1.0 {
                    Status::TotalCoverageButNoPanic
                } else {
                    // We favor small inputs with good code coverage.
                    let score = {
                        let complexity = complexity_of_input(&runner.input) as Score;
                        let new_function_coverage = runner.coverage.in_range(&function_range);
                        let score: Score = (0.2 * runner.num_instructions as f64)
                            + (0.1
                                * new_function_coverage.improvement_on(&function_coverage) as f64)
                            - 0.4 * complexity;
                        score.clamp(0.1, Score::MAX)
                    };
                    pool.add(runner.input, score);
                    self.create_new_fuzzing_case(pool, &total_coverage + &runner.coverage)
                }
            }
            RunResult::Panicked(panic) => Status::FoundPanic {
                input: runner.input,
                panic,
                tracer: runner.tracer,
            },
        }
    }
    fn create_new_fuzzing_case(&self, pool: InputPool, total_coverage: Coverage) -> Status {
        let runner = Runner::new(self.lir.clone(), self.function, pool.generate_new_input());
        Status::StillFuzzing {
            pool,
            total_coverage,
            runner,
        }
    }
}
