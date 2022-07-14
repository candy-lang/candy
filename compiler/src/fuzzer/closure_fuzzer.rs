use super::{generator::generate_n_values, utils::did_need_in_closure_cause_panic};
use crate::{
    compiler::hir,
    database::Database,
    input::Input,
    vm::{
        tracer::Tracer,
        use_provider::DbUseProvider,
        value::{Closure, Value},
        Status, Vm,
    },
};

pub fn fuzz_closure(
    db: &Database,
    input: &Input,
    closure: Closure,
    closure_id: &hir::Id,
    mut num_instructions: usize,
) -> ClosureFuzzResult {
    while num_instructions > 0 {
        let arguments = generate_n_values(closure.num_args);
        let result = test_closure_with_args(
            db.clone(),
            closure.clone(),
            closure_id,
            arguments.clone(),
            num_instructions,
        );

        let num_instructions_executed = match result {
            TestResult::DidNotFinishRunning => {
                break;
            }
            TestResult::FinishedRunningWithoutPanicking {
                num_instructions_executed,
            } => num_instructions_executed,

            TestResult::ArgumentsDidNotFulfillNeeds {
                num_instructions_executed,
            } => num_instructions_executed,

            TestResult::InternalPanic { message, tracer } => {
                return ClosureFuzzResult::PanickedForArguments {
                    arguments,
                    message,
                    tracer,
                }
            }
        };
        num_instructions -= num_instructions_executed;
    }
    ClosureFuzzResult::NoProblemFound
}

pub enum ClosureFuzzResult {
    NoProblemFound,
    PanickedForArguments {
        arguments: Vec<Value>,
        message: Value,
        tracer: Tracer,
    },
}

fn test_closure_with_args(
    db: &Database,
    closure: Closure,
    closure_id: &hir::Id,
    arguments: Vec<Value>,
    num_instructions: usize,
) -> TestResult {
    let mut vm = Vm::new();

    {
        let use_provider = DbUseProvider { db };
        vm.set_up_closure_execution(&use_provider, closure, arguments);
        vm.run(&use_provider, num_instructions);
    }

    match vm.status() {
        Status::Running => TestResult::DidNotFinishRunning,
        Status::Done => TestResult::FinishedRunningWithoutPanicking {
            num_instructions_executed: vm.num_instructions_executed,
        },
        Status::Panicked(message) => {
            // If a `needs` directly inside the tested closure was not
            // satisfied, then the panic is closure's fault, but our fault.
            let is_our_fault =
                did_need_in_closure_cause_panic(db, &closure_id, vm.tracer.log().last().unwrap());
            if is_our_fault {
                TestResult::ArgumentsDidNotFulfillNeeds {
                    num_instructions_executed: vm.num_instructions_executed,
                }
            } else {
                TestResult::InternalPanic {
                    message,
                    tracer: vm.tracer,
                }
            }
        }
    }
}
enum TestResult {
    DidNotFinishRunning,
    FinishedRunningWithoutPanicking { num_instructions_executed: usize },
    ArgumentsDidNotFulfillNeeds { num_instructions_executed: usize },
    InternalPanic { message: Value, tracer: Tracer },
}
