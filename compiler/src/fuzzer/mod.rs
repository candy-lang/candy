mod fuzzer;
mod generator;
mod utils;

pub use self::{
    fuzzer::{Fuzzer, Status},
    utils::FuzzablesFinder,
};
use crate::{
    compiler::hir::Id,
    database::Database,
    module::Module,
    vm::{
        context::{DbUseProvider, RunForever, RunLimitedNumberOfInstructions},
        Closure, Heap, Pointer, Vm,
    },
};
use itertools::Itertools;
use tracing::{error, info};

pub async fn fuzz(db: &Database, module: Module) {
    let (fuzzables_heap, fuzzables): (Heap, Vec<(Id, Pointer)>) = {
        let mut tracer = FuzzablesFinder::new();
        let mut vm = Vm::new();
        vm.set_up_for_running_module_closure(Closure::of_module(db, module.clone()).unwrap());
        vm.run(&mut DbUseProvider { db }, &mut RunForever, &mut tracer);
        let result = vm.tear_down();
        (tracer.heap, tracer.fuzzables)
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
                error!("The fuzzer discovered an input that crashes {id}:");
                error!(
                    "Calling `{id} {}` doesn't work because {reason}.",
                    arguments.iter().map(|arg| format!("{arg:?}")).join(" "),
                );
                error!("Events:\n {tracer:?}");
                error!(
                    "This is the stack trace:\n{}",
                    tracer.format_panic_stack_trace_to_root_fiber(db)
                );
            }
        }
    }
}
