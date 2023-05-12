#![feature(let_chains, round_char_boundary)]

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
    cst::CstDb,
    mir_optimize::OptimizeMir,
    module::Module,
    position::PositionConversionDb,
    {hir::Id, TracingConfig, TracingMode},
};
use candy_vm::{
    context::{RunForever, RunLimitedNumberOfInstructions},
    fiber::Panic,
    mir_to_lir::compile_lir,
    tracer::stack_trace::StackTracer,
    vm::Vm,
};
use std::sync::Arc;
use tracing::{error, info};

pub fn fuzz<DB>(db: &DB, module: Module) -> Vec<FailingFuzzCase>
where
    DB: AstToHir + CstDb + OptimizeMir + PositionConversionDb,
{
    let tracing = TracingConfig {
        register_fuzzables: TracingMode::All,
        calls: TracingMode::Off,
        evaluated_expressions: TracingMode::Off,
    };
    let (lir, _) = compile_lir(db, module, tracing);
    let lir = Arc::new(lir);

    let fuzzables = {
        let mut tracer = FuzzablesFinder::default();
        let mut vm = Vm::for_module(lir.clone(), &mut tracer);
        vm.run(&mut RunForever, &mut tracer);
        tracer.into_fuzzables()
    };

    info!(
        "Now, the fuzzing begins. So far, we have {} functions to fuzz.",
        fuzzables.len(),
    );

    let mut failing_cases = vec![];

    for (id, function) in fuzzables {
        info!("Fuzzing {id}.");
        let mut fuzzer = Fuzzer::new(lir.clone(), function, id.clone());
        fuzzer.run(&mut RunLimitedNumberOfInstructions::new(100000));

        match fuzzer.into_status() {
            Status::StillFuzzing { .. } => {}
            Status::FoundPanic {
                input,
                panic,
                tracer,
            } => {
                error!("The fuzzer discovered an input that crashes {id}:");
                let case = FailingFuzzCase {
                    function: id,
                    input,
                    panic,
                    tracer,
                };
                case.dump(db);
                failing_cases.push(case);
            }
        }
    }

    failing_cases
}

pub struct FailingFuzzCase<'h> {
    function: Id,
    input: Input<'h>,
    panic: Panic,
    tracer: StackTracer<'h>,
}

impl FailingFuzzCase<'_> {
    pub fn dump<DB>(&self, db: &DB)
    where
        DB: AstToHir + PositionConversionDb,
    {
        error!(
            "Calling `{} {}` panics: {}",
            self.function, self.input, self.panic.reason,
        );
        error!("{} is responsible.", self.panic.responsible);
        error!(
            "This is the stack trace:\n{}",
            self.tracer.format_panic_stack_trace_to_root_fiber(db),
        );
    }
}
