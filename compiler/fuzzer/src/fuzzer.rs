use crate::{
    input::Input,
    input_pool::{InputPool, Score},
    runner::{RunResult, Runner},
    utils::collect_symbols_in_heap,
    values::complexity_of_input,
};
use candy_frontend::hir::Id;
use candy_vm::{
    context::ExecutionController,
    fiber::ExecutionPanicked,
    heap::{Data, Function, Heap},
    lir::Lir,
    tracer::stack_trace::StackTracer,
};
use std::sync::Arc;
use tracing::trace;

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
        runner: Runner<Arc<Lir>>,
    },
    // TODO: In the future, also add a state for trying to simplify the input.
    FoundPanic {
        input: Input,
        panicked: ExecutionPanicked,
        tracer: StackTracer,
    },
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

        Self {
            lir,
            function_heap: heap,
            function,
            function_id,
            status: Some(Status::StillFuzzing { pool, runner }),
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
        while !matches!(status, Status::FoundPanic { .. })
            && execution_controller.should_continue_running()
        {
            status = match status {
                Status::StillFuzzing { pool, runner } => {
                    self.continue_fuzzing(execution_controller, pool, runner)
                }
                // We already found some arguments that caused the function to panic,
                // so there's nothing more to do.
                Status::FoundPanic {
                    input,
                    panicked,
                    tracer,
                } => Status::FoundPanic {
                    input,
                    panicked,
                    tracer,
                },
            };
        }
        self.status = Some(status);
    }

    fn continue_fuzzing(
        &self,
        execution_controller: &mut impl ExecutionController,
        mut pool: InputPool,
        mut runner: Runner<Arc<Lir>>,
    ) -> Status {
        runner.run(execution_controller);
        let Some(result) = runner.result else {
            return Status::StillFuzzing { pool, runner };
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
        trace!("{}", result.to_string(&call_string));
        match result {
            RunResult::Timeout => self.create_new_fuzzing_case(pool),
            RunResult::Done { .. } | RunResult::NeedsUnfulfilled { .. } => {
                // TODO: For now, our "coverage" is just the number of executed
                // instructions. In the future, we should instead actually look
                // at what parts of the code ran. This way, we can boost inputs
                // that achieve big coverage with few instructions.
                let coverage = runner.num_instructions;

                // We favor small inputs with good code coverage.
                let score = {
                    let coverage = coverage as Score;
                    let complexity = complexity_of_input(&runner.input) as Score;
                    let score: Score = 0.1 * coverage - 0.4 * complexity;
                    score.clamp(0.1, Score::MAX)
                };
                pool.add(runner.input, score);
                self.create_new_fuzzing_case(pool)
            }
            RunResult::Panicked(panicked) => Status::FoundPanic {
                input: runner.input,
                panicked,
                tracer: runner.tracer,
            },
        }
    }
    fn create_new_fuzzing_case(&self, pool: InputPool) -> Status {
        let runner = Runner::new(self.lir.clone(), self.function, pool.generate_new_input());
        Status::StillFuzzing { pool, runner }
    }
}
