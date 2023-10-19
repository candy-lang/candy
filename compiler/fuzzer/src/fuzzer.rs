use crate::{
    coverage::Coverage,
    input::Input,
    input_pool::{InputPool, Score},
    runner::{RunResult, Runner},
    utils::collect_symbols_in_heap,
    values::InputGeneration,
};
use candy_frontend::hir::Id;
use candy_vm::{
    byte_code::ByteCode,
    heap::{Data, Function, Heap},
    tracer::stack_trace::StackTracer,
    Panic,
};
use itertools::Itertools;
use std::rc::Rc;
use tracing::debug;

pub struct Fuzzer {
    pub byte_code: Rc<ByteCode>,
    pub function_heap: Heap,
    pub function: Function,
    pub function_id: Id,
    pool: InputPool,
    status: Option<Status>, // only `None` during transitions
}

// TODO: Decrease enum variant sizes and size differences
#[allow(clippy::large_enum_variant)]
pub enum Status {
    StillFuzzing {
        total_coverage: Coverage,
        runner: Runner<Rc<ByteCode>>,
    },
    // TODO: In the future, also add a state for trying to simplify the input.
    FoundPanic {
        input: Input,
        panic: Panic,
        heap: Heap,
        tracer: StackTracer,
    },
}

impl Fuzzer {
    #[must_use]
    pub fn new(byte_code: Rc<ByteCode>, function: Function, function_id: Id) -> Self {
        let mut heap = Heap::default();
        let function: Function = Data::from(function.clone_to_heap(&mut heap))
            .try_into()
            .unwrap();

        // TODO: Collect `InlineTag`s by walking `function`
        let pool = InputPool::new(
            function.argument_count(),
            collect_symbols_in_heap(&heap).into_iter().collect_vec(),
        );
        let runner = Runner::new(byte_code.clone(), function, pool.generate_new_input());

        let num_instructions = byte_code.instructions.len();
        Self {
            byte_code,
            function_heap: heap,
            function,
            function_id,
            pool,
            status: Some(Status::StillFuzzing {
                total_coverage: Coverage::none(num_instructions),
                runner,
            }),
        }
    }

    #[must_use]
    pub fn byte_code(&self) -> Rc<ByteCode> {
        self.byte_code.clone()
    }

    #[must_use]
    pub fn status(&self) -> &Status {
        self.status.as_ref().unwrap()
    }
    #[must_use]
    pub fn into_status(self) -> Status {
        self.status.unwrap()
    }

    #[must_use]
    pub const fn input_pool(&self) -> &InputPool {
        &self.pool
    }

    pub fn run(&mut self, max_instructions: usize) {
        let mut status = self.status.take().unwrap();
        let mut instructions_left = max_instructions;

        while matches!(status, Status::StillFuzzing { .. }) && instructions_left > 0 {
            status = match status {
                Status::StillFuzzing {
                    total_coverage,
                    runner,
                } => self.continue_fuzzing(&mut instructions_left, total_coverage, runner),
                // We already found some arguments that caused the function to panic,
                // so there's nothing more to do.
                Status::FoundPanic {
                    input,
                    panic,
                    heap,
                    tracer,
                } => Status::FoundPanic {
                    input,
                    panic,
                    heap,
                    tracer,
                },
            };
        }
        self.status = Some(status);
    }

    fn continue_fuzzing(
        &mut self,
        instructions_left: &mut usize,
        total_coverage: Coverage,
        mut runner: Runner<Rc<ByteCode>>,
    ) -> Status {
        runner.run(instructions_left);
        let Some(result) = runner.take_result() else {
            return Status::StillFuzzing {
                total_coverage,
                runner,
            };
        };

        let call_string = format!(
            "`{} {}`",
            self.function_id
                .keys
                .last()
                .map_or_else(|| "{…}".to_string(), ToString::to_string),
            runner.input,
        );
        debug!("{}", result.to_string(&call_string));
        match result {
            RunResult::Timeout => self.create_new_fuzzing_case(total_coverage),
            RunResult::Done { .. } | RunResult::NeedsUnfulfilled { .. } => {
                let function_range = self.byte_code.range_of_function(&self.function_id);
                let function_coverage = total_coverage.in_range(&function_range);

                // We favor small inputs with good code coverage.
                #[allow(clippy::cast_precision_loss)]
                let score = {
                    let complexity = runner.input.complexity() as Score;
                    let new_function_coverage = runner.coverage.in_range(&function_range);
                    let coverage_improvement =
                        new_function_coverage.improvement_on(&function_coverage);

                    let score = (runner.num_instructions as f64)
                        .mul_add(1.5, 0.1 * coverage_improvement as f64);
                    let score: Score = complexity.mul_add(-0.4, score);
                    score.clamp(0.1, Score::MAX)
                };
                self.pool.add(runner.input, result, score);
                self.create_new_fuzzing_case(&total_coverage + &runner.coverage)
            }
            RunResult::Panicked {
                heap,
                tracer,
                panic,
            } => Status::FoundPanic {
                input: runner.input,
                panic,
                heap,
                tracer,
            },
        }
    }
    fn create_new_fuzzing_case(&self, total_coverage: Coverage) -> Status {
        let runner = Runner::new(
            self.byte_code.clone(),
            self.function,
            self.pool.generate_new_input(),
        );
        Status::StillFuzzing {
            total_coverage,
            runner,
        }
    }
}
