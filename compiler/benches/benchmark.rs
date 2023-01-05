use std::{collections::HashMap, sync::Arc};

use candy::{
    compiler::{hir, lir::Lir, mir_to_lir::MirToLir, TracingConfig},
    database::Database,
    module::{InMemoryModuleProvider, Module, ModuleKind, Package},
    vm::{
        context::{DbUseProvider, RunForever},
        tracer::dummy::DummyTracer,
        Closure, ExecutionResult, Packet, Status, Struct, Text, Vm,
    },
};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lazy_static::lazy_static;

const TRACING: TracingConfig = TracingConfig::off();
lazy_static! {
    static ref MODULE: Module = Module {
        package: Package::User("/".into()),
        path: vec![],
        kind: ModuleKind::Code,
    };
}

fn hello_world(message: String) -> String {
    let source_code = format!("main _ := \"{message}\"");
    let module_provider = InMemoryModuleProvider::for_modules(HashMap::from([(
        MODULE.clone(),
        source_code.as_ref(),
    )]));
    let db = Database::new(Box::new(module_provider));

    let lir = compile(&db);

    let Packet { heap, address } = run(&db, lir.as_ref().to_owned());

    let text: Text = heap.get(address).data.to_owned().try_into().unwrap();
    text.value
}

fn compile(db: &Database) -> Arc<Lir> {
    db.lir(MODULE.clone(), TRACING.clone()).unwrap()
}

fn run(db: &Database, lir: Lir) -> Packet {
    let module_closure = Closure::of_module_lir(lir);
    let mut tracer = DummyTracer::default();
    let use_provider = DbUseProvider {
        db,
        tracing: TRACING.clone(),
    };

    // Run once to generate exports.
    let mut vm = Vm::default();
    vm.set_up_for_running_module_closure(MODULE.clone(), module_closure);
    vm.run(&use_provider, &mut RunForever, &mut tracer);
    if let Status::WaitingForOperations = vm.status() {
        panic!("The module waits on channel operations. Perhaps, the code tried to read from a channel without sending a packet into it.");
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
            panic!("The module panicked: {reason}");
        }
    };

    // Run the `main` function.
    let main = heap.create_symbol("Main".to_string());
    let main = match exported_definitions.get(&heap, main) {
        Some(main) => main,
        None => panic!("The module doesn't contain a main function."),
    };

    let mut vm = Vm::default();
    let environment = heap.create_struct(Default::default());
    vm.set_up_for_running_closure(heap, main, vec![environment], hir::Id::platform());
    vm.run(&use_provider, &mut RunForever, &mut tracer);
    match vm.tear_down() {
        ExecutionResult::Finished(return_value) => return_value,
        ExecutionResult::Panicked { reason, .. } => panic!("The main function panicked: {reason}"),
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("hello_world", |b| {
        b.iter(|| hello_world(black_box("Hello, world!".to_string())))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
