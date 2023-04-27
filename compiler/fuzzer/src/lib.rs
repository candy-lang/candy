#![feature(round_char_boundary)]

mod fuzzer;
mod input;
mod input_pool;
mod runner;
mod utils;
mod values;

use self::input::Input;
pub use self::{
    fuzzer::{Fuzzer, Status},
    utils::FuzzablesFinder,
};
use candy_frontend::{
    ast_to_hir::AstToHir,
    module::Module,
    position::PositionConversionDb,
    {hir::Id, TracingConfig, TracingMode},
};
use candy_vm::{
    context::{DbUseProvider, RunForever, RunLimitedNumberOfInstructions},
    heap::{Closure, Heap},
    mir_to_lir::MirToLir,
    tracer::full::FullTracer,
    vm::Vm,
};
use tracing::{error, info};

pub fn fuzz<DB>(db: &DB, module: Module) -> Vec<FailingFuzzCase>
where
    DB: AstToHir + MirToLir + PositionConversionDb,
{
    let tracing = TracingConfig {
        register_fuzzables: TracingMode::All,
        calls: TracingMode::Off,
        evaluated_expressions: TracingMode::Off,
    };

    let fuzzables = {
        let mut heap = Heap::default();
        let closure =
            Closure::create_from_module(&mut heap, db, module.clone(), tracing.clone()).unwrap();
        let mut vm = Vm::default();
        vm.set_up_for_running_module_closure(heap, module, closure);

        let mut tracer = FuzzablesFinder::default();
        vm.run(
            &DbUseProvider {
                db,
                tracing: tracing.clone(),
            },
            &mut RunForever,
            &mut tracer,
        );
        tracer.fuzzables
    };

    info!(
        "Now, the fuzzing begins. So far, we have {} closures to fuzz.",
        fuzzables.len(),
    );

    let mut failing_cases = vec![];

    for (id, closure) in fuzzables {
        info!("Fuzzing {id}.");
        let mut fuzzer = Fuzzer::new(closure, id.clone());
        fuzzer.run(
            &mut DbUseProvider {
                db,
                tracing: tracing.clone(),
            },
            &mut RunLimitedNumberOfInstructions::new(100000),
        );
        match fuzzer.into_status() {
            Status::StillFuzzing { .. } => {}
            Status::FoundPanic {
                input,
                reason,
                responsible,
                tracer,
            } => {
                error!("The fuzzer discovered an input that crashes {id}:");
                let case = FailingFuzzCase {
                    closure: id,
                    input,
                    reason,
                    responsible,
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
    input: Input,
    reason: String,
    responsible: Id,
    tracer: FullTracer,
}

impl FailingFuzzCase {
    pub fn dump<DB>(&self, db: &DB)
    where
        DB: AstToHir + PositionConversionDb,
    {
        error!(
            "Calling `{} {}` panics: {}",
            self.closure, self.input, self.reason,
        );
        error!("{} is responsible.", self.responsible,);
        error!(
            "This is the stack trace:\n{}",
            self.tracer.format_panic_stack_trace_to_root_fiber(db)
        );
    }
}
