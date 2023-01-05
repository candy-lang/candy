use std::{collections::HashMap, fs, path::PathBuf, sync::Arc};

use candy::{
    compiler::{hir, lir::Lir, mir_to_lir::MirToLir, TracingConfig},
    database::Database,
    module::{InMemoryModuleProvider, Module, ModuleKind, Package},
    vm::{
        context::{DbUseProvider, RunForever},
        tracer::dummy::DummyTracer,
        Closure, ExecutionResult, Packet, Status, Struct, Vm,
    },
};
use lazy_static::lazy_static;
use walkdir::WalkDir;

const TRACING: TracingConfig = TracingConfig::off();
lazy_static! {
    static ref PACKAGE: Package = Package::User("/".into());
    static ref MODULE: Module = Module {
        package: PACKAGE.clone(),
        path: vec!["benchmark".to_string()],
        kind: ModuleKind::Code,
    };
}

pub fn setup() -> Database {
    let mut module_provider = load_core();
    module_provider.add_str(&MODULE, r#"_ = use "..Core""#);
    let db = Database::new(Box::new(module_provider));

    // Load `Core` into the cache.
    db.lir(MODULE.clone(), TRACING.clone()).unwrap();

    db
}
fn load_core() -> InMemoryModuleProvider {
    let mut modules = HashMap::new();

    let packages_directory: PathBuf = "../packages".into();
    let core_directory: PathBuf = "../packages/Core".into();
    for file in WalkDir::new(&core_directory)
        .into_iter()
        .map(|it| it.unwrap())
        .filter(|it| it.file_type().is_file())
        .filter(|it| it.file_name().to_string_lossy().ends_with(".candy"))
    {
        let path = file.path();

        let mut module = Module::from_package_root_and_file(
            packages_directory.clone(),
            path.to_owned(),
            ModuleKind::Code,
        );
        module.package = PACKAGE.clone();

        let source_code = fs::read_to_string(path).unwrap();
        modules.insert(module, source_code);
    }
    InMemoryModuleProvider::for_modules(modules)
}

pub fn compile_and_run(db: &mut Database, source_code: &str) {
    let lir = compile(db, source_code);
    run(db, lir.as_ref().to_owned());
}

pub fn compile(db: &mut Database, source_code: &str) -> Arc<Lir> {
    db.did_open_module(&MODULE, source_code.as_bytes().to_owned());

    db.lir(MODULE.clone(), TRACING.clone()).unwrap()
}

pub fn run(db: &Database, lir: Lir) -> Packet {
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
