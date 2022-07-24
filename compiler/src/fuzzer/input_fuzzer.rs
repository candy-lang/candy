use crate::{
    compiler::hir,
    database::Database,
    fuzzer::{closure_fuzzer::Fuzzer, Status},
    input::Input,
    vm::{
        self,
        tracer::Tracer,
        use_provider::DbUseProvider,
        value::{Closure, Value},
        TearDownResult, Vm,
    },
};

pub async fn fuzz_input(db: &Database, input: Input) -> Vec<ClosurePanic> {
    let mut vm = {
        let mut vm = Vm::new();
        let module_closure = Closure::of_input(db, input.clone()).unwrap();
        let use_provider = DbUseProvider { db };
        vm.set_up_module_closure_execution(&use_provider, module_closure);
        vm.run(&use_provider, 1000);
        vm
    };

    match vm.status() {
        vm::Status::Running => {
            log::warn!("The VM didn't finish running, so we're not fuzzing it.");
            return vec![];
        }
        vm::Status::Done => log::debug!("The VM is done."),
        vm::Status::Panicked(value) => {
            log::error!("The VM panicked with value {value}.");
            log::error!("{}", vm.tracer.format_stack_trace(db, input.clone()));
            return vec![];
        }
    }
    let TearDownResult {
        fuzzable_closures, ..
    } = vm.tear_down_module_closure_execution();

    log::info!(
        "Now, the fuzzing begins. So far, we have {} closures to fuzz.",
        fuzzable_closures.len()
    );

    let mut panics = vec![];
    for (id, closure) in fuzzable_closures {
        let mut fuzzer = Fuzzer::new(db, closure.clone(), id.clone());
        for _ in 0..20 {
            fuzzer.run(db, 100);
        }
        match fuzzer.status {
            Status::StillFuzzing { .. } => {}
            Status::PanickedForArguments {
                arguments,
                message,
                tracer,
            } => panics.push(ClosurePanic {
                closure,
                closure_id: id,
                arguments,
                message,
                tracer,
            }),
            Status::TemporarilyUninitialized => unreachable!(),
        }
    }
    panics
}

pub struct ClosurePanic {
    pub closure: Closure,
    pub closure_id: hir::Id,
    pub arguments: Vec<Value>,
    pub message: Value,
    pub tracer: Tracer,
}
