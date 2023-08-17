use super::input::Input;
use crate::coverage::Coverage;
use candy_frontend::hir::Id;
use candy_vm::VmFinished;
use candy_vm::{
    environment::StateAfterRunWithoutHandles,
    heap::{Function, Heap, HirId, InlineObject, InlineObjectSliceCloneToHeap},
    lir::Lir,
    tracer::stack_trace::StackTracer,
    Panic, Vm,
};
use rustc_hash::FxHashMap;
use std::borrow::Borrow;

const MAX_INSTRUCTIONS: usize = 1_000_000;

pub struct Runner<L: Borrow<Lir>> {
    pub lir: L,
    pub vm: Option<Vm<L, StackTracer>>, // Is consumed when the runner is finished.
    pub input: Input,
    pub num_instructions: usize,
    pub coverage: Coverage,
    pub result: Option<RunResult>,
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

impl<L: Borrow<Lir> + Clone> Runner<L> {
    pub fn new(lir: L, function: Function, input: Input) -> Self {
        let mut heap = Heap::default();
        let num_instructions = lir.borrow().instructions.len();

        let mut mapping = FxHashMap::default();
        let function = function
            .clone_to_heap_with_mapping(&mut heap, &mut mapping)
            .try_into()
            .unwrap();
        let arguments = input
            .arguments
            .clone_to_heap_with_mapping(&mut heap, &mut mapping);
        let responsible = HirId::create(&mut heap, true, Id::fuzzer());

        let vm = Vm::for_function(
            lir.clone(),
            heap,
            function,
            &arguments,
            responsible,
            StackTracer::default(),
        );

        Self {
            lir,
            vm: Some(vm),
            input,
            num_instructions: 0,
            coverage: Coverage::none(num_instructions),
            result: None,
        }
    }

    pub fn run(&mut self, instructions_left: &mut usize) {
        assert!(self.vm.is_some());
        assert!(self.result.is_none());

        let mut vm = self.vm.take().unwrap();
        while *instructions_left > 0 {
            if let Some(ip) = vm.next_instruction() {
                self.coverage.add(ip);
            }
            self.num_instructions += 1;
            *instructions_left -= 1;

            match vm.run_without_handles() {
                StateAfterRunWithoutHandles::Running(new_vm) => vm = new_vm,
                StateAfterRunWithoutHandles::Finished(VmFinished {
                    heap,
                    result: Ok(return_value),
                    ..
                }) => {
                    self.result = Some(RunResult::Done { heap, return_value });
                    return;
                }
                StateAfterRunWithoutHandles::Finished(VmFinished {
                    heap,
                    tracer,
                    result: Err(panic),
                }) => {
                    self.result = Some(if panic.responsible == Id::fuzzer() {
                        RunResult::NeedsUnfulfilled {
                            reason: panic.reason,
                        }
                    } else {
                        RunResult::Panicked {
                            heap,
                            tracer,
                            panic,
                        }
                    });
                    return;
                }
            }

            if self.num_instructions > MAX_INSTRUCTIONS {
                self.result = Some(RunResult::Timeout);
            }
        }
        self.vm = Some(vm);
    }
}
