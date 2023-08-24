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
    heap::{Heap, HirId, Struct},
    mir_to_byte_code::compile_byte_code,
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
    db.module_provider.load_package_from_file_system("Builtins");
    db.module_provider.add(&MODULE, data.to_vec());

    let byte_code = compile_byte_code(&db, MODULE.clone(), TRACING.clone()).0;

    let mut heap = Heap::default();
    let VmFinished { result, .. } =
        Vm::for_module(&byte_code, &mut heap, DummyTracer).run_forever_without_handles(&mut heap);
    let Ok(exports) = result else {
        println!("The module panicked.");
        return;
    };
    let Ok(main) = exports.into_main_function(&heap) else {
        println!("The module doesn't export a main function.");
        return;
    };

    // Run the `main` function.
    let environment = Struct::create(&mut heap, true, &Default::default());
    let responsible = HirId::create(&mut heap, true, hir::Id::user());
    let VmFinished { result, .. } = Vm::for_function(
        &byte_code,
        &mut heap,
        main,
        &[environment.into()],
        responsible,
        DummyTracer,
    )
    .run_forever_without_handles(&mut heap);
    match result {
        Ok(return_value) => {
            println!("The main function returned: {return_value:?}")
        }
        Err(panic) => {
            panic!("The main function panicked: {}", panic.reason)
        }
    }
});
