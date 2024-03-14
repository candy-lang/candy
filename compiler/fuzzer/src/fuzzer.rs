use crate::{
    coverage::Coverage,
    input::Input,
    input_pool::{InputPool, Score},
    runner::{RunResult, Runner},
    utils::collect_symbols_in_heap,
};
use candy_frontend::hir::Id;
use candy_vm::{
    byte_code::ByteCode,
    heap::{Function, Heap},
    tracer::stack_trace::StackTracer,
    Panic,
};
use itertools::Itertools;
use std::rc::Rc;
use tracing::debug;

pub struct Fuzzer {
    pub byte_code: Rc<ByteCode>,
    /// This heap lives as long as the fuzzer and houses our copy of the
    /// function to fuzz, our input pool, and current input.
    pub persistent_heap: Heap,
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
        input: Input,
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

// Very similar to `Status`, but this one is self-contained (has its own heap).
#[allow(clippy::large_enum_variant)]
pub enum FuzzerResult {
    StillFuzzing {
        total_coverage: Coverage,
        heap: Heap,
        input: Input,
        runner: Runner<Rc<ByteCode>>,
    },
    FoundPanic {
        heap: Heap,
        input: Input,
        panic: Panic,
        tracer: StackTracer,
    },
}

impl Fuzzer {
    #[must_use]
    pub fn new(byte_code: Rc<ByteCode>, function: Function, function_id: Id) -> Self {
        let mut persistent_heap = Heap::default();
        let function: Function = function
            .clone_to_heap(&mut persistent_heap)
            .try_into()
            .unwrap();

        // TODO: Collect `InlineTag`s by walking `function`
        let pool = InputPool::new(
            function.argument_count(),
            collect_symbols_in_heap(&persistent_heap)
                .into_iter()
                .collect_vec(),
        );

        let input = pool.generate_new_input(&mut persistent_heap);
        // The input is owned by the `InputPool` and our heap. The `Runner`
        // creates a copy in its heap.
        let runner = Runner::new(byte_code.clone(), function, &input);

        let num_instructions = byte_code.instructions.len();
        Self {
            byte_code,
            persistent_heap,
            function,
            function_id,
            pool,
            status: Some(Status::StillFuzzing {
                total_coverage: Coverage::none(num_instructions),
                input,
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
    pub fn into_result(mut self) -> FuzzerResult {
        match self.status.unwrap() {
            Status::StillFuzzing {
                total_coverage,
                input,
                runner,
            } => {
                input.dup(&mut self.persistent_heap);
                self.pool.drop(&mut self.persistent_heap);
                FuzzerResult::StillFuzzing {
                    total_coverage,
                    heap: self.persistent_heap,
                    input,
                    runner,
                }
            }
            Status::FoundPanic {
                heap,
                input,
                panic,
                tracer,
            } => FuzzerResult::FoundPanic {
                heap,
                input,
                panic,
                tracer,
            },
        }
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
                    input,
                    runner,
                } => self.continue_fuzzing(&mut instructions_left, total_coverage, input, runner),
                // We already found some arguments that caused the function to panic,
                // so there's nothing more to do.
                status @ Status::FoundPanic { .. } => status,
            };
        }
        self.status = Some(status);
    }

    fn continue_fuzzing(
        &mut self,
        instructions_left: &mut usize,
        total_coverage: Coverage,
        input: Input,
        mut runner: Runner<Rc<ByteCode>>,
    ) -> Status {
        runner.run(instructions_left);
        let Some(result) = runner.take_result() else {
            return Status::StillFuzzing {
                total_coverage,
                input,
                runner,
            };
        };

        let call_string = format!("`{} {}`", self.function_id.function_name(), input);
        debug!("{}", result.to_string(&call_string));
        match result {
            RunResult::Timeout => self.create_new_fuzzing_case(total_coverage),
            RunResult::Done { .. } | RunResult::NeedsUnfulfilled { .. } => {
                let function_range = self.byte_code.range_of_function(&self.function_id);
                let function_coverage = total_coverage.in_range(&function_range);

                // We favor small inputs with good code coverage.
                #[allow(clippy::cast_precision_loss)]
                let score = {
                    let complexity = input.complexity() as Score;
                    let new_function_coverage = runner.coverage.in_range(&function_range);
                    let coverage_improvement =
                        new_function_coverage.improvement_on(&function_coverage);

                    let score = (runner.num_instructions as f64)
                        .mul_add(1.5, 0.1 * coverage_improvement as f64);
                    let score: Score = complexity.mul_add(-0.4, score);
                    score.clamp(0.1, Score::MAX)
                };

                // This must use our copy of the input, not the runner's.
                self.pool.add(input, result, score);

                self.create_new_fuzzing_case(&total_coverage + &runner.coverage)
            }
            RunResult::Panicked {
                heap,
                tracer,
                panic,
            } => Status::FoundPanic {
                heap,
                input: runner.input,
                panic,
                tracer,
            },
        }
    }
    fn create_new_fuzzing_case(&mut self, total_coverage: Coverage) -> Status {
        let input = self.pool.generate_new_input(&mut self.persistent_heap);
        let runner = Runner::new(self.byte_code.clone(), self.function, &input);
        Status::StillFuzzing {
            total_coverage,
            input,
            runner,
        }
    }
}
