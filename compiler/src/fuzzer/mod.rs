use crate::{
    compiler::{hir_to_lir::HirToLir, lir::Instruction},
    database::Database,
    input::Input,
    vm::{dump_panicked_vm, value::Value, Status, Vm},
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
    while let Some(closure_address) = vm.fuzzable_closures.pop() {
        let closure = vm.heap.export_without_dropping(closure_address);
        let num_args = if let Value::Closure { body, .. } = closure {
            vm.chunks[body].num_args
        } else {
            panic!("The VM registered a fuzzable closure that's not a closure.");
        };
        info!("Fuzzing closure {} with {} arguments.", closure, num_args);

        for arg in generate_fuzzing_arguments(num_args) {
            let address = vm.heap.import(arg);
            vm.data_stack.push(address);
        }
        vm.data_stack.push(closure_address);
        vm.run_instruction(Instruction::Call { num_args });

        vm.status = Status::Running;
        vm.run(1000);
        match vm.status() {
            Status::Running => {
                info!("VM didn't finish running, so we're not fuzzing it.");
                return;
            }
            Status::Done(value) => info!("VM is done: {}", value),
            Status::Panicked(value) => {
                error!("The VM panicked during fuzzing:");
                dump_panicked_vm(&db, input, &vm, value);
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
