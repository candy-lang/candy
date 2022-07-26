use super::{generator::generate_n_values, utils::did_need_in_closure_cause_panic};
use crate::{
    compiler::hir,
    database::Database,
    vm::{
        self,
        tracer::Tracer,
        use_provider::DbUseProvider,
        value::{Closure, Value},
        Vm,
    },
};
use std::mem;

pub struct Fuzzer {
    pub closure: Closure,
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
        arguments: Vec<Value>,
    },
    // TODO: In the future, also add a state for trying to simplify the
    // arguments.
    PanickedForArguments {
        arguments: Vec<Value>,
        reason: String,
        tracer: Tracer,
    },
}

impl Status {
    fn new_fuzzing_attempt(db: &Database, closure: Closure) -> Status {
        let arguments = generate_n_values(closure.num_args);

        let use_provider = DbUseProvider { db };
        let mut vm = Vm::new();
        vm.set_up_closure_execution(&use_provider, closure, arguments.clone());

        Status::StillFuzzing { vm, arguments }
    }
}
impl Fuzzer {
    pub fn new(db: &Database, closure: Closure, closure_id: hir::Id) -> Self {
        Self {
            closure: closure.clone(),
            closure_id,
            status: Some(Status::new_fuzzing_attempt(db, closure)),
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
                vm::Status::Done => (Status::new_fuzzing_attempt(db, self.closure.clone()), 0),
                vm::Status::Panicked { reason } => {
                    // If a `needs` directly inside the tested closure was not
                    // satisfied, then the panic is not closure's fault, but our
                    // fault.
                    let is_our_fault =
                        did_need_in_closure_cause_panic(db, &self.closure_id, &vm.tracer);
                    let status = if is_our_fault {
                        Status::new_fuzzing_attempt(db, self.closure.clone())
                    } else {
                        Status::PanickedForArguments {
                            arguments: arguments.clone(),
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
                arguments,
                reason,
                tracer,
            } => (
                Status::PanickedForArguments {
                    arguments,
                    reason,
                    tracer,
                },
                0,
            ),
        }
    }
}
