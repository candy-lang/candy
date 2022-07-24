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
    pub status: Status,
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
    // TODO: Find a better way of handling this. The fuzzer's status is a state
    // machine and during transitioning to a new state (aka running the fuzzer),
    // we'd like to consume the old status and then produce a new status.
    // Rust's ownership rules don't let us take ownership of the status (leaving
    // it uninitialized), even if it's "just temporarily" while we're
    // transitioning. The reason is that our state machine code could panic and
    // in that case, some status needs to be there to be freed.
    // In the future, we could use `unsafe` to set the status to `uninit()`. But
    // currently, I'm not 100% that our VM won't panic, so we instead set it to
    // this temporary value.
    TemporarilyUninitialized,
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
            status: Status::new_fuzzing_attempt(db, closure),
        }
    }

    pub fn run(&mut self, db: &Database, num_instructions: usize) {
        self.status = match mem::replace(&mut self.status, Status::TemporarilyUninitialized) {
            Status::StillFuzzing { mut vm, arguments } => match &vm.status {
                vm::Status::Running => {
                    let use_provider = DbUseProvider { db };
                    vm.run(&use_provider, num_instructions);
                    Status::StillFuzzing { vm, arguments }
                }
                vm::Status::Done => {
                    // The VM finished running without panicking.
                    Status::new_fuzzing_attempt(db, self.closure.clone())
                }
                vm::Status::Panicked { reason } => {
                    // If a `needs` directly inside the tested closure was not
                    // satisfied, then the panic is not closure's fault, but our
                    // fault.
                    let is_our_fault =
                        did_need_in_closure_cause_panic(db, &self.closure_id, &vm.tracer);
                    if is_our_fault {
                        Status::new_fuzzing_attempt(db, self.closure.clone())
                    } else {
                        Status::PanickedForArguments {
                            arguments: arguments.clone(),
                            reason: reason.clone(),
                            tracer: vm.tracer.clone(),
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
            Status::TemporarilyUninitialized => unreachable!(),
        }
    }
}
