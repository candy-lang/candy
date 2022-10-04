mod fuzzer;
mod generator;
mod utils;

pub use self::fuzzer::{Fuzzer, Status};
use crate::{
    compiler::hir::Id,
    database::Database,
    module::Module,
    vm::{
        context::{DbUseProvider, RunForever, RunLimitedNumberOfInstructions},
        tracer::DummyTracer,
        Closure, Heap, Pointer, Vm,
    },
};
use itertools::Itertools;
use tracing::info;

pub async fn fuzz(db: &Database, module: Module) {
    let (fuzzables_heap, fuzzables): (Heap, Vec<(Id, Pointer)>) = {
        let mut vm = Vm::new();
        vm.set_up_for_running_module_closure(Closure::of_module(db, module.clone()).unwrap());
        vm.run(&mut DbUseProvider { db }, &mut RunForever, &mut DummyTracer);
        let result = vm.tear_down();
        (todo!(), todo!())
    };

    info!(
        "Now, the fuzzing begins. So far, we have {} closures to fuzz.",
        fuzzables.len()
    );

    for (id, closure) in fuzzables {
        let mut fuzzer = Fuzzer::new(&fuzzables_heap, closure, id.clone());
        fuzzer.run(
            db,
            &mut DbUseProvider { db },
            &mut RunLimitedNumberOfInstructions::new(1000),
        );
        match fuzzer.status() {
            Status::StillFuzzing { .. } => {}
            Status::PanickedForArguments {
                arguments,
                reason,
                tracer,
            } => {
                info!("The fuzzer discovered an input that crashes {id}:");
                info!(
                    "Calling `{id} {}` doesn't work because {reason}.",
                    arguments.iter().map(|arg| format!("{arg:?}")).join(" "),
                );
                info!("This was the stack trace:");
                // tracer.dump_stack_trace(db);
                todo!();

                // module.dump_associated_debug_file("trace", &tracer.full_trace().format(heap));
                todo!();
            }
        }
    }
}
