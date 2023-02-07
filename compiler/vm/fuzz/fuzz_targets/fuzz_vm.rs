#![no_main]

use candy_frontend::{
    ast::AstDbStorage,
    ast_to_hir::AstToHirStorage,
    cst::CstDbStorage,
    cst_to_ast::CstToAstStorage,
    hir::{self, HirDbStorage},
    hir_to_mir::HirToMirStorage,
    mir_optimize::OptimizeMirStorage,
    module::{
        InMemoryModuleProvider, Module, ModuleDbStorage, ModuleKind, ModuleProvider,
        ModuleProviderOwner, Package,
    },
    position::PositionConversionStorage,
    rcst_to_cst::RcstToCstStorage,
    string_to_rcst::StringToRcstStorage,
    TracingConfig,
};
use candy_vm::{
    context::{DbUseProvider, RunForever},
    fiber::ExecutionResult,
    heap::{Closure, Struct},
    mir_to_lir::{MirToLir, MirToLirStorage},
    tracer::DummyTracer,
    vm::{Status, Vm},
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

#[derive()]
#[salsa::database(
    AstDbStorage,
    AstToHirStorage,
    CstDbStorage,
    CstToAstStorage,
    HirDbStorage,
    HirToMirStorage,
    MirToLirStorage,
    ModuleDbStorage,
    OptimizeMirStorage,
    PositionConversionStorage,
    RcstToCstStorage,
    StringToRcstStorage
)]
#[derive(Default)]
pub struct Database {
    storage: salsa::Storage<Self>,
    module_provider: InMemoryModuleProvider,
}
impl salsa::Database for Database {}
impl ModuleProviderOwner for Database {
    fn get_module_provider(&self) -> &dyn ModuleProvider {
        &self.module_provider
    }
}

fuzz_target!(|data: &[u8]| {
    let mut db = Database::default();
    db.module_provider.add(&MODULE, data.to_vec());

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
