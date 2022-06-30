use std::fs;

use crate::{
    compiler::{
        hir::{self, Expression, HirDb, Lambda},
        hir_to_lir::HirToLir,
    },
    database::Database,
    input::Input,
    vm::{dump_panicked_vm, tracer::TraceEntry, value::Value, Status, Vm},
};
use log::{error, info};

pub fn fuzz(db: &Database, input: Input) {
    let lir = db.lir(input.clone()).unwrap();

    let mut vm = Vm::new(lir.chunks.clone());
    vm.run(1000);
    match vm.status() {
        Status::Running => {
            info!("VM didn't finish running, so we're not fuzzing it.");
            return;
        }
        Status::Done(value) => info!("VM is done: {}", value),
        Status::Panicked(value) => {
            dump_panicked_vm(&db, input, &vm, value);
            return;
        }
    }

    info!(
        "Now, the fuzzing begins. So far, we have {} closures to fuzz.",
        vm.fuzzable_closures.len()
    );
    while let Some((closure_id, closure_address)) = vm.fuzzable_closures.pop() {
        let closure = vm.heap.export_without_dropping(closure_address);
        let num_args = if let Value::Closure { body, .. } = closure {
            vm.chunks[body].num_args
        } else {
            panic!("The VM registered a fuzzable closure that's not a closure.");
        };
        info!(
            "Fuzzing closure {} (id {}) with {} arguments.",
            closure, closure_id, num_args
        );

        let arguments = generate_fuzzing_arguments(num_args);
        vm.run_closure(closure_address, arguments);

        vm.run(1000);
        match vm.status() {
            Status::Running => {
                info!("VM didn't finish running, so we're not fuzzing it.");
                return;
            }
            Status::Done(value) => info!("VM is done: {}", value),
            Status::Panicked(value) => {
                let did_need_in_closure_cause_panic =
                    did_need_in_closure_cause_panic(db, &closure_id, vm.tracer.log.last().unwrap());
                if did_need_in_closure_cause_panic {
                    error!("The closure crashed for some input, but it's the fuzzer's fault.");
                } else {
                    error!("The fuzzer discovered an input that crashes the closure:");
                    dump_panicked_vm(&db, input.clone(), &vm, value);

                    let trace = vm.tracer.correlate_and_dump();
                    // PathBuff::new(input.to_path().unwrap())
                    let mut trace_file = input.to_path().unwrap();
                    trace_file.set_extension("candy.trace");
                    fs::write(trace_file.clone(), trace).unwrap();
                    info!(
                        "Trace has been written to `{}`.",
                        trace_file.as_path().display()
                    );
                }

                return;
            }
        }
    }
}

fn generate_fuzzing_arguments(num: usize) -> Vec<Value> {
    let mut args = vec![];
    for _ in 0..num {
        args.push(Value::Int(0));
    }
    args
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
