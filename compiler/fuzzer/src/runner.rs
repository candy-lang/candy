use candy_frontend::hir::Id;
use candy_vm::{
    self,
    context::{
        CombiningExecutionController, CountingExecutionController, ExecutionController, UseProvider,
    },
    heap::{Closure, Data, Heap, InlineObjectSliceCloneToHeap},
    tracer::full::FullTracer,
    vm::{self, Vm},
};

use super::input::Input;

const MAX_INSTRUCTIONS: usize = 10000;

pub struct Runner {
    pub vm: Option<Vm>, // Is consumed when the runner is finished.
    pub input: Input,
    pub tracer: FullTracer,
    pub num_instructions: usize,
    pub result: Option<RunResult>,
}

pub enum RunResult {
    /// Executing the closure with the input took more than `MAX_INSTRUCTIONS`.
    Timeout,

    /// The execution finished successfully with a value.
    Done,

    /// The execution panicked and the caller of the closure (aka the fuzzer) is
    /// at fault.
    NeedsUnfulfilled { reason: String },

    /// The execution panicked with an internal panic. This indicates an error
    /// in the code that should be shown to the user.
    Panicked { reason: String, responsible: Id },
}
impl RunResult {
    pub fn to_string(&self, call: &str) -> String {
        match self {
            RunResult::Timeout => format!("{call} timed out."),
            RunResult::Done => format!("{call} returned."),
            RunResult::NeedsUnfulfilled { reason } => {
                format!("{call} panicked and it's our fault: {reason}")
            }
            RunResult::Panicked { reason, .. } => format!("{call} panicked internally: {reason}"),
        }
    }
}

impl Runner {
    pub fn new(closure: Closure, input: Input) -> Self {
        let mut vm_heap = Heap::default();
        let closure = Data::from(closure.clone_to_heap(&mut vm_heap))
            .try_into()
            .unwrap();
        let arguments = input.arguments.clone_to_heap(&mut vm_heap);

        let mut vm = Vm::default();
        vm.set_up_for_running_closure(vm_heap, closure, &arguments, Id::fuzzer());

        Runner {
            vm: Some(vm),
            input,
            tracer: FullTracer::default(),
            num_instructions: 0,
            result: None,
        }
    }

    pub fn run(
        &mut self,
        use_provider: &impl UseProvider,
        execution_controller: &mut impl ExecutionController,
    ) {
        assert!(self.vm.is_some());
        assert!(self.result.is_none());

        let mut instruction_counter = CountingExecutionController::default();
        let mut execution_controller =
            CombiningExecutionController::new(execution_controller, &mut instruction_counter);

        while matches!(self.vm.as_ref().unwrap().status(), vm::Status::CanRun)
            && execution_controller.should_continue_running()
        {
            self.vm.as_mut().unwrap().run(
                use_provider,
                &mut execution_controller,
                &mut self.tracer,
            );
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
            // Because the fuzzer never sends channels as inputs, the closure
            // waits on some internal concurrency operations that will never be
            // completed. This most likely indicates an error in the code, but
            // it's of course valid to have a function that never returns. Thus,
            // this should be treated just like a regular timeout.
            vm::Status::WaitingForOperations => Some(RunResult::Timeout),
            vm::Status::Done => Some(RunResult::Done),
            vm::Status::Panicked {
                reason,
                responsible,
            } => Some(if responsible == Id::fuzzer() {
                RunResult::NeedsUnfulfilled { reason }
            } else {
                RunResult::Panicked {
                    reason,
                    responsible,
                }
            }),
        };
    }
}
