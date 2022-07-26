mod fuzzer;
mod generator;
mod utils;

pub use self::fuzzer::{Fuzzer, Status};
use crate::{
    database::Database,
    input::Input,
    vm::{
        self,
        use_provider::DbUseProvider,
        value::{Closure, Value},
        TearDownResult, Vm,
    },
};
use itertools::Itertools;
use log;
use std::fs;

pub async fn fuzz(db: &Database, input: Input) {
    let mut vm = {
        let mut vm = Vm::new();
        let module_closure = Closure::of_input(db, input.clone()).unwrap();
        let use_provider = DbUseProvider { db };
        vm.set_up_module_closure_execution(&use_provider, module_closure);
        vm.run(&use_provider, 1000);

        loop {
            vm.run(&use_provider, 10000);
            match vm.status() {
                vm::Status::Running => log::info!("VM is still running."),
                vm::Status::Done => {
                    let TearDownResult { return_value, .. } =
                        vm.tear_down_module_closure_execution();
                    log::info!("VM is done. Export map: {return_value}");
                    break vm;
                }
                vm::Status::Panicked(value) => {
                    log::error!("VM panicked with value {value}.");
                    log::error!("This is the stack trace:");
                    vm.tracer.dump_stack_trace(&db, input.clone());
                    return;
                }
            }
        }
    };

    let TearDownResult {
        fuzzable_closures, ..
    } = vm.tear_down_module_closure_execution();

    log::info!(
        "Now, the fuzzing begins. So far, we have {} closures to fuzz.",
        fuzzable_closures.len()
    );

    for (id, closure) in fuzzable_closures {
        let mut fuzzer = Fuzzer::new(db, closure.clone(), id.clone());
        for _ in 0..20 {
            fuzzer.run(db, 100);
        }
        match fuzzer.status() {
            Status::StillFuzzing { .. } => {}
            Status::PanickedForArguments {
                arguments,
                message,
                tracer,
            } => {
                log::error!("The fuzzer discovered an input that crashes {id}:");
                log::error!(
                    "Calling `{id} {}` doesn't work because {}.",
                    arguments.iter().map(|it| format!("{}", it)).join(" "),
                    match message {
                        Value::Text(message) => message.to_string(),
                        other => format!("{}", other),
                    },
                );
                log::error!("This was the stack trace:");
                tracer.dump_stack_trace(&db, input.clone());

                let trace = tracer.dump_call_tree();
                let mut trace_file = input.to_path().unwrap();
                trace_file.set_extension("candy.trace");
                fs::write(trace_file.clone(), trace).unwrap();
                log::info!(
                    "Trace has been written to `{}`.",
                    trace_file.as_path().display()
                );
            }
        }
    }
}
