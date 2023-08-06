#![feature(let_chains, round_char_boundary)]

mod coverage;
mod fuzzer;
mod input;
mod input_pool;
mod runner;
mod utils;
mod values;

use self::input::Input;
pub use self::{
    fuzzer::{Fuzzer, Status},
    input_pool::InputPool,
    runner::RunResult,
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
    execution_controller::RunLimitedNumberOfInstructions,
    fiber::Panic,
    heap::{DisplayWithSymbolTable, SymbolTable},
    mir_to_lir::compile_lir,
    tracer::stack_trace::StackTracer,
    vm::Vm,
};
use std::rc::Rc;
use tracing::{debug, error, info};

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
    let lir = Rc::new(lir);

    let (_heap, fuzzables) = {
        let mut tracer = FuzzablesFinder::default();
        let result = Vm::for_module(lir.clone(), &mut tracer).run_until_completion(&mut tracer);
        (result.heap, tracer.into_fuzzables())
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
            Status::StillFuzzing { total_coverage, .. } => {
                let coverage = total_coverage
                    .in_range(&lir.range_of_function(&id))
                    .relative_coverage();
                debug!("Achieved a coverage of {:.1} %.", coverage * 100.0);
            }
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
                case.dump(db, &lir.symbol_table);
                failing_cases.push(case);
            }
        }
    }

    failing_cases
}

pub struct FailingFuzzCase {
    function: Id,
    input: Input,
    panic: Panic,
    #[allow(dead_code)]
    tracer: StackTracer,
}

impl FailingFuzzCase {
    #[allow(unused_variables)]
    pub fn dump<DB>(&self, db: &DB, symbol_table: &SymbolTable)
    where
        DB: AstToHir + PositionConversionDb,
    {
        error!(
            "Calling `{} {}` panics: {}",
            self.function,
            self.input.to_string(symbol_table),
            self.panic.reason,
        );
        error!("{} is responsible.", self.panic.responsible);
        // Segfaults: https://github.com/candy-lang/candy/issues/458
        // error!(
        //     "This is the stack trace:\n{}",
        //     self.tracer.format_panic_stack_trace_to_root_fiber(db),
        // );
    }
}
