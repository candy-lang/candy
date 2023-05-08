use candy_frontend::hir::Id;
use candy_vm::{
    self,
    context::{CombiningExecutionController, CountingExecutionController, ExecutionController},
    fiber::ExecutionPanicked,
    heap::{Function, HirId, InlineObjectSliceCloneToHeap},
    lir::Lir,
    tracer::stack_trace::StackTracer,
    vm::{self, Vm},
};

use super::input::Input;
use rustc_hash::FxHashMap;
use std::borrow::Borrow;

const MAX_INSTRUCTIONS: usize = 10000;

pub struct Runner<L: Borrow<Lir>> {
    pub vm: Option<Vm<L, StackTracer>>, // Is consumed when the runner is finished.
    pub input: Input,
    pub tracer: StackTracer,
    pub num_instructions: usize,
    pub result: Option<RunResult>,
}

pub enum RunResult {
    /// Executing the function with the input took more than `MAX_INSTRUCTIONS`.
    Timeout,

    /// The execution finished successfully with a value.
    Done,

    /// The execution panicked and the caller of the function (aka the fuzzer)
    /// is at fault.
    NeedsUnfulfilled { reason: String },

    /// The execution panicked with an internal panic. This indicates an error
    /// in the code that should be shown to the user.
    Panicked(ExecutionPanicked),
}
impl RunResult {
    pub fn to_string(&self, call: &str) -> String {
        match self {
            RunResult::Timeout => format!("{call} timed out."),
            RunResult::Done => format!("{call} returned."),
            RunResult::NeedsUnfulfilled { reason } => {
                format!("{call} panicked and it's our fault: {reason}")
            }
            RunResult::Panicked(panicked) => {
                format!("{call} panicked internally: {}", panicked.reason)
            }
        }
    }
}

impl<L: Borrow<Lir>> Runner<L> {
    pub fn new(lir: L, function: Function, input: Input) -> Self {
        let (mut heap, constant_mapping) = lir.borrow().constant_heap.clone();

        let mut mapping = FxHashMap::default();
        let function = function
            .clone_to_heap_with_mapping(&mut heap, &mut mapping)
            .try_into()
            .unwrap();
        let arguments = input
            .arguments
            .clone_to_heap_with_mapping(&mut heap, &mut mapping);
        let responsible = HirId::create(&mut heap, Id::fuzzer());

        let mut tracer = StackTracer::default();
        let vm = Vm::for_function(
            lir,
            heap,
            constant_mapping,
            function,
            &arguments,
            responsible,
            &mut tracer,
        );

        Runner {
            vm: Some(vm),
            input,
            tracer,
            num_instructions: 0,
            result: None,
        }
    }

    pub fn run(&mut self, execution_controller: &mut impl ExecutionController) {
        assert!(self.vm.is_some());
        assert!(self.result.is_none());

        let mut instruction_counter = CountingExecutionController::default();
        let mut execution_controller =
            CombiningExecutionController::new(execution_controller, &mut instruction_counter);

        while matches!(self.vm.as_ref().unwrap().status(), vm::Status::CanRun)
            && execution_controller.should_continue_running()
        {
            self.vm
                .as_mut()
                .unwrap()
                .run(&mut execution_controller, &mut self.tracer);
        }

        self.num_instructions += instruction_counter.num_instructions;

        self.result = match self.vm.as_ref().unwrap().status() {
            vm::Status::CanRun => {
                if self.num_instructions > MAX_INSTRUCTIONS {
                    Some(RunResult::Timeout)
                } else {
                    None
                }
            }
            // Because the fuzzer never sends channels as inputs, the function
            // waits on some internal concurrency operations that will never be
            // completed. This most likely indicates an error in the code, but
            // it's of course valid to have a function that never returns. Thus,
            // this should be treated just like a regular timeout.
            vm::Status::WaitingForOperations => Some(RunResult::Timeout),
            vm::Status::Done => Some(RunResult::Done),
            vm::Status::Panicked(panicked) => Some(if panicked.responsible == Id::fuzzer() {
                RunResult::NeedsUnfulfilled {
                    reason: panicked.reason,
                }
            } else {
                RunResult::Panicked(panicked)
            }),
        };
    }
}
