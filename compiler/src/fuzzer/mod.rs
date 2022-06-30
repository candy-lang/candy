mod generator;

use self::generator::generate_n_values;
use crate::{
    compiler::{
        hir::{self, Expression, HirDb, Lambda},
        hir_to_lir::HirToLir,
    },
    database::Database,
    input::Input,
    vm::{tracer::TraceEntry, value::Value, Status, Vm},
};
use itertools::Itertools;
use log;
use std::fs;

pub fn fuzz(db: &Database, input: Input) {
    let lir = db.lir(input.clone()).unwrap();

    let mut vm = Vm::new(lir.chunks.clone());
    vm.run(1000);
    match vm.status() {
        Status::Running => {
            log::warn!("VM didn't finish running, so we're not fuzzing it.");
            return;
        }
        Status::Done(value) => log::debug!("VM is done: {}", value),
        Status::Panicked(value) => {
            log::error!("VM panicked with value {}.", value);
            log::error!("{}", vm.tracer.format_stack_trace(db, input));
            return;
        }
    }

    log::info!(
        "Now, the fuzzing begins. So far, we have {} closures to fuzz.",
        vm.fuzzable_closures.len()
    );

    fuzz_vm(db, input, &vm, 0);
}

fn fuzz_vm(db: &Database, input: Input, vm: &Vm, num_fuzzable_closures_to_skip: usize) {
    'test_different_closures: for (closure_id, closure_address) in vm
        .fuzzable_closures
        .iter()
        .skip(num_fuzzable_closures_to_skip)
    {
        let closure = vm.heap.export_without_dropping(*closure_address);
        let num_args = if let Value::Closure { body, .. } = closure {
            vm.chunks[body].num_args
        } else {
            panic!("The VM registered a fuzzable closure that's not a closure.");
        };
        log::info!("Fuzzing {}.", closure_id);

        let fuzz_count = num_args * 20;

        for _ in 0..fuzz_count {
            // Snapshot a VM so we can run the fuzzing in the copy without modifying
            // the original VM.
            let mut vm = vm.clone();
            let arguments = generate_n_values(num_args);

            match test_closure_with_args(
                db,
                closure_id,
                &mut vm,
                *closure_address,
                arguments.clone(),
            ) {
                TestResult::StillRunning => {
                    log::warn!("The fuzzer is giving up because the VM didn't finish running.")
                }
                TestResult::NoPanic => {}
                TestResult::WrongInputs => {} // This is the fuzzer's fault.
                TestResult::InternalPanic(message) => {
                    log::error!("The fuzzer discovered an input that crashes the closure:");
                    log::error!(
                        "Calling `{} {}` doesn't work because {}.",
                        closure_id,
                        arguments.iter().map(|it| format!("{}", it)).join(" "),
                        match message {
                            Value::Text(message) => message,
                            other => format!("{}", other),
                        },
                    );
                    log::error!("This was the stack trace:");
                    vm.tracer.dump_stack_trace(db, input.clone());

                    let trace = vm.tracer.dump_call_tree();
                    // PathBuff::new(input.to_path().unwrap())
                    let mut trace_file = input.to_path().unwrap();
                    trace_file.set_extension("candy.trace");
                    fs::write(trace_file.clone(), trace).unwrap();
                    log::info!(
                        "Trace has been written to `{}`.",
                        trace_file.as_path().display()
                    );
                    continue 'test_different_closures;
                }
            }
        }
        log::debug!("Couldn't find any issues with this function.");
    }
}

fn test_closure_with_args(
    db: &Database,
    closure_id: &hir::Id,
    vm: &mut Vm,
    closure_address: usize,
    arguments: Vec<Value>,
) -> TestResult {
    vm.run_closure(closure_address, arguments);

    vm.run(1000);
    match vm.status() {
        Status::Running => TestResult::StillRunning,
        Status::Done(_) => TestResult::NoPanic,
        Status::Panicked(message) => {
            // If a needs directly inside the tested closure was
            // dissatisfied, then the panic is not the fault of the code
            // inside the code, but of the caller.
            let is_our_fault =
                did_need_in_closure_cause_panic(db, &closure_id, vm.tracer.log().last().unwrap());
            if is_our_fault {
                TestResult::WrongInputs
            } else {
                TestResult::InternalPanic(message)
            }
        }
    }
}
enum TestResult {
    StillRunning,
    NoPanic,
    WrongInputs,
    InternalPanic(Value),
}

fn did_need_in_closure_cause_panic(
    db: &Database,
    closure_id: &hir::Id,
    trace_entry: &TraceEntry,
) -> bool {
    if let TraceEntry::NeedsStarted { id, .. } = trace_entry {
        let mut id = id.parent().unwrap();
        loop {
            if &id == closure_id {
                return true;
            }

            match db
                .find_expression(id.clone())
                .expect("Parent of a `needs` call is a parameter.")
            {
                Expression::Lambda(Lambda { fuzzable, .. }) => {
                    if fuzzable {
                        return false; // The needs is in a different fuzzable lambda.
                    }
                }
                _ => panic!("Only lambdas can be the parent of a `needs` call."),
            };

            match id.parent() {
                Some(parent_id) => id = parent_id,
                None => return false,
            }
        }
    }
    return false;
}
