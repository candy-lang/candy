mod fuzzer;
mod generator;
mod utils;

pub use self::fuzzer::{Fuzzer, Status};
use crate::{
    database::Database,
    module::Module,
    vm::{use_provider::DbUseProvider, value::Closure, TearDownResult, Vm},
};
use itertools::Itertools;

pub async fn fuzz(db: &Database, module: Module) {
    let mut vm = {
        let mut vm = Vm::new();
        let module_closure = Closure::of_module(db, module.clone()).unwrap();
        let use_provider = DbUseProvider { db };
        vm.set_up_module_closure_execution(&use_provider, module_closure);
        vm.run_synchronously_until_completion(db).ok();
        vm
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
        fuzzer.run(db, 1000);
        match fuzzer.status() {
            Status::StillFuzzing { .. } => {}
            Status::PanickedForArguments {
                arguments,
                reason,
                tracer,
            } => {
                log::error!("The fuzzer discovered an input that crashes {id}:");
                log::error!(
                    "Calling `{id} {}` doesn't work because {reason}.",
                    arguments.iter().map(|it| format!("{}", it)).join(" "),
                );
                log::error!("This was the stack trace:");
                tracer.dump_stack_trace(db);

                module.dump_associated_debug_file("trace", &tracer.dump_call_tree());
            }
        }
    }
}
