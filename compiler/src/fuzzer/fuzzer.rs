use super::generator::generate_n_values;
use crate::{
    compiler::hir,
    vm::{
        self,
        context::{ExecutionController, UseProvider},
        tracer::FullTracer,
        Closure, Heap, Packet, Pointer, Vm,
    },
};
use itertools::Itertools;
use std::mem;

pub struct Fuzzer {
    pub closure_heap: Heap,
    pub closure: Pointer,
    pub closure_id: hir::Id,
    status: Option<Status>, // only `None` during transitions
}
pub enum Status {
    // TODO: Have some sort of timeout or track how long we've been running. If
    // a function either goes into an infinite loop or does some error-prone
    // stuff, we'll never find the errors if we accidentally first choose an
    // input that triggers the loop.
    StillFuzzing {
        vm: Vm,
        arguments: Vec<Packet>,
        tracer: FullTracer,
    },
    // TODO: In the future, also add a state for trying to simplify the
    // arguments.
    PanickedForArguments {
        arguments: Vec<Packet>,
        reason: String,
        tracer: FullTracer,
    },
}

impl Status {
    fn new_fuzzing_attempt(closure_heap: &Heap, closure: Pointer) -> Status {
        let num_args = {
            let closure: Closure = closure_heap.get(closure).data.clone().try_into().unwrap();
            closure.num_args
        };

        let mut vm_heap = Heap::default();
        let closure = closure_heap.clone_single_to_other_heap(&mut vm_heap, closure);
        let arguments = generate_n_values(num_args);
        let argument_addresses = arguments
            .iter()
            .map(|arg| arg.clone_to_other_heap(&mut vm_heap))
            .collect_vec();

        let mut vm = Vm::new();
        vm.set_up_for_running_closure(vm_heap, closure, &argument_addresses);

        Status::StillFuzzing {
            vm,
            arguments,
            tracer: FullTracer::new(),
        }
    }
}
impl Fuzzer {
    pub fn new(closure_heap: &Heap, closure: Pointer, closure_id: hir::Id) -> Self {
        // The given `closure_heap` may contain other fuzzable closures.
        let mut heap = Heap::default();
        let closure = closure_heap.clone_single_to_other_heap(&mut heap, closure);

        let status = Status::new_fuzzing_attempt(&heap, closure);
        Self {
            closure_heap: heap,
            closure,
            closure_id,
            status: Some(status),
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
        while matches!(status, Status::StillFuzzing { .. })
            && execution_controller.should_continue_running()
        {
            status = self.map_status(status, use_provider, execution_controller);
        }
        self.status = Some(status);
    }
    fn map_status<U: UseProvider, E: ExecutionController>(
        &self,
        status: Status,
        use_provider: &mut U,
        execution_controller: &mut E,
    ) -> Status {
        match status {
            Status::StillFuzzing { mut vm, arguments, mut tracer } => match vm.status() {
                vm::Status::CanRun => {
                    vm.run(use_provider, execution_controller, &mut tracer);
                    Status::StillFuzzing { vm, arguments, tracer }
                }
                vm::Status::WaitingForOperations => panic!("Fuzzing should not have to wait on channel operations because arguments were not channels."),
                // The VM finished running without panicking.
                vm::Status::Done => Status::new_fuzzing_attempt(&self.closure_heap, self.closure),
                vm::Status::Panicked { reason, responsible } => {
                    // If a `needs` directly inside the tested closure was not
                    // satisfied, then the panic is not closure's fault, but our
                    // fault.
                    let is_our_fault = responsible.is_none();
                    if is_our_fault {
                        Status::new_fuzzing_attempt(&self.closure_heap, self.closure)
                    } else {
                        Status::PanickedForArguments {
                            arguments,
                            reason,
                            tracer,
                        }
                    }
                }
            },
            // We already found some arguments that caused the closure to panic,
            // so there's nothing more to do.
            Status::PanickedForArguments {
                arguments,
                reason,
                tracer,
            } => Status::PanickedForArguments {
                arguments,
                reason,
                tracer,
            },
        }
    }
}
