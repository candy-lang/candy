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
        tracer::FullTracer,
        Closure, Heap, Packet, Pointer, Vm,
    },
};
use itertools::Itertools;
use tracing::{error, info};

pub async fn fuzz(db: &Database, module: Module) -> Vec<FailingFuzzCase> {
    let (fuzzables_heap, fuzzables): (Heap, Vec<(Id, Pointer)>) = {
        let mut tracer = FuzzablesFinder::default();
        let mut vm = Vm::new();
        vm.set_up_for_running_module_closure(Closure::of_module(db, module).unwrap());
        vm.run(&DbUseProvider { db }, &mut RunForever, &mut tracer);
        (tracer.heap, tracer.fuzzables)
    };

    info!(
        "Now, the fuzzing begins. So far, we have {} closures to fuzz.",
        fuzzables.len()
    );

    let mut failing_cases = vec![];

    for (id, closure) in fuzzables {
        info!("Fuzzing {id}.");
        let mut fuzzer = Fuzzer::new(&fuzzables_heap, closure, id.clone());
        fuzzer.run(
            &mut DbUseProvider { db },
            &mut RunLimitedNumberOfInstructions::new(1000),
        );
        match fuzzer.into_status() {
            Status::StillFuzzing { .. } => {}
            Status::PanickedForArguments {
                arguments,
                reason,
                tracer,
            } => {
                error!("The fuzzer discovered an input that crashes {id}:");
                let case = FailingFuzzCase {
                    closure: id,
                    arguments,
                    reason,
                    tracer,
                };
                case.dump(db);
                failing_cases.push(case);
            }
        }
    }

    failing_cases
}

pub struct FailingFuzzCase {
    closure: Id,
    arguments: Vec<Packet>,
    reason: String,
    tracer: FullTracer,
}

impl FailingFuzzCase {
    pub fn dump(&self, db: &Database) {
        error!(
            "Calling `{} {}` doesn't work because {}.",
            self.closure,
            self.arguments
                .iter()
                .map(|arg| format!("{arg:?}"))
                .join(" "),
            self.reason,
        );
        error!(
            "This is the stack trace:\n{}",
            self.tracer.format_panic_stack_trace_to_root_fiber(db)
        );
    }
}
