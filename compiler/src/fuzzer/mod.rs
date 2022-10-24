mod fuzzer;
mod generator;
mod utils;

pub use self::fuzzer::{Fuzzer, Status};
use crate::{
    database::Database,
    module::Module,
    vm::{
        context::{DbUseProvider, RunForever, RunLimitedNumberOfInstructions},
        Closure, Vm,
    },
};
use itertools::Itertools;
use tracing::info;

pub async fn fuzz(db: &Database, module: Module) {
    let (fuzzables_heap, fuzzables) = {
        let mut vm = Vm::new();
        vm.set_up_for_running_module_closure(Closure::of_module(db, module.clone()).unwrap());
        vm.run(&mut DbUseProvider { db }, &mut RunForever);
        let result = vm.tear_down();
        (result.heap, result.fuzzable_closures)
    };

    info!(
        "Now, the fuzzing begins. So far, we have {} closures to fuzz.",
        fuzzables.len()
    );

    for (id, closure) in fuzzables {
        info!("Fuzzing {id}.");
        let mut fuzzer = Fuzzer::new(&fuzzables_heap, closure, id.clone());
        fuzzer.run(
            db,
            &mut DbUseProvider { db },
            &mut RunLimitedNumberOfInstructions::new(1000),
        );
        match fuzzer.status() {
            Status::StillFuzzing { .. } => {}
            Status::PanickedForArguments {
                heap,
                arguments,
                reason,
                tracer,
            } => {
                info!("The fuzzer discovered an input that crashes {id}:");
                info!(
                    "Calling `{id} {}` doesn't work because {reason}.",
                    arguments
                        .iter()
                        .map(|argument| argument.format(heap))
                        .join(" "),
                );
                info!("This was the stack trace:");
                tracer.dump_stack_trace(db, heap);

                module.dump_associated_debug_file("trace", &tracer.full_trace().format(heap));
            }
        }
    }
}
