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
    context::RunForever,
    fiber::ExecutionResult,
    heap::Struct,
    mir_to_lir::compile_lir,
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

    let lir = compile_lir(&db, MODULE.clone(), TRACING.clone()).0;
    let mut tracer = DummyTracer::default();

    // Run once to generate exports.
    let mut vm = Vm::for_module_closure(lir.clone());
    vm.run(&mut RunForever, &mut tracer);
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

    let environment = heap.create_struct(Default::default());
    let platform = heap.create_hir_id(hir::Id::platform());
    let mut vm = Vm::for_closure(lir, heap, main, vec![environment], platform);
    vm.run(&mut RunForever, &mut tracer);
    match vm.tear_down() {
        ExecutionResult::Finished(return_value) => {
            println!("The main function returned: {return_value:?}")
        }
        ExecutionResult::Panicked { reason, .. } => panic!("The main function panicked: {reason}"),
    }
});
