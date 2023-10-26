#![no_main]

use candy_frontend::{
    ast::AstDbStorage,
    ast_to_hir::AstToHirStorage,
    cst::CstDbStorage,
    cst_to_ast::CstToAstStorage,
    hir::HirDbStorage,
    hir_to_mir::{ExecutionTarget, HirToMirStorage},
    lir_optimize::OptimizeLirStorage,
    mir_optimize::OptimizeMirStorage,
    mir_to_lir::MirToLirStorage,
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
    heap::{Heap, Struct},
    lir_to_byte_code::compile_byte_code,
    tracer::DummyTracer,
    PopulateInMemoryProviderFromFileSystem, Vm, VmFinished,
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

#[salsa::database(
    AstDbStorage,
    AstToHirStorage,
    CstDbStorage,
    CstToAstStorage,
    HirDbStorage,
    HirToMirStorage,
    MirToLirStorage,
    ModuleDbStorage,
    OptimizeLirStorage,
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
    db.module_provider.load_package_from_file_system("Builtins");
    db.module_provider.add(&MODULE, data.to_vec());

    let byte_code = compile_byte_code(
        &db,
        ExecutionTarget::MainFunction(MODULE.clone()),
        TRACING.clone(),
    )
    .0;

    let mut heap = Heap::default();
    let environment = Struct::create(&mut heap, true, &Default::default());
    let VmFinished { result, .. } =
        Vm::for_main_function(&byte_code, &mut heap, environment, DummyTracer)
            .run_forever_without_handles(&mut heap);
    match result {
        Ok(return_value) => {
            println!("The main function returned: {return_value:?}")
        }
        Err(panic) => {
            panic!("The program panicked: {}", panic.reason)
        }
    }
});
