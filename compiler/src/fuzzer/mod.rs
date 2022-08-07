mod fuzzer;
mod generator;
mod utils;

pub use self::fuzzer::{Fuzzer, Status};
use crate::{
    database::Database,
    module::Module,
    vm::{use_provider::DbUseProvider, Closure, Vm},
};
use itertools::Itertools;
use tracing::{error, info};

pub async fn fuzz(db: &Database, module: Module) {
    let (fuzzables_heap, fuzzables) = {
        let result = Vm::new_for_running_module_closure(
            &DbUseProvider { db },
            Closure::of_module(db, module.clone()).unwrap(),
        )
        .run_synchronously_until_completion(db);
        (result.heap, result.fuzzable_closures)
    };

    info!(
        "Now, the fuzzing begins. So far, we have {} closures to fuzz.",
        fuzzables.len()
    );

    for (id, closure) in fuzzables {
        let mut fuzzer = Fuzzer::new(db, &fuzzables_heap, closure, id.clone());
        fuzzer.run(db, 1000);
        match fuzzer.status() {
            Status::StillFuzzing { .. } => {}
            Status::PanickedForArguments {
                heap,
                arguments,
                reason,
                tracer,
            } => {
                error!("The fuzzer discovered an input that crashes {id}:");
                error!(
                    "Calling `{id} {}` doesn't work because {reason}.",
                    arguments
                        .iter()
                        .map(|argument| argument.format(heap))
                        .join(" "),
                );
                error!("This was the stack trace:");
                tracer.dump_stack_trace(db, heap);

                module.dump_associated_debug_file("trace", &tracer.format_call_tree(heap));
            }
        }
    }
}
