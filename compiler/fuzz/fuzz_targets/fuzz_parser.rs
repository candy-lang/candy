#![no_main]

use std::collections::HashMap;

use candy::{
    compiler::{hir, mir_to_lir::MirToLir, TracingConfig},
    database::Database,
    module::{InMemoryModuleProvider, Module, ModuleKind, Package},
    vm::{
        context::{DbUseProvider, RunForever},
        tracer::dummy::DummyTracer,
        Closure, ExecutionResult, Status, Struct, Vm,
    },
};
use lazy_static::lazy_static;
use libfuzzer_sys::fuzz_target;

const TRACING: TracingConfig = TracingConfig::off();
lazy_static! {
    static ref PACKAGE: Package = Package::User("/".into());
    static ref MODULE: Module = Module {
        package: PACKAGE.clone(),
        path: vec!["fuzzer".to_string()],
        kind: ModuleKind::Code,
    };
}

fuzz_target!(|data: &[u8]| {
    let Ok(source_code) = String::from_utf8(data.to_vec()) else {
        return;
    };

    let module_provider = InMemoryModuleProvider::for_modules::<String>(HashMap::new());
    let mut db = Database::new(Box::new(module_provider));

    db.did_open_module(&MODULE, source_code.as_bytes().to_owned());
    let lir = db
        .lir(MODULE.clone(), TRACING.clone())
        .unwrap()
        .as_ref()
        .to_owned();

    let module_closure = Closure::of_module_lir(lir);
    let mut tracer = DummyTracer::default();
    let use_provider = DbUseProvider {
        db: &db,
        tracing: TRACING.clone(),
    };

    // Run once to generate exports.
    let mut vm = Vm::default();
    vm.set_up_for_running_module_closure(MODULE.clone(), module_closure);
    vm.run(&use_provider, &mut RunForever, &mut tracer);
    if let Status::WaitingForOperations = vm.status() {
        println!("The module waits on channel operations. Perhaps, the code tried to read from a channel without sending a packet into it.");
        return;
    }

    let (mut heap, exported_definitions): (_, Struct) = match vm.tear_down() {
        ExecutionResult::Finished(return_value) => {
            let exported = return_value
                .heap
                .get(return_value.address)
                .data
                .clone()
                .try_into()
                .unwrap();
            (return_value.heap, exported)
        }
        ExecutionResult::Panicked { reason, .. } => {
            println!("The module panicked: {reason}");
            return;
        }
    };

    // Run the `main` function.
    let main = heap.create_symbol("Main".to_string());
    let Some(main) = exported_definitions.get(&heap, main) else {
        println!("The module doesn't contain a main function.");
        return;
    };

    let mut vm = Vm::default();
    let environment = heap.create_struct(Default::default());
    vm.set_up_for_running_closure(heap, main, vec![environment], hir::Id::platform());
    vm.run(&use_provider, &mut RunForever, &mut tracer);
    match vm.tear_down() {
        ExecutionResult::Finished(return_value) => {
            println!("The main function returned: {return_value:?}")
        }
        ExecutionResult::Panicked { reason, .. } => panic!("The main function panicked: {reason}"),
    }
});
