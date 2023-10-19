use super::input::Input;
use crate::coverage::Coverage;
use candy_frontend::hir::Id;
use candy_vm::VmFinished;
use candy_vm::{
    byte_code::ByteCode,
    environment::StateAfterRunWithoutHandles,
    heap::{Function, Heap, HirId, InlineObject, InlineObjectSliceCloneToHeap},
    tracer::stack_trace::StackTracer,
    Panic, Vm,
};
use rustc_hash::FxHashMap;
use std::borrow::Borrow;

const MAX_INSTRUCTIONS: usize = 1_000_000;

pub struct Runner<B: Borrow<ByteCode>> {
    pub byte_code: B,
    state: Option<State<B>>,
    pub input: Input,
    pub num_instructions: usize,
    pub coverage: Coverage,
}
enum State<B: Borrow<ByteCode>> {
    Running { heap: Heap, vm: Vm<B, StackTracer> },
    Finished(RunResult),
}

#[must_use]
pub enum RunResult {
    /// Executing the function with the input took more than `MAX_INSTRUCTIONS`.
    Timeout,

    /// The execution finished successfully with a value.
    Done {
        heap: Heap,
        return_value: InlineObject,
    },

    /// The execution panicked and the caller of the function (aka the fuzzer)
    /// is at fault.
    NeedsUnfulfilled { reason: String },

    /// The execution panicked with an internal panic. This indicates an error
    /// in the code that should be shown to the user.
    Panicked {
        heap: Heap,
        tracer: StackTracer,
        panic: Panic,
    },
}
impl RunResult {
    #[must_use]
    pub fn to_string(&self, call: &str) -> String {
        match self {
            Self::Timeout => format!("{call} timed out."),
            Self::Done { return_value, .. } => format!("{call} returned {return_value}."),
            Self::NeedsUnfulfilled { reason } => {
                format!("{call} panicked and it's our fault: {reason}")
            }
            Self::Panicked { panic, .. } => {
                format!("{call} panicked internally: {}", panic.reason)
            }
        }
    }
}

impl<B: Borrow<ByteCode> + Clone> Runner<B> {
    pub fn new(byte_code: B, function: Function, input: Input) -> Self {
        let mut heap = Heap::default();
        let num_instructions = byte_code.borrow().instructions.len();

        let mut mapping = FxHashMap::default();
        let function = function
            .clone_to_heap_with_mapping(&mut heap, &mut mapping)
            .try_into()
            .unwrap();
        let mut arguments = input
            .arguments
            .clone_to_heap_with_mapping(&mut heap, &mut mapping);
        arguments.push(HirId::create(&mut heap, true, Id::fuzzer()).into());

        let vm = Vm::for_function(
            byte_code.clone(),
            &mut heap,
            function,
            &arguments,
            StackTracer::default(),
        );

        Self {
            byte_code,
            state: Some(State::Running { heap, vm }),
            input,
            num_instructions: 0,
            coverage: Coverage::none(num_instructions),
        }
    }

    pub fn run(&mut self, instructions_left: &mut usize) {
        let State::Running { mut heap, mut vm } = self.state.take().unwrap() else {
            panic!("Runner is not running anymore.");
        };

        while *instructions_left > 0 {
            if let Some(ip) = vm.next_instruction() {
                self.coverage.add(ip);
            }
            self.num_instructions += 1;
            *instructions_left -= 1;

            match vm.run_without_handles(&mut heap) {
                StateAfterRunWithoutHandles::Running(new_vm) => vm = new_vm,
                StateAfterRunWithoutHandles::Finished(VmFinished {
                    result: Ok(return_value),
                    ..
                }) => {
                    self.state = Some(State::Finished(RunResult::Done { heap, return_value }));
                    return;
                }
                StateAfterRunWithoutHandles::Finished(VmFinished {
                    tracer,
                    result: Err(panic),
                }) => {
                    let result = if panic.responsible == Id::fuzzer() {
                        RunResult::NeedsUnfulfilled {
                            reason: panic.reason,
                        }
                    } else {
                        RunResult::Panicked {
                            heap,
                            tracer,
                            panic,
                        }
                    };
                    self.state = Some(State::Finished(result));
                    return;
                }
            }

            if self.num_instructions > MAX_INSTRUCTIONS {
                self.state = Some(State::Finished(RunResult::Timeout));
            }
        }
        self.state = Some(State::Running { heap, vm });
    }

    pub fn take_result(&mut self) -> Option<RunResult> {
        match self.state.take().unwrap() {
            running @ State::Running { .. } => {
                self.state = Some(running);
                None
            }
            State::Finished(result) => Some(result),
        }
    }
}
