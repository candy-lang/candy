use tracing::trace;

use crate::{
    input::Input,
    input_pool::{InputPool, Score},
    runner::{RunResult, Runner},
    utils::collect_symbols_in_heap,
    values::complexity_of_input,
};

use candy_frontend::hir::Id;
use candy_vm::{
    context::{ExecutionController, UseProvider},
    heap::{Closure, Data, Heap, Pointer},
    tracer::full::FullTracer,
};
use std::mem;

pub struct Fuzzer {
    pub closure_heap: Heap,
    pub closure: Pointer,
    pub closure_id: Id,
    status: Option<Status>, // only `None` during transitions
}
pub enum Status {
    StillFuzzing {
        pool: InputPool,
        runner: Runner,
    },
    // TODO: In the future, also add a state for trying to simplify the input.
    FoundPanic {
        input: Input,
        reason: String,
        responsible: Id,
        tracer: FullTracer,
    },
}

impl Fuzzer {
    pub fn new(closure_heap: &Heap, closure: Pointer, closure_id: Id) -> Self {
        assert!(matches!(closure_heap.get(closure).data, Data::Closure(_)));

        // The given `closure_heap` may contain other fuzzable closures.
        let mut heap = Heap::default();
        let closure = closure_heap.clone_single_to_other_heap(&mut heap, closure);

        let pool = {
            let closure: Closure = heap.get(closure).data.clone().try_into().unwrap();
            InputPool::new(closure.num_args, collect_symbols_in_heap(&heap))
        };
        let runner = Runner::new(&heap, closure, pool.generate_new_input());

        Self {
            closure_heap: heap,
            closure,
            closure_id,
            status: Some(Status::StillFuzzing { pool, runner }),
        }
    }

    pub fn status(&self) -> &Status {
        self.status.as_ref().unwrap()
    }
    pub fn into_status(self) -> Status {
        self.status.unwrap()
    }

    pub fn run<U: UseProvider, E: ExecutionController>(
        &mut self,
        use_provider: &mut U,
        execution_controller: &mut E,
    ) {
        let mut status = mem::replace(&mut self.status, None).unwrap();
        while !matches!(status, Status::FoundPanic { .. })
            && execution_controller.should_continue_running()
        {
            status = match status {
                Status::StillFuzzing { pool, runner } => {
                    self.continue_fuzzing(use_provider, execution_controller, pool, runner)
                }
                // We already found some arguments that caused the closure to panic,
                // so there's nothing more to do.
                Status::FoundPanic {
                    input,
                    reason,
                    responsible,
                    tracer,
                } => Status::FoundPanic {
                    input,
                    reason,
                    responsible,
                    tracer,
                },
            };
        }
        self.status = Some(status);
    }

    fn continue_fuzzing<U: UseProvider, E: ExecutionController>(
        &self,
        use_provider: &mut U,
        execution_controller: &mut E,
        mut pool: InputPool,
        mut runner: Runner,
    ) -> Status {
        runner.run(use_provider, execution_controller);
        let Some(result) = runner.result else {
            return Status::StillFuzzing { pool, runner };
        };

        let call_string = format!(
            "`{} {}`",
            self.closure_id
                .keys
                .last()
                .map(|closure_name| closure_name.to_string())
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
            RunResult::Panicked {
                reason,
                responsible,
            } => Status::FoundPanic {
                input: runner.input,
                reason,
                responsible,
                tracer: runner.tracer,
            },
        }
    }
    fn create_new_fuzzing_case(&self, pool: InputPool) -> Status {
        let runner = Runner::new(&self.closure_heap, self.closure, pool.generate_new_input());
        Status::StillFuzzing { pool, runner }
    }
}
