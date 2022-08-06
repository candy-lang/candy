use super::{generator::generate_n_values, utils::did_need_in_closure_cause_panic};
use crate::{
    compiler::hir,
    database::Database,
    vm::{self, tracer::Tracer, use_provider::DbUseProvider, Closure, Heap, Pointer, Vm},
};
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
        arguments: Vec<Pointer>,
    },
    // TODO: In the future, also add a state for trying to simplify the
    // arguments.
    PanickedForArguments {
        arguments_heap: Heap,
        arguments: Vec<Pointer>,
        reason: String,
        tracer: Tracer,
    },
}

impl Status {
    fn new_fuzzing_attempt(db: &Database, closure_heap: &Heap, closure: Pointer) -> Status {
        let num_args = {
            let closure: Closure = closure_heap.get(closure).data.clone().try_into().unwrap();
            closure.num_args
        };

        let mut vm_heap = Heap::default();
        let closure = closure_heap.clone_single_to_other_heap(&mut vm_heap, closure);
        let arguments = generate_n_values(&mut vm_heap, num_args);

        let use_provider = DbUseProvider { db };
        let vm = Vm::new_for_running_closure(vm_heap, &use_provider, closure, &arguments);

        Status::StillFuzzing { vm, arguments }
    }
}
impl Fuzzer {
    pub fn new(db: &Database, closure_heap: &Heap, closure: Pointer, closure_id: hir::Id) -> Self {
        // The given `closure_heap` may contain other fuzzable closures.
        let mut heap = Heap::default();
        let closure = closure_heap.clone_single_to_other_heap(&mut heap, closure);

        let status = Status::new_fuzzing_attempt(db, &heap, closure);
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

    pub fn run(&mut self, db: &Database, mut num_instructions: usize) {
        let mut status = mem::replace(&mut self.status, None).unwrap();
        while matches!(status, Status::StillFuzzing { .. }) {
            let (new_status, num_instructions_executed) =
                self.map_status(db, status, num_instructions);
            status = new_status;

            if num_instructions_executed >= num_instructions {
                break;
            } else {
                num_instructions -= num_instructions_executed;
            }
        }
        self.status = Some(status);
    }
    fn map_status(
        &self,
        db: &Database,
        status: Status,
        num_instructions: usize,
    ) -> (Status, usize) {
        match status {
            Status::StillFuzzing { mut vm, arguments } => match &vm.status {
                vm::Status::Running => {
                    let use_provider = DbUseProvider { db };
                    let num_instructions_executed_before = vm.num_instructions_executed;
                    vm.run(&use_provider, num_instructions);
                    let num_instruction_executed =
                        vm.num_instructions_executed - num_instructions_executed_before;
                    (
                        Status::StillFuzzing { vm, arguments },
                        num_instruction_executed,
                    )
                }
                // The VM finished running without panicking.
                vm::Status::Done => (
                    Status::new_fuzzing_attempt(db, &self.closure_heap, self.closure),
                    0,
                ),
                vm::Status::Panicked { reason } => {
                    // If a `needs` directly inside the tested closure was not
                    // satisfied, then the panic is not closure's fault, but our
                    // fault.
                    let is_our_fault =
                        did_need_in_closure_cause_panic(db, &self.closure_id, &vm.tracer);
                    let status = if is_our_fault {
                        Status::new_fuzzing_attempt(db, &self.closure_heap, self.closure)
                    } else {
                        let mut arguments_heap = Heap::default();
                        vm.heap
                            .clone_multiple_to_other_heap(&mut arguments_heap, &arguments);
                        Status::PanickedForArguments {
                            arguments_heap,
                            arguments,
                            reason: reason.clone(),
                            tracer: vm.tracer.clone(),
                        }
                    };
                    (status, 0)
                }
            },
            // We already found some arguments that caused the closure to panic,
            // so there's nothing more to do.
            Status::PanickedForArguments {
                arguments_heap,
                arguments,
                reason,
                tracer,
            } => (
                Status::PanickedForArguments {
                    arguments_heap,
                    arguments,
                    reason,
                    tracer,
                },
                0,
            ),
        }
    }
}
